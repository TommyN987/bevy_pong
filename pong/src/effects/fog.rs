//! Fog's shell half (M2 spec 2.6.3, 2.2.9): a band centred on midfield that
//! hides the ball, its trail, and its glow, and nothing else.
//!
//! The base views cannot show where the ball is hidden, so Fog has a view
//! of its own (R1): a band of mist on the floor, as wide as the flight's
//! fog. It is there from the frame a flight starts, before the ball reaches
//! it, and under the arena's furniture, which stays in sight.
//!
//! Where the band is and what it hides is [`FogBandPlugin`]'s, and testable
//! headless; how the mist looks is added by [`shell_half`]'s view, with the
//! render assets: a material of its own, drawn by `fog.wesl`.

use bevy::app::{App, Plugin, RunFixedMainLoop, Startup, Update};
use bevy::asset::{Asset, AssetPath, Assets, Handle, embedded_asset, embedded_path};
use bevy::camera::visibility::Visibility;
use bevy::color::{Color, LinearRgba};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Added, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec3;
use bevy::mesh::{Mesh, Mesh2d};
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::shape::Rectangle;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d};
use bevy::state::condition::in_state;
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::transform::components::Transform;
use pong_core::domain::EffectId;
use pong_core::features::ball::Ball;
use pong_core::features::effects::fog::Fog;
use pong_core::features::effects::{ActiveEffects, BallFlight};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

use super::{Icon, ShellHalf};
use crate::audio::sound::Sound;
use crate::audio::synth::{PLAIN, phrase, tone};
use crate::views::layer;
use crate::views::motion::ViewSystems;
use crate::views::trail::TrailGhost;

/// A cloud: something that hides what is behind it.
const ICON: Icon = Icon {
    glyph: "☁",
    color: Color::srgb(0.78, 0.84, 0.92),
};

/// Fog's icon, cue, and view.
///
/// - Cue: [`join_cue`].
/// - View: [`view`], the band.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: Some(ICON),
        join_cue: Some(join_cue),
        view: Some(view),
    }
}

/// One long, low chord of two close notes: something settling over the
/// field.
fn join_cue() -> Sound {
    phrase(&[tone(&[196.0, 233.08], 0.5)], PLAIN)
}

/// The band, placed and drawn.
///
/// Precondition: `DefaultPlugins` are added first. The shader is embedded
/// through the asset plugin, and the material's plugin draws nothing if it
/// is built before the renderer.
fn view(app: &mut App) {
    embedded_asset!(app, "fog.wesl");
    app.add_plugins((FogBandPlugin, Material2dPlugin::<FogMaterial>::default()))
        .add_systems(Startup, create_the_band_look)
        .add_systems(
            RunFixedMainLoop,
            attach_the_band_look.in_set(ViewSystems::Attach),
        )
        .add_systems(
            Update,
            fit_the_mist_to_the_band
                .after(fit_the_band)
                .run_if(in_state(AppState::InMatch)),
        );
}

/// The mist where it is thickest. Dim, far below the glow of the bodies and
/// under Wind's arrows: the fog is scenery, and what is drawn above it must
/// still read. Its alpha is how much of the floor the thickest mist covers.
const MIST: LinearRgba = LinearRgba::new(0.09, 0.10, 0.12, 0.6);

/// The band of fog: one per match, centred on midfield.
///
/// Invariant: `width` is the fog of the flight the ball is in, or zero
/// when there is none; the band's `Transform` is scaled to it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FogBand {
    width: f32,
}

impl FogBand {
    /// How wide the band is, in arena units.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }
}

/// Places the band during a match and hides the ball and its trail inside
/// it. Touches only the band, and `Visibility` on balls and trail ghosts.
///
/// Preconditions: the core plugin group (`ActiveEffects`, `Tuning`, the
/// states, the ball), and the motion and trail plugins, which draw the
/// bodies this one hides.
pub struct FogBandPlugin;

impl Plugin for FogBandPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_the_band)
            .add_systems(
                Update,
                (fit_the_band, hide_what_is_in_the_band)
                    .chain()
                    .run_if(in_state(AppState::InMatch)),
            );
    }
}

/// One band per match, at midfield, with no width and out of sight.
fn spawn_the_band(mut commands: Commands) {
    commands.spawn((
        FogBand { width: 0.0 },
        Transform::from_xyz(0.0, 0.0, layer::FOG),
        Visibility::Hidden,
        DespawnOnExit(AppState::InMatch),
    ));
}

