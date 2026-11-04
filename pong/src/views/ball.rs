//! Ball view (spec 1.6.2): a glowing disc on the core's ball entity, sized to
//! the tuned radius. Where it is drawn is [`super::motion`]'s.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Added, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec3;
use bevy::mesh::Mesh2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::transform::components::Transform;
use pong_core::features::ball::Ball;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::motion::ViewSystems;
use super::{UnitMeshes, layer};

/// Above 1.0 in linear light, so bloom makes it glow.
const BALL: Color = Color::linear_rgb(4.0, 3.2, 1.2);

/// Dresses each new ball in the frame it appears and keeps it sized to tuning.
pub struct BallViewPlugin;

impl Plugin for BallViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_ball_look)
            .add_systems(
                RunFixedMainLoop,
                attach_ball_look.in_set(ViewSystems::Attach),
            )
            .add_systems(Update, fit_ball.run_if(in_state(AppState::InMatch)));
    }
}

#[derive(Resource, Debug, Clone)]
struct BallLook(Handle<ColorMaterial>);

fn create_ball_look(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.insert_resource(BallLook(materials.add(BALL)));
}

fn attach_ball_look(
    mut commands: Commands,
    meshes: Res<UnitMeshes>,
    look: Res<BallLook>,
    balls: Query<Entity, Added<Ball>>,
) {
    for entity in &balls {
        commands.entity(entity).insert((
            Mesh2d(meshes.circle.clone()),
            MeshMaterial2d(look.0.clone()),
            Transform::from_xyz(0.0, 0.0, layer::BALL),
        ));
    }
}

fn fit_ball(tuning: Res<Tuning>, mut balls: Query<&mut Transform, With<Ball>>) {
    let radius = tuning.ball().radius().value();
    for mut transform in &mut balls {
        transform.scale = Vec3::new(radius, radius, 1.0);
    }
}
