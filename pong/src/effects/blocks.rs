//! Blocks' shell half (M2 spec 2.6.6): solid blocks on the floor that the
//! ball bounces off.
//!
//! The base views cannot show where a block stands, so Blocks has a view
//! of its own (R1): one rectangle on the floor for every block of the
//! flight, as large and as tilted as the block the ball bounces off. A
//! block is there at its full size from the frame its flight starts,
//! before the ball reaches it, since it is solid from that step on; it
//! shows new for a moment and settles. While the flight has spent its
//! bounces (M2 spec 2.2.6.1) every block shows spent: the ball flies
//! through it until the next return.
//!
//! Where the blocks stand, and whether they show new or spent, is
//! [`FloorBlocksPlugin`]'s, and testable headless; how one looks is added
//! by [`shell_half`]'s view, with the render assets.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Added, Changed, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::{Quat, Vec3};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::shape::Rectangle;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::Time;
use bevy::transform::components::Transform;
use pong_core::domain::EffectId;
use pong_core::features::ball::Ball;
use pong_core::features::effects::blocks::Blocks;
use pong_core::features::effects::{ActiveEffects, BallFlight};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;
use pong_core::kernel::obstacle::Obstacle;

use super::{Icon, ShellHalf};
use crate::audio::sound::Sound;
use crate::audio::synth::{BRASS, phrase, rest, tone};
use crate::views::layer;
use crate::views::motion::ViewSystems;

/// A slab leaning over: a block at a tilt.
const ICON: Icon = Icon {
    glyph: "▰",
    color: Color::srgb(0.96, 0.78, 0.52),
};

/// Blocks' icon, cue, and view.
///
/// - Cue: [`join_cue`].
/// - View: [`view`], the blocks on the floor.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: Some(ICON),
        join_cue: Some(join_cue),
        view: Some(view),
    }
}

/// Two low knocks and a note that stays: something heavy set down.
fn join_cue() -> Sound {
    phrase(
        &[
            tone(&[196.0], 0.07),
            rest(0.05),
            tone(&[196.0], 0.07),
            rest(0.05),
            tone(&[261.63], 0.28),
        ],
        BRASS,
    )
}

/// The blocks, placed and drawn.
fn view(app: &mut App) {
    app.add_plugins(FloorBlocksPlugin)
        .add_systems(Startup, create_the_block_look)
        .add_systems(
            RunFixedMainLoop,
            attach_the_block_look.in_set(ViewSystems::Attach),
        )
        .add_systems(
            Update,
            colour_the_blocks
                .after(settle_the_new_blocks)
                .run_if(in_state(AppState::InMatch)),
        );
}

/// How long a new block shows new, in seconds: once, and slower than
/// anything that could read as a flash (R11).
pub const APPEAR_SECONDS: f32 = 0.25;

/// One block as drawn on the floor.
///
/// Invariant: `block` is the `index`-th block of the flight the ball is
/// in, as the core's Blocks has it, and the entity's `Transform` lies on
/// its centre, as large as it is and turned as it is tilted; `spent` is
/// whether that flight has spent its bounces.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FloorBlock {
    index: usize,
    block: Obstacle,
    /// Seconds since it appeared, up to [`APPEAR_SECONDS`].
    shown_for: f32,
    spent: bool,
}

impl FloorBlock {
    /// Which block of the flight this is, from zero: a drawn block, then
    /// its copy, in the order they appeared.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// How new this block shows: one in the frame it appears, falling to
    /// nothing over [`APPEAR_SECONDS`] and never rising again.
    #[must_use]
    pub fn freshness(self) -> f32 {
        1.0 - (self.shown_for / APPEAR_SECONDS).clamp(0.0, 1.0)
    }

    /// Whether the flight has spent its bounces, so that the ball flies
    /// through this block.
    #[must_use]
    pub const fn spent(self) -> bool {
        self.spent
    }

    /// Where this block is drawn, how large, and how turned: its whole
    /// size from the first frame, since the ball bounces off the whole of
    /// it from the first step.
    fn drawn(self) -> Transform {
        let size = 2.0 * self.block.half_size();
        Transform::from_translation(self.block.centre().extend(layer::BLOCK))
            .with_rotation(Quat::from_rotation_z(self.block.tilt()))
            .with_scale(Vec3::new(size.x, size.y, 1.0))
    }

