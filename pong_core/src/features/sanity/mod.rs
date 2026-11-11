//! Sanity (spec 1.4.7): the last phase of every fixed step checks that the
//! simulation is still sound, and reports a [`SimFault`] when it is not.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::error::BevyError;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res};
use bevy::time::{Fixed, Time};

use crate::domain::SimFault;
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::effects::BallFlight;
use crate::features::paddle::Paddle;
use crate::features::tuning::Tuning;
use crate::flow::phases::FixedStepPhase;
use crate::kernel::flight::Flight;
use crate::kernel::integrate::Motion;

/// Checks the simulation's invariants in the sanity phase of every step in
/// which the phases run: in play and in the waits between rallies
/// (ADR 0004), so not in `BallInPlay`.
///
/// One code path in both builds (docs/architecture.md, error policy): a
/// broken invariant writes a [`SimFault`] and returns it as the system's
/// error. The app's error handler decides the rest: the dev policy panics;
/// the release policy logs it at error level, and rules answers the message
/// by restarting the rally.
pub struct SanityPlugin;

impl Plugin for SanityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, check_invariants.in_set(FixedStepPhase::Sanity));
    }
}

/// Exactly one ball, sound, in a flight of sane length; both paddles sound.
/// Reports the first fault found.
///
/// # Errors
/// The [`SimFault`] found, after writing it as a message.
fn check_invariants(
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    balls: Query<(&Position, &Velocity, &BallFlight), With<Ball>>,
    paddles: Query<&Paddle>,
    mut faults: MessageWriter<SimFault>,
) -> Result<(), BevyError> {
    let verdict = check_world(&tuning, time.delta_secs(), &balls, &paddles);
    if let Err(fault) = &verdict {
        faults.write(fault.clone());
    }
    Ok(verdict?)
}

fn check_world(
    tuning: &Tuning,
    step_seconds: f32,
    balls: &Query<(&Position, &Velocity, &BallFlight), With<Ball>>,
    paddles: &Query<&Paddle>,
) -> Result<(), SimFault> {
    let count = balls.iter().len();
    let Ok((position, velocity, flight)) = balls.single() else {
        return Err(SimFault::BallCount { count });
    };
    let ball = Motion {
        position: position.0,
        velocity: velocity.0,
    };
    check_ball(ball, tuning, step_seconds)?;
    check_flight(flight.flight(), tuning, step_seconds)?;
    paddles
        .iter()
        .try_for_each(|paddle| check_paddle(*paddle, tuning))
}

/// Float slack on the wall checks: a wall bounce folds the ball back to the
/// limit exactly, but rounding may leave it a hair beyond.
const TOLERANCE: f32 = 1e-3;

/// The ball is sound when its velocity is finite, it is inside the walls,
/// and it is no farther past a goal line than it can be: the rules score a
/// ball once it is wholly past the line, and the step that scores is the last
/// of its frame, so it ends at most one step at the speed cap beyond that.
///
/// `step_seconds` is the fixed timestep.
fn check_ball(ball: Motion, tuning: &Tuning, step_seconds: f32) -> Result<(), SimFault> {
    if !ball.velocity.is_finite() {
        return Err(SimFault::BallVelocityNotFinite {
            velocity: ball.velocity,
        });
    }
    let shape = tuning.ball();
    let radius = shape.radius();
    let limit_x =
        tuning.arena().half_extents().x + radius.value() + shape.speed_cap() * step_seconds;
    let limit_y = tuning.arena().center_limit_y(radius) + TOLERANCE;
    let at = ball.position;
    let inside = at.is_finite() && at.x.abs() <= limit_x && at.y.abs() <= limit_y;
    if inside {
        Ok(())
    } else {
        Err(SimFault::BallOutsideArena { position: at })
    }
}

