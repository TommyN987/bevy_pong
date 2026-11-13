//! Spin's shell half (M2 spec 2.6.4): a return from a moving paddle curves
//! the way the paddle moved.
//!
//! The base views cannot show which way a return was spun, so Spin has a
//! view of its own (R1): one bright arrowhead riding on the ball for the
//! whole of a spun flight, on the side the ball will curve to and pointing
//! that way, larger as the dial rises. It is there from the frame of the
//! hit, before the curve has grown enough to see.
//!
//! The mark is a child entity of the ball, so it follows the ball's drawn
//! position, hides when the ball is hidden, and is despawned with it. Which
//! ball it rides and where it sits beside it is [`SpinMarkPlugin`]'s, and
//! testable headless; how it looks is added by [`shell_half`]'s view, with
//! the render assets.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::camera::visibility::Visibility;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Added, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::{Vec2, Vec3};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::shape::Triangle2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::transform::components::Transform;
use pong_core::domain::EffectId;
use pong_core::features::ball::Ball;
use pong_core::features::effects::spin::Spin;
use pong_core::features::effects::{ActiveEffects, BallFlight};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::{Icon, ShellHalf};
use crate::audio::sound::Sound;
use crate::audio::synth::{PLAIN, phrase, tone};
use crate::views::motion::ViewSystems;

/// An arrow turning round on itself: something spinning.
const ICON: Icon = Icon {
    glyph: "↻",
    color: Color::srgb(1.0, 0.45, 0.85),
};

/// Spin's icon, cue, and view.
///
/// - Cue: [`join_cue`].
/// - View: [`view`], the mark on the ball.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: Some(ICON),
        join_cue: Some(join_cue),
        view: Some(view),
    }
}

/// Three quick plain notes, up, back, and up again, then a higher one held:
/// something whirled round and let go.
fn join_cue() -> Sound {
    phrase(
        &[
            tone(&[523.25], 0.07),
            tone(&[659.25], 0.07),
            tone(&[523.25], 0.07),
            tone(&[783.99], 0.24),
        ],
        PLAIN,
    )
}

/// The mark, placed and drawn.
fn view(app: &mut App) {
    app.add_plugins(SpinMarkPlugin)
        .add_systems(Startup, create_the_mark_look)
        .add_systems(
            RunFixedMainLoop,
            attach_the_mark_look.in_set(ViewSystems::Attach),
        );
}

/// The mark's width and height while the dial has barely moved, in ball
/// radii: the ball's transform is scaled by its radius, and the mark, its
/// child, with it.
const FAINT: Vec2 = Vec2::new(1.2, 1.0);
/// Its width and height at full dial, in ball radii: about a ball across.
const FULL: Vec2 = Vec2::new(2.4, 2.0);
/// From the ball's centre to the mark's near edge, in ball radii: clear of
/// the ball's rim.
const CLEAR_OF_THE_BALL: f32 = 1.4;
/// How far in front of the ball the mark is drawn. A child's draw order is
/// its parent's plus its own, so the mark is over the ball wherever the
/// ball's layer is.
const IN_FRONT: f32 = 0.1;
/// Bright magenta: above 1.0 in linear light, so bloom makes it glow as the
/// ball does, in a hue neither the ball nor Wind's arrows have.
const MARK: Color = Color::linear_rgb(3.4, 0.7, 2.6);

/// Spin's mark on a ball: its child, hidden unless the ball's flight is
/// spun.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpinMark;

/// Gives each ball its mark and places it: beside the ball on the side a
/// spun flight curves to, pointing that way, hidden in any other flight.
/// Touches only the mark's parent, `Transform`, and `Visibility`.
///
/// Precondition: the core plugin group (`ActiveEffects`, `Tuning`, the
/// states, the ball).
pub struct SpinMarkPlugin;

impl Plugin for SpinMarkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (give_each_ball_its_mark, place_the_marks)
                .chain()
                .run_if(in_state(AppState::InMatch)),
        );
    }
}

