//! Arena view (spec 1.6.2): the floor and the centre line, sized from tuning.

use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec3;
use bevy::mesh::Mesh2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::transform::components::Transform;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::{UnitMeshes, layer};

/// Just above the black backdrop, so the space beside it reads as bars.
const FLOOR: Color = Color::linear_rgb(0.012, 0.014, 0.022);
/// Dim, far below the bodies' glow.
const CENTRE_LINE: Color = Color::linear_rgb(0.08, 0.09, 0.13);
/// Width of the centre line in arena units.
const CENTRE_LINE_WIDTH: f32 = 0.4;

/// Spawns the floor and centre line for each match and keeps them sized to
/// the arena in tuning, so a reload resizes them.
pub struct ArenaViewPlugin;

impl Plugin for ArenaViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_arena_look)
            .add_systems(OnEnter(AppState::InMatch), spawn_arena)
            .add_systems(Update, fit_arena.run_if(in_state(AppState::InMatch)));
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ArenaPart {
    Floor,
    CentreLine,
}

#[derive(Resource, Debug, Clone)]
struct ArenaLook {
    floor: Handle<ColorMaterial>,
    centre_line: Handle<ColorMaterial>,
}

fn create_arena_look(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.insert_resource(ArenaLook {
        floor: materials.add(FLOOR),
        centre_line: materials.add(CENTRE_LINE),
    });
}

fn spawn_arena(mut commands: Commands, meshes: Res<UnitMeshes>, look: Res<ArenaLook>) {
    for (part, material, z) in [
        (ArenaPart::Floor, &look.floor, layer::ARENA),
        (ArenaPart::CentreLine, &look.centre_line, layer::CENTRE_LINE),
    ] {
        commands.spawn((
            part,
            Mesh2d(meshes.square.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_xyz(0.0, 0.0, z),
            DespawnOnExit(AppState::InMatch),
        ));
    }
}

fn fit_arena(tuning: Res<Tuning>, mut parts: Query<(&ArenaPart, &mut Transform)>) {
    let size = 2.0 * tuning.arena().half_extents();
    for (part, mut transform) in &mut parts {
        transform.scale = match part {
            ArenaPart::Floor => size.extend(1.0),
            ArenaPart::CentreLine => Vec3::new(CENTRE_LINE_WIDTH, size.y, 1.0),
        };
    }
}