    /// The colour this block is drawn in: [`SPENT`] while the ball flies
    /// through it, else from [`NEW`] to [`SETTLED`] as it settles.
    fn colour(self) -> Color {
        if self.spent {
            return Color::linear_rgb(SPENT[0], SPENT[1], SPENT[2]);
        }
        let fresh = self.freshness();
        let [red, green, blue] =
            [0, 1, 2].map(|part| SETTLED[part] + (NEW[part] - SETTLED[part]) * fresh);
        Color::linear_rgb(red, green, blue)
    }
}

/// Stands the flight's blocks on the floor during a match and takes them
/// off again. Touches only the blocks it spawns.
///
/// Preconditions: the core plugin group (`ActiveEffects`, `Tuning`, the
/// states, the ball).
pub struct FloorBlocksPlugin;

impl Plugin for FloorBlocksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (stand_the_flights_blocks, settle_the_new_blocks)
                .chain()
                .run_if(in_state(AppState::InMatch)),
        );
    }
}

/// Makes the floor hold exactly the blocks of the flight the ball is in:
/// spawns each block the floor lacks, new, and takes off each the flight
/// no longer has. Marks every block spent while the flight has spent its
/// bounces, and solid again when it has not.
///
/// The blocks are read from the start the flight began at and the seed of
/// the flight's own rally, through the same [`Blocks::placed`] the flight's
/// obstacles were contributed from, so what is drawn is what the ball
/// bounces off. Through a point pause the last flight is still on the
/// ball and the next rally's seed is already drawn: the blocks stand where
/// their rally stood them until the countdown. A flight's start does not
/// change while it lasts and a rally's blocks only grow in number, so
/// within a rally a block appears where a flight starts and none moves or
/// leaves (M2 spec 2.4.3, R11). With Blocks out of play, or no flight, as
/// in a countdown, the floor is empty.
///
/// Postcondition: a block left on the floor when the match ends goes with
/// the match. A block is written only when something of it changes, so a
/// settled, solid block is left alone.
fn stand_the_flights_blocks(
    mut commands: Commands,
    tuning: Res<Tuning>,
    active: Res<ActiveEffects>,
    flights: Query<&BallFlight, With<Ball>>,
    mut floor: Query<(Entity, &mut FloorBlock, &mut Transform)>,
) {
    let flight = flights
        .iter()
        .next()
        .and_then(BallFlight::in_flight)
        .filter(|_| active.ids().contains(&EffectId::Blocks));
    let placed = flight.map_or_else(Vec::new, |flight| {
        Blocks::new(&tuning, flight.rally()).placed(&flight.began_at())
    });
    let spent = flight.is_some_and(|flight| flight.obstacles_in_flight().spent());
    let mut stands = vec![false; placed.len()];
    for (entity, mut floor_block, mut transform) in &mut floor {
        let Some(&block) = placed.get(floor_block.index) else {
            commands.entity(entity).despawn();
            continue;
        };
        stands[floor_block.index] = true;
        // A reloaded tuning may move, turn, or resize a block already there.
        if floor_block.block != block {
            floor_block.block = block;
            *transform = floor_block.drawn();
        }
        if floor_block.spent != spent {
            floor_block.spent = spent;
        }
    }
    for (index, &block) in placed.iter().enumerate() {
        if stands[index] {
            continue;
        }
        let new = FloorBlock {
            index,
            block,
            shown_for: 0.0,
            spent,
        };
        commands.spawn((new, new.drawn(), DespawnOnExit(AppState::InMatch)));
    }
}

/// Ages each new block until it has settled, [`APPEAR_SECONDS`] on, and
/// leaves a settled block alone.
///
/// The blocks settle on the frame clock, so one that appears as play is
/// paused finishes settling through the pause.
fn settle_the_new_blocks(time: Res<Time>, mut floor: Query<&mut FloorBlock>) {
    for mut floor_block in &mut floor {
        if floor_block.shown_for < APPEAR_SECONDS {
            floor_block.shown_for = (floor_block.shown_for + time.delta_secs()).min(APPEAR_SECONDS);
        }
    }
}

/// A settled block, in linear light: stone, plain against the floor and
/// duller than anything that plays. Under 1.0, as all three colours are: a
/// block is furniture and does not glow.
const SETTLED: [f32; 3] = [0.30, 0.24, 0.15];
/// A block in the frame it appears: the same stone, lit.
const NEW: [f32; 3] = [0.85, 0.70, 0.45];
/// A block the ball flies through: barely off the floor, still there to
/// see where it will be solid again.
const SPENT: [f32; 3] = [0.050, 0.044, 0.036];

