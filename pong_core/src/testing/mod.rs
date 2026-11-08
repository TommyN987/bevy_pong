//! Headless test harness (spec 1.5.1): `MinimalPlugins`, the asset system, and
//! the core plugin group, with time driven by hand (one fixed step per frame
//! unless [`set_frame_length`] says otherwise), a constant seed, fake seat
//! intents, and message recorders.
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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bevy::MinimalPlugins;
use bevy::app::{App, FixedPostUpdate, FixedUpdate, PluginGroup, PluginGroupBuilder};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{ScheduleBuildSettings, Schedules, SingleThreadedExecutor, SystemSet};
use bevy::ecs::system::ResMut;
use bevy::math::Vec2;
use bevy::state::state::{NextState, State};
use bevy::time::{Fixed, Time, TimeUpdateStrategy};

use crate::app::CorePlugins;
use crate::app::error_policy::{ErrorPolicy, ErrorPolicyPlugin};
use crate::domain::{MatchConfig, Seat};
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::persistence::io::SavePaths;
use crate::features::seats::{PaddleIntent, SeatIntents};
use crate::flow::phases::FixedStepPhase;
use crate::flow::states::AppState;
use crate::flow::states::MatchState;

/// A core app with no window and no real clock. Every [`frame`] advances one
/// fixed step at the core's rate until [`set_frame_length`] changes it; `seed`
/// seeds the random source and the schedule shuffle; a system error panics in
/// every build profile, so a soak run in release still fails.
///
/// Bevy's first frame has zero delta, so the first [`frame`] runs no fixed step.
pub fn headless_app(seed: u64) -> App {
    headless_app_with(seed, |core| core)
}

/// [`headless_app`] with the core plugin group adjusted before it is added:
/// `|core| core.set(ErrorPolicyPlugin(ErrorPolicy::Release))` tests the
/// release error policy; `|core| core.disable::<SanityPlugin>()` lets a test
/// break an invariant on purpose. `adjust` receives the group with the dev
/// policy already set.
///
/// # Panics
/// If `adjust` sets or disables a plugin the group does not hold.
pub fn headless_app_with(
    seed: u64,
    adjust: impl FnOnce(PluginGroupBuilder) -> PluginGroupBuilder,
) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        adjust(CorePlugins { seed }.set(ErrorPolicyPlugin(ErrorPolicy::Dev))),
    ));
    set_frame_length(&mut app, FrameLength::Steps(1));
    // Spec 1.5.3: systems with conflicting access and no declared order run in
    // an order the seed decides, so a test cannot pass by accident of order.
    app.configure_schedules(ScheduleBuildSettings {
        shuffle_seed: Some(seed),
        ..ScheduleBuildSettings::default()
    });
    // A test runs many small frames. When the shell is built in the same cargo
    // run, feature unification turns on Bevy's multi-threaded executor, whose
    // dispatch costs more per schedule than the systems do (the soak went from
    // 3 s to 19 s). Every schedule runs single-threaded here, which the shuffle
    // also asks for: the multi-threaded executor does not keep the shuffled order.
    //
    // Both the shuffle seed and the executor reach only the schedules that exist
    // now, after every core plugin has built (Bevy applies neither to schedules
    // created later). A test that adds systems to a schedule no plugin created
    // gets Bevy's defaults there.
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
    app
}

/// How long [`boot`] waits for `Boot` to end. Boot waits on the tuning load,
/// which Bevy's I/O threads run in wall-clock time, not in frames: on a busy
/// machine 1,000 fast frames once passed before it finished (phase 4 report).
const BOOT_LIMIT: Duration = Duration::from_secs(10);

/// Runs frames until the app reaches `Menu`.
///
/// # Panics
/// If the app is still in `Boot` after [`BOOT_LIMIT`] of wall-clock time.
pub fn boot(app: &mut App) {
    boot_within(app, BOOT_LIMIT);
}

