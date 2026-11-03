//! Headless test harness (spec 1.5.1): `MinimalPlugins`, the asset system, and
//! the core plugin group, with time driven one fixed step per `update`, a
//! constant seed, fake seat intents, and message recorders.
//!
//! Compiled for this crate's own tests and, behind the `testing` cargo
//! feature, for integration tests and the shell's tests. Every helper that
//! can fail panics with the reason: a broken fixture should stop the test.

// The harness is test code: a helper that cannot do what the test asked
// stops the test with a message, like `expect` in a `#[cfg(test)]` module.
#![expect(
    clippy::expect_used,
    reason = "test harness: a failed precondition stops the test with the reason"
)]

use bevy::MinimalPlugins;
use bevy::app::{App, FixedPostUpdate, FixedUpdate, PluginGroup};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{Schedules, SingleThreadedExecutor, SystemSet};
use bevy::ecs::system::ResMut;
use bevy::math::Vec2;
use bevy::state::state::{NextState, State};
use bevy::time::{Fixed, Time, TimeUpdateStrategy};

use crate::app::CorePlugins;
use crate::app::error_policy::{ErrorPolicy, ErrorPolicyPlugin};
use crate::app::phases::FixedStepPhase;
use crate::app::states::AppState;
use crate::domain::{MatchConfig, Random, Seat};
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::rules::MatchState;
use crate::features::seats::{PaddleIntent, SeatIntents};

/// Fixed steps per second in every harness app.
pub const STEP_HZ: f64 = 60.0;

/// A core app with no window and no real clock. Every [`step`] advances
/// exactly one fixed step; the random source is seeded with `seed`; a system
/// error panics in every build profile, so a soak run in release still fails.
pub fn headless_app(seed: u64) -> App {
    let fixed = Time::<Fixed>::from_hz(STEP_HZ);
    let timestep = fixed.timestep();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        CorePlugins.set(ErrorPolicyPlugin(ErrorPolicy::Dev)),
    ))
    .insert_resource(fixed)
    .insert_resource(TimeUpdateStrategy::ManualDuration(timestep))
    .insert_resource(Random::seeded(seed));
    // A test runs many small frames. When the shell is built in the same cargo
    // run, feature unification turns on Bevy's multi-threaded executor, whose
    // dispatch costs more per schedule than the systems do (the soak went from
    // 3 s to 19 s). Every schedule runs single-threaded here.
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
    app
}

/// Runs frames until the app reaches `Menu`.
///
/// # Panics
/// If the app never leaves `Boot`.
pub fn boot(app: &mut App) {
    run_until(
        app,
        |app| app_state(app) == AppState::Menu,
        1_000,
        "the app never left Boot",
    );
}

/// Installs `config` and enters `InMatch`, which begins in `Countdown`.
///
/// # Panics
/// If the app is not in `InMatch` after one frame.
pub fn enter_match(app: &mut App, config: MatchConfig) {
    app.insert_resource(config);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::InMatch);
    app.update();
    assert_eq!(app_state(app), AppState::InMatch, "match did not start");
}

/// Leaves the match for `Menu`, as the shell does from `MatchOver` or a pause.
///
/// # Panics
/// If the app is not in `Menu` after one frame.
pub fn leave_match(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Menu);
    app.update();
    assert_eq!(app_state(app), AppState::Menu, "match did not end");
}

/// [`enter_match`], then frames until play begins: the countdown has run and
/// the first serve has been requested.
///
/// # Panics
/// If play never begins.
pub fn start_match(app: &mut App, config: MatchConfig) {
    enter_match(app, config);
    run_until(
        app,
        |app| match_state(app) == MatchState::Playing,
        1_000,
        "play never began",
    );
}

/// The current app state.
#[must_use]
pub fn app_state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

/// The current match substate.
///
/// # Panics
/// Outside a match, where the substate does not exist.
#[must_use]
pub fn match_state(app: &App) -> MatchState {
    *app.world()
        .get_resource::<State<MatchState>>()
        .expect("a match in progress")
        .get()
}

