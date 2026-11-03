//! Headless test harness (spec 1.5.1): `MinimalPlugins`, the asset system, and
//! the core plugin group, with time driven one fixed step per `update` and a
//! constant seed. Under `cfg(test)` until plan 3.8 puts it behind a cargo feature.

use bevy::MinimalPlugins;
use bevy::app::{App, FixedPostUpdate};
use bevy::asset::AssetPlugin;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::state::state::{NextState, State};
use bevy::time::{Fixed, Time, TimeUpdateStrategy};

use bevy::ecs::query::With;
use bevy::math::Vec2;

use crate::app::CorePlugins;
use crate::app::states::AppState;
use crate::domain::{MatchConfig, Random};
use crate::features::ball::{Ball, Position, Velocity};

/// Fixed steps per second in every harness app.
pub(crate) const STEP_HZ: f64 = 60.0;

/// A core app with no window and no real clock. Every [`step`] advances
/// exactly one fixed step; the random source is seeded with `seed`.
pub(crate) fn headless_app(seed: u64) -> App {
    let fixed = Time::<Fixed>::from_hz(STEP_HZ);
    let timestep = fixed.timestep();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), CorePlugins))
        .insert_resource(fixed)
        .insert_resource(TimeUpdateStrategy::ManualDuration(timestep))
        .insert_resource(Random::seeded(seed));
    app
}

/// Runs frames until the app reaches `Menu`; panics if it never does.
pub(crate) fn boot(app: &mut App) {
    for _ in 0..1_000 {
        app.update();
        if *app.world().resource::<State<AppState>>().get() == AppState::Menu {
            return;
        }
    }
    panic!("app never left Boot");
}

/// Installs `config`, enters `InMatch`, and runs the frames that apply the
/// transition and its `OnEnter` systems.
pub(crate) fn start_match(app: &mut App, config: MatchConfig) {
    app.insert_resource(config);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::InMatch);
    app.update();
    assert_eq!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::InMatch,
        "match did not start"
    );
}

/// One frame, which is one fixed step at [`STEP_HZ`].
pub(crate) fn step(app: &mut App) {
    app.update();
}

/// Everything written of message type `M` since the last [`take_recorded`].
/// Call [`record`] once per type first.
#[derive(Resource)]
pub(crate) struct Recorded<M: Message>(Vec<M>);

/// Starts recording every `M` written during fixed steps.
pub(crate) fn record<M: Message + Clone>(app: &mut App) {
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
pub(crate) fn take_recorded<M: Message>(app: &mut App) -> Vec<M> {
    std::mem::take(&mut app.world_mut().resource_mut::<Recorded<M>>().0)
}

/// The ball's position and velocity. Panics unless exactly one ball exists.
pub(crate) fn ball_state(app: &mut App) -> (Vec2, Vec2) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&Position, &Velocity), With<Ball>>();
    let mut balls = query.iter(app.world());
    let (position, velocity) = balls.next().expect("a ball exists");
    assert!(balls.next().is_none(), "more than one ball");
    (position.0, velocity.0)
}

/// Scripts the ball: puts it at `position` moving at `velocity`.
pub(crate) fn place_ball(app: &mut App, position: Vec2, velocity: Vec2) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&mut Position, &mut Velocity), With<Ball>>();
    let (mut p, mut v) = query.single_mut(app.world_mut()).expect("exactly one ball");
    p.0 = position;
    v.0 = velocity;
}