/// [`boot`] with its wall-clock limit given.
fn boot_within(app: &mut App, limit: Duration) {
    let start = Instant::now();
    while app_state(app) != AppState::Menu {
        assert!(
            start.elapsed() < limit,
            "the app never left Boot within {limit:?}"
        );
        app.update();
    }
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

/// One frame: `Update` once and as many fixed steps as the frame length gives.
pub fn frame(app: &mut App) {
    app.update();
}

/// How much game time one [`frame`] advances, measured in fixed steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameLength {
    /// Half a fixed step: frames alternate between one fixed step and none, as
    /// on a display faster than the fixed rate. The step rounds to whole
    /// nanoseconds, so a pair of halves can fall a nanosecond short and the
    /// alternation slips by one frame now and then.
    HalfStep,
    /// This many whole fixed steps per frame, as on a slow or hitching display.
    Steps(u32),
}

/// The longest frame Bevy accepts before clamping (`Time<Virtual>`'s default
/// maximum delta). Longer frames would run fewer steps than asked.
const MAX_FRAME: Duration = Duration::from_millis(250);

/// Makes every following [`frame`] advance `length` of game time.
///
/// # Panics
/// If `length` is `Steps(0)`, or longer than the 250 ms Bevy accepts in one
/// frame. At 60 Hz the step rounds up to 16,666,667 ns, so 14 steps fit and
/// 15 do not.
pub fn set_frame_length(app: &mut App, length: FrameLength) {
    let timestep = app.world().resource::<Time<Fixed>>().timestep();
    let duration = match length {
        FrameLength::HalfStep => timestep / 2,
        FrameLength::Steps(0) => panic!("a frame of zero steps never advances time"),
        FrameLength::Steps(steps) => timestep * steps,
    };
    assert!(
        duration <= MAX_FRAME,
        "{length:?} is {duration:?}, longer than the {MAX_FRAME:?} Bevy runs in one frame"
    );
    app.insert_resource(TimeUpdateStrategy::ManualDuration(duration));
}

/// The length of one fixed step, as the core's clock has it.
#[must_use]
pub fn fixed_timestep(app: &App) -> Duration {
    app.world().resource::<Time<Fixed>>().timestep()
}

/// Fakes `seat`'s keys asking for `intent`, as the shell's keyboard adapter
/// reports it once per frame. The level stays until reported again.
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

/// Everything recorded of `M` so far, without draining it.
///
/// # Panics
/// If [`record`] was not called for `M`.
#[must_use]
pub fn recorded<M: Message>(app: &App) -> &[M] {
    &app.world()
        .get_resource::<Recorded<M>>()
        .expect("a recorder for this message type")
        .0
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

/// A fresh directory under the system temp directory, removed with
/// everything in it when dropped: the temp-dir `SavePaths` fake (spec 1.5.1).
/// Std only; no `tempfile` crate.
#[derive(Debug)]
pub struct TempSaveDir {
    root: PathBuf,
}

impl TempSaveDir {
    /// A new, empty directory no other `TempSaveDir` shares.
    ///
    /// # Panics
    /// If the directory cannot be made.
    #[must_use]
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("bevy_pong-test-{}-{n}", std::process::id()));
        // A crashed run whose process id came round again may have left one.
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("a stale test directory is removable");
        }
        std::fs::create_dir_all(&root).expect("the system temp directory is writable");
        Self { root }
    }

    /// The directory itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Save paths inside it, settings and profiles in separate directories,
    /// as the XDG config and data directories are. Neither exists yet.
    #[must_use]
    pub fn save_paths(&self) -> SavePaths {
        SavePaths::new(self.root.join("config"), self.root.join("data"))
    }
}

impl Drop for TempSaveDir {
    /// Best effort: a directory left behind in the system temp directory
    /// harms nothing, and a panic in `drop` would hide the test's own failure.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Default for TempSaveDir {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::app::{App, FixedPostUpdate, Update};
    use bevy::ecs::error::BevyError;
    use bevy::ecs::resource::Resource;
    use bevy::ecs::system::ResMut;

    use std::time::Duration;

    use bevy::MinimalPlugins;

    use super::{FrameLength, TempSaveDir, boot_within, frame, headless_app, set_frame_length};
    use crate::app::boot::BootPlugin;
    use crate::flow::FlowPlugin;

