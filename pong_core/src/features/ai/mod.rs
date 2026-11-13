//! AI, mirror controller (spec 1.4.6): steers each paddle the match config
//! gives to the computer, aiming where the kernel predicts the ball will cross.
//!
//! Under effects (M2 spec 2.3.12) the star's handicap is not the whole flaw:
//! the effects in play add error and delay, and the AI predicts under the
//! flight it believes in, which may not be the one the ball flies.

use std::time::Duration;

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::error::BevyError;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::{Fixed, Time};

use crate::domain::{Controller, Difficulty, MatchConfig, Random};
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::effects::{ActiveEffects, BallFlight, FlightFlaw};
use crate::features::paddle::Paddle;
use crate::features::seats::{PaddleIntent, PaddleIntents};
use crate::features::tuning::Tuning;
use crate::flow::phases::FixedStepPhase;
use crate::flow::states::AppState;
use crate::kernel::flight::Flight;
use crate::kernel::geometry::Side;
use crate::kernel::integrate::Motion;
use crate::kernel::predict::crossing_y;

/// Spawns a [`MirrorController`] for every side held by [`Controller::Ai`],
/// for the length of a match, and lets each write its side's
/// [`PaddleIntents`] in the intent phase. Seats writes the other sides, so
/// the two never touch the same side and need no order.
///
/// The steering runs whenever the phases do, not only in `Playing`: it moves
/// no ball, so it stays out of `BallInPlay`, and the AI lines up between
/// rallies as players do (ADR 0004).
///
/// Precondition: [`MatchConfig`] exists on entering `AppState::InMatch`; the
/// menu inserts it first.
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_controllers)
            .add_systems(FixedUpdate, steer_ai_paddles.in_set(FixedStepPhase::Intent));
    }
}

fn spawn_controllers(mut commands: Commands, config: Res<MatchConfig>) {
    for side in [Side::Left, Side::Right] {
        if let Controller::Ai(difficulty) = config.controller(side) {
            commands.spawn((
                MirrorController::new(side, difficulty),
                DespawnOnExit(AppState::InMatch),
            ));
        }
    }
}

/// What the controllers may see of the ball: the ball itself, and the
/// effects in play that colour how an AI reads its flight.
#[derive(SystemParam)]
struct SeenBall<'w, 's> {
    balls: Query<'w, 's, (&'static Position, &'static Velocity, &'static BallFlight), With<Ball>>,
    effects: Res<'w, ActiveEffects>,
}

/// Each controller reads the ball as it is now and writes its side's intent.
///
/// The serve also runs in the intent phase, unordered against this system,
/// and the serve's flight begins in the effects phase after both. A ball in
/// no flight is not an approach ([`Sighting::flight`]), so on the step of a
/// serve the AI sees no approach, whether it ran before the serve or after:
/// it plans on the next step, under the serve's flight. Every other turn
/// comes from a return in an earlier step, which began its flight then, and
/// is seen at once.
///
/// # Errors
/// With a controller to steer: not exactly one ball, or no paddle on the
/// controller's side. The match was assembled against spec 1.4.2 or 1.4.3.
/// A match of humans only reads no ball.
fn steer_ai_paddles(
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    mut random: ResMut<Random>,
    mut intents: ResMut<PaddleIntents>,
    seen: SeenBall,
    paddles: Query<&Paddle>,
    mut controllers: Query<&mut MirrorController>,
) -> Result<(), BevyError> {
    if controllers.is_empty() {
        return Ok(());
    }
    let (position, velocity, flight) = seen
        .balls
        .single()
        .map_err(|err| BevyError::from(format!("AI steering needs exactly one ball: {err}")))?;
    let sighting = Sighting {
        ball: Motion {
            position: position.0,
            velocity: velocity.0,
        },
        flight: flight.in_flight(),
        effects: &seen.effects,
    };
    for mut controller in &mut controllers {
        let side = controller.side;
        let Some(paddle) = paddles.iter().find(|paddle| paddle.side() == side) else {
            return Err(BevyError::from(format!(
                "AI steering found no paddle on the {side:?} side"
            )));
        };
        let intent = controller.steer(sighting, *paddle, &tuning, time.delta(), &mut random);
        intents.set(side, intent);
    }
    Ok(())
}

/// What a controller sees of the ball in one fixed step.
#[derive(Clone, Copy)]
pub struct Sighting<'a> {
    /// The ball's motion now.
    pub ball: Motion,
    /// The flight the ball is in. `None` while it is in none: between
    /// rallies, and on the step a serve has launched it and its flight has
    /// yet to begin.
    pub flight: Option<&'a Flight>,
    /// The effects in play: asked for their flaws when an approach is
    /// planned (M2 spec 2.3.12).
    pub effects: &'a ActiveEffects,
}

/// The computer's player on one side: an entity of its own, apart from the
/// paddle it steers, alive for one match.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MirrorController {
    side: Side,
    difficulty: Difficulty,
    /// Whether the ball flew toward this side on the last step, so the step
    /// it turns toward this side is seen.
    was_approaching: bool,
    plan: Plan,
}

/// Where the controller is taking its paddle.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Plan {
    /// The ball is not coming, or will never arrive: hold the centre, where
    /// the next serve starts.
    Centre,
    /// The ball is coming: once `hold_for` has run out, meet it at the aim's
    /// target.
    Meet {
        /// The crossing the AI believes in plus this approach's error. Fixed
        /// for the approach.
        aim: Aim,
        /// What remains of the reaction delay, the star's plus the effects'.
        /// The paddle holds still while it is not zero; each held step
        /// spends one timestep of it.
        hold_for: Duration,
    },
}

/// Where a controller aims during an approach: read-only, for the overlay
/// (plan 10.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aim {
    /// The y the paddle steers to: the crossing predicted under the flight
    /// the AI believes in, plus `error`.
    pub target_y: f32,
    /// This approach's draw from the spread: the star's `prediction_error`
    /// plus the error the effects in play add, either way.
    pub error: f32,
}

impl MirrorController {
    /// The side whose paddle this controller steers.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// This approach's aim; `None` while the paddle holds the centre.
    #[must_use]
    pub const fn aim(&self) -> Option<Aim> {
        match self.plan {
            Plan::Centre => None,
            Plan::Meet { aim, .. } => Some(aim),
        }
    }

