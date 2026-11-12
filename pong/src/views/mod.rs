//! Views (spec 1.6.2): the core's bodies drawn as `Mesh2d` in arena units,
//! one module per core body, beside the presentation modules that serve them
//! all: camera, motion, trail, shake (docs/architecture.md, rule 8).
//!
//! Where a body is drawn is [`motion`]'s, the same for every body and testable
//! headless; how it looks is its own module's. Visuals are inserted onto the
//! core's entities, so the core's `DespawnOnExit` removes them too.

use bevy::app::{App, Plugin, Startup};
use bevy::asset::{Assets, Handle};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, ResMut};
use bevy::mesh::Mesh;
use bevy::shape::{Circle, Rectangle};

pub mod arena;
pub mod ball;
pub mod camera;
pub mod motion;
pub mod paddle;
pub mod shake;
pub mod trail;

/// Draw order, back to front.
mod layer {
    pub const ARENA: f32 = 0.0;
    pub const CENTRE_LINE: f32 = 0.5;
    pub const PADDLE: f32 = 1.0;
    pub const TRAIL: f32 = 1.5;
    pub const BALL: f32 = 2.0;
}

/// Every view: the camera, the arena, the ball, the paddles, and their motion.
///
/// Preconditions: `DefaultPlugins` (rendering, meshes, `ColorMaterial`) and
/// the core plugin group.
pub struct ViewsPlugin;

impl Plugin for ViewsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_unit_meshes).add_plugins((
            motion::MotionPlugin,
            trail::TrailPlugin,
            shake::ShakePlugin,
            camera::CameraPlugin,
            arena::ArenaViewPlugin,
            ball::BallViewPlugin,
            paddle::PaddleViewPlugin,
            crate::effects::EffectViewsPlugin,
        ));
    }
}

/// Shapes of unit size, scaled per body by its `Transform`, so a size change
/// (a tuning reload, later the chaos effects) is a scale and never a new mesh.
#[derive(Resource, Debug, Clone)]
struct UnitMeshes {
    /// One by one, centred.
    square: Handle<Mesh>,
    /// Radius one, centred.
    circle: Handle<Mesh>,
}

fn create_unit_meshes(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(UnitMeshes {
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        circle: meshes.add(Circle::new(1.0)),
    });
}
