//! Soak (spec 1.5.2): matches with random intents for both seats over at least
//! a hundred thousand fixed steps, panic handler on, no fault. The nextest
//! slow-timeout of thirty seconds (`.config/nextest.toml`) fails a slow run.
//!
//! Until the sanity phase exists (plan 6.1), the test checks the ball's
//! invariants itself after every step.

use std::num::NonZeroU8;

use pong_core::app::states::AppState;
use pong_core::domain::{Controller, MatchConfig, Seat};
use pong_core::features::rules::MatchState;
use pong_core::features::seats::PaddleIntent;
use pong_core::features::tuning::Tuning;
use pong_core::kernel::random::Pcg32;
use pong_core::testing::{
    STEP_HZ, app_state, ball_state, boot, enter_match, headless_app, leave_match, match_state,
    report_intent, step,
};

/// At least this many fixed steps, every one of them with fresh random intents.
const STEPS: u32 = 100_000;

fn random_intent(random: &mut Pcg32) -> PaddleIntent {
    match random.next_u32() % 3 {
        0 => PaddleIntent::Still,
        1 => PaddleIntent::Up,
        _ => PaddleIntent::Down,
    }
}

fn two_humans_to(points: u8) -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        points_to_win: NonZeroU8::new(points).expect("a positive target"),
    }
}

#[test]
fn a_hundred_thousand_fixed_steps_of_random_intents_raise_no_fault() {
    let mut app = headless_app(152);
    boot(&mut app);
    let config = two_humans_to(5);
    enter_match(&mut app, config);

    let (half, radius, limit_y) = {
        let tuning = app.world().resource::<Tuning>();
        let arena = tuning.arena();
        let radius = tuning.ball().radius();
        (
            arena.half_extents(),
            radius.value(),
            arena.center_limit_y(radius),
        )
    };
    let mut intents = Pcg32::seed(42);
    let mut matches_ended = 0;

    for fixed_step in 0..STEPS {
        if match_state(&app) == MatchState::MatchOver {
            matches_ended += 1;
            leave_match(&mut app);
            enter_match(&mut app, config);
        }
        report_intent(&mut app, Seat::Left, random_intent(&mut intents));
        report_intent(&mut app, Seat::Right, random_intent(&mut intents));
        step(&mut app);

        let (position, velocity) = ball_state(&mut app);
        assert!(
            position.is_finite() && velocity.is_finite(),
            "step {fixed_step}: ball at {position} moving {velocity}"
        );
        assert!(
            position.y.abs() <= limit_y + 1e-3,
            "step {fixed_step}: ball through a wall at {position}"
        );
        // A ball is scored once wholly past the goal line, within the step that
        // takes it there; it can be no farther out than one step's travel.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a step length in f32 units"
        )]
        let step_travel = velocity.length() / STEP_HZ as f32;
        assert!(
            position.x.abs() <= half.x + radius + step_travel + 1e-3,
            "step {fixed_step}: ball far beyond the goal at {position}"
        );
    }

    assert_eq!(app_state(&app), AppState::InMatch);
    assert!(matches_ended >= 1, "no match ended in {STEPS} fixed steps");
}
