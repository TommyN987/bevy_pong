//! Ball view (spec 1.6.2): a glowing disc on the core's ball entity, sized to
//! the tuned radius. Where it is drawn is [`super::motion`]'s.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::color::{Color, LinearRgba};
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
use super::trail::{GHOSTS, TrailGhost};
use super::{UnitMeshes, layer};

/// Above 1.0 in linear light, so bloom makes it glow.
const BALL: Color = Color::linear_rgb(4.0, 3.2, 1.2);

/// How much of the ball ghost `back` keeps, in size and in light: less the
/// further back, never none.
fn ghost_fade(back: u8) -> f32 {
    1.0 - f32::from(back) / f32::from(GHOSTS + 1)
}

/// How bright the nearest ghosts are next to the ball, before their fade.
const GHOST_LIGHT: f32 = 0.6;

/// Ghost `back`'s colour: the ball's, dimmed in linear light by its fade.
/// Scaling a colour scales its alpha too, so alpha is set back to opaque:
/// on the dark backdrop, dimmer reads as fainter, with no blending.
fn ghost_color(back: u8) -> Color {
    let dimmed = BALL.to_linear() * (GHOST_LIGHT * ghost_fade(back));
    Color::LinearRgba(LinearRgba {
        alpha: 1.0,
        ..dimmed
    })
}

/// Dresses each new ball in the frame it appears and keeps it sized to tuning.
pub struct BallViewPlugin;

impl Plugin for BallViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_ball_look)
            .add_systems(
                RunFixedMainLoop,
                (attach_ball_look, attach_ghost_look).in_set(ViewSystems::Attach),
            )
            .add_systems(
                Update,
                (fit_ball, fit_ghosts).run_if(in_state(AppState::InMatch)),
            );
    }
}

/// The ball's material, then one per ghost, nearest first.
#[derive(Resource, Debug, Clone)]
struct BallLook {
    ball: Handle<ColorMaterial>,
    ghosts: Vec<Handle<ColorMaterial>>,
}

fn create_ball_look(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands.insert_resource(BallLook {
        ball: materials.add(BALL),
        ghosts: (1..=GHOSTS)
            .map(|back| materials.add(ghost_color(back)))
            .collect(),
    });
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
            MeshMaterial2d(look.ball.clone()),
            Transform::from_xyz(0.0, 0.0, layer::BALL),
        ));
    }
}

/// A ghost under the ball and over the paddles; where it is drawn is
/// [`super::trail`]'s.
fn attach_ghost_look(
    mut commands: Commands,
    meshes: Res<UnitMeshes>,
    look: Res<BallLook>,
    ghosts: Query<(Entity, &TrailGhost), Added<TrailGhost>>,
) {
    for (entity, ghost) in &ghosts {
        let Some(material) = look.ghosts.get(usize::from(ghost.back()) - 1) else {
            continue;
        };
        commands.entity(entity).insert((
            Mesh2d(meshes.circle.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_xyz(0.0, 0.0, layer::TRAIL),
        ));
    }
}

fn fit_ball(tuning: Res<Tuning>, mut balls: Query<&mut Transform, With<Ball>>) {
    let radius = tuning.ball().radius().value();
    for mut transform in &mut balls {
        transform.scale = Vec3::new(radius, radius, 1.0);
    }
}

fn fit_ghosts(tuning: Res<Tuning>, mut ghosts: Query<(&TrailGhost, &mut Transform)>) {
    let radius = tuning.ball().radius().value();
    for (ghost, mut transform) in &mut ghosts {
        let size = radius * ghost_fade(ghost.back());
        transform.scale = Vec3::new(size, size, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use bevy::color::LinearRgba;

    use super::{BALL, ghost_color, ghost_fade};
    use crate::views::trail::GHOSTS;

    #[test]
    fn a_ghost_further_back_keeps_less_of_the_ball_but_never_none() {
        let fades: Vec<f32> = (1..=GHOSTS).map(ghost_fade).collect();
        assert!(fades.windows(2).all(|pair| pair[1] < pair[0]), "{fades:?}");
        assert!(
            fades.iter().all(|&fade| fade > 0.0 && fade < 1.0),
            "{fades:?}"
        );
    }

    #[test]
    fn a_ghost_is_the_balls_hue_dimmer_and_opaque() {
        let ball = BALL.to_linear();
        for back in 1..=GHOSTS {
            let ghost = ghost_color(back).to_linear();
            assert!(ghost.red < ball.red, "{back}: {ghost:?}");
            let ratio = |channel: fn(LinearRgba) -> f32| channel(ghost) / channel(ball);
            assert!(
                (ratio(|c| c.red) - ratio(|c| c.blue)).abs() < 1e-5,
                "{back}: hue"
            );
            assert!((ghost.alpha - 1.0).abs() < f32::EPSILON, "{back}: alpha");
        }
    }
}
