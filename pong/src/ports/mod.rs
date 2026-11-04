//! Adapters that feed the core's port resources (spec 1.6.1, docs/architecture.md
//! "Boundaries"): one module per source of input.

use bevy::app::{App, Plugin, RunFixedMainLoop, RunFixedMainLoopSystems};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::state::condition::in_state;
use pong_core::flow::states::MatchState;

pub mod focus;
pub mod keyboard;

/// Reports seat intents from the keyboard, then forgets them and asks for a
/// pause on focus loss, once per frame just before the fixed steps.
///
/// Before the fixed steps, not in `Update`: every step of a frame reads that
/// frame's keys rather than the previous frame's. The focus handler runs
/// second so its clear is the last write before the steps read.
///
/// Preconditions: Bevy's `InputPlugin` (keyboard) and `WindowPlugin`, both in
/// `DefaultPlugins`, and the core plugin group.
pub struct PortsPlugin;

impl Plugin for PortsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            RunFixedMainLoop,
            (
                keyboard::report_seat_intents.run_if(in_state(MatchState::Playing)),
                focus::pause_on_focus_loss,
            )
                .chain()
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        );
    }
}
