//! AI, mirror controller (spec 1.4.6): steers each paddle the match config
//! gives to the computer, aiming where the kernel predicts the ball will cross.

use std::time::Duration;

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::error::BevyError;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::{Fixed, Time};

use crate::domain::{Controller, Difficulty, MatchConfig, Random};
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::paddle::Paddle;
use crate::features::seats::{PaddleIntent, PaddleIntents};
use crate::features::tuning::Tuning;
use crate::flow::phases::FixedStepPhase;
use crate::flow::states::AppState;
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

/// Each controller reads the ball as it is now and writes its side's intent.
///
/// The serve also runs in the intent phase, unordered against this system:
/// on the step the ball is served toward an AI, the AI sees the ball either
/// still at rest or already served, as the schedule falls, so its reaction
/// may start one step late. Every other turn comes from a collision in an
/// earlier step and is seen at once.
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
    balls: Query<(&Position, &Velocity), With<Ball>>,
    paddles: Query<&Paddle>,
    mut controllers: Query<&mut MirrorController>,
) -> Result<(), BevyError> {
    if controllers.is_empty() {
        return Ok(());
    }
    let (position, velocity) = balls
        .single()
        .map_err(|err| BevyError::from(format!("AI steering needs exactly one ball: {err}")))?;
    let ball = Motion {
        position: position.0,
        velocity: velocity.0,
    };
    for mut controller in &mut controllers {
        let side = controller.side;
        let Some(paddle) = paddles.iter().find(|paddle| paddle.side() == side) else {
            return Err(BevyError::from(format!(
                "AI steering found no paddle on the {side:?} side"
            )));
        };
        let intent = controller.steer(ball, *paddle, &tuning, time.delta(), &mut random);
        intents.set(side, intent);
    }
    Ok(())
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
    /// The ball is coming: once `hold_for` has run out, meet it at `target_y`.
    Meet {
        /// The predicted crossing plus this approach's error. Fixed for the
        /// approach.
        target_y: f32,
        /// What remains of the reaction delay. The paddle holds still while
        /// it is not zero; each held step spends one timestep of it.
        hold_for: Duration,
    },
}

impl MirrorController {
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

    /// What this controller presses for one fixed step, given the ball's
    /// motion now and its paddle (spec 1.4.6).
    ///
    /// - The ball approaches when it flies toward this side's goal.
    /// - On the step the ball turns toward this side, the target becomes the
    ///   kernel's predicted crossing of the paddle's contact line
    ///   ([`crossing_y`] at [`Paddle::contact_x`]) plus an error drawn once
    ///   from `±prediction_error`. It is not recomputed until the next approach.
    /// - The paddle holds still until `reaction_delay` has passed since the
    ///   turn, then steers to the target. A zero delay steers on the turn step.
    /// - While the ball is not approaching, or an approach has no crossing,
    ///   the paddle steers to the centre.
    ///
    /// Preconditions (the caller's): `paddle` is on this controller's side;
    /// `ball` is finite; `dt` is the fixed timestep.
    /// Postcondition: `random` advanced by exactly one draw on a turn with a
    /// crossing, by none otherwise.
    pub fn steer(
        &mut self,
        ball: Motion,
        paddle: Paddle,
        tuning: &Tuning,
        dt: Duration,
        random: &mut Random,
    ) -> PaddleIntent {
        let approaching = ball.velocity.x * self.side.sign() > 0.0;
        if !approaching {
            self.plan = Plan::Centre;
        } else if !self.was_approaching {
            self.plan = self.plan_approach(ball, paddle, tuning, dt, random);
        }
        self.was_approaching = approaching;

        let step_travel = tuning.paddle().speed() * dt.as_secs_f32();
        match &mut self.plan {
            Plan::Centre => intent_toward(paddle, 0.0, step_travel),
            Plan::Meet { target_y, hold_for } if hold_for.is_zero() => {
                intent_toward(paddle, *target_y, step_travel)
            }
            Plan::Meet { hold_for, .. } => {
                *hold_for = hold_for.saturating_sub(dt);
                PaddleIntent::Still
            }
        }
    }