/// The shape of a block, made once: a square of side one, scaled and
/// turned per block by its `Transform`.
#[derive(Resource, Debug, Clone)]
struct BlockLook {
    shape: Handle<Mesh>,
}

fn create_the_block_look(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(BlockLook {
        shape: meshes.add(Rectangle::new(1.0, 1.0)),
    });
}

/// Gives each new block its shape and a colour of its own, which
/// [`colour_the_blocks`] then keeps: opaque, since a block's colour never
/// has less than full alpha.
fn attach_the_block_look(
    mut commands: Commands,
    look: Res<BlockLook>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    blocks: Query<(Entity, &FloorBlock), Added<FloorBlock>>,
) {
    for (block, floor_block) in &blocks {
        commands.entity(block).insert((
            Mesh2d(look.shape.clone()),
            MeshMaterial2d(materials.add(floor_block.colour())),
        ));
    }
}

/// Draws each block that has changed in the colour it now has: settling,
/// spent, or solid again.
fn colour_the_blocks(
    mut materials: ResMut<Assets<ColorMaterial>>,
    blocks: Query<(&FloorBlock, &MeshMaterial2d<ColorMaterial>), Changed<FloorBlock>>,
) {
    for (floor_block, material) in &blocks {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.color = floor_block.colour();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::color::{Color, LinearRgba};
    use bevy::math::Vec2;
    use pong_core::kernel::obstacle::Obstacle;

    use super::{APPEAR_SECONDS, FloorBlock, NEW, SETTLED, SPENT};

    fn block_shown_for(shown_for: f32, spent: bool) -> FloorBlock {
        FloorBlock {
            index: 0,
            block: Obstacle::new(Vec2::ZERO, Vec2::new(1.0, 7.0), 0.3),
            shown_for,
            spent,
        }
    }

    fn brightness(block: FloorBlock) -> f32 {
        let LinearRgba {
            red, green, blue, ..
        } = block.colour().to_linear();
        red + green + blue
    }

    /// R11: it settles once, and never freshens on the way.
    #[test]
    fn a_block_shows_new_then_settles_and_never_freshens() {
        let shares: Vec<f32> = (0..=20_u8)
            .map(|step| block_shown_for(APPEAR_SECONDS * f32::from(step) / 20.0, false).freshness())
            .collect();
        assert!((shares[0] - 1.0).abs() < 1e-6, "{shares:?}");
        assert!(shares[20].abs() < 1e-6, "{shares:?}");
        assert!(
            shares.windows(2).all(|pair| pair[1] <= pair[0]),
            "{shares:?}"
        );
        assert!(
            block_shown_for(10.0 * APPEAR_SECONDS, false)
                .freshness()
                .abs()
                < 1e-6
        );
    }

    /// R1 for the bounce budget: a spent block is plainly dimmer than a
    /// settled one, and a new one brighter; and whether a block is spent
    /// decides its colour, however new it is.
    #[test]
    fn a_new_block_is_brighter_than_a_settled_one_and_a_spent_one_far_dimmer() {
        let new = brightness(block_shown_for(0.0, false));
        let settled = brightness(block_shown_for(APPEAR_SECONDS, false));
        let spent = brightness(block_shown_for(APPEAR_SECONDS, true));
        assert!(new > 2.0 * settled, "new {new}, settled {settled}");
        assert!(settled > 4.0 * spent, "settled {settled}, spent {spent}");
        assert_eq!(
            block_shown_for(0.0, true).colour(),
            block_shown_for(APPEAR_SECONDS, true).colour()
        );
    }

    /// A block is furniture: no colour of it reaches the light that blooms,
    /// and each is opaque.
    #[test]
    fn no_colour_of_a_block_glows_and_each_is_opaque() {
        for colour in [SETTLED, NEW, SPENT] {
            assert!(colour.iter().all(|part| (0.0..1.0).contains(part)));
        }
        for block in [block_shown_for(0.0, false), block_shown_for(1.0, true)] {
            assert!(matches!(block.colour(), Color::LinearRgba(colour) if colour.alpha >= 1.0));
        }
    }
}
