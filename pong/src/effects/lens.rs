//! Lens's shell half (M2 spec 2.6.5): lenses on the floor that pull the
//! ball toward their centre lines.
//!
//! The base views cannot show where a lens lies, so Lens has a view of its
//! own (R1): one disc on the floor for every lens of the flight, ringed,
//! with the centre line it pulls toward drawn across it. A lens is there
//! from the frame its flight starts, before the ball reaches it, and above
//! the fog, which hides the ball only (M2 spec 2.2.9).
//!
//! Where the lenses lie is [`FloorLensesPlugin`]'s, and testable headless;
//! how one looks is added by [`shell_half`]'s view, with the render assets.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::curve::{Curve, EaseFunction};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Added, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec3;
use bevy::mesh::{Mesh, Mesh2d};
use bevy::shape::{Annulus, Circle, Rectangle};
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::Time;
use bevy::transform::components::Transform;
use pong_core::domain::EffectId;
use pong_core::features::ball::Ball;
use pong_core::features::effects::lens::{Lens, PlacedLens};
use pong_core::features::effects::{ActiveEffects, BallFlight};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::{Icon, ShellHalf};
use crate::audio::sound::Sound;
use crate::audio::synth::{PLAIN, phrase, tone};
use crate::views::layer;
use crate::views::motion::ViewSystems;

/// A ring around a ring: a lens seen from above.
const ICON: Icon = Icon {
    glyph: "◎",
    color: Color::srgb(0.62, 0.95, 0.80),
};

/// Lens's icon, cue, and view.
///
/// - Cue: [`join_cue`].
/// - View: [`view`], the lenses on the floor.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: Some(ICON),
        join_cue: Some(join_cue),
        view: Some(view),
    }
}

/// Three quick notes closing in on the last from above and below:
/// something drawn toward a line.
fn join_cue() -> Sound {
    phrase(
        &[
            tone(&[659.25], 0.09),
            tone(&[392.0], 0.09),
            tone(&[523.25], 0.3),
        ],
        PLAIN,
    )
}

/// The lenses, placed and drawn.
fn view(app: &mut App) {
    app.add_plugins(FloorLensesPlugin)
        .add_systems(Startup, create_the_lens_look)
        .add_systems(
            RunFixedMainLoop,
            attach_the_lens_look.in_set(ViewSystems::Attach),
        );
}

/// How long a new lens takes to grow to its size, in seconds: once, and
/// slower than anything that could read as a flash (R11).
pub const APPEAR_SECONDS: f32 = 0.25;
/// The share of its size a lens has in the frame it appears.
const NEWBORN: f32 = 0.05;

/// One lens as drawn on the floor.
///
/// Invariant: `lens` is the `index`-th lens of the flight the ball is in,
/// as the core's Lens has it, and the entity's `Transform` lies on its
/// centre, scaled to its radius times [`grown`] of `shown_for`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FloorLens {
    index: usize,
    lens: PlacedLens,
    /// Seconds since it appeared, up to [`APPEAR_SECONDS`].
    shown_for: f32,
}

impl FloorLens {
    /// Which lens of the flight this is, from zero: a drawn lens, then its
    /// copy, in the order they appeared.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Where this lens is drawn, and how large for now.
    fn drawn(self) -> Transform {
        let radius = self.lens.radius * grown(self.shown_for);
        Transform::from_translation(self.lens.centre.extend(layer::LENS))
            .with_scale(Vec3::new(radius, radius, 1.0))
    }
}

/// The share of its size a lens has `shown_for` seconds after it appeared:
/// from [`NEWBORN`] to the whole, fast at first and settling, never
/// shrinking on the way.
fn grown(shown_for: f32) -> f32 {
    let settled = EaseFunction::CubicOut.sample_clamped(shown_for / APPEAR_SECONDS);
    NEWBORN + (1.0 - NEWBORN) * settled
}

/// Puts the flight's lenses on the floor during a match and takes them off
/// again. Touches only the lenses it spawns.
///
/// Preconditions: the core plugin group (`ActiveEffects`, `Tuning`, the
/// states, the ball).
pub struct FloorLensesPlugin;

impl Plugin for FloorLensesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (lay_the_flights_lenses, grow_the_new_lenses)
                .chain()
                .run_if(in_state(AppState::InMatch)),
        );
    }
}

/// Makes the floor hold exactly the lenses of the flight the ball is in:
/// spawns each lens the floor lacks, newborn, and takes off each the flight
/// no longer has.
///
/// The lenses are read from the start the flight began at and the seed of
/// the flight's own rally, through the same [`Lens::placed`] the core's
/// pull is built from, so what is drawn is what pulls. Through a point
/// pause the last flight is still on the ball and the next rally's seed is
/// already drawn: the lenses lie where their rally laid them until the
/// countdown. A flight's start does not change while
/// it lasts and a rally's lenses only grow in number, so within a rally a
/// lens appears where a flight starts and none moves or leaves (M2 spec
/// 2.4.3, R11). With Lens out of play, or no flight, as in a countdown,
/// the floor is empty.
///
/// Postcondition: a lens that leaves takes its ring and centre line with
/// it, by the hierarchy; a lens left on the floor when the match ends goes
/// with the match.
fn lay_the_flights_lenses(
    mut commands: Commands,
    tuning: Res<Tuning>,
    active: Res<ActiveEffects>,
    flights: Query<&BallFlight, With<Ball>>,
    mut floor: Query<(Entity, &mut FloorLens)>,
) {
    let placed = flights
        .iter()
        .next()
        .and_then(BallFlight::in_flight)
        .filter(|_| active.ids().contains(&EffectId::Lens))
        .map_or_else(Vec::new, |flight| {
            Lens::new(&tuning, flight.rally()).placed(&flight.began_at())
        });
    let mut lies = vec![false; placed.len()];
    for (entity, mut floor_lens) in &mut floor {
        let Some(&lens) = placed.get(floor_lens.index) else {
            commands.entity(entity).despawn();
            continue;
        };
        lies[floor_lens.index] = true;
        // A reloaded tuning may move or resize a lens already there.
        if floor_lens.lens != lens {
            floor_lens.lens = lens;
        }
    }
    for (index, &lens) in placed.iter().enumerate() {
        if lies[index] {
            continue;
        }
        let newborn = FloorLens {
            index,
            lens,
            shown_for: 0.0,
        };
        commands.spawn((newborn, newborn.drawn(), DespawnOnExit(AppState::InMatch)));
    }
}