    /// The plan for an approach that begins now: meet the ball where the
    /// kernel predicts it crosses the paddle's contact line, off by one draw
    /// from `±prediction_error`, after the reaction delay. No crossing, no draw.
    fn plan_approach(
        self,
        ball: Motion,
        paddle: Paddle,
        tuning: &Tuning,
        dt: Duration,
        random: &mut Random,
    ) -> Plan {
        let crossing = crossing_y(
            tuning.arena(),
            tuning.ball().radius(),
            ball,
            paddle.contact_x(tuning),
            dt.as_secs_f32(),
        );
        let Some(predicted_y) = crossing else {
            return Plan::Centre;
        };
        let handicap = tuning.ai().level(self.difficulty);
        let spread = handicap.prediction_error();
        Plan::Meet {
            target_y: predicted_y + random.next_f32_in(-spread, spread),
            hold_for: handicap.reaction_delay(),
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
    use std::time::Duration;

    use bevy::app::App;
    use bevy::math::Vec2;

    use super::{MirrorController, intent_toward};
    use crate::domain::{
        BallHitPaddle, Controller, Difficulty, MatchConfig, PointScored, Random, Seat,
    };
    use crate::features::paddle::Paddle;
    use crate::features::seats::PaddleIntent;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::FIXED_HZ;
    use crate::flow::states::MatchState;
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::kernel::random::Pcg32;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        boot, enter_match, frame, headless_app, leave_match, match_state, place_ball, record,
        recorded, start_match, take_recorded,
    };

    fn match_of(left: Controller, right: Controller) -> MatchConfig {
        MatchConfig {
            left,
            right,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
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
            "three_stars: (reaction_delay_seconds: 0.05, prediction_error: 1.0)",
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
            let intent = ai.steer(now, paddle, tuning, step(), random);
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
                let intent = ai(Side::Right).steer(course, paddle, &tuning, step(), &mut random);
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
            ai.steer(coming, paddle, &tuning, step(), &mut random),
            PaddleIntent::Up
        );
        assert_eq!(
            ai.steer(returned, paddle, &tuning, step(), &mut random),
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
            ai.steer(away, paddle, &tuning, step(), &mut random),
            PaddleIntent::Down
        );
        let after_turn: Vec<PaddleIntent> = (0..5)
            .map(|_| ai.steer(coming, paddle, &tuning, step(), &mut random))
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
            ai.steer(away, paddle, &tuning, step(), &mut random);
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

    #[test]
    fn a_changed_ball_course_mid_approach_does_not_move_the_target() {
        let tuning = perfect();
        let mut random = Random::seeded(1);
        let mut ai = ai(Side::Right);
        let paddle = paddle_at(Side::Right, 0.0, &tuning);
        // The turn aims at the crossing at y = 36.5.
        ai.steer(
            ball(Vec2::ZERO, Vec2::new(60.0, 30.0)),
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
            ai(Side::Right).steer(crawling, paddle, &tuning, step(), &mut random),
            PaddleIntent::Down
        );
    }

    // Plan 5.1 done-conditions: launches as the left side could return them,
    // against three stars on the right with a crafted handicap.

    /// How a launch at the AI ended.
    #[derive(Debug, PartialEq)]
    enum Outcome {
        Returned,
        Missed,
    }

    /// `count` balls as a left paddle could send them: from just in front of
    /// its contact line at a seeded height, at any angle the paddle can give,
    /// at any speed from the serve to the cap.
    ///
    /// One unit in front, not on the line: a ball touching the idle left
    /// paddle's face counts as struck by it at time zero (`swept_circle_rect`
    /// treats a touching start as an overlap) and leaves on another course.
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
                let speed = random.next_f32_in(shape.serve_speed(), shape.speed_rule().cap());
                ball(Vec2::new(from_x, y), Vec2::new(cos, sin) * speed)
            })
            .collect()
    }

    /// A match in play, a human seat idle on the left, three stars on the
    /// right playing with `tuning`'s handicap.
    fn ai_on_the_right_playing(tuning: Tuning) -> App {
        let mut app = headless_app(5);
        record::<BallHitPaddle>(&mut app);
        record::<PointScored>(&mut app);
        boot(&mut app);
        app.insert_resource(tuning);
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
            if recorded::<BallHitPaddle>(app)
                .iter()
                .any(|hit| hit.side == Side::Right)
            {
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
}
