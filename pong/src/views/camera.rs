//! Camera (spec 1.6.2): fits the arena to the window, aspect kept, and
//! develops HDR light through bloom, `TonyMcMapface`, and deband dither.
//!
//! `ScalingMode::AutoMin` always shows the whole arena; on a window that is
//! not the arena's shape it shows extra world beyond it, where nothing is
//! drawn, so the dark backdrop reads as letterbox bars (phase 4 report).

use bevy::app::{App, Plugin, Startup, Update};
use bevy::camera::{Camera2d, ClearColor, Hdr, OrthographicProjection, Projection, ScalingMode};
use bevy::color::Color;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::post_process::bloom::Bloom;
use pong_core::features::tuning::Tuning;

use super::shake::Shake;

/// Outside the arena: the letterbox bars.
const BACKDROP: Color = Color::BLACK;

/// Spawns the one camera at startup and refits it whenever tuning changes,
/// so a reloaded arena size refits the view. Window resizes are Bevy's: it
/// updates the projection itself.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(BACKDROP))
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                fit_camera_to_arena.run_if(resource_exists_and_changed::<Tuning>),
            );
    }
}

/// `Camera2d` defaults to `Linear` tonemapping and no dither; both are set here.
/// The projection is fitted once tuning has loaded.
fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Hdr,
        Bloom::NATURAL,
        Tonemapping::TonyMcMapface,
        DebandDither::Enabled,
        Shake::default(),
    ));
}

fn fit_camera_to_arena(tuning: Res<Tuning>, mut cameras: Query<&mut Projection, With<Camera2d>>) {
    let size = 2.0 * tuning.arena().half_extents();
    for mut projection in &mut cameras {
        *projection = Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: size.x,
                min_height: size.y,
            },
            ..OrthographicProjection::default_2d()
        });
    }
}
