//! Effects (M2 spec 2.2, 2.3; ADR 0011): the contract every effect
//! implements, the effects in play, and the flight they compose on the ball.
//!
//! An effect never touches the simulation itself. At each flight start (a
//! serve or a paddle return) it is asked once for its
//! [`Contribution`](crate::kernel::flight::Contribution); the base composes
//! the answers into the ball's [`Flight`], which physics and the AI's
//! prediction read. One module per effect; effects do not import each other.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::state::state::OnEnter;

use crate::domain::{EffectId, Serve};
use crate::features::ball::Ball;
use crate::features::tuning::Tuning;
use crate::flow::phases::{BallInPlay, FixedStepPhase};
use crate::flow::states::{AppState, MatchState};
use crate::kernel::flight::{Contribution, Flight, FlightStart};

pub mod speed;

/// One effect's core half (ADR 0011).
///
/// No method has a default body: an effect states its identity and its
/// stance on every channel, or it does not compile. `Send + Sync + 'static`
/// because the effects in play live in a resource.
pub trait Effect: Send + Sync + 'static {
    /// Which effect this is.
    fn id(&self) -> EffectId;

    /// This effect's share of the flight that begins at `start`. Called once
    /// per flight; the answer holds until the next flight starts.
    fn contribute(&self, start: &FlightStart) -> Contribution;
}

/// The core's registration of every effect: the one place in this crate that
/// matches on [`EffectId`]. A new variant does not compile until it is built here.
fn build(id: EffectId, tuning: &Tuning) -> Box<dyn Effect> {
    match id {
        EffectId::Speed => Box::new(speed::Speed::new(tuning)),
    }
}

/// Wires every effect into the schedule (M2 spec 2.3.11), so an effect
/// writes systems only for state the base lacks.
///
/// - Makes [`BallFlight`] a required component of [`Ball`]. Precondition
///   (Bevy's): no `Ball` exists before this plugin builds.
/// - Entering a match puts the mode's effects in play. Until modes exist
///   (M2 plan phase 2) that is Speed alone: Classic Pong.
/// - Entering a countdown clears the ball's flight; the serve starts the
///   rally's first flight in the effects phase. Physics starts every later
///   flight at a paddle return, through [`ActiveEffects::start_flight`].
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveEffects>()
            .register_required_components::<Ball, BallFlight>()
            .add_systems(OnEnter(AppState::InMatch), put_classic_effects_in_play)
            .add_systems(OnEnter(MatchState::Countdown), clear_flights)
            .add_systems(
                FixedUpdate,
                (
                    rebuild_effects_from_changed_tuning.in_set(FixedStepPhase::Effects),
                    start_flight_on_serve
                        .in_set(FixedStepPhase::Effects)
                        .in_set(BallInPlay),
                ),
            );
    }
}

/// The effects in play, in no meaningful order: every channel composes by sum.
///
/// Invariant: `effects` holds exactly the effects `ids` names, each built
/// from the tuning last seen.
#[derive(Resource, Default)]
pub struct ActiveEffects {
    ids: Vec<EffectId>,
    effects: Vec<Box<dyn Effect>>,
}

impl ActiveEffects {
    /// Puts exactly `ids` in play, each built from `tuning`.
    pub fn set(&mut self, ids: &[EffectId], tuning: &Tuning) {
        self.ids = ids.to_vec();
        self.effects = ids.iter().map(|id| build(*id, tuning)).collect();
    }

    /// Puts one more effect in play, as given: a test's probe, which no
    /// tuning builds.
    #[cfg(test)]
    pub(crate) fn add(&mut self, effect: Box<dyn Effect>) {
        self.ids.push(effect.id());
        self.effects.push(effect);
    }

    /// The effects in play.
    #[must_use]
    pub fn ids(&self) -> &[EffectId] {
        &self.ids
    }