/// A flight is sound while it is no longer than the tuned limit (M2 spec
/// 2.5.6). Every flight ends at a paddle return or a goal; one that does
/// neither for this long is a stalled or trapped ball, which no effect's own
/// guard need rule out. The clock runs only while the ball is in play, so
/// neither a pause nor a countdown counts.
///
/// `step_seconds` is the fixed timestep.
fn check_flight(flight: &Flight, tuning: &Tuning, step_seconds: f32) -> Result<(), SimFault> {
    let seconds = f32::from(flight.steps()) * step_seconds;
    let limit = tuning.ball().flight_limit_seconds();
    if seconds <= limit {
        Ok(())
    } else {
        Err(SimFault::FlightTooLong { seconds, limit })
    }
}

/// The paddle is sound when its centre is finite and the whole paddle is
/// inside the walls.
fn check_paddle(paddle: Paddle, tuning: &Tuning) -> Result<(), SimFault> {
    let reach = paddle.center_y().abs() + paddle.height() / 2.0;
    let inside =
        paddle.center_y().is_finite() && reach <= tuning.arena().half_extents().y + TOLERANCE;
    if inside {
        Ok(())
    } else {
        Err(SimFault::PaddleOutsideArena {
            side: paddle.side(),
            center_y: paddle.center_y(),
            height: paddle.height(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::math::Vec2;

    use super::{check_ball, check_flight, check_paddle};
    use crate::app::error_policy::{ErrorPolicy, ErrorPolicyPlugin};
    use crate::domain::{Controller, MatchConfig, Mode, Seat, SeatPlayers, Serve, SimFault};
    use crate::features::ball::{Ball, Position, Velocity};
    use crate::features::paddle::Paddle;
    use crate::features::rules::Score;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::flow::states::MatchState;
    use crate::kernel::flight::Flight;
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::testing::{
        ball_state, boot, enter_match, frame, headless_app, headless_app_with, match_state,
        place_ball, record, run_until, start_match, take_recorded,
    };

    // The embedded tuning: goal lines at x = ±80, walls at y = ±45, ball
    // radius 1, speed cap 250. At 60 Hz one step at the cap is 4.1667, so a
    // ball's centre may reach x = ±85.1667 and y = ±44.

    const STEP: f32 = 1.0 / 60.0;

    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn ball(position: Vec2, velocity: Vec2) -> Motion {
        Motion { position, velocity }
    }

    #[test]
    fn a_ball_in_flight_inside_the_arena_is_sound() {
        let flying = ball(Vec2::new(30.0, -20.0), Vec2::new(-120.0, 60.0));
        assert_eq!(check_ball(flying, &tuning(), STEP), Ok(()));
    }

    #[test]
    fn a_ball_with_a_nan_velocity_is_a_fault() {
        let broken = ball(Vec2::ZERO, Vec2::new(f32::NAN, 10.0));
        assert!(matches!(
            check_ball(broken, &tuning(), STEP),
            Err(SimFault::BallVelocityNotFinite { .. })
        ));
    }

    #[test]
    fn a_ball_at_a_nan_position_is_a_fault() {
        let broken = ball(Vec2::new(0.0, f32::NAN), Vec2::new(60.0, 0.0));
        assert!(matches!(
            check_ball(broken, &tuning(), STEP),
            Err(SimFault::BallOutsideArena { .. })
        ));
    }

    #[test]
    fn a_ball_through_a_wall_is_a_fault() {
        for y in [44.5, -44.5] {
            let through = ball(Vec2::new(0.0, y), Vec2::new(60.0, 0.0));
            assert_eq!(
                check_ball(through, &tuning(), STEP),
                Err(SimFault::BallOutsideArena {
                    position: through.position
                }),
                "y = {y}"
            );
        }
    }

    /// ADR 0004: a ball resting past the goal line after a point is legal.
    #[test]
    fn a_ball_one_step_at_the_cap_past_a_goal_is_sound_and_any_farther_is_a_fault() {
        for sign in [1.0, -1.0] {
            let scored = ball(Vec2::new(sign * 85.1, 10.0), Vec2::new(sign * 250.0, 0.0));
            assert_eq!(
                check_ball(scored, &tuning(), STEP),
                Ok(()),
                "x = {}",
                sign * 85.1
            );
            let lost = ball(Vec2::new(sign * 85.3, 10.0), Vec2::new(sign * 250.0, 0.0));
            assert_eq!(
                check_ball(lost, &tuning(), STEP),
                Err(SimFault::BallOutsideArena {
                    position: lost.position
                }),
                "x = {}",
                sign * 85.3
            );
        }
    }

    #[test]
    fn a_paddle_against_a_wall_is_sound() {
        let tuning = tuning();
        let mut paddle = Paddle::new(Side::Left, tuning.paddle());
        paddle.move_by(1_000.0, tuning.arena());
        assert_eq!(check_paddle(paddle, &tuning), Ok(()));
    }

    #[test]
    fn a_paddle_at_a_nan_height_is_a_fault() {
        let tuning = tuning();
        let mut paddle = Paddle::new(Side::Right, tuning.paddle());
        // The clamp keeps a paddle inside the walls but passes NaN through.
        paddle.move_by(f32::NAN, tuning.arena());
        assert!(matches!(
            check_paddle(paddle, &tuning),
            Err(SimFault::PaddleOutsideArena {
                side: Side::Right,
                ..
            })
        ));
    }

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
            mode: Mode::Classic,
        }
    }

    /// A match in play under the harness's dev error policy.
    fn playing() -> App {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app
    }

    /// Spec 1.4.7: a developer sees the fault the first time it happens.
    #[test]
    #[should_panic(expected = "ball velocity [NaN, 0] is not finite")]
    fn under_the_dev_policy_an_injected_nan_velocity_panics_in_that_step() {
        let mut app = playing();
        place_ball(&mut app, Vec2::ZERO, Vec2::new(f32::NAN, 0.0));
        frame(&mut app);
    }

    /// Plan 6.1: M1 plays with exactly one ball.
    #[test]
    #[should_panic(expected = "2 balls in the match")]
    fn under_the_dev_policy_a_second_ball_panics() {
        let mut app = playing();
        app.world_mut().spawn((
            Ball,
            Position(Vec2::new(10.0, 10.0)),
            Velocity(Vec2::new(60.0, 0.0)),
        ));
        frame(&mut app);
    }

    /// ADR 0004: the checks run in the waits between rallies too.
    #[test]
    #[should_panic(expected = "is not finite")]
    fn the_checks_run_during_the_countdown() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(&mut app, two_humans());
        assert_eq!(match_state(&app), MatchState::Countdown);
        place_ball(&mut app, Vec2::ZERO, Vec2::new(0.0, f32::INFINITY));
        frame(&mut app);
        frame(&mut app);
    }

    /// ADR 0004: the ball rests where it went out until the next countdown.
    #[test]
    fn a_ball_resting_past_the_goal_after_a_point_raises_no_fault() {
        let mut app = playing();
        // Above the right paddle, so it flies out untouched at the cap.
        place_ball(&mut app, Vec2::new(70.0, 30.0), Vec2::new(250.0, 0.0));
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::PointScored,
            30,
            "no point was scored",
        );
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Countdown,
            200,
            "the point pause never ended",
        );
    }

    /// Spec 1.4.7: a child never sees a crash. The fault is reported once,
    /// the rally starts over from a countdown, and the serve comes as usual.
    #[test]
    fn under_the_release_policy_an_injected_nan_velocity_restarts_the_rally() {
        let mut app =
            headless_app_with(1, |core| core.set(ErrorPolicyPlugin(ErrorPolicy::Release)));
        record::<SimFault>(&mut app);
        record::<Serve>(&mut app);
        boot(&mut app);
        start_match(&mut app, two_humans());
        frame(&mut app);
        take_recorded::<Serve>(&mut app);

        place_ball(&mut app, Vec2::ZERO, Vec2::new(f32::NAN, 0.0));
        frame(&mut app);
        let faults = take_recorded::<SimFault>(&mut app);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(matches!(faults[0], SimFault::BallVelocityNotFinite { .. }));

        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        let (position, velocity) = ball_state(&mut app);
        assert_eq!((position, velocity), (Vec2::ZERO, Vec2::ZERO));
        assert_eq!(*app.world().resource::<Score>(), Score::default());

        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Playing,
            1_000,
            "the countdown never ended",
        );
        frame(&mut app);
        assert_eq!(take_recorded::<Serve>(&mut app).len(), 1);
        assert!(ball_state(&mut app).1.is_finite());
        assert_eq!(take_recorded::<SimFault>(&mut app), Vec::new());
    }

    /// A flight that has run for `steps` fixed steps.
    fn flight_of(steps: u16) -> Flight {
        let mut flight = Flight::default();
        for _ in 0..steps {
            flight.tick();
        }
        flight
    }

    /// The embedded limit is 20 s: 1,200 steps at 60 Hz.
    #[test]
    fn a_flight_is_sound_up_to_the_tuned_limit_and_a_fault_beyond_it() {
        assert_eq!(check_flight(&flight_of(0), &tuning(), STEP), Ok(()));
        assert_eq!(check_flight(&flight_of(1_100), &tuning(), STEP), Ok(()));
        assert!(matches!(
            check_flight(&flight_of(1_300), &tuning(), STEP),
            Err(SimFault::FlightTooLong { limit, .. }) if (limit - 20.0).abs() < 1e-6
        ));
    }

    /// The embedded tuning with flights limited to half a second.
    fn short_flights() -> Tuning {
        let ron = EMBEDDED_RON.replace("flight_limit_seconds: 20.0", "flight_limit_seconds: 0.5");
        assert_ne!(
            ron, EMBEDDED_RON,
            "the flight limit was not found in the RON"
        );
        Tuning::from_ron(&ron).expect("a short flight limit is valid")
    }

    /// M2 spec 2.5.6's done-condition, the dev half: a ball bouncing between
    /// the walls with no way to a paddle or a goal is reported.
    #[test]
    #[should_panic(expected = "without a return or a goal")]
    fn under_the_dev_policy_an_endless_flight_panics_once_past_the_limit() {
        let mut app = playing();
        app.insert_resource(short_flights());
        place_ball(&mut app, Vec2::ZERO, Vec2::new(0.0, 60.0));
        for _ in 0..40 {
            frame(&mut app);
        }
    }

    /// The release half: the heal restarts the rally, and the new rally's
    /// flight starts its clock again.
    #[test]
    fn under_the_release_policy_an_endless_flight_restarts_the_rally() {
        let mut app =
            headless_app_with(1, |core| core.set(ErrorPolicyPlugin(ErrorPolicy::Release)));
        record::<SimFault>(&mut app);
        boot(&mut app);
        app.insert_resource(short_flights());
        start_match(&mut app, two_humans());
        place_ball(&mut app, Vec2::ZERO, Vec2::new(0.0, 60.0));
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Countdown,
            60,
            "the endless flight was never healed",
        );
        let faults = take_recorded::<SimFault>(&mut app);
        assert!(
            faults
                .iter()
                .all(|fault| matches!(fault, SimFault::FlightTooLong { .. })),
            "{faults:?}"
        );
        assert!(
            matches!(faults.first(), Some(SimFault::FlightTooLong { .. })),
            "the endless flight raised no fault"
        );
        // The countdown cleared the flight, so the waiting ball raises nothing more.
        for _ in 0..10 {
            frame(&mut app);
        }
        assert_eq!(take_recorded::<SimFault>(&mut app), Vec::new());
    }

    /// The clock runs only in play: a ball waiting through a countdown longer
    /// than the limit is no endless flight.
    #[test]
    fn a_countdown_longer_than_the_limit_raises_no_fault() {
        let mut app = headless_app(1);
        boot(&mut app);
        app.insert_resource(short_flights());
        enter_match(&mut app, two_humans());
        for _ in 0..90 {
            frame(&mut app);
        }
        assert_eq!(match_state(&app), MatchState::Countdown);
    }
}
