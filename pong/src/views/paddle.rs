//! Paddle view (spec 1.6.2): a glowing bar on each core paddle entity, sized
//! to its rectangle. Where it is drawn is [`super::motion`]'s.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::Added;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::mesh::Mesh2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::transform::components::Transform;
use pong_core::features::paddle::Paddle;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::motion::ViewSystems;
use super::{UnitMeshes, layer};

/// Above 1.0 in linear light, so bloom makes it glow. One colour for both
/// sides until profiles bring their own (plan phase 8).
const PADDLE: Color = Color::linear_rgb(0.6, 2.4, 3.0);

/// Dresses each new paddle in the frame it appears and keeps it sized to its
/// rectangle, which follows its height and the tuned thickness.
pub struct PaddleViewPlugin;

impl Plugin for PaddleViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_paddle_look)
            .add_systems(
                RunFixedMainLoop,
                attach_paddle_look.in_set(ViewSystems::Attach),
            )
            .add_systems(Update, fit_paddles.run_if(in_state(AppState::InMatch)));
    }
}

#[derive(Resource, Debug, Clone)]
struct PaddleLook(Handle<ColorMaterial>);

fn create_paddle_look(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.insert_resource(PaddleLook(materials.add(PADDLE)));
}

fn attach_paddle_look(
    mut commands: Commands,
    meshes: Res<UnitMeshes>,
    look: Res<PaddleLook>,
    paddles: Query<Entity, Added<Paddle>>,
) {
    for entity in &paddles {
        commands.entity(entity).insert((
            Mesh2d(meshes.square.clone()),
            MeshMaterial2d(look.0.clone()),
            Transform::from_xyz(0.0, 0.0, layer::PADDLE),
        ));
    }
}

fn fit_paddles(tuning: Res<Tuning>, mut paddles: Query<(&Paddle, &mut Transform)>) {
    for (paddle, mut transform) in &mut paddles {
        transform.scale = paddle.rect(&tuning).size().extend(1.0);
    }
}