    /// The flight that begins at `start`: every effect in play asked once
    /// for its contribution (M2 spec 2.2.4).
    #[must_use]
    pub fn start_flight(&self, start: FlightStart) -> Flight {
        Flight::start(
            start,
            self.effects.iter().map(|effect| effect.contribute(&start)),
        )
    }
}

/// The flight a ball is in: the rally's return count, the flight's clock, and
/// the composed contributions of the effects in play.
///
/// Invariant: the flight is replaced only at a flight start, by
/// [`begin`](Self::begin), and cleared only between rallies; in between, only
/// its clock advances.
#[derive(Component, Debug, Default)]
pub struct BallFlight(Flight);

impl BallFlight {
    /// The flight, for physics and prediction to read.
    #[must_use]
    pub const fn flight(&self) -> &Flight {
        &self.0
    }

    /// Starts a new flight in place of the old.
    pub fn begin(&mut self, flight: Flight) {
        self.0 = flight;
    }

    /// Advances the flight's clock by one fixed step.
    pub const fn tick(&mut self) {
        self.0.tick();
    }
}

fn put_classic_effects_in_play(mut active: ResMut<ActiveEffects>, tuning: Res<Tuning>) {
    active.set(&[EffectId::Speed], &tuning);
}

/// A reloaded tuning file (dev override) reaches the effects in play: each is
/// rebuilt from the new values before the next flight starts.
fn rebuild_effects_from_changed_tuning(mut active: ResMut<ActiveEffects>, tuning: Res<Tuning>) {
    if !tuning.is_changed() {
        return;
    }
    let ids = active.ids().to_vec();
    active.set(&ids, &tuning);
}

/// Every countdown starts a new rally with the ball at rest (ADR 0003): no
/// returns yet, no forces.
fn clear_flights(mut flights: Query<&mut BallFlight, With<Ball>>) {
    for mut flight in &mut flights {
        flight.begin(Flight::default());
    }
}