/// One mark per ball, as the ball's child and hidden.
///
/// Postcondition: the mark is despawned with its ball, by the hierarchy;
/// it needs no scope of its own.
fn give_each_ball_its_mark(mut commands: Commands, balls: Query<Entity, Added<Ball>>) {
    for ball in &balls {
        commands.spawn((
            SpinMark,
            ChildOf(ball),
            Transform::from_xyz(0.0, 0.0, IN_FRONT),
            Visibility::Hidden,
        ));
    }
}

/// The mark's width and height when the spin's dial is at `strength`, from
/// zero to one of its full drift.
fn mark_size(strength: f32) -> Vec2 {
    FAINT.lerp(FULL, strength.clamp(0.0, 1.0))
}

/// Shows each mark while Spin is in play and its ball's flight is spun,
/// and places it by the flight's drift: on the side the ball will curve to,
/// tip that way, sized by the dial.
///
/// The drift is read from the start the flight began at, through the same
/// [`Spin::drift`] the core's field is built from, so what is shown is what
/// then curves. A flight's start does not change while it lasts, so the
/// mark changes only where a flight starts or a rally ends (R11).
///
/// `Visibility::Inherited`, not `Visible`: a hidden ball hides its mark.
fn place_the_marks(
    tuning: Res<Tuning>,
    active: Res<ActiveEffects>,
    flights: Query<&BallFlight, With<Ball>>,
    mut marks: Query<(&ChildOf, &mut Transform, &mut Visibility), With<SpinMark>>,
) {
    let in_play = active.ids().contains(&EffectId::Spin);
    let spin = Spin::new(&tuning);
    let max_drift = tuning.effects().spin().max_drift();
    for (child_of, mut transform, mut visibility) in &mut marks {
        let drift = flights
            .get(child_of.parent())
            .ok()
            .and_then(BallFlight::in_flight)
            .filter(|_| in_play)
            .map_or(0.0, |flight| spin.drift(&flight.began_at()));
        // A drift other than zero means a full drift above zero.
        let shown = if drift.abs() > 0.0 && max_drift > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
        if shown == Visibility::Hidden {
            continue;
        }
        let toward = drift.signum();
        let size = mark_size(drift.abs() / max_drift);
        let beside = CLEAR_OF_THE_BALL + size.y / 2.0;
        transform.translation = Vec3::new(0.0, toward * beside, IN_FRONT);
        // The unit mark's tip is at +y: a negative height turns it over.
        transform.scale = Vec3::new(size.x, toward * size.y, 1.0);
    }
}

/// The mark's shape and colour, made once.
#[derive(Resource, Debug, Clone)]
struct MarkLook {
    /// One by one, its tip at the top.
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
}

fn create_the_mark_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let unit_mark = Triangle2d::new(
        Vec2::new(0.0, 0.5),
        Vec2::new(-0.5, -0.5),
        Vec2::new(0.5, -0.5),
    );
    commands.insert_resource(MarkLook {
        mesh: meshes.add(unit_mark),
        material: materials.add(MARK),
    });
}

fn attach_the_mark_look(
    mut commands: Commands,
    look: Res<MarkLook>,
    marks: Query<Entity, Added<SpinMark>>,
) {
    for entity in &marks {
        commands.entity(entity).insert((
            Mesh2d(look.mesh.clone()),
            MeshMaterial2d(look.material.clone()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::{FAINT, FULL, mark_size};

    fn assert_near(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() <= 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn the_mark_grows_from_faint_to_full_with_the_dial_and_no_further() {
        assert_near(mark_size(0.0), FAINT);
        assert_near(mark_size(0.5), FAINT.midpoint(FULL));
        assert_near(mark_size(1.0), FULL);
        assert_near(mark_size(7.0), FULL);
        assert_near(mark_size(-1.0), FAINT);
    }
}
