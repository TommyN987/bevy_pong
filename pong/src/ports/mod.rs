//! Adapters that feed the core's port resources (spec 1.6.1, docs/architecture.md
//! "Boundaries"): one module per source of input.

use bevy::app::{App, Plugin, RunFixedMainLoop, RunFixedMainLoopSystems, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_exists;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::paddles_move;

pub mod focus;
pub mod keyboard;
pub mod save_paths;

/// Reports seat intents from the keyboard whenever the paddles move (in play
/// and between rallies, ADR 0004), then forgets them and asks for a pause on
/// focus loss, once per frame just before the fixed steps. Also logs
/// the held seat keys for the ghosting check (spec 1.8.2).
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
                keyboard::report_seat_intents.run_if(paddles_move),
                focus::pause_on_focus_loss,
            )
                .chain()
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        )
        .add_systems(
            Update,
            keyboard::log_held_seat_keys.run_if(resource_exists::<Tuning>),
        );
    }
}