/// The serve starts the rally's first flight. The ball feature launches the
/// ball in the intent phase; this runs in the effects phase of the same step,
/// before the ball first moves.
fn start_flight_on_serve(
    mut serves: MessageReader<Serve>,
    active: Res<ActiveEffects>,
    mut flights: Query<&mut BallFlight, With<Ball>>,
) {
    if serves.read().last().is_none() {
        return;
    }
    for mut flight in &mut flights {
        flight.begin(active.start_flight(FlightStart::SERVE));
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;
    use std::sync::{Arc, Mutex};

    use bevy::app::App;
    use bevy::ecs::query::With;
    use bevy::math::Vec2;
    use bevy::state::state::NextState;

    use super::{ActiveEffects, BallFlight, Effect};
    use crate::domain::{BallHitPaddle, Controller, EffectId, MatchConfig, Seat, SeatPlayers};
    use crate::features::ball::{Ball, Position, Velocity};
    use crate::features::seats::PaddleIntent;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::states::MatchState;
    use crate::kernel::flight::{Contribution, Field, FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::kernel::predict::crossing_y;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        ball_state, boot, enter_match, fixed_timestep, frame, headless_app, match_state,
        place_ball, record, report_intent, run_until, start_match, take_recorded,
    };

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn playing() -> App {
        let mut app = headless_app(1);
        record::<BallHitPaddle>(&mut app);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app
    }

    /// Into the right paddle's face within one step, whatever the speed named.
    fn into_the_right_paddle(app: &mut App, velocity: Vec2) {
        place_ball(app, Vec2::new(72.0, 2.0), velocity);
        frame(app);
    }

    fn returns(app: &mut App) -> u16 {
        app.world_mut()
            .query_filtered::<&BallFlight, With<Ball>>()
            .single(app.world())
            .expect("exactly one ball")
            .flight()
            .returns()
    }

    /// The speed a ball leaves its `returns`-th return at, by the tuning in `app`.
    fn launch_speed(app: &App, returns: u16) -> f32 {
        let shape = app.world().resource::<Tuning>().ball();
        let rule = shape.speed_rule();
        rule.launch(
            shape.serve_speed(),
            rule.per_hit_increase() * f32::from(returns),
        )
    }

    /// Records every flight start it is asked about and contributes nothing.
    /// It borrows Speed's name: the identity enum is closed, and a probe is
    /// no effect of the game.
    struct Probe(Arc<Mutex<Vec<FlightStart>>>);

    impl Effect for Probe {
        fn id(&self) -> EffectId {
            EffectId::Speed
        }

        fn contribute(&self, start: &FlightStart) -> Contribution {
            self.0
                .lock()
                .expect("the probe's log is not poisoned")
                .push(*start);
            Contribution {
                speed_gain: 0.0,
                field: None,
            }
        }
    }

    #[test]
    fn a_match_puts_speed_alone_in_play() {
        let app = playing();
        assert_eq!(
            app.world().resource::<ActiveEffects>().ids(),
            [EffectId::Speed]
        );
    }

    /// M2 spec 2.2.4's done-condition: asked exactly once per flight, at the
    /// serve and at each return, and told the paddle's motion at a hit.
    #[test]
    fn an_effect_is_asked_once_per_flight_and_sees_the_paddles_motion_at_a_hit() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(&mut app, two_humans());
        let asked = Arc::new(Mutex::new(Vec::new()));
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .add(Box::new(Probe(Arc::clone(&asked))));
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Playing,
            1_000,
            "play never began",
        );
        for _ in 0..5 {
            frame(&mut app);
        }
        let seen = |asked: &Arc<Mutex<Vec<FlightStart>>>| {
            asked
                .lock()
                .expect("the probe's log is not poisoned")
                .clone()
        };
        assert_eq!(
            seen(&asked),
            [FlightStart::SERVE],
            "the serve starts one flight"
        );

        report_intent(&mut app, Seat::Right, PaddleIntent::Up);
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        let first_return = FlightStart {
            returns: 1,
            hit: Some(Hit {
                side: Side::Right,
                paddle_motion: StepMotion::Up,
            }),
        };
        assert_eq!(seen(&asked), [FlightStart::SERVE, first_return]);

        report_intent(&mut app, Seat::Right, PaddleIntent::Still);
        for _ in 0..5 {
            frame(&mut app);
        }
        assert_eq!(seen(&asked).len(), 2, "nothing is asked in mid-flight");
    }

    #[test]
    fn each_return_counts_once_and_a_countdown_starts_the_count_again() {
        let mut app = playing();
        assert_eq!(returns(&mut app), 0);
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_eq!(returns(&mut app), 1);
        frame(&mut app);
        assert_eq!(returns(&mut app), 1, "a flight keeps its count");

        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        assert_eq!(returns(&mut app), 0);
    }

    /// M2 spec 2.0.4's done-condition: the launch speed follows the return
    /// count, not the speed the ball arrived at.
    #[test]
    fn a_ball_given_extra_speed_in_flight_leaves_its_next_hit_at_the_counts_speed() {
        let mut leaving = Vec::new();
        for arriving in [Vec2::new(80.0, 0.0), Vec2::new(150.0, 90.0)] {
            let mut app = playing();
            into_the_right_paddle(&mut app, arriving);
            assert_eq!(
                take_recorded::<BallHitPaddle>(&mut app).len(),
                1,
                "{arriving:?}"
            );
            assert_near(ball_state(&mut app).1.length(), launch_speed(&app, 1), 1e-3);
            leaving.push(ball_state(&mut app).1.length());
        }
        assert_near(leaving[0], leaving[1], 1e-3);
        assert_near(leaving[0], 87.0, 1e-3);
    }

    /// A ball that clips a paddle's end is touched, not returned: it flies on
    /// in the same flight at the same speed. M1 raised the speed on a face
    /// hit only; the return count keeps that.
    #[test]
    fn a_clip_on_a_paddles_end_is_a_touch_but_starts_no_flight() {
        let mut app = playing();
        // The right paddle spans x 74..76 and y -7..7: straight down onto its top end.
        place_ball(&mut app, Vec2::new(75.0, 9.0), Vec2::new(0.0, -200.0));
        frame(&mut app);
        assert_eq!(
            take_recorded::<BallHitPaddle>(&mut app),
            vec![BallHitPaddle { side: Side::Right }]
        );
        assert_eq!(returns(&mut app), 0);
        let (_, velocity) = ball_state(&mut app);
        assert!(
            velocity.y > 0.0,
            "the end mirrors the ball back up: {velocity:?}"
        );
        assert_near(velocity.length(), 200.0, 1e-3);
    }

    /// A reloaded tuning file (dev override) reaches the effects in play.
    #[test]
    fn a_reloaded_increase_reaches_speed_before_the_next_return() {
        let mut app = playing();
        let steeper = EMBEDDED_RON.replace("per_hit_increase: 7.0", "per_hit_increase: 20.0");
        assert_ne!(
            steeper, EMBEDDED_RON,
            "the increase was not found in the RON"
        );
        app.insert_resource(Tuning::from_ron(&steeper).expect("a steeper ramp is valid"));
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        assert_near(ball_state(&mut app).1.length(), 100.0, 1e-3);
    }

    /// A push that swings with time in flight: wrong unless the clock runs.
    #[derive(Debug)]
    struct Swing;

    impl Field for Swing {
        fn lateral(&self, _motion: Motion, seconds_in_flight: f32) -> f32 {
            60.0 * (4.0 * seconds_in_flight).cos()
        }
    }

    /// Contributes [`Swing`] to every flight, under Speed's borrowed name.
    struct Swinger;

    impl Effect for Swinger {
        fn id(&self) -> EffectId {
            EffectId::Speed
        }

        fn contribute(&self, _start: &FlightStart) -> Contribution {
            Contribution {
                speed_gain: 0.0,
                field: Some(Box::new(Swing)),
            }
        }
    }

    /// M2 spec 2.2.1 through the systems: the physics step applies the
    /// flight's force and runs its clock, so the ball goes where the kernel
    /// predicts from the same flight.
    #[test]
    fn under_a_field_the_ball_crosses_where_the_kernel_predicts_from_its_flight() {
        let mut app = playing();
        app.world_mut()
            .resource_mut::<ActiveEffects>()
            .add(Box::new(Swinger));
        // The return starts a flight that carries the field; the ball leaves leftward.
        into_the_right_paddle(&mut app, Vec2::new(200.0, 0.0));
        for _ in 0..10 {
            frame(&mut app);
        }
        let tuning = app.world().resource::<Tuning>().clone();
        let dt = fixed_timestep(&app).as_secs_f32();
        let target_x = -60.0;
        let predicted = {
            let mut query = app
                .world_mut()
                .query_filtered::<(&Position, &Velocity, &BallFlight), With<Ball>>();
            let (position, velocity, flight) = query.single(app.world()).expect("exactly one ball");
            let now = Motion {
                position: position.0,
                velocity: velocity.0,
            };
            crossing_y(
                tuning.arena(),
                tuning.ball().radius(),
                now,
                flight.flight(),
                target_x,
                dt,
            )
            .expect("the ball flies toward the target")
        };
        let unforced = {
            let (position, velocity) = ball_state(&mut app);
            position.y + velocity.y * (target_x - position.x) / velocity.x
        };
        assert!(
            (predicted - unforced).abs() > 3.0,
            "the field bends the course: {predicted} against {unforced}"
        );
        let mut before = ball_state(&mut app).0;
        for _ in 0..600 {
            frame(&mut app);
            let after = ball_state(&mut app).0;
            if after.x <= target_x {
                let fraction = (target_x - before.x) / (after.x - before.x);
                assert_near(before.y + fraction * (after.y - before.y), predicted, 1e-2);
                return;
            }
            before = after;
        }
        panic!("the ball never reached x = {target_x}");
    }
}