/// Makes the band as wide as the fog of the flight the ball is in, and as
/// tall as the arena; shows it while it has a width.
///
/// The width is read from the start the flight began at, through the same
/// [`Fog::width`] the AI's flaw is sized by. A flight's start does not
/// change while it lasts, so the band changes only where a flight starts or
/// a rally ends (M2 spec 2.4.3, R11). With Fog out of play, or no flight,
/// as in a countdown, the band has no width.
fn fit_the_band(
    tuning: Res<Tuning>,
    active: Res<ActiveEffects>,
    flights: Query<&BallFlight, With<Ball>>,
    mut bands: Query<(&mut FogBand, &mut Transform, &mut Visibility)>,
) {
    let fog = Fog::new(&tuning);
    let width = flights
        .iter()
        .next()
        .and_then(BallFlight::in_flight)
        .filter(|_| active.ids().contains(&EffectId::Fog))
        .map_or(0.0, |flight| fog.width(&flight.began_at()));
    let shown = if width > 0.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let height = 2.0 * tuning.arena().half_extents().y;
    for (mut band, mut transform, mut visibility) in &mut bands {
        if band.width.to_bits() != width.to_bits() {
            band.width = width;
        }
        if *visibility != shown {
            *visibility = shown;
        }
        transform.scale = Vec3::new(width, height, 1.0);
    }
}

/// Hides each ball and each trail ghost whose drawn centre is inside the
/// band, and shows the rest (M2 spec 2.6.3). Each body on its own, so a
/// trail follows its ball into the fog and out of it. Nothing else is
/// touched: the fog hides the ball only (M2 spec 2.2.9).
///
/// - Precondition: the bodies' `Transform`s are this frame's, as the motion
///   and trail plugins write them before `Update`.
/// - Postcondition: a body is `Hidden` exactly while the band has a width
///   and the body's centre is nearer midfield than half of it; otherwise
///   it is `Inherited`.
/// - Invariant: this system is the only writer of `Visibility` on balls and
///   trail ghosts. A second writer would need a channel to compose with it.
///
/// A ball's glow is its own light and its Spin mark its child, so both go
/// with it. The value is inserted, not mutated: a body has a `Visibility`
/// only once it has a mesh under Bevy's visibility plugin, so headless it
/// has none until this system gives it one.
fn hide_what_is_in_the_band(
    mut commands: Commands,
    bands: Query<&FogBand>,
    balls: Query<(Entity, &Transform, Option<&Visibility>), With<Ball>>,
    ghosts: Query<(Entity, &Transform, Option<&Visibility>), With<TrailGhost>>,
) {
    let half_width = bands.iter().next().map_or(0.0, |band| band.width / 2.0);
    for (body, transform, visibility) in balls.iter().chain(&ghosts) {
        let shown = if transform.translation.x.abs() < half_width {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if visibility != Some(&shown) {
            commands.entity(body).insert(shown);
        }
    }
}

/// The mist: Fog's own material, drawn by `fog.wesl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct FogMaterial {
    /// The mist where it is thickest; alpha is how much it covers there.
    #[uniform(0)]
    color: LinearRgba,
    /// Half the band's width, in arena units: where the mist's edges are.
    #[uniform(1)]
    half_width: f32,
}

impl Material2d for FogMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("fog.wesl")).with_source("embedded"),
        )
    }

    /// Blended over the floor; the trait's default is opaque.
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// The band's shape and its mist, made once.
#[derive(Resource, Debug, Clone)]
struct BandLook {
    /// One by one, centred.
    mesh: Handle<Mesh>,
    material: Handle<FogMaterial>,
}

fn create_the_band_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
) {
    commands.insert_resource(BandLook {
        mesh: meshes.add(Rectangle::new(1.0, 1.0)),
        material: materials.add(FogMaterial {
            color: MIST,
            half_width: 0.0,
        }),
    });
}

fn attach_the_band_look(
    mut commands: Commands,
    look: Res<BandLook>,
    bands: Query<Entity, Added<FogBand>>,
) {
    for entity in &bands {
        commands.entity(entity).insert((
            Mesh2d(look.mesh.clone()),
            MeshMaterial2d(look.material.clone()),
        ));
    }
}

/// Tells the mist where its edges are. Written only when the band's width
/// changes, once a flight: a change to a material is uploaded anew.
fn fit_the_mist_to_the_band(
    look: Res<BandLook>,
    bands: Query<&FogBand>,
    mut materials: ResMut<Assets<FogMaterial>>,
) {
    let Some(half_width) = bands.iter().next().map(|band| band.width / 2.0) else {
        return;
    };
    let fitted = materials
        .get(&look.material)
        .is_some_and(|mist| mist.half_width.to_bits() == half_width.to_bits());
    if fitted {
        return;
    }
    if let Some(mut mist) = materials.get_mut(&look.material) {
        mist.half_width = half_width;
    }
}
