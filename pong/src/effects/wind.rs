//! Wind's shell half (M2 spec 2.6.2): a steady pull on the ball, up or down,
//! its direction chosen per rally.
//!
//! The base views cannot show a wind, so it has a view of its own (R1):
//! small arrowheads across the floor that point and drift the way it blows.
//! They are there from the countdown on, since the core chooses the
//! direction before the countdown, and drift faster as the dial rises.
//!
//! Where the arrows are is [`WindArrowsPlugin`]'s, and testable headless;
//! how one looks is added by [`shell_half`]'s view, with the render assets.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Assets, Handle};
use bevy::camera::visibility::Visibility;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Added, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::{Vec2, Vec3};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::shape::Triangle2d;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::state::state::{OnEnter, State};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::Time;
use bevy::transform::components::Transform;
use pong_core::domain::EffectId;
use pong_core::features::ball::Ball;
use pong_core::features::effects::wind::WindDirection;
use pong_core::features::effects::{ActiveEffects, BallFlight};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::{AppState, MatchState};

use super::{Icon, ShellHalf};
use crate::audio::sound::Sound;
use crate::audio::synth::{PLAIN, phrase, tone};
use crate::views::layer;
use crate::views::motion::ViewSystems;

/// Three waves, one over the other: moving air.
const ICON: Icon = Icon {
    glyph: "≋",
    color: Color::srgb(0.55, 0.85, 1.0),
};

/// Wind's icon, cue, and view.
///
/// - Cue: [`join_cue`].
/// - View: [`view`], the arrows.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: Some(ICON),
        join_cue: Some(join_cue),
        view: Some(view),
    }
}

/// Two plain notes rising a fifth, the second held: a gust picking up.
fn join_cue() -> Sound {
    phrase(&[tone(&[392.0], 0.12), tone(&[587.33], 0.3)], PLAIN)
}

/// The arrows, placed and drawn.
fn view(app: &mut App) {
    app.add_plugins(WindArrowsPlugin)
        .add_systems(Startup, create_the_arrow_look)
        .add_systems(
            RunFixedMainLoop,
            attach_the_arrow_look.in_set(ViewSystems::Attach),
        );
}

/// How many arrows drift across the floor.
const ARROWS: u8 = 14;
/// An arrow's width and height, in arena units: about a ball across.
const ARROW_SIZE: Vec2 = Vec2::new(2.4, 2.0);
/// How fast the arrows drift while the dial is at zero, as in every
/// countdown, in arena units per second: slow, and plainly moving.
const BREEZE: f32 = 12.0;
/// How fast they drift at full dial.
const GALE: f32 = 60.0;
/// Dim and bluish on the dark floor: under 1.0 in linear light, so no bloom.
/// The arrows are scenery; the ball and the paddles stay the brightest.
const ARROW: Color = Color::linear_rgb(0.10, 0.17, 0.24);

/// One of Wind's arrows. Spawned with the match, hidden; shown only while
/// Wind is in play.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindArrow {
    index: u8,
}

impl WindArrow {
    /// Which arrow this is, from zero: its column, left to right.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.index
    }
}

/// Places Wind's arrows during a match: where each is, which way it points,
/// and whether it shows. Touches only `Transform` and `Visibility`.
///
/// Precondition: the core plugin group (`ActiveEffects`, `Tuning`, the
/// states).
pub struct WindArrowsPlugin;

impl Plugin for WindArrowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_the_arrows)
            .add_systems(Update, place_the_arrows.run_if(in_state(AppState::InMatch)));
    }
}

/// How far up or down an arrow's centre may be: half the arena's height,
/// less half an arrow, so a whole arrow is always inside.
fn reach(tuning: &Tuning) -> f32 {
    tuning.arena().half_extents().y - ARROW_SIZE.y / 2.0
}

/// One arrow per column across the arena, each at its own height: the
/// columns evenly spaced, the heights spread by the golden ratio, so no row
/// of arrows moves as one line.
fn spawn_the_arrows(mut commands: Commands, tuning: Res<Tuning>) {
    const GOLDEN_RATIO: f32 = 0.618_034;
    let half_width = tuning.arena().half_extents().x;
    let reach = reach(&tuning);
    for index in 0..ARROWS {
        let column = f32::from(index);
        let x = half_width * (2.0 * (column + 0.5) / f32::from(ARROWS) - 1.0);
        let y = reach * (2.0 * (column * GOLDEN_RATIO).fract() - 1.0);
        commands.spawn((
            WindArrow { index },
            Transform::from_xyz(x, y, layer::WIND).with_scale(ARROW_SIZE.extend(1.0)),
            Visibility::Hidden,
            DespawnOnExit(AppState::InMatch),
        ));
    }
}

