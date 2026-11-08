//! Paddle view (spec 1.6.2): a glowing bar on each core paddle entity, sized
//! to its rectangle, in its player's profile colour (plan 8.1). Where it is
//! drawn is [`super::motion`]'s.

use bevy::app::{App, Plugin, RunFixedMainLoop, Update};
use bevy::asset::Assets;
use bevy::color::{Color, LinearRgba, Srgba};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::Added;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::mesh::Mesh2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::transform::components::Transform;
use pong_core::domain::{MatchConfig, Player};
use pong_core::features::paddle::Paddle;
use pong_core::features::persistence::model::Profiles;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;
use pong_core::kernel::geometry::Side;

use super::motion::ViewSystems;
use super::{UnitMeshes, layer};

/// How bright a paddle's brightest channel is in linear light: above 1.0,
/// so bloom makes it glow.
const GLOW: f32 = 3.0;

/// The paddle of a Guest or the computer, which have no colour of their own.
const PADDLE: Color = Color::linear_rgb(0.6, 2.4, GLOW);

/// `color` brightened in linear light until its brightest channel is at
/// [`GLOW`], so every palette colour glows as much; its hue is kept. Black
/// stays black.
fn glowing(color: Srgba) -> Color {
    let linear = LinearRgba::from(color);
    let brightest = linear.red.max(linear.green).max(linear.blue);
    let scaled = if brightest > 0.0 {
        linear * (GLOW / brightest)
    } else {
        linear
    };
    Color::LinearRgba(LinearRgba {
        alpha: 1.0,
        ..scaled
    })
}

/// The colour of the paddle on `side`: its player's profile colour, glowing;
/// [`PADDLE`] for a Guest, the computer, or a profile no longer saved.
fn paddle_color(side: Side, config: &MatchConfig, profiles: &Profiles) -> Color {
    match config.player_on(side) {
        Some(Player::Profile(id)) => profiles
            .as_slice()
            .iter()
            .find(|profile| profile.id() == id)
            .map_or(PADDLE, |profile| glowing(profile.color())),
        Some(Player::Guest) | None => PADDLE,
    }
}

/// Dresses each new paddle in the frame it appears and keeps it sized to its
/// rectangle, which follows its height and the tuned thickness.
pub struct PaddleViewPlugin;

impl Plugin for PaddleViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            RunFixedMainLoop,
            attach_paddle_look.in_set(ViewSystems::Attach),
        )
        .add_systems(Update, fit_paddles.run_if(in_state(AppState::InMatch)));
    }
}

/// Each paddle gets a material of its own, in its player's colour; it goes
/// with the paddle at the end of the match.
fn attach_paddle_look(
    mut commands: Commands,
    meshes: Res<UnitMeshes>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<MatchConfig>,
    profiles: Res<Profiles>,
    paddles: Query<(Entity, &Paddle), Added<Paddle>>,
) {
    for (entity, paddle) in &paddles {
        let color = paddle_color(paddle.side(), &config, &profiles);
        commands.entity(entity).insert((
            Mesh2d(meshes.square.clone()),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(0.0, 0.0, layer::PADDLE),
        ));
    }
}

fn fit_paddles(tuning: Res<Tuning>, mut paddles: Query<(&Paddle, &mut Transform)>) {
    for (paddle, mut transform) in &mut paddles {
        transform.scale = paddle.rect(&tuning).size().extend(1.0);
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::color::{Color, LinearRgba, Srgba};
    use pong_core::domain::{
        Controller, Difficulty, MatchConfig, Player, ProfileId, Seat, SeatPlayers,
    };
    use pong_core::features::persistence::model::{Profile, ProfileName, Profiles, Records};
    use pong_core::kernel::geometry::Side;

    use super::{GLOW, PADDLE, glowing, paddle_color};

    fn zoe_in(color: Srgba) -> Profiles {
        let zoe = Profile::new(
            ProfileId(1),
            ProfileName::new("Zoë").expect("valid"),
            color,
            Records::default(),
        )
        .expect("valid");
        Profiles::new(vec![zoe]).expect("one profile")
    }

    fn zoe_left_against(right: Controller) -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right,
            players: SeatPlayers {
                left_seat: Player::Profile(ProfileId(1)),
                right_seat: Player::Guest,
            },
            points_to_win: NonZeroU8::MIN,
        }
    }

    fn linear(color: Color) -> LinearRgba {
        color.to_linear()
    }

    /// Bloom makes it glow: the brightest channel at the default paddle's.
    #[test]
    fn a_colour_glows_with_its_hue_kept_and_its_brightest_channel_at_the_glow() {
        let picked = Srgba::rgb(0.2, 0.8, 0.9);
        let glow = linear(glowing(picked));
        let plain = LinearRgba::from(picked);
        assert!((glow.blue - GLOW).abs() < 1e-5, "{glow:?}");
        assert!((glow.green / glow.blue - plain.green / plain.blue).abs() < 1e-5);
        assert!((glow.red / glow.blue - plain.red / plain.blue).abs() < 1e-5);
        assert!((glow.alpha - 1.0).abs() < f32::EPSILON);
        assert!(
            (linear(PADDLE).blue - GLOW).abs() < 1e-5,
            "the default glows as much"
        );
    }

    #[test]
    fn a_profiles_paddle_glows_in_its_colour() {
        let config = zoe_left_against(Controller::Human(Seat::Right));
        assert_eq!(
            paddle_color(Side::Left, &config, &zoe_in(Srgba::RED)),
            glowing(Srgba::RED)
        );
    }

    #[test]
    fn a_guest_or_computer_paddle_keeps_the_default_glow() {
        let profiles = zoe_in(Srgba::RED);
        let guest = zoe_left_against(Controller::Human(Seat::Right));
        assert_eq!(paddle_color(Side::Right, &guest, &profiles), PADDLE);
        let computer = zoe_left_against(Controller::Ai(Difficulty::OneStar));
        assert_eq!(paddle_color(Side::Right, &computer, &profiles), PADDLE);
    }
}