    /// Without the tuning feature, nothing ever resolves tuning, so the app
    /// stays in `Boot` and the limit is what ends the wait.
    #[test]
    #[should_panic(expected = "the app never left Boot within")]
    fn boot_gives_up_once_its_wall_clock_limit_has_passed() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, FlowPlugin, BootPlugin));
        boot_within(&mut app, Duration::from_millis(50));
    }

    #[derive(Resource, Default)]
    struct FixedSteps(u32);

    fn count_fixed_step(mut steps: ResMut<FixedSteps>) {
        steps.0 += 1;
    }

    /// Fixed steps run by each of `frames` frames of `length`, after the first
    /// frame, whose delta Bevy reports as zero.
    fn steps_per_frame(length: FrameLength, frames: usize) -> Vec<u32> {
        let mut app = headless_app(1);
        app.init_resource::<FixedSteps>()
            .add_systems(FixedPostUpdate, count_fixed_step);
        set_frame_length(&mut app, length);
        frame(&mut app);
        (0..frames)
            .map(|_| {
                let before = app.world().resource::<FixedSteps>().0;
                frame(&mut app);
                app.world().resource::<FixedSteps>().0 - before
            })
            .collect()
    }

    #[test]
    fn a_frame_of_n_steps_runs_n_fixed_steps_up_to_the_longest_frame_bevy_accepts() {
        assert_eq!(steps_per_frame(FrameLength::Steps(1), 3), [1, 1, 1]);
        assert_eq!(steps_per_frame(FrameLength::Steps(4), 3), [4, 4, 4]);
        assert_eq!(steps_per_frame(FrameLength::Steps(14), 2), [14, 14]);
    }

    #[test]
    fn half_step_frames_never_run_two_fixed_steps_in_a_row_and_run_one_every_other_frame() {
        let steps = steps_per_frame(FrameLength::HalfStep, 40);
        assert!(steps.iter().all(|&n| n <= 1), "{steps:?}");
        assert!(
            steps.windows(2).all(|pair| pair[0] + pair[1] <= 1),
            "{steps:?}"
        );
        // Nanosecond rounding loses at most one step over the run.
        assert!((19..=20).contains(&steps.iter().sum::<u32>()), "{steps:?}");
    }

    #[test]
    #[should_panic(expected = "longer than")]
    fn a_frame_longer_than_bevy_accepts_is_refused() {
        let mut app: App = headless_app(1);
        set_frame_length(&mut app, FrameLength::Steps(15));
    }

    fn failing_system() -> Result<(), BevyError> {
        Err(BevyError::from("deliberate test failure"))
    }

    #[derive(Resource, Default)]
    struct RunOrder(Vec<&'static str>);

    fn first(mut order: ResMut<RunOrder>) {
        order.0.push("first");
    }

    fn second(mut order: ResMut<RunOrder>) {
        order.0.push("second");
    }

    /// Spec 1.5.3: systems with conflicting access and no declared order run
    /// in an order the seed decides, so an order a test relies on by accident
    /// shows up as a failure under another seed.
    #[test]
    fn unordered_conflicting_systems_run_in_a_seed_dependent_order() {
        let orders: HashSet<Vec<&str>> = (0..16)
            .map(|seed| {
                let mut app = headless_app(seed);
                app.init_resource::<RunOrder>()
                    .add_systems(Update, (first, second));
                app.update();
                app.world().resource::<RunOrder>().0.clone()
            })
            .collect();
        assert_eq!(orders.len(), 2, "{orders:?}");
    }

    #[test]
    #[should_panic(expected = "deliberate test failure")]
    fn a_harness_app_panics_on_a_system_error_in_every_build_profile() {
        let mut app = headless_app(0);
        app.add_systems(Update, failing_system);
        app.update();
    }

    #[test]
    fn a_temp_save_dir_is_fresh_and_empty_and_gone_once_dropped() {
        let held = TempSaveDir::new();
        let other = TempSaveDir::new();
        assert_ne!(held.path(), other.path());
        let entries = std::fs::read_dir(held.path()).expect("the directory exists");
        assert_eq!(entries.count(), 0);
        std::fs::write(held.path().join("left.ron"), "()").expect("writable");
        let path = held.path().to_owned();
        drop(held);
        assert!(
            !path.exists(),
            "{} outlived its TempSaveDir",
            path.display()
        );
    }
}