/// `y` moved by `travel` and kept within `-reach..=reach`: an arrow that
/// passes one wall comes back in at the other.
///
/// Precondition: `|travel| <= 2 * reach`, as one frame's drift is.
fn drifted(y: f32, travel: f32, reach: f32) -> f32 {
    let moved = y + travel;
    if moved > reach {
        moved - 2.0 * reach
    } else if moved < -reach {
        moved + 2.0 * reach
    } else {
        moved
    }
}

/// How fast the arrows drift when the wind's dial is at `strength`, from
/// zero to one of its full drift.
fn drift_speed(strength: f32) -> f32 {
    BREEZE + strength.clamp(0.0, 1.0) * (GALE - BREEZE)
}

/// Shows the arrows while Wind is in play, pointing the way the rally's
/// seed has it blow, and drifts them that way: at a breeze while the dial
/// is at zero, faster with the drift of the flight the ball is in.
///
/// The direction is read from the rally's seed, as the core's Wind reads
/// it, so what is shown in a countdown is what then blows. The arrows hold
/// still while play is paused and once the match is over.
fn place_the_arrows(
    time: Res<Time>,
    tuning: Res<Tuning>,
    active: Res<ActiveEffects>,
    match_state: Res<State<MatchState>>,
    flights: Query<&BallFlight, With<Ball>>,
    mut arrows: Query<(&mut Transform, &mut Visibility), With<WindArrow>>,
) {
    let shown = if active.ids().contains(&EffectId::Wind) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let direction = WindDirection::of(active.rally_seed()).sign();
    let wind = tuning.effects().wind();
    let returns = flights
        .iter()
        .next()
        .map_or(0, |flight| flight.flight().returns());
    let strength = if wind.max_drift() > 0.0 {
        wind.drift().at(returns) / wind.max_drift()
    } else {
        0.0
    };
    let still = matches!(
        match_state.get(),
        MatchState::Paused | MatchState::MatchOver
    );
    let travel = if still {
        0.0
    } else {
        direction * drift_speed(strength) * time.delta_secs()
    };
    let reach = reach(&tuning);
    for (mut transform, mut visibility) in &mut arrows {
        if *visibility != shown {
            *visibility = shown;
        }
        if shown == Visibility::Hidden {
            continue;
        }
        transform.translation.y = drifted(transform.translation.y, travel, reach);
        // The unit arrow's tip is at +y: a negative height turns it over.
        transform.scale = Vec3::new(ARROW_SIZE.x, direction * ARROW_SIZE.y, 1.0);
    }
}

/// The arrow's shape and colour, made once.
#[derive(Resource, Debug, Clone)]
struct ArrowLook {
    /// One by one, its tip at the top.
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
}

fn create_the_arrow_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let unit_arrow = Triangle2d::new(
        Vec2::new(0.0, 0.5),
        Vec2::new(-0.5, -0.5),
        Vec2::new(0.5, -0.5),
    );
    commands.insert_resource(ArrowLook {
        mesh: meshes.add(unit_arrow),
        material: materials.add(ARROW),
    });
}

fn attach_the_arrow_look(
    mut commands: Commands,
    look: Res<ArrowLook>,
    arrows: Query<Entity, Added<WindArrow>>,
) {
    for entity in &arrows {
        commands.entity(entity).insert((
            Mesh2d(look.mesh.clone()),
            MeshMaterial2d(look.material.clone()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{BREEZE, GALE, drift_speed, drifted};

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn an_arrow_inside_its_reach_moves_by_its_travel() {
        assert_near(drifted(10.0, 0.5, 44.0), 10.5);
        assert_near(drifted(10.0, -0.5, 44.0), 9.5);
        assert_near(drifted(43.5, 0.5, 44.0), 44.0);
    }

    #[test]
    fn an_arrow_past_one_wall_comes_back_in_at_the_other() {
        assert_near(drifted(43.8, 0.5, 44.0), -43.7);
        assert_near(drifted(-43.8, -0.5, 44.0), 43.7);
    }

    #[test]
    fn the_drift_runs_from_a_breeze_to_a_gale_and_no_further() {
        assert_near(drift_speed(0.0), BREEZE);
        assert_near(drift_speed(0.5), f32::midpoint(BREEZE, GALE));
        assert_near(drift_speed(1.0), GALE);
        assert_near(drift_speed(7.0), GALE);
        assert_near(drift_speed(-1.0), BREEZE);
    }
}