/// Frames until `done` holds, at most `max`.
///
/// # Panics
/// With `what` if `done` still does not hold after `max` frames.
pub fn run_until(app: &mut App, done: impl Fn(&App) -> bool, max: u32, what: &str) {
    for _ in 0..max {
        if done(app) {
            return;
        }
        app.update();
    }
    if done(app) {
        return;
    }
    panic!("{what} within {max} frames");
}

/// One frame, which is one fixed step at [`STEP_HZ`].
pub fn step(app: &mut App) {
    app.update();
}

/// Fakes `seat`'s keys asking for `intent`, as the shell's keyboard adapter
/// reports it every `Update`. The level stays until reported again.
pub fn report_intent(app: &mut App, seat: Seat, intent: PaddleIntent) {
    app.world_mut()
        .resource_mut::<SeatIntents>()
        .report(seat, intent);
}

/// Everything written of message type `M` since the last [`take_recorded`].
/// Call [`record`] once per type first.
#[derive(Resource)]
pub struct Recorded<M: Message>(Vec<M>);

/// Starts recording every `M` written during fixed steps.
pub fn record<M: Message + Clone>(app: &mut App) {
    app.insert_resource(Recorded::<M>(Vec::new()))
        .add_systems(FixedPostUpdate, record_messages::<M>);
}

fn record_messages<M: Message + Clone>(
    mut reader: MessageReader<M>,
    mut recorded: ResMut<Recorded<M>>,
) {
    recorded.0.extend(reader.read().cloned());
}

/// Drains the recording of `M`.
///
/// # Panics
/// If [`record`] was not called for `M`.
pub fn take_recorded<M: Message>(app: &mut App) -> Vec<M> {
    std::mem::take(
        &mut app
            .world_mut()
            .get_resource_mut::<Recorded<M>>()
            .expect("a recorder for this message type")
            .0,
    )
}

/// The ball's position and velocity.
///
/// # Panics
/// Unless exactly one ball exists.
pub fn ball_state(app: &mut App) -> (Vec2, Vec2) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&Position, &Velocity), With<Ball>>();
    let mut balls = query.iter(app.world());
    let (position, velocity) = balls.next().expect("a ball exists");
    assert!(balls.next().is_none(), "more than one ball");
    (position.0, velocity.0)
}

/// Scripts the ball: puts it at `position` moving at `velocity`.
///
/// # Panics
/// Unless exactly one ball exists.
pub fn place_ball(app: &mut App, position: Vec2, velocity: Vec2) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&mut Position, &mut Velocity), With<Ball>>();
    let (mut p, mut v) = query.single_mut(app.world_mut()).expect("exactly one ball");
    p.0 = position;
    v.0 = velocity;
}

/// Names of `FixedUpdate` systems that belong to no [`FixedStepPhase`].
/// Builds the schedule first, so it works on an app that has not run.
///
/// # Panics
/// If the `FixedUpdate` schedule cannot be built.
#[must_use]
pub fn unphased_fixed_step_systems(app: &mut App) -> Vec<String> {
    use std::collections::HashSet;

    app.world_mut()
        .schedule_scope(FixedUpdate, |world, schedule| {
            schedule
                .initialize(world)
                .expect("the FixedUpdate schedule builds");
            let graph = schedule.graph();
            let phased: HashSet<_> = FixedStepPhase::IN_ORDER
                .iter()
                .filter_map(|phase| graph.systems_in_set(phase.intern()).ok())
                .flat_map(|keys| keys.iter().copied())
                .collect();
            // Once built, the systems live in the executable, not the graph; only
            // their keys and set membership remain on the graph side.
            schedule
                .systems()
                .expect("an initialized schedule lists its systems")
                .filter(|(key, _)| !phased.contains(key))
                .map(|(_, system)| system.name().to_string())
                // Bevy inserts its own sync points between phases when a system
                // uses commands; they are not core systems and have no phase.
                .filter(|name| name != "bevy_ecs::apply_deferred")
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use bevy::app::Update;
    use bevy::ecs::error::BevyError;

    use super::headless_app;

    fn failing_system() -> Result<(), BevyError> {
        Err(BevyError::from("deliberate test failure"))
    }

    #[test]
    #[should_panic(expected = "deliberate test failure")]
    fn a_harness_app_panics_on_a_system_error_in_every_build_profile() {
        let mut app = headless_app(0);
        app.add_systems(Update, failing_system);
        app.update();
    }
}