    /// A controller for the paddle on `side`, playing at `difficulty`, that
    /// has not yet seen the ball.
    #[must_use]
    pub const fn new(side: Side, difficulty: Difficulty) -> Self {
        Self {
            side,
            difficulty,
            was_approaching: false,
            plan: Plan::Centre,
        }
    }

    /// What this controller presses for one fixed step, given what it sees
    /// of the ball and its paddle (spec 1.4.6, M2 spec 2.3.12).
    ///
    /// - The ball approaches when it is in a flight and flies toward this
    ///   side's goal.
    /// - On the step the approach begins, the effects in play are asked for
    ///   their flaws at this controller's difficulty
    ///   ([`ActiveEffects::flaw_over`]). The target becomes the kernel's
    ///   predicted crossing of the paddle's contact line under the believed
    ///   flight ([`crossing_y`] at [`Paddle::contact_x`]) plus an error drawn
    ///   once from the spread, either way: the star's `prediction_error` plus
    ///   the effects' added error. It is not recomputed until the next approach.
    /// - The paddle holds still until the star's `reaction_delay` plus the
    ///   effects' added delay has passed since then, then steers to the
    ///   target. A zero delay steers on that step.
    /// - While the ball is not approaching, or an approach has no crossing,
    ///   the paddle steers to the centre.
    ///
    /// Preconditions (the caller's): `paddle` is on this controller's side;
    /// `seen.ball` is finite; `seen.flight`, if any, was started with
    /// `seen.effects` in play; `dt` is the fixed timestep.
    /// Postcondition: `random` advanced by exactly one draw on the step an
    /// approach with a crossing begins, by none otherwise.
    pub fn steer(
        &mut self,
        seen: Sighting,
        paddle: Paddle,
        tuning: &Tuning,
        dt: Duration,
        random: &mut Random,
    ) -> PaddleIntent {
        let approach = seen
            .flight
            .filter(|_| seen.ball.velocity.x * self.side.sign() > 0.0);
        match approach {
            None => self.plan = Plan::Centre,
            Some(flight) if !self.was_approaching => {
                let flaw = seen.effects.flaw_over(flight, self.difficulty);
                self.plan = self.plan_approach(seen.ball, &flaw, paddle, tuning, dt, random);
            }
            Some(_) => {}
        }
        self.was_approaching = approach.is_some();

        let step_travel = tuning.paddle().speed() * dt.as_secs_f32();
        match &mut self.plan {
            Plan::Centre => intent_toward(paddle, 0.0, step_travel),
            Plan::Meet { aim, hold_for } if hold_for.is_zero() => {
                intent_toward(paddle, aim.target_y, step_travel)
            }
            Plan::Meet { hold_for, .. } => {
                *hold_for = hold_for.saturating_sub(dt);
                PaddleIntent::Still
            }
        }
    }

    /// The plan for an approach that begins now, read with `flaw`: meet the
    /// ball where the kernel predicts it crosses the paddle's contact line
    /// under the believed flight, off by one draw from the star's spread
    /// widened by the effects' added error, after the star's reaction delay
    /// lengthened by the effects' added delay. No crossing, no draw.
    fn plan_approach(
        self,
        ball: Motion,
        flaw: &FlightFlaw,
        paddle: Paddle,
        tuning: &Tuning,
        dt: Duration,
        random: &mut Random,
    ) -> Plan {
        let crossing = crossing_y(
            tuning.arena(),
            tuning.ball().radius(),
            ball,
            &flaw.believed,
            paddle.contact_x(tuning),
            dt.as_secs_f32(),
        );
        let Some(predicted_y) = crossing else {
            return Plan::Centre;
        };
        let handicap = tuning.ai().level(self.difficulty);
        let spread = handicap.prediction_error() + flaw.added_error;
        let error = random.next_f32_in(-spread, spread);
        Plan::Meet {
            aim: Aim {
                target_y: predicted_y + error,
                error,
            },
            hold_for: handicap.reaction_delay().saturating_add(flaw.added_delay),
        }
    }
}