/// Grows each new lens to its size over [`APPEAR_SECONDS`], and keeps every
/// lens drawn where and as large as it is. Writes a `Transform` only when
/// it changes, so a grown lens is left alone.
///
/// The lenses grow on the frame clock, so one that appears as play is
/// paused finishes growing through the pause.
fn grow_the_new_lenses(time: Res<Time>, mut floor: Query<(&mut FloorLens, &mut Transform)>) {
    for (mut floor_lens, mut transform) in &mut floor {
        if floor_lens.shown_for < APPEAR_SECONDS {
            floor_lens.shown_for = (floor_lens.shown_for + time.delta_secs()).min(APPEAR_SECONDS);
        }
        let drawn = floor_lens.drawn();
        if *transform != drawn {
            *transform = drawn;
        }
    }
}

/// Where a lens's ring ends, from its centre, in radii: the ring is the
/// outermost sixteenth of the disc.
const INSIDE_THE_RING: f32 = 0.94;
/// How thick the centre line is drawn, in radii.
const LINE_THICKNESS: f32 = 0.07;
/// How far above the disc its ring is drawn, and its centre line above
/// that. A child's draw order is its parent's plus its own, and all three
/// are opaque, which the depth decides: each needs a height of its own.
const ABOVE: f32 = 0.02;

/// The disc: barely lighter than the floor, so the ball, its trail, and
/// Wind's arrows read on it. Under 1.0 in linear light, as all three parts
/// are: a lens is furniture and does not glow.
const DISC: Color = Color::linear_rgb(0.020, 0.050, 0.045);
/// The ring: where the lens ends.
const RING: Color = Color::linear_rgb(0.10, 0.30, 0.24);
/// The centre line: what the ball is pulled toward, and the brightest part.
const CENTRE_LINE: Color = Color::linear_rgb(0.26, 0.70, 0.56);

/// The shapes and colours of a lens, made once. Each shape is for a lens of
/// radius one, scaled per lens by its `Transform`.
#[derive(Resource, Debug, Clone)]
struct LensLook {
    disc: (Handle<Mesh>, Handle<ColorMaterial>),
    ring: (Handle<Mesh>, Handle<ColorMaterial>),
    centre_line: (Handle<Mesh>, Handle<ColorMaterial>),
}

fn create_the_lens_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(LensLook {
        disc: (meshes.add(Circle::new(1.0)), materials.add(DISC)),
        ring: (
            meshes.add(Annulus::new(INSIDE_THE_RING, 1.0)),
            materials.add(RING),
        ),
        centre_line: (
            meshes.add(Rectangle::new(2.0 * INSIDE_THE_RING, LINE_THICKNESS)),
            materials.add(CENTRE_LINE),
        ),
    });
}

/// Gives each new lens its disc, and its ring and centre line as children,
/// so they lie, grow, and leave with it.
fn attach_the_lens_look(
    mut commands: Commands,
    look: Res<LensLook>,
    lenses: Query<Entity, Added<FloorLens>>,
) {
    for lens in &lenses {
        commands.entity(lens).insert((
            Mesh2d(look.disc.0.clone()),
            MeshMaterial2d(look.disc.1.clone()),
        ));
        for (above, (mesh, material)) in [(ABOVE, &look.ring), (2.0 * ABOVE, &look.centre_line)] {
            commands.spawn((
                ChildOf(lens),
                Transform::from_xyz(0.0, 0.0, above),
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{APPEAR_SECONDS, NEWBORN, grown};

    #[test]
    fn a_lens_grows_from_newborn_to_whole_and_no_further() {
        assert!((grown(0.0) - NEWBORN).abs() < 1e-6);
        assert!((grown(APPEAR_SECONDS) - 1.0).abs() < 1e-6);
        assert!((grown(10.0 * APPEAR_SECONDS) - 1.0).abs() < 1e-6);
        assert!((grown(-1.0) - NEWBORN).abs() < 1e-6);
    }

    /// R11: it settles, and never shrinks on the way.
    #[test]
    fn a_lens_never_shrinks_as_it_grows() {
        let sizes: Vec<f32> = (0..=20_u8)
            .map(|step| grown(APPEAR_SECONDS * f32::from(step) / 20.0))
            .collect();
        assert!(sizes.windows(2).all(|pair| pair[1] >= pair[0]), "{sizes:?}");
        // Fast at first: most of the way there in half the time.
        assert!(sizes[10] > 0.8, "{sizes:?}");
    }
}