/// The intent that brings `paddle` to `target_y` without jitter: still once
/// the centre is within half a step's travel of the target, bound included,
/// else toward it. A paddle that moves was more than half a step away, so it
/// lands within half a step and holds there: it never moves back.
///
/// A target beyond a wall keeps pressing toward it; the paddle's own clamp
/// holds it at the wall.
///
/// Precondition: `step_travel > 0`, how far one step of a held key moves it.
fn intent_toward(paddle: Paddle, target_y: f32, step_travel: f32) -> PaddleIntent {
    let gap = target_y - paddle.center_y();
    if gap.abs() <= step_travel / 2.0 {
        PaddleIntent::Still
    } else if gap > 0.0 {
        PaddleIntent::Up
    } else {
        PaddleIntent::Down
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;
    use std::sync::LazyLock;
    use std::time::Duration;

    use bevy::app::App;
    use bevy::ecs::query::With;
    use bevy::math::Vec2;

    use super::{MirrorController, Sighting, intent_toward};
    use crate::domain::{
        BallHitPaddle, Controller, Difficulty, EffectId, MatchConfig, Mode, PointScored, Random,
        Seat, SeatPlayers,
    };
    use crate::features::ball::Ball;
    use crate::features::effects::probe::Misjudged;
    use crate::features::effects::{ActiveEffects, BallFlight, Sandbox};
    use crate::features::paddle::Paddle;
    use crate::features::seats::PaddleIntent;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::FIXED_HZ;
    use crate::flow::states::MatchState;
    use crate::kernel::flight::Flight;
    use crate::kernel::flight::FlightStart;
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::kernel::predict::crossing_y;
    use crate::kernel::random::Pcg32;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        ball_state, boot, enter_match, fixed_timestep, frame, headless_app, leave_match,
        match_state, place_ball, record, recorded, start_match, take_recorded,
    };

    fn match_of(left: Controller, right: Controller) -> MatchConfig {
        MatchConfig {
            left,
            right,
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
            mode: Mode::Classic,
        }
    }

    /// Each controller's side and difficulty, left side first.
    fn controllers(app: &mut App) -> Vec<(Side, Difficulty)> {
        let mut found: Vec<(Side, Difficulty)> = app
            .world_mut()
            .query::<&MirrorController>()
            .iter(app.world())
            .map(|controller| (controller.side, controller.difficulty))
            .collect();
        found.sort_by_key(|(side, _)| *side == Side::Right);
        found
    }

    #[test]
    fn entering_a_match_spawns_one_controller_per_ai_side_and_none_for_a_human_side() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(
            &mut app,
            match_of(
                Controller::Human(Seat::Left),
                Controller::Ai(Difficulty::TwoStars),
            ),
        );
        assert_eq!(
            controllers(&mut app),
            vec![(Side::Right, Difficulty::TwoStars)]
        );
    }

    #[test]
    fn a_match_between_two_ais_spawns_a_controller_for_each_side() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(
            &mut app,
            match_of(
                Controller::Ai(Difficulty::OneStar),
                Controller::Ai(Difficulty::ThreeStars),
            ),
        );
        assert_eq!(
            controllers(&mut app),
            vec![
                (Side::Left, Difficulty::OneStar),
                (Side::Right, Difficulty::ThreeStars)
            ]
        );
    }

    #[test]
    fn leaving_the_match_despawns_the_controllers() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(
            &mut app,
            match_of(
                Controller::Ai(Difficulty::OneStar),
                Controller::Ai(Difficulty::ThreeStars),
            ),
        );
        leave_match(&mut app);
        assert_eq!(controllers(&mut app), Vec::new());
    }

    /// The paddle on `side` in a running app.
    fn paddle_on(app: &mut App, side: Side) -> Paddle {
        app.world_mut()
            .query::<&Paddle>()
            .iter(app.world())
            .copied()
            .find(|paddle| paddle.side() == side)
            .expect("one paddle per side in a match")
    }

    /// ADR 0004: players line up between rallies, and so does the AI.
    #[test]
    fn during_the_countdown_an_ai_paddle_returns_to_the_centre_while_the_ball_waits() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(
            &mut app,
            match_of(
                Controller::Human(Seat::Left),
                Controller::Ai(Difficulty::TwoStars),
            ),
        );
        let arena = app.world().resource::<Tuning>().arena();
        let mut paddles = app.world_mut().query::<&mut Paddle>();
        for mut paddle in paddles.iter_mut(app.world_mut()) {
            if paddle.side() == Side::Right {
                paddle.move_by(20.0, arena);
            }
        }
        // Twenty units at 1.5 a step: fourteen steps, well inside the countdown.
        for _ in 0..30 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert_near(
            paddle_on(&mut app, Side::Right).center_y(),
            0.0,
            0.75 + 1e-3,
        );
    }

    // Steering (plan 5.1 phase B). Numbers from the embedded tuning: paddle
    // speed 90, so one step of a held key travels 1.5 at 60 Hz; the paddle's
    // centre stays within ±38; a ball's centre touches the right paddle at
    // x = 73 and the left at x = -73. No course below reaches a wall.

    /// The fixed timestep, as `Time::<Fixed>::from_hz(FIXED_HZ)` makes it.
    fn step() -> Duration {
        Duration::from_secs_f64(1.0 / FIXED_HZ)
    }

    /// The embedded tuning with three stars' handicap replaced.
    fn three_stars_with(delay_seconds: f32, error: f32) -> Tuning {
        let ron = EMBEDDED_RON.replace(
            "three_stars: (reaction_delay_seconds: 0.24, prediction_error: 3.0)",
            &format!(
                "three_stars: (reaction_delay_seconds: {delay_seconds:?}, prediction_error: {error:?})"
            ),
        );
        assert_ne!(ron, EMBEDDED_RON, "three stars' entry not found in the RON");
        Tuning::from_ron(&ron).expect("the crafted handicap is valid")
    }

    /// Three stars with no delay and no error: a perfect player.
    fn perfect() -> Tuning {
        three_stars_with(0.0, 0.0)
    }

    fn ai(side: Side) -> MirrorController {
        MirrorController::new(side, Difficulty::ThreeStars)
    }

    fn paddle_at(side: Side, y: f32, tuning: &Tuning) -> Paddle {
        let mut paddle = Paddle::new(side, tuning.paddle());
        paddle.move_by(y, tuning.arena());
        paddle
    }

    fn ball(position: Vec2, velocity: Vec2) -> Motion {
        Motion { position, velocity }
    }

    static EMPTY_FLIGHT: LazyLock<Flight> = LazyLock::new(Flight::default);
    static NO_EFFECTS: LazyLock<ActiveEffects> = LazyLock::new(ActiveEffects::default);

    /// `ball` as a controller sees it in a flight with no effect in play.
    fn flying(ball: Motion) -> Sighting<'static> {
        Sighting {
            ball,
            flight: Some(&EMPTY_FLIGHT),
            effects: &NO_EFFECTS,
        }
    }

    fn step_travel(tuning: &Tuning) -> f32 {
        tuning.paddle().speed() * step().as_secs_f32()
    }

    /// Steers `ai` for `steps` fixed steps while the ball flies straight from
    /// `start`, moving the paddle by each intent as the paddle feature does.
    /// Returns where the paddle ends and every intent pressed.
    fn steer_for(
        ai: &mut MirrorController,
        mut paddle: Paddle,
        start: Motion,
        steps: u16,
        tuning: &Tuning,
        random: &mut Random,
    ) -> (Paddle, Vec<PaddleIntent>) {
        let travel = step_travel(tuning);
        let mut intents = Vec::new();
        for i in 0..steps {
            let flown = start.velocity * step().as_secs_f32() * f32::from(i);
            let now = ball(start.position + flown, start.velocity);
            let intent = ai.steer(flying(now), paddle, tuning, step(), random);
            let dy = match intent {
                PaddleIntent::Up => travel,
                PaddleIntent::Down => -travel,
                PaddleIntent::Still => 0.0,
            };
            paddle.move_by(dy, tuning.arena());
            intents.push(intent);
        }
        (paddle, intents)
    }

    #[test]
    fn the_paddle_holds_within_half_a_step_of_its_target_and_otherwise_moves_toward_it() {
        let tuning = perfect();
        let paddle = paddle_at(Side::Right, 0.0, &tuning);
        let travel = 1.5;
        assert_eq!(intent_toward(paddle, 0.7, travel), PaddleIntent::Still);
        assert_eq!(intent_toward(paddle, -0.7, travel), PaddleIntent::Still);
        // Exactly half a step: a move would land exactly half a step past, and
        // the next step would move back. Holding here is what rules out jitter.
        assert_eq!(intent_toward(paddle, 0.75, travel), PaddleIntent::Still);
        assert_eq!(intent_toward(paddle, -0.75, travel), PaddleIntent::Still);
        assert_eq!(intent_toward(paddle, 0.8, travel), PaddleIntent::Up);
        assert_eq!(intent_toward(paddle, -0.8, travel), PaddleIntent::Down);
        assert_eq!(intent_toward(paddle, 30.0, travel), PaddleIntent::Up);
    }

    #[test]
    fn a_ball_not_coming_sends_the_paddle_to_the_centre() {
        let tuning = perfect();
        let mut random = Random::seeded(1);
        let away = ball(Vec2::ZERO, Vec2::new(-60.0, 30.0));
        let at_rest = ball(Vec2::ZERO, Vec2::ZERO);
        for (y, toward_centre) in [
            (20.0, PaddleIntent::Down),
            (-20.0, PaddleIntent::Up),
            (0.0, PaddleIntent::Still),
        ] {
            let paddle = paddle_at(Side::Right, y, &tuning);
            for course in [away, at_rest] {
                let intent =
                    ai(Side::Right).steer(flying(course), paddle, &tuning, step(), &mut random);
                assert_eq!(intent, toward_centre, "paddle at {y}, ball {course:?}");
            }
        }
    }

    #[test]
    fn once_the_ball_is_returned_the_paddle_heads_back_to_the_centre() {
        let tuning = perfect();
        let mut random = Random::seeded(1);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 20.0, &tuning);
        // Crossing at y = 36.5, above the paddle.
        let coming = ball(Vec2::ZERO, Vec2::new(60.0, 30.0));
        let returned = ball(Vec2::new(73.0, 36.5), Vec2::new(-60.0, 30.0));
        assert_eq!(
            ai.steer(flying(coming), paddle, &tuning, step(), &mut random),
            PaddleIntent::Up
        );
        assert_eq!(
            ai.steer(flying(returned), paddle, &tuning, step(), &mut random),
            PaddleIntent::Down
        );
    }

    #[test]
    fn with_no_delay_and_no_error_the_paddle_meets_the_predicted_crossing_on_either_side() {
        let tuning = perfect();
        for (side, velocity) in [
            (Side::Right, Vec2::new(60.0, 30.0)),
            (Side::Left, Vec2::new(-60.0, 30.0)),
        ] {
            let mut random = Random::seeded(1);
            // From the centre spot, the ball reaches x = ±73 at y = 36.5; it
            // is level with the paddle now, so steering to the ball's current
            // height would press nothing.
            let (paddle, intents) = steer_for(
                &mut ai(side),
                paddle_at(side, 0.0, &tuning),
                ball(Vec2::ZERO, velocity),
                40,
                &tuning,
                &mut random,
            );
            assert_eq!(intents[0], PaddleIntent::Up, "{side:?}: first step");
            assert_near(paddle.center_y(), 36.5, 0.75 + 1e-3);
            assert_eq!(
                intents.last(),
                Some(&PaddleIntent::Still),
                "{side:?}: settled"
            );
        }
    }

    #[test]
    fn the_paddle_holds_still_until_the_reaction_delay_has_passed_since_the_turn() {
        // Two and a half steps: the delay ends between steps 2 and 3.
        let tuning = three_stars_with(2.5 / 60.0, 0.0);
        let mut random = Random::seeded(1);
        let mut ai = ai(Side::Right);
        // Above the centre and below the crossing at 36.5: drifting to the
        // centre presses Down, meeting the ball presses Up, holding presses nothing.
        let paddle = paddle_at(Side::Right, 20.0, &tuning);
        let away = ball(Vec2::ZERO, Vec2::new(-60.0, 30.0));
        let coming = ball(Vec2::ZERO, Vec2::new(60.0, 30.0));
        assert_eq!(
            ai.steer(flying(away), paddle, &tuning, step(), &mut random),
            PaddleIntent::Down
        );
        let after_turn: Vec<PaddleIntent> = (0..5)
            .map(|_| ai.steer(flying(coming), paddle, &tuning, step(), &mut random))
            .collect();
        assert_eq!(
            after_turn,
            vec![
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Up,
                PaddleIntent::Up,
            ]
        );
    }

    #[test]
    fn the_aim_error_is_drawn_once_per_approach_within_the_tuned_spread() {
        let error = 10.0;
        let tuning = three_stars_with(0.0, error);
        let mut random = Random::seeded(7);
        let mut ai = ai(Side::Right);
        let mut paddle = paddle_at(Side::Right, 0.0, &tuning);
        // Straight across: the crossing is at y = 0.
        let coming = ball(Vec2::ZERO, Vec2::new(60.0, 0.0));
        let away = ball(Vec2::ZERO, Vec2::new(-60.0, 0.0));
        let mut rests = Vec::new();
        for approach in 0..20 {
            ai.steer(flying(away), paddle, &tuning, step(), &mut random);
            let (settled, intents) = steer_for(&mut ai, paddle, coming, 40, &tuning, &mut random);
            // One draw per approach: the target holds, so the paddle comes to rest.
            assert!(
                intents[30..].iter().all(|i| *i == PaddleIntent::Still),
                "approach {approach} never settled: {intents:?}"
            );
            assert_near(settled.center_y(), 0.0, error + 0.75 + 1e-3);
            rests.push(settled.center_y());
            paddle = settled;
        }
        // A fresh draw per approach: the rests spread wider than one step.
        let lowest = rests.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = rests.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            highest - lowest > 1.5,
            "every approach aimed alike: {rests:?}"
        );
    }

    /// Plan 10.1: the overlay reads the aim; the target is the crossing plus
    /// the error, which stays within the spread; no approach, no aim.
    #[test]
    fn an_approachs_aim_is_the_crossing_plus_its_error_and_none_without_one() {
        let spread = 10.0;
        let tuning = three_stars_with(0.0, spread);
        let mut random = Random::seeded(7);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 0.0, &tuning);
        assert_eq!(ai.side(), Side::Right);
        assert_eq!(ai.aim(), None, "a fresh controller holds the centre");
        // Straight across: the crossing is at y = 0.
        ai.steer(
            flying(ball(Vec2::ZERO, Vec2::new(60.0, 0.0))),
            paddle,
            &tuning,
            step(),
            &mut random,
        );
        let aim = ai.aim().expect("an approach aims");
        assert_near(aim.target_y, aim.error, 1e-4);
        assert!(aim.error.abs() <= spread, "{aim:?}");
        assert!(aim.error != 0.0, "a drawn error, not none");
        ai.steer(
            flying(ball(Vec2::ZERO, Vec2::new(-60.0, 0.0))),
            paddle,
            &tuning,
            step(),
            &mut random,
        );
        assert_eq!(ai.aim(), None, "the ball went back");
    }

    #[test]
    fn a_changed_ball_course_mid_approach_does_not_move_the_target() {
        let tuning = perfect();
        let mut random = Random::seeded(1);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 0.0, &tuning);
        // The turn aims at the crossing at y = 36.5.
        ai.steer(
            flying(ball(Vec2::ZERO, Vec2::new(60.0, 30.0))),
            paddle,
            &tuning,
            step(),
            &mut random,
        );
        // Later steps see a course that would cross at y = -36.5.
        let (paddle, _) = steer_for(
            &mut ai,
            paddle,
            ball(Vec2::ZERO, Vec2::new(60.0, -30.0)),
            40,
            &tuning,
            &mut random,
        );
        assert_near(paddle.center_y(), 36.5, 0.75 + 1e-3);
    }

    #[test]
    fn an_approach_too_slow_to_ever_arrive_sends_the_paddle_to_the_centre() {
        let tuning = perfect();
        let mut random = Random::seeded(1);
        let paddle = paddle_at(Side::Right, 20.0, &tuning);
        // Over a million seconds to reach the paddle: beyond the prediction's reach.
        let crawling = ball(Vec2::ZERO, Vec2::new(1e-4, 0.0));
        assert_eq!(
            ai(Side::Right).steer(flying(crawling), paddle, &tuning, step(), &mut random),
            PaddleIntent::Down
        );
    }

    // M2 plan 4.1: the AI under effects. The effects in play add to the
    // star's handicap and decide the flight the AI believes in (M2 spec 2.3.12).

    /// The serve launches the ball one phase before its flight begins. A
    /// ball in no flight is not yet coming: the AI plans once the flight is
    /// there to plan under, whichever of the serve and the AI ran first.
    #[test]
    fn a_ball_flying_here_in_no_flight_is_not_an_approach_until_its_flight_begins() {
        let tuning = perfect();
        let mut random = Random::seeded(1);
        let mut untouched = Random::seeded(1);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 20.0, &tuning);
        // Crossing at y = 36.5, above the paddle; the centre is below it.
        let coming = ball(Vec2::ZERO, Vec2::new(60.0, 30.0));
        let served = Sighting {
            flight: None,
            ..flying(coming)
        };
        assert_eq!(
            ai.steer(served, paddle, &tuning, step(), &mut random),
            PaddleIntent::Down,
            "no flight yet: the centre"
        );
        assert_eq!(ai.aim(), None);
        assert_eq!(
            random.next_f32_in(0.0, 1.0).to_bits(),
            untouched.next_f32_in(0.0, 1.0).to_bits(),
            "no approach, no draw"
        );
        assert_eq!(
            ai.steer(flying(coming), paddle, &tuning, step(), &mut random),
            PaddleIntent::Up,
            "the flight began: the crossing"
        );
        assert!(ai.aim().is_some());
    }

    /// A flight begun under `effects` at the serve.
    fn served_under(effects: &ActiveEffects) -> Flight {
        effects.start_flight(FlightStart::SERVE)
    }

    /// First form, error: an effect's added error widens the aim's spread
    /// beyond the star's own, here none.
    #[test]
    fn an_effects_added_error_widens_the_spread_of_the_aim() {
        let added = 10.0;
        let tuning = perfect();
        let mut effects = ActiveEffects::default();
        effects.add(Box::new(Misjudged::costing(added, Duration::ZERO)));
        let flight = served_under(&effects);
        let mut random = Random::seeded(7);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 0.0, &tuning);
        // Straight across: the crossing is at y = 0.
        let coming = ball(Vec2::ZERO, Vec2::new(60.0, 0.0));
        let away = ball(Vec2::ZERO, Vec2::new(-60.0, 0.0));
        let mut errors = Vec::new();
        for _ in 0..20 {
            ai.steer(flying(away), paddle, &tuning, step(), &mut random);
            let seen = Sighting {
                ball: coming,
                flight: Some(&flight),
                effects: &effects,
            };
            ai.steer(seen, paddle, &tuning, step(), &mut random);
            let aim = ai.aim().expect("an approach aims");
            assert_near(aim.target_y, aim.error, 1e-4);
            assert!(aim.error.abs() <= added, "{aim:?}");
            errors.push(aim.error);
        }
        let lowest = errors.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = errors.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            highest - lowest > added / 2.0,
            "the effect's error never reached the aim: {errors:?}"
        );
    }

    /// First form, delay: an effect's added delay holds the paddle beyond
    /// the star's own, here none.
    #[test]
    fn an_effects_added_delay_holds_the_paddle_longer() {
        let tuning = perfect();
        let mut effects = ActiveEffects::default();
        // Two and a half steps: the delay ends between steps 2 and 3.
        effects.add(Box::new(Misjudged::costing(
            0.0,
            Duration::from_secs_f32(2.5 / 60.0),
        )));
        let flight = served_under(&effects);
        let mut random = Random::seeded(1);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 20.0, &tuning);
        // Crossing at y = 36.5, above the paddle.
        let seen = Sighting {
            ball: ball(Vec2::ZERO, Vec2::new(60.0, 30.0)),
            flight: Some(&flight),
            effects: &effects,
        };
        let after_turn: Vec<PaddleIntent> = (0..5)
            .map(|_| ai.steer(seen, paddle, &tuning, step(), &mut random))
            .collect();
        assert_eq!(
            after_turn,
            vec![
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Up,
                PaddleIntent::Up,
            ]
        );
    }

    /// The effects' flaw is added to the star's own handicap, and replaces
    /// none of it: a star's spread of 4 and an effect's of 6 aim up to 10
    /// off, and a star's delay of two steps and an effect's of two and a
    /// half hold the paddle for four and a half.
    #[test]
    fn an_effects_flaw_adds_to_the_stars_own_error_and_delay() {
        let (own_error, added_error) = (4.0, 6.0);
        let tuning = three_stars_with(2.0 / 60.0, own_error);
        let mut effects = ActiveEffects::default();
        effects.add(Box::new(Misjudged::costing(
            added_error,
            Duration::from_secs_f32(2.5 / 60.0),
        )));
        let flight = served_under(&effects);
        let mut random = Random::seeded(7);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 20.0, &tuning);
        // Level through the centre: the crossing is at y = 0, far below the
        // paddle, so every aim presses Down once the paddle may move.
        let seen = Sighting {
            ball: ball(Vec2::ZERO, Vec2::new(60.0, 0.0)),
            flight: Some(&flight),
            effects: &effects,
        };
        let away = ball(Vec2::ZERO, Vec2::new(-60.0, 0.0));

        let after_turn: Vec<PaddleIntent> = (0..6)
            .map(|_| ai.steer(seen, paddle, &tuning, step(), &mut random))
            .collect();
        assert_eq!(
            after_turn,
            vec![
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Still,
                PaddleIntent::Down,
            ],
            "four and a half steps of delay: neither term alone"
        );

        let mut widest: f32 = 0.0;
        for _ in 0..60 {
            ai.steer(flying(away), paddle, &tuning, step(), &mut random);
            ai.steer(seen, paddle, &tuning, step(), &mut random);
            let error = ai.aim().expect("an approach aims").error;
            assert!(error.abs() <= own_error + added_error, "{error}");
            widest = widest.max(error.abs());
        }
        assert!(
            widest > own_error.max(added_error),
            "no aim in sixty was off by more than either term alone: {widest}"
        );
    }

    /// Where a perfect AI on the right aims at a ball from the centre spot,
    /// level, pushed up by 20, when it believes the push to be `believed_push`.
    fn aim_under_a_push_believed_as(believed_push: Option<f32>) -> f32 {
        let tuning = perfect();
        let mut effects = ActiveEffects::default();
        effects.add(Box::new(Misjudged::pushing(20.0, believed_push)));
        let flight = served_under(&effects);
        let mut ai = ai(Side::Right);
        let seen = Sighting {
            ball: ball(Vec2::ZERO, Vec2::new(60.0, 0.0)),
            flight: Some(&flight),
            effects: &effects,
        };
        ai.steer(
            seen,
            paddle_at(Side::Right, 0.0, &tuning),
            &tuning,
            step(),
            &mut Random::seeded(1),
        );
        ai.aim().expect("an approach aims").target_y
    }

    /// Second form: the AI predicts under the flight it believes in. Told
    /// the truth it aims where the ball will cross; blind to the push it
    /// aims straight across; underrating it, in between.
    #[test]
    fn the_ai_aims_where_the_flight_it_believes_in_takes_the_ball() {
        let tuning = perfect();
        let mut effects = ActiveEffects::default();
        effects.add(Box::new(Misjudged::pushing(20.0, Some(20.0))));
        let true_crossing = crossing_y(
            tuning.arena(),
            tuning.ball().radius(),
            ball(Vec2::ZERO, Vec2::new(60.0, 0.0)),
            &served_under(&effects),
            73.0,
            step().as_secs_f32(),
        )
        .expect("the ball flies toward the paddle");
        // 73 units at 60 a second under a push of 20: about 15 up.
        assert!(true_crossing > 10.0, "{true_crossing}");

        assert_near(
            aim_under_a_push_believed_as(Some(20.0)),
            true_crossing,
            1e-4,
        );
        assert_near(aim_under_a_push_believed_as(None), 0.0, 1e-4);
        let underrated = aim_under_a_push_believed_as(Some(10.0));
        assert!(
            underrated > 1.0 && underrated < true_crossing - 1.0,
            "{underrated} against {true_crossing}"
        );
    }

    /// The controller on `side` in a running app.
    fn controller_on(app: &mut App, side: Side) -> MirrorController {
        app.world_mut()
            .query::<&MirrorController>()
            .iter(app.world())
            .copied()
            .find(|controller| controller.side == side)
            .expect("a controller on that side")
    }

    /// The serve and the AI's steering share a phase with no order between
    /// them, and the serve's flight begins a phase later. Whichever runs
    /// first, the AI sees no approach on the serve's step and plans on the
    /// next, under the serve's flight.
    ///
    /// The test cannot see which of the two ran first: nothing in the app
    /// shows it once the AI behaves alike in both orders. That both orders
    /// occur among these twelve seeds was seen while the AI still differed
    /// (M2 phase 4 report: eight of the twelve ran the AI after the serve).
    /// A change to the schedule's shuffle could leave one order untested
    /// and this test green.
    #[test]
    fn whatever_the_system_order_the_ai_plans_a_served_ball_one_step_on_under_its_flight() {
        for seed in 0..12 {
            let mut app = headless_app(seed);
            boot(&mut app);
            app.insert_resource(perfect());
            enter_match(
                &mut app,
                match_of(
                    Controller::Ai(Difficulty::ThreeStars),
                    Controller::Ai(Difficulty::ThreeStars),
                ),
            );
            // A few steps into the countdown: escalation has set the effects
            // in play, and the base has rebuilt them from the replaced
            // tuning, which would drop a probe added before.
            for _ in 0..5 {
                frame(&mut app);
            }
            assert_eq!(match_state(&app), MatchState::Countdown);
            app.world_mut()
                .resource_mut::<ActiveEffects>()
                .add(Box::new(Misjudged::pushing(20.0, Some(20.0))));
            let aims = |app: &mut App| {
                [Side::Left, Side::Right].map(|side| controller_on(app, side).aim())
            };

            let served = (0..1_000).any(|_| {
                frame(&mut app);
                ball_state(&mut app).1.x != 0.0
            });
            assert!(served, "seed {seed}: no serve");
            assert_eq!(
                aims(&mut app),
                [None, None],
                "seed {seed}: planned on the serve's step"
            );

            frame(&mut app);
            let (position, velocity) = ball_state(&mut app);
            let side = if velocity.x > 0.0 {
                Side::Right
            } else {
                Side::Left
            };
            let aim = controller_on(&mut app, side)
                .aim()
                .unwrap_or_else(|| panic!("seed {seed}: no plan one step after the serve"));
            let tuning = app.world().resource::<Tuning>().clone();
            let dt = fixed_timestep(&app).as_secs_f32();
            let contact_x = paddle_on(&mut app, side).contact_x(&tuning);
            let now = Motion { position, velocity };
            let mut flights = app.world_mut().query_filtered::<&BallFlight, With<Ball>>();
            let flight = flights.single(app.world()).expect("exactly one ball");
            assert_near(flight.flight().force(now, dt).y, 20.0, 1e-6);
            let under_the_flight = crossing_y(
                tuning.arena(),
                tuning.ball().radius(),
                now,
                flight.flight(),
                contact_x,
                dt,
            )
            .expect("the served ball flies toward the paddle");
            assert!(
                (aim.target_y - under_the_flight).abs() < 1e-2,
                "seed {seed}: aimed at {} with the ball crossing at {under_the_flight}",
                aim.target_y
            );
        }
    }

    // Plan 5.1 done-conditions: launches as the left side could return them,
    // against three stars on the right with a crafted handicap.

    /// How a launch at the AI ended.
    #[derive(Debug, PartialEq)]
    enum Outcome {
        /// The AI's paddle turned the ball back.
        Returned,
        /// The ball went past: untouched, or clipped by the paddle's end,
        /// which touches it and lets it fly on to the goal.
        Missed,
    }

    /// `count` balls as a left paddle could send them: from just in front of
    /// its contact line at a seeded height, at any angle the paddle can give,
    /// at any speed from the serve to the cap.
    ///
    /// One unit in front, not on the line: a start that rounding puts a hair
    /// inside the idle left paddle's reach counts as struck by it at time
    /// zero (`swept_circle_rect` deflects an overlapping start) and leaves on
    /// another course.
    fn launches(count: usize, tuning: &Tuning) -> Vec<Motion> {
        let mut random = Pcg32::seed(2026);
        let shape = tuning.ball();
        let angle = shape.max_deflection().radians();
        let from_x = Paddle::new(Side::Left, tuning.paddle()).contact_x(tuning) + 1.0;
        let limit_y = tuning.arena().center_limit_y(shape.radius());
        (0..count)
            .map(|_| {
                let y = random.next_f32_in(-limit_y, limit_y);
                let (sin, cos) = random.next_f32_in(-angle, angle).sin_cos();
                let speed = random.next_f32_in(shape.serve_speed(), shape.speed_cap());
                ball(Vec2::new(from_x, y), Vec2::new(cos, sin) * speed)
            })
            .collect()
    }

    /// A match in play, a human seat idle on the left, three stars on the
    /// right playing with `tuning`'s handicap.
    fn ai_on_the_right_playing(tuning: Tuning) -> App {
        ai_on_the_right_playing_after(tuning, |_| {})
    }

    /// [`ai_on_the_right_playing`], with `prepare` run on the booted app
    /// before the match starts.
    fn ai_on_the_right_playing_after(tuning: Tuning, prepare: impl FnOnce(&mut App)) -> App {
        let mut app = headless_app(5);
        record::<BallHitPaddle>(&mut app);
        record::<PointScored>(&mut app);
        boot(&mut app);
        app.insert_resource(tuning);
        prepare(&mut app);
        start_match(
            &mut app,
            match_of(
                Controller::Human(Seat::Left),
                Controller::Ai(Difficulty::ThreeStars),
            ),
        );
        // The first serve is spent before the launches take over the ball.
        frame(&mut app);
        frame(&mut app);
        app
    }

    /// Launches `launch` at the AI and plays until it is returned or scores.
    ///
    /// First the ball rests on the centre spot for half a second, and the AI
    /// heads for the centre, as on the leg of a real rally that flies away
    /// from it: that leg lasts at least 0.58 s at the cap, enough to cross
    /// from either wall.
    fn launch_at_ai(app: &mut App, launch: Motion) -> Outcome {
        place_ball(app, Vec2::ZERO, Vec2::ZERO);
        for _ in 0..30 {
            frame(app);
        }
        assert_eq!(match_state(app), MatchState::Playing, "play stopped");
        take_recorded::<BallHitPaddle>(app);
        take_recorded::<PointScored>(app);
        place_ball(app, launch.position, launch.velocity);
        // The slowest launch, at the serve speed and the steepest angle,
        // takes 3.65 s to reach the AI.
        for _ in 0..300 {
            frame(app);
            let touched = recorded::<BallHitPaddle>(app)
                .iter()
                .any(|hit| hit.side == Side::Right);
            // A touch that leaves the ball flying on toward the goal is an
            // end clip, not a return: wait for the point.
            if touched && ball_state(app).1.x < 0.0 {
                return Outcome::Returned;
            }
            if !recorded::<PointScored>(app).is_empty() {
                return Outcome::Missed;
            }
        }
        panic!("launch {launch:?} was neither returned nor scored");
    }

    #[test]
    fn with_no_delay_and_no_error_the_ai_returns_every_ball() {
        let tuning = perfect();
        let balls = launches(200, &tuning);
        let mut app = ai_on_the_right_playing(tuning);
        for (i, launch) in balls.into_iter().enumerate() {
            assert_eq!(
                launch_at_ai(&mut app, launch),
                Outcome::Returned,
                "launch {i}: {launch:?}"
            );
        }
    }

    #[test]
    fn with_a_large_error_the_ai_misses_some_balls_over_a_seeded_run() {
        // Thirty units off either way; the paddle reaches eight units from
        // its centre to the ball's.
        let tuning = three_stars_with(0.0, 30.0);
        let balls = launches(20, &tuning);
        let mut app = ai_on_the_right_playing(tuning);
        let missed = balls
            .into_iter()
            .any(|launch| launch_at_ai(&mut app, launch) == Outcome::Missed);
        assert!(
            missed,
            "thirty units of error never missed in twenty launches"
        );
    }

    // The stars as tuned (the user's ruling, M2 phase 4 report): no star is
    // unbeatable. What beats three stars is earned, not rolled: a ball fast
    // enough, placed far enough from the centre the AI waits at.

    /// How three stars as tuned answer one ball sent level from the left.
    fn three_stars_as_tuned_against(y: f32, speed: f32) -> Outcome {
        let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
        let from_x = Paddle::new(Side::Left, tuning.paddle()).contact_x(&tuning) + 1.0;
        let mut app = ai_on_the_right_playing(tuning);
        launch_at_ai(&mut app, ball(Vec2::new(from_x, y), Vec2::new(speed, 0.0)))
    }

    #[test]
    fn at_the_speed_cap_a_level_ball_along_the_wall_beats_three_stars() {
        // One unit inside the wall, where the ball's centre turns at 44.
        assert_eq!(three_stars_as_tuned_against(43.0, 250.0), Outcome::Missed);
        assert_eq!(three_stars_as_tuned_against(-43.0, 250.0), Outcome::Missed);
    }

    #[test]
    fn three_stars_return_the_same_ball_sent_slower_or_nearer_the_centre() {
        assert_eq!(three_stars_as_tuned_against(43.0, 80.0), Outcome::Returned);
        assert_eq!(three_stars_as_tuned_against(43.0, 150.0), Outcome::Returned);
        assert_eq!(three_stars_as_tuned_against(20.0, 250.0), Outcome::Returned);
    }

    // M2 spec 2.3.12 for Shrink, whose flaw is inherent (M2 spec 2.6.1): the
    // shorter paddle forgives less of the star's own error. Its raised flaw
    // is its raised dial.

    /// [`ai_on_the_right_playing`] with Shrink at its full dial: the sandbox
    /// forces it beside Speed and holds the return count where the embedded
    /// dial is full, so every flight, the serve's too, has the paddles at
    /// their minimum.
    fn ai_on_the_right_playing_at_shrinks_full_dial(tuning: Tuning) -> App {
        let mut app = ai_on_the_right_playing_after(tuning, |app| {
            let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
            sandbox.toggle(EffectId::Shrink, &[EffectId::Speed]);
            sandbox.hold_returns(Some(15));
        });
        let shape = app.world().resource::<Tuning>().paddle();
        assert_near(
            paddle_on(&mut app, Side::Right).height(),
            shape.height_bounds().min(),
            1e-4,
        );
        app
    }

    #[test]
    fn at_shrinks_full_dial_an_ai_with_no_delay_and_no_error_returns_every_ball() {
        let tuning = perfect();
        let balls = launches(200, &tuning);
        let mut app = ai_on_the_right_playing_at_shrinks_full_dial(tuning);
        for (i, launch) in balls.into_iter().enumerate() {
            assert_eq!(
                launch_at_ai(&mut app, launch),
                Outcome::Returned,
                "launch {i}: {launch:?}"
            );
        }
    }

    /// Six units of error: inside the whole paddle's reach of eight (half
    /// its height of 14, plus the ball's radius), outside the shrunk
    /// paddle's reach of five.
    #[test]
    fn an_error_whole_paddles_forgive_misses_some_balls_at_shrinks_full_dial() {
        let tuning = three_stars_with(0.0, 6.0);
        let balls = launches(40, &tuning);

        let mut whole = ai_on_the_right_playing(tuning.clone());
        for (i, launch) in balls.iter().enumerate() {
            assert_eq!(
                launch_at_ai(&mut whole, *launch),
                Outcome::Returned,
                "whole paddles, launch {i}: {launch:?}"
            );
        }

        let mut shrunk = ai_on_the_right_playing_at_shrinks_full_dial(tuning);
        let missed = balls
            .into_iter()
            .any(|launch| launch_at_ai(&mut shrunk, launch) == Outcome::Missed);
        assert!(
            missed,
            "six units of error never missed in forty launches at the minimum height"
        );
    }
    // M2 spec 2.3.12 for Wind, whose flaw is the second form (M2 spec
    // 2.6.2): each star believes a share of the true drift. R5: the AI
    // predicts the wind through the shared math, and the share is the dial
    // of its flaw. R2: the same balls in the same wind are returned or
    // missed by how the paddle is played.

    /// Three stars with no handicap of their own, believing `share` of the
    /// wind's true drift.
    fn no_handicap_believing(share: f32) -> Tuning {
        let old = "three_stars: 0.9";
        let ron = EMBEDDED_RON
            .replace(
                "three_stars: (reaction_delay_seconds: 0.24, prediction_error: 3.0)",
                "three_stars: (reaction_delay_seconds: 0.0, prediction_error: 0.0)",
            )
            .replace(old, &format!("three_stars: {share:?}"));
        assert!(EMBEDDED_RON.contains(old), "three stars' share not found");
        Tuning::from_ron(&ron).expect("the crafted flaw is valid")
    }

    /// [`ai_on_the_right_playing`] in Wind, beside Speed, with every flight
    /// starting at `returns`: 12 is the embedded wind's full dial, 24 units
    /// of drift a crossing.
    fn ai_on_the_right_playing_in_wind(tuning: Tuning, returns: u16) -> App {
        ai_on_the_right_playing_after(tuning, |app| {
            let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
            sandbox.toggle(EffectId::Wind, &[EffectId::Speed]);
            sandbox.hold_returns(Some(returns));
        })
    }

    #[test]
    fn in_full_wind_an_ai_that_believes_all_of_it_and_has_no_handicap_returns_every_ball() {
        let tuning = no_handicap_believing(1.0);
        let balls = launches(200, &tuning);
        let mut app = ai_on_the_right_playing_in_wind(tuning, 12);
        for (i, launch) in balls.into_iter().enumerate() {
            assert_eq!(
                launch_at_ai(&mut app, launch),
                Outcome::Returned,
                "launch {i}: {launch:?}"
            );
        }
    }

    /// Believing two fifths of 24 units leaves the aim up to 14 off: past
    /// the paddle's reach of eight on a long enough crossing.
    #[test]
    fn in_full_wind_an_ai_that_believes_too_little_of_it_misses_some_balls() {
        let tuning = no_handicap_believing(0.4);
        let balls = launches(40, &tuning);
        let mut app = ai_on_the_right_playing_in_wind(tuning, 12);
        let missed = balls
            .into_iter()
            .any(|launch| launch_at_ai(&mut app, launch) == Outcome::Missed);
        assert!(
            missed,
            "believing two fifths of the wind never missed in forty launches"
        );
    }

    /// M2 spec 2.6.2: the error scales with strength. The same misjudging
    /// AI misses nothing while the dial is at zero: there is no wind to
    /// misjudge.
    #[test]
    fn with_the_dial_at_zero_the_same_misjudging_ai_returns_every_ball() {
        let tuning = no_handicap_believing(0.4);
        let balls = launches(40, &tuning);
        let mut app = ai_on_the_right_playing_in_wind(tuning, 0);
        for (i, launch) in balls.into_iter().enumerate() {
            assert_eq!(
                launch_at_ai(&mut app, launch),
                Outcome::Returned,
                "launch {i}: {launch:?}"
            );
        }
    }
}
