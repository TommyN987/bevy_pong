//! Tuning (spec 1.4.8): one embedded RON file, validated at load into the
//! [`Tuning`] resource. Every range check goes through a kernel constructor,
//! so this module owns no range table of its own.

mod file;

use std::collections::HashSet;
use std::num::{NonZeroU8, NonZeroU16};
use std::time::Duration;

use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::io::Reader;
use bevy::asset::{
    Asset, AssetApp, AssetEvent, AssetLoadFailedEvent, AssetLoader, AssetPath, AssetServer, Assets,
    Handle, LoadContext, LoadState, embedded_asset, embedded_path,
};
use bevy::color::Srgba;
use bevy::ecs::error::BevyError;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::{not, resource_exists};
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::reflect::TypePath;

use crate::domain::{Difficulty, EffectId, Mode, Notice, Seat};
use crate::flow::FIXED_HZ;
use crate::kernel::geometry::{Arena, InvalidArena, InvalidLengthBounds, LengthBounds, Radius};
use crate::kernel::obstacle::BounceRule;
use crate::kernel::ramp::{FullBeforeStart, InvalidItemCount, ItemCount, Ramp};
use crate::kernel::reflect::{InvalidSpeedRule, MaxDeflection, SpeedRule};
use crate::kernel::{OutOfRange, Range};
use file::TuningFile;

/// The tuning file compiled into the binary. The asset system serves the same
/// bytes at runtime; this copy exists so CI can validate the file synchronously.
pub const EMBEDDED_RON: &str = include_str!("tuning.ron");

/// Loads the embedded tuning file through the asset system and promotes it into
/// the [`Tuning`] resource once validated.
///
/// Contract: `AssetPlugin` must be added before this plugin (ADR 0001). The
/// resource is absent until the load completes; `Boot` (spec 1.4.10) waits on it.
/// Going through the asset system rather than parsing [`EMBEDDED_RON`] directly is
/// what lets dev builds hot-reload the file later (spec 1.4.8, plan 3.9).
pub struct TuningPlugin;

impl Plugin for TuningPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "tuning.ron");
        app.init_asset::<Tuning>()
            .register_asset_loader(TuningLoader)
            .add_message::<Notice>()
            .init_resource::<ShownRefusal>()
            .add_systems(Startup, begin_loading)
            .add_systems(
                Update,
                (
                    promote_when_loaded.run_if(not(resource_exists::<Tuning>)),
                    (adopt_reloaded_tuning, notice_refused_override)
                        .run_if(resource_exists::<Tuning>),
                ),
            );
    }
}

/// The in-flight or completed load of the tuning asset.
#[derive(Resource)]
struct TuningHandle(Handle<Tuning>);

fn begin_loading(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(TuningHandle(asset_server.load(embedded_tuning_path())));
}

/// Where the asset system serves the embedded tuning file, as
/// `load_embedded_asset!` would resolve it from this module.
fn embedded_tuning_path() -> AssetPath<'static> {
    AssetPath::from_path_buf(embedded_path!("tuning.ron")).with_source("embedded")
}

/// Copies the loaded asset into the resource. A failed load is a programmer
/// error for the embedded file (CI validates it), so it goes to the app's error
/// handler: panic in dev, log in release.
fn promote_when_loaded(
    mut commands: Commands,
    handle: Res<TuningHandle>,
    asset_server: Res<AssetServer>,
    assets: Res<Assets<Tuning>>,
) -> Result<(), BevyError> {
    if let Some(tuning) = assets.get(&handle.0) {
        commands.insert_resource(tuning.clone());
        return Ok(());
    }
    if let LoadState::Failed(err) = asset_server.load_state(&handle.0) {
        return Err(BevyError::from(format!(
            "embedded tuning failed to load: {err}"
        )));
    }
    Ok(())
}

/// Dev override, the accepting half (spec 1.4.8): when the asset system
/// reloads the tuning file and the loader accepted it, the resource takes the
/// new values. A refused file never reaches `Assets`, so the resource keeps
/// the last good values by construction.
fn adopt_reloaded_tuning(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<Tuning>>,
    handle: Res<TuningHandle>,
    assets: Res<Assets<Tuning>>,
    mut shown: ResMut<ShownRefusal>,
) {
    let reloaded = events
        .read()
        .any(|event| matches!(event, AssetEvent::Modified { id } if *id == handle.0.id()));
    if !reloaded {
        return;
    }
    if let Some(tuning) = assets.get(&handle.0) {
        commands.insert_resource(tuning.clone());
        shown.0 = None;
    }
}

/// The refusal the player has been told about, as the error's text, until a
/// reload succeeds. Invariant: `Some` only after a notice went out and before
/// the next adopted reload.
///
/// Bevy 0.20.1 reports one refused reload twice: `reload` retries a failed
/// typed load as an untyped one, and both fail, possibly in different frames.
/// A refusal equal to the one shown is not announced again.
#[derive(Resource, Default)]
struct ShownRefusal(Option<String>);

/// Dev override, the refusing half: a reloaded file the loader refused leaves
/// the resource as it is and tells the player why through one [`Notice`]. The
/// asset server has already logged the error.
fn notice_refused_override(
    mut failures: MessageReader<AssetLoadFailedEvent<Tuning>>,
    handle: Res<TuningHandle>,
    mut shown: ResMut<ShownRefusal>,
    mut notices: MessageWriter<Notice>,
) {
    for failure in failures.read().filter(|f| f.id == handle.0.id()) {
        let refusal = failure.error.to_string();
        if shown.0.as_deref() == Some(refusal.as_str()) {
            continue;
        }
        notices.write(Notice {
            text: format!("Tuning override refused, keeping the last good values: {refusal}"),
        });
        shown.0 = Some(refusal);
    }
}

/// Reads the RON bytes and validates them into a [`Tuning`] asset.
#[derive(TypePath)]
struct TuningLoader;

/// Why the asset loader failed: unreadable bytes, or a file [`Tuning::from_ron`] refused.
#[derive(Debug, thiserror::Error)]
pub enum TuningLoadError {
    /// The asset source could not deliver the bytes.
    #[error("could not read tuning: {0}")]
    Io(#[from] std::io::Error),
    /// The bytes are not UTF-8 text.
    #[error("tuning is not UTF-8: {0}")]
    NotUtf8(#[from] std::str::Utf8Error),
    /// The text parsed or validated badly.
    #[error(transparent)]
    Invalid(#[from] TuningError),
}

impl AssetLoader for TuningLoader {
    type Asset = Tuning;
    type Settings = ();
    type Error = TuningLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        (): &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(Tuning::from_ron(std::str::from_utf8(&bytes)?)?)
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

/// Validated tuning values. Invariant: every field passed its kernel constructor,
/// so no system re-checks a tuning value. Constructed only by [`Tuning::from_ron`].
///
/// Both an asset (what the loader produces) and a resource (what systems read).
#[derive(Debug, Clone, PartialEq, Resource, Asset, TypePath)]
pub struct Tuning {
    arena: Arena,
    paddle: PaddleTuning,
    ball: BallTuning,
    modes: ModesTuning,
    effects: EffectsTuning,
    match_flow: MatchFlowTuning,
    ai: AiTuning,
    bindings: BindingsTuning,
    palette: Vec<Srgba>,
}

/// Every mode's data. Invariant: Chaos's per-hit increase is below Classic's
/// (M2 spec 2.6.0).
#[derive(Debug, Clone, PartialEq)]
pub struct ModesTuning {
    classic: ModeTuning,
    chaos: ModeTuning,
}

/// One mode as data (M2 spec 2.1.1): its speed ramp, its effects, and its
/// schedule. Classic is the mode with an empty bag.
///
/// Invariants: no effect is named twice across `always` and `bag`; the
/// thresholds rise strictly and none is above a hundred.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeTuning {
    speed_rule: SpeedRule,
    always: Vec<EffectId>,
    bag: Vec<EffectId>,
    thresholds_percent: Vec<u8>,
    rotate_every_points: NonZeroU8,
}

/// Each effect's own numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectsTuning {
    shrink: ShrinkTuning,
    wind: WindTuning,
    spin: SpinTuning,
    fog: FogTuning,
    lens: LensTuning,
    blocks: BlocksTuning,
}

/// Shrink's dial (M2 spec 2.6.1), as the factor on paddle height: one until
/// the dial starts to move, `height_min / height_default` once it is full.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShrinkTuning {
    height_factor: Ramp,
}

/// Wind's dial (M2 spec 2.6.2), as the drift over one full crossing in arena
/// units, and its AI flaw (M2 spec 2.3.12): the share of the true drift an
/// AI of each star expects.
///
/// Invariant: the drift is never negative and stays under half the playable
/// height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindTuning {
    drift: Ramp,
    believed: PerStar,
}

/// Spin's dial (M2 spec 2.6.4), as the drift over one full crossing from a
/// moving-paddle hit in arena units, and its AI flaw (M2 spec 2.3.12): the
/// share of the true drift an AI of each star expects.
///
/// Invariant: the drift is never negative and stays under half the playable
/// height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinTuning {
    drift: Ramp,
    believed: PerStar,
}

/// Fog's dial (M2 spec 2.6.3), as the width of its band in arena units, and
/// its AI flaw (M2 spec 2.3.12): the error added to each star's aim at full
/// dial.
///
/// Invariant: the width is never negative and stays under half the
/// crossing width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogTuning {
    width: Ramp,
    added_error: PerStar,
}

/// Lens's dial (M2 spec 2.6.5), as the count of lenses drawn, the shape
/// every lens has, and its AI flaw (M2 spec 2.3.12): the flights for which
/// an AI of each star does not see a new lens, and the share of a seen
/// lens's pull it expects.
///
/// Invariants: the radius is positive and at most half the playable height;
/// a lens across and the clear run fit between midfield and a paddle's
/// contact line; the largest bend is from nothing to
/// [`LARGEST_BEND_DEGREES`](Self::LARGEST_BEND_DEGREES).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensTuning {
    count: ItemCount,
    radius: f32,
    max_bend: f32,
    clear_run: f32,
    unseen_flights: PerStar<u16>,
    believed: PerStar,
}

/// Blocks' dial (M2 spec 2.6.6), as the count of blocks drawn, the shape
/// every block has and the range its tilt is drawn from, and its AI flaw
/// (M2 spec 2.3.12): the flights for which an AI of each star does not see
/// a new block, and the share of a seen block's tilt it believes.
///
/// Invariants: a block is the default paddle's length and the paddle's
/// thickness; its tilt from upright is above nothing and under a right
/// angle, the least at most the most; the clear run is more than a ball at
/// the speed cap travels in one fixed step; a block's centre has room
/// ([`Tuning::block_room`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlocksTuning {
    count: ItemCount,
    half_size: Vec2,
    min_tilt: f32,
    max_tilt: f32,
    clear_run: f32,
    unseen_flights: PerStar<u16>,
    believed: PerStar,
}

/// Where the centre of a block may lie on the left half of the arena, so
/// that at any tilt the block keeps clear of the ball's serve, of its
/// mirrored copy, of the paddle, and of both walls (M2 spec 2.6.6).
///
/// Invariant (validated tuning's): `nearest` is at most `farthest`, and
/// `highest` is not negative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockRoom {
    nearest: f32,
    farthest: f32,
    highest: f32,
}

impl BlockRoom {
    /// The least a centre lies from midfield. A block then keeps a ball's
    /// width off the midfield line: a ball served from it touches no block,
    /// and a ball passes between a block and its copy.
    #[must_use]
    pub const fn nearest(self) -> f32 {
        self.nearest
    }

    /// The most a centre lies from midfield. A block grown by the ball's
    /// radius then stays the clear run short of the paddle's contact line.
    #[must_use]
    pub const fn farthest(self) -> f32 {
        self.farthest
    }

    /// The most a centre lies above or below mid-height. A block then
    /// keeps a ball's width off each wall, so a ball passes between them.
    #[must_use]
    pub const fn highest(self) -> f32 {
        self.highest
    }
}

/// One number of an effect's AI flaw for each star (M2 spec 2.3.12); what
/// the number means is its owner's to say.
///
/// Invariant: no number is negative. One entry per [`Difficulty`], so adding
/// a level makes the compiler ask for its number.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PerStar<T = f32> {
    one_star: T,
    two_stars: T,
    three_stars: T,
}

/// Keys per seat. Invariant: no key is bound twice across both seats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingsTuning {
    left: SeatKeys,
    right: SeatKeys,
}

/// One seat's four keys (spec decisions: W/A/S/D left, arrows right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeatKeys {
    up: KeyCode,
    down: KeyCode,
    action: KeyCode,
    back: KeyCode,
}

/// Countdown and points-to-win choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchFlowTuning {
    countdown: Duration,
    point_pause: Duration,
    points_to_win_options: Vec<NonZeroU8>,
    points_to_win_default: NonZeroU8,
}

/// One entry per [`Difficulty`]; adding a level makes the compiler ask for its numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiTuning {
    one_star: DifficultyTuning,
    two_stars: DifficultyTuning,
    three_stars: DifficultyTuning,
}

/// The mirror AI's handicap at one difficulty (spec 1.4.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DifficultyTuning {
    reaction_delay: Duration,
    prediction_error: f32,
}

/// Paddle size bounds and speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaddleTuning {
    height_bounds: LengthBounds,
    height_default: f32,
    thickness: f32,
    speed: f32,
    inset: f32,
}

/// Ball size, speeds, and angles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallTuning {
    radius: Radius,
    serve_speed: f32,
    speed_cap: f32,
    flight_limit_seconds: f32,
    contacts_per_step: NonZeroU8,
    obstacle_bounces_per_flight: NonZeroU16,
    max_deflection: MaxDeflection,
    serve_cone: MaxDeflection,
}

/// Why a tuning file was refused. Each variant names the value to fix.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TuningError {
    /// Not valid RON, or a field the schema does not know.
    #[error("tuning does not parse: {0}")]
    Parse(#[from] ron::de::SpannedError),
    /// A single value outside its range, at this path in the file.
    #[error("tuning {path}: {refused}")]
    Field {
        /// Dotted path in the file, for example `ball.radius`.
        path: &'static str,
        /// The value and the range it had to be in.
        refused: OutOfRange,
    },
    /// Arena dimensions refused by the kernel.
    #[error("tuning arena: {0}")]
    Arena(#[from] InvalidArena),
    /// Ball speed cap or per-hit increase refused by the kernel.
    #[error("tuning ball speed: {0}")]
    BallSpeedRule(#[from] InvalidSpeedRule),
    /// Paddle height bounds refused by the kernel.
    #[error("tuning paddle height: {0}")]
    PaddleHeightBounds(#[from] InvalidLengthBounds),
    /// The serve would start faster than the cap allows.
    #[error("tuning ball.serve_speed {serve_speed} is above ball.speed_cap {cap}")]
    ServeSpeedAboveCap {
        /// The serve speed given.
        serve_speed: f32,
        /// The cap given.
        cap: f32,
    },
    /// A mode names one effect twice across its `always` and `bag` lists.
    #[error("tuning modes.{mode} names the effect {effect:?} more than once")]
    ModeNamesEffectTwice {
        /// `classic` or `chaos`.
        mode: &'static str,
        /// The effect named twice.
        effect: EffectId,
    },
    /// A mode's thresholds do not rise strictly, or one is above a hundred.
    #[error(
        "tuning modes.{mode}.thresholds_percent {thresholds:?} must rise strictly and stay within 100"
    )]
    ModeThresholdsDoNotRise {
        /// `classic` or `chaos`.
        mode: &'static str,
        /// The thresholds given.
        thresholds: Vec<u8>,
    },
    /// Chaos must ramp its speed more gently than Classic (M2 spec 2.6.0).
    #[error(
        "tuning modes.chaos.per_hit_increase {chaos} is not below modes.classic.per_hit_increase {classic}"
    )]
    ChaosRampNotBelowClassic {
        /// Classic's increase given.
        classic: f32,
        /// Chaos's increase given.
        chaos: f32,
    },
    /// Shrink's dial would be full before it starts to move.
    #[error("tuning effects.shrink: {0}")]
    ShrinkRamp(#[from] FullBeforeStart),
    /// Wind's dial would be full before it starts to move.
    #[error("tuning effects.wind: {0}")]
    WindRamp(#[source] FullBeforeStart),
    /// At full dial the wind would carry a ball from the middle of the arena
    /// to a wall, or farther, in one crossing (M2 spec 2.6.2).
    #[error(
        "tuning effects.wind.max_drift {max_drift} is not under half the playable height {half_playable_height}"
    )]
    WindDriftNotUnderHalfThePlayableHeight {
        /// The drift given.
        max_drift: f32,
        /// Half of `arena.height` less the ball's diameter.
        half_playable_height: f32,
    },
    /// Spin's dial would be full before it starts to move.
    #[error("tuning effects.spin: {0}")]
    SpinRamp(#[source] FullBeforeStart),
    /// At full dial one spun return would carry a ball from the middle of
    /// the arena to a wall, or farther, in one crossing.
    #[error(
        "tuning effects.spin.max_drift {max_drift} is not under half the playable height {half_playable_height}"
    )]
    SpinDriftNotUnderHalfThePlayableHeight {
        /// The drift given.
        max_drift: f32,
        /// Half of `arena.height` less the ball's diameter.
        half_playable_height: f32,
    },
    /// Fog's dial would be full before it starts to move.
    #[error("tuning effects.fog: {0}")]
    FogRamp(#[source] FullBeforeStart),
    /// At full dial the fog would leave the two paddles less visible run
    /// between them than it hides (M2 spec 2.6.3).
    #[error(
        "tuning effects.fog.max_width {max_width} is not under half the crossing width {half_crossing_width}"
    )]
    FogNotUnderHalfTheCrossingWidth {
        /// The width given.
        max_width: f32,
        /// Half the distance between the two paddles' contact lines.
        half_crossing_width: f32,
    },
    /// Lens's count would be full before it starts to move, or would start
    /// to move at the serve (M2 spec 2.3.9).
    #[error("tuning effects.lens: {0}")]
    LensCount(#[source] InvalidItemCount),
    /// A lens's pull is sized by measure, and the measure reaches this bend
    /// and no farther (M2 spec 2.6.5).
    #[error(
        "tuning effects.lens.max_bend_degrees {max_bend_degrees} is above {largest_degrees}, the largest bend a lens's pull is sized for"
    )]
    LensBendAboveTheLargest {
        /// The bend given.
        max_bend_degrees: f32,
        /// [`LensTuning::LARGEST_BEND_DEGREES`].
        largest_degrees: f32,
    },
    /// A lens across and the clear run before the paddle do not fit between
    /// midfield and the paddle's contact line (M2 spec 2.6.5).
    #[error(
        "tuning effects.lens.radius {radius} and effects.lens.clear_run {clear_run} leave a lens no room on its half of the crossing, {half_crossing_width} wide"
    )]
    LensHasNoRoomOnItsHalf {
        /// The radius given.
        radius: f32,
        /// The clear run given.
        clear_run: f32,
        /// Half the distance between the two paddles' contact lines.
        half_crossing_width: f32,
    },
    /// A lens would reach past where the ball's centre can go, so a ball
    /// could bounce off a wall inside it.
    #[error(
        "tuning effects.lens.radius {radius} is above half the playable height {half_playable_height}"
    )]
    LensTallerThanThePlayableHeight {
        /// The radius given.
        radius: f32,
        /// Half of `arena.height` less the ball's diameter.
        half_playable_height: f32,
    },
    /// Blocks' count would be full before it starts to move, or would start
    /// to move at the serve (M2 spec 2.3.9).
    #[error("tuning effects.blocks: {0}")]
    BlocksCount(#[source] InvalidItemCount),
    /// The most a block may tilt is below the least.
    #[error(
        "tuning effects.blocks.max_tilt_degrees {max_tilt_degrees} is below effects.blocks.min_tilt_degrees {min_tilt_degrees}"
    )]
    BlocksTiltRangeReversed {
        /// The least tilt given.
        min_tilt_degrees: f32,
        /// The most tilt given.
        max_tilt_degrees: f32,
    },
    /// A ball could bounce off a block and reach a paddle's contact line in
    /// one fixed step: the swept paddle contact and the prediction's last
    /// step both take that step's path for a straight line (M2 spec 2.2.6).
    #[error(
        "tuning effects.blocks.clear_run {clear_run} is not above {step_travel}, what a ball at the speed cap travels in one step"
    )]
    BlocksClearRunWithinOneStep {
        /// The clear run given.
        clear_run: f32,
        /// `ball.speed_cap` over the fixed rate.
        step_travel: f64,
    },
    /// A block clear of midfield and of the clear run before the paddle
    /// does not fit on its half of the crossing (M2 spec 2.6.6).
    #[error(
        "tuning effects.blocks.clear_run {clear_run} leaves a block {length} long no room on its half of the crossing, {half_crossing_width} wide"
    )]
    BlocksHaveNoRoomOnTheirHalf {
        /// The clear run given.
        clear_run: f32,
        /// A block's length: `paddle.height_default`.
        length: f32,
        /// Half the distance between the two paddles' contact lines.
        half_crossing_width: f32,
    },
    /// A block, at any tilt, would not leave a ball's width to each wall.
    #[error(
        "tuning paddle.height_default {length} makes a block too long to stand a ball's width clear of both walls in an arena {arena_height} high"
    )]
    BlockHasNoRoomBetweenTheWalls {
        /// A block's length: `paddle.height_default`.
        length: f32,
        /// `arena.height`.
        arena_height: f32,
    },
    /// No points-to-win choices offered.
    #[error("tuning match_flow.points_to_win_options is empty")]
    PointsToWinOptionsEmpty,
    /// The default points-to-win is not one of the offered choices.
    #[error("tuning match_flow.points_to_win_default {default} is not among the options")]
    PointsToWinDefaultNotOffered {
        /// The default given.
        default: NonZeroU8,
    },
    /// The same key appears twice across both seats' bindings.
    #[error("tuning bindings bind {key:?} more than once")]
    DuplicateKey {
        /// The key bound twice.
        key: KeyCode,
    },
    /// No colors to choose a profile color from.
    #[error("tuning palette is empty")]
    PaletteEmpty,
    /// One channel of one palette entry outside its range.
    #[error("tuning palette[{index}].{channel}: {refused}")]
    PaletteChannel {
        /// Position in the palette list.
        index: usize,
        /// `red`, `green`, or `blue`.
        channel: &'static str,
        /// The value and the range it had to be in.
        refused: OutOfRange,
    },
    /// The default paddle height is not within its own bounds.
    #[error("tuning paddle.height_default {default} is outside {min}..={max}")]
    PaddleHeightDefaultOutsideBounds {
        /// The default given.
        default: f32,
        /// The lower bound given.
        min: f32,
        /// The upper bound given.
        max: f32,
    },
    /// The tallest paddle would not fit between the walls.
    #[error("tuning paddle.height_max {height_max} is above arena.height {arena_height}")]
    PaddleTallerThanArena {
        /// The largest paddle height given.
        height_max: f32,
        /// The arena height given.
        arena_height: f32,
    },
    /// The ball would not fit between the walls.
    #[error(
        "tuning ball.radius {radius} leaves no room between the walls of arena.height {arena_height}"
    )]
    BallTooBigForArena {
        /// The radius given.
        radius: f32,
        /// The arena height given.
        arena_height: f32,
    },
    /// A paddle would stand on or past the centre line.
    #[error(
        "tuning paddle.inset {inset} plus paddle.thickness {thickness} reaches the centre line, {half_width} from each goal"
    )]
    PaddleReachesCentreLine {
        /// The inset given.
        inset: f32,
        /// The thickness given.
        thickness: f32,
        /// Half the arena width.
        half_width: f32,
    },
    /// At the cap the ball could cross the whole playable height in one fixed
    /// step, more than the single wall fold of a physics step undoes: the
    /// wall step would cut such a launch to its bound (M2 spec 2.2.2).
    #[error(
        "tuning ball.speed_cap {cap} carries the ball across the playable height {playable_height} in one fixed step"
    )]
    SpeedCapCrossesArenaInOneStep {
        /// The cap given.
        cap: f32,
        /// `arena.height` less the ball's diameter: the range of its centre.
        playable_height: f32,
    },
    /// A key the shell keeps for itself: Escape pauses and backs out everywhere.
    #[error("tuning bindings bind {key:?}, which is reserved")]
    ReservedKey {
        /// The reserved key bound.
        key: KeyCode,
    },
}

impl Tuning {
    /// Parses and validates RON text.
    ///
    /// # Errors
    /// The first problem found, naming the value to fix.
    pub fn from_ron(text: &str) -> Result<Self, TuningError> {
        let file: TuningFile = ron::from_str(text)?;
        let paddle = PaddleTuning::validate(&file.paddle)?;
        let tuning = Self {
            arena: Arena::new(file.arena.width, file.arena.height)?,
            paddle,
            ball: BallTuning::validate(&file.ball)?,
            modes: ModesTuning::validate(file.modes, file.ball.speed_cap)?,
            effects: EffectsTuning::validate(&file.effects, paddle)?,
            match_flow: MatchFlowTuning::validate(file.match_flow)?,
            ai: AiTuning::validate(&file.ai)?,
            bindings: BindingsTuning::validate(&file.bindings)?,
            palette: validate_palette(&file.palette)?,
        };
        tuning.check_everything_fits_the_arena()?;
        Ok(tuning)
    }

    /// Rules across sections, each a precondition somewhere downstream: the
    /// paddle's wall clamp, the ball's wall fold, and the swept collision.
    fn check_everything_fits_the_arena(&self) -> Result<(), TuningError> {
        let arena_height = 2.0 * self.arena.half_extents().y;
        let half_width = self.arena.half_extents().x;
        let height_max = self.paddle.height_bounds.max();
        if height_max > arena_height {
            return Err(TuningError::PaddleTallerThanArena {
                height_max,
                arena_height,
            });
        }
        let radius = self.ball.radius.value();
        if 2.0 * radius >= arena_height {
            return Err(TuningError::BallTooBigForArena {
                radius,
                arena_height,
            });
        }
        if self.paddle.inset + self.paddle.thickness >= half_width {
            return Err(TuningError::PaddleReachesCentreLine {
                inset: self.paddle.inset,
                thickness: self.paddle.thickness,
                half_width,
            });
        }
        let cap = self.ball.speed_cap;
        let playable_height = 2.0 * self.arena.center_limit_y(self.ball.radius);
        if f64::from(cap) / FIXED_HZ >= f64::from(playable_height) {
            return Err(TuningError::SpeedCapCrossesArenaInOneStep {
                cap,
                playable_height,
            });
        }
        let max_drift = self.effects.wind.max_drift();
        if max_drift >= playable_height / 2.0 {
            return Err(TuningError::WindDriftNotUnderHalfThePlayableHeight {
                max_drift,
                half_playable_height: playable_height / 2.0,
            });
        }
        let max_drift = self.effects.spin.max_drift();
        if max_drift >= playable_height / 2.0 {
            return Err(TuningError::SpinDriftNotUnderHalfThePlayableHeight {
                max_drift,
                half_playable_height: playable_height / 2.0,
            });
        }
        let max_width = self.effects.fog.max_width();
        let half_crossing_width = self.crossing_width() / 2.0;
        if max_width >= half_crossing_width {
            return Err(TuningError::FogNotUnderHalfTheCrossingWidth {
                max_width,
                half_crossing_width,
            });
        }
        let lens = self.effects.lens;
        if 2.0 * lens.radius + lens.clear_run > half_crossing_width {
            return Err(TuningError::LensHasNoRoomOnItsHalf {
                radius: lens.radius,
                clear_run: lens.clear_run,
                half_crossing_width,
            });
        }
        if lens.radius > playable_height / 2.0 {
            return Err(TuningError::LensTallerThanThePlayableHeight {
                radius: lens.radius,
                half_playable_height: playable_height / 2.0,
            });
        }
        let blocks = self.effects.blocks;
        let step_travel = f64::from(cap) / FIXED_HZ;
        if f64::from(blocks.clear_run) <= step_travel {
            return Err(TuningError::BlocksClearRunWithinOneStep {
                clear_run: blocks.clear_run,
                step_travel,
            });
        }
        let room = self.block_room();
        let length = 2.0 * blocks.half_size.y;
        if room.farthest < room.nearest {
            return Err(TuningError::BlocksHaveNoRoomOnTheirHalf {
                clear_run: blocks.clear_run,
                length,
                half_crossing_width,
            });
        }
        if room.highest < 0.0 {
            return Err(TuningError::BlockHasNoRoomBetweenTheWalls {
                length,
                arena_height,
            });
        }
        Ok(())
    }

    /// Where the centre of a block may lie on the left half (M2 spec
    /// 2.6.6). A block reaches at most its half diagonal from its centre,
    /// whatever its tilt, and the room is reckoned with that reach.
    #[must_use]
    pub fn block_room(&self) -> BlockRoom {
        let blocks = self.effects.blocks;
        let radius = self.ball.radius.value();
        let reach = blocks.half_size.length();
        BlockRoom {
            nearest: reach + 2.0 * radius,
            farthest: self.crossing_width() / 2.0 - blocks.clear_run - reach - radius,
            highest: self.arena.center_limit_y(self.ball.radius) - radius - reach,
        }
    }

    /// How far the ball's centre travels in one full crossing: the distance
    /// between the two paddles' contact lines, each one ball radius in front
    /// of a paddle's face. Positive, since validation keeps the paddles
    /// short of the centre line.
    #[must_use]
    pub fn crossing_width(&self) -> f32 {
        let behind_a_contact_line =
            self.paddle.inset + self.paddle.thickness + self.ball.radius.value();
        2.0 * (self.arena.half_extents().x - behind_a_contact_line)
    }

    /// The data of `mode`: its speed ramp, its effects, and its schedule.
    #[must_use]
    pub const fn mode(&self, mode: Mode) -> &ModeTuning {
        match mode {
            Mode::Classic => &self.modes.classic,
            Mode::Chaos { .. } => &self.modes.chaos,
        }
    }

    /// Each effect's own numbers.
    #[must_use]
    pub const fn effects(&self) -> EffectsTuning {
        self.effects
    }

    /// Keys per seat.
    #[must_use]
    pub const fn bindings(&self) -> BindingsTuning {
        self.bindings
    }

    /// Profile colors in menu order, non-empty, opaque.
    #[must_use]
    pub fn palette(&self) -> &[Srgba] {
        &self.palette
    }

    /// Countdown and points-to-win choices.
    #[must_use]
    pub const fn match_flow(&self) -> &MatchFlowTuning {
        &self.match_flow
    }

    /// AI handicaps per difficulty.
    #[must_use]
    pub const fn ai(&self) -> AiTuning {
        self.ai
    }

    /// The playfield.
    #[must_use]
    pub const fn arena(&self) -> Arena {
        self.arena
    }

    /// Paddle size bounds and speed.
    #[must_use]
    pub const fn paddle(&self) -> PaddleTuning {
        self.paddle
    }

    /// Ball size, speeds, and angles.
    #[must_use]
    pub const fn ball(&self) -> BallTuning {
        self.ball
    }
}

impl PaddleTuning {
    fn validate(file: &file::PaddleFile) -> Result<Self, TuningError> {
        let height_bounds = LengthBounds::new(file.height_min, file.height_max)?;
        let height_default = checked(
            "paddle.height_default",
            Range::PositiveFinite,
            file.height_default,
        )?;
        if !height_bounds.contains(height_default) {
            return Err(TuningError::PaddleHeightDefaultOutsideBounds {
                default: height_default,
                min: height_bounds.min(),
                max: height_bounds.max(),
            });
        }
        Ok(Self {
            height_bounds,
            height_default,
            thickness: checked("paddle.thickness", Range::PositiveFinite, file.thickness)?,
            speed: checked("paddle.speed", Range::PositiveFinite, file.speed)?,
            inset: checked("paddle.inset", Range::NonNegativeFinite, file.inset)?,
        })
    }

    /// Smallest and largest paddle height; the paddle's size setter clamps to these.
    #[must_use]
    pub const fn height_bounds(self) -> LengthBounds {
        self.height_bounds
    }

    /// Paddle height at match start, within the bounds.
    #[must_use]
    pub const fn height_default(self) -> f32 {
        self.height_default
    }

    /// Paddle thickness along x.
    #[must_use]
    pub const fn thickness(self) -> f32 {
        self.thickness
    }

    /// Paddle travel per second.
    #[must_use]
    pub const fn speed(self) -> f32 {
        self.speed
    }

    /// Distance from the goal line to the paddle's back face; zero puts the
    /// back face on the line.
    #[must_use]
    pub const fn inset(self) -> f32 {
        self.inset
    }
}

impl BallTuning {
    fn validate(file: &file::BallFile) -> Result<Self, TuningError> {
        let speed_cap = Range::PositiveFinite
            .check(file.speed_cap)
            .map_err(InvalidSpeedRule::Cap)?;
        let serve_speed = checked("ball.serve_speed", Range::PositiveFinite, file.serve_speed)?;
        if serve_speed > file.speed_cap {
            return Err(TuningError::ServeSpeedAboveCap {
                serve_speed,
                cap: file.speed_cap,
            });
        }
        Ok(Self {
            radius: Radius::new(file.radius).map_err(|refused| TuningError::Field {
                path: "ball.radius",
                refused,
            })?,
            serve_speed,
            speed_cap,
            flight_limit_seconds: checked(
                "ball.flight_limit_seconds",
                Range::PositiveFinite,
                file.flight_limit_seconds,
            )?,
            contacts_per_step: file.contacts_per_step,
            obstacle_bounces_per_flight: file.obstacle_bounces_per_flight,
            max_deflection: deflection("ball.max_deflection_degrees", file.max_deflection_degrees)?,
            serve_cone: deflection("ball.serve_cone_degrees", file.serve_cone_degrees)?,
        })
    }

    /// Ball radius.
    #[must_use]
    pub const fn radius(self) -> Radius {
        self.radius
    }

    /// Speed at serve, never above the cap.
    #[must_use]
    pub const fn serve_speed(self) -> f32 {
        self.serve_speed
    }

    /// The highest speed a ball can leave a paddle at, in every mode: the
    /// base's bound on the speed channel (ADR 0011).
    #[must_use]
    pub const fn speed_cap(self) -> f32 {
        self.speed_cap
    }

    /// The longest a flight may last, in seconds, before it is a fault
    /// (M2 spec 2.5.6).
    #[must_use]
    pub const fn flight_limit_seconds(self) -> f32 {
        self.flight_limit_seconds
    }

    /// Largest angle from straight-across a paddle hit can produce.
    #[must_use]
    pub const fn max_deflection(self) -> MaxDeflection {
        self.max_deflection
    }

    /// How the ball bounces off obstacles: the base's bounds on the
    /// obstacle channel (M2 spec 2.2.6, 2.2.6.1). The contacts in one fixed
    /// step and the bounces in one flight are tuned; the horizontal floor
    /// is the paddle's, so an obstacle sends the ball no steeper than
    /// [`max_deflection`](Self::max_deflection); and the speed cap is the
    /// launch's, so an obstacle sends it no faster than
    /// [`speed_cap`](Self::speed_cap) (M2 spec 2.2.2).
    #[must_use]
    pub const fn bounce_rule(self) -> BounceRule {
        BounceRule::new(self.contacts_per_step, self.max_deflection)
            .with_bounces_per_flight(self.obstacle_bounces_per_flight)
            .no_faster_than(self.speed_cap)
    }

    /// Largest angle from straight-across a serve can take; the serve samples within it.
    #[must_use]
    pub const fn serve_cone(self) -> MaxDeflection {
        self.serve_cone
    }
}

impl ModesTuning {
    fn validate(file: file::ModesFile, speed_cap: f32) -> Result<Self, TuningError> {
        let classic = ModeTuning::validate("classic", file.classic, speed_cap)?;
        let chaos = ModeTuning::validate("chaos", file.chaos, speed_cap)?;
        let (classic_increase, chaos_increase) = (
            classic.speed_rule.per_hit_increase(),
            chaos.speed_rule.per_hit_increase(),
        );
        if chaos_increase >= classic_increase {
            return Err(TuningError::ChaosRampNotBelowClassic {
                classic: classic_increase,
                chaos: chaos_increase,
            });
        }
        Ok(Self { classic, chaos })
    }
}

impl ModeTuning {
    fn validate(
        mode: &'static str,
        file: file::ModeFile,
        speed_cap: f32,
    ) -> Result<Self, TuningError> {
        let speed_rule = SpeedRule::new(file.per_hit_increase, speed_cap)?;
        let mut named = HashSet::new();
        for effect in file.always.iter().chain(&file.bag) {
            if !named.insert(*effect) {
                return Err(TuningError::ModeNamesEffectTwice {
                    mode,
                    effect: *effect,
                });
            }
        }
        let rising = file
            .thresholds_percent
            .windows(2)
            .all(|pair| pair[0] < pair[1]);
        let within = file
            .thresholds_percent
            .iter()
            .all(|percent| *percent <= 100);
        if !(rising && within) {
            return Err(TuningError::ModeThresholdsDoNotRise {
                mode,
                thresholds: file.thresholds_percent,
            });
        }
        Ok(Self {
            speed_rule,
            always: file.always,
            bag: file.bag,
            thresholds_percent: file.thresholds_percent,
            rotate_every_points: file.rotate_every_points,
        })
    }

    /// The mode's per-hit increase, with the cap every mode shares.
    #[must_use]
    pub const fn speed_rule(&self) -> SpeedRule {
        self.speed_rule
    }

    /// Effects in play from the first serve to the last.
    #[must_use]
    pub fn always(&self) -> &[EffectId] {
        &self.always
    }

    /// Effects that join and rotate as the match goes on.
    #[must_use]
    pub fn bag(&self) -> &[EffectId] {
        &self.bag
    }

    /// Leader's progress toward the points target, in whole percent, at which
    /// one more bag effect joins. Strictly rising.
    #[must_use]
    pub fn thresholds_percent(&self) -> &[u8] {
        &self.thresholds_percent
    }

    /// Points scored between rotations of the bag.
    #[must_use]
    pub const fn rotate_every_points(&self) -> NonZeroU8 {
        self.rotate_every_points
    }
}

impl EffectsTuning {
    fn validate(file: &file::EffectsFile, paddle: PaddleTuning) -> Result<Self, TuningError> {
        let smallest = paddle.height_bounds.min() / paddle.height_default;
        Ok(Self {
            shrink: ShrinkTuning {
                height_factor: Ramp::new(
                    file.shrink.starts_at_return,
                    file.shrink.full_at_return,
                    1.0,
                    smallest,
                )?,
            },
            wind: WindTuning::validate(&file.wind)?,
            spin: SpinTuning::validate(&file.spin)?,
            fog: FogTuning::validate(&file.fog)?,
            lens: LensTuning::validate(&file.lens)?,
            blocks: BlocksTuning::validate(&file.blocks, paddle)?,
        })
    }

    /// Shrink's numbers.
    #[must_use]
    pub const fn shrink(self) -> ShrinkTuning {
        self.shrink
    }

    /// Wind's numbers.
    #[must_use]
    pub const fn wind(self) -> WindTuning {
        self.wind
    }

    /// Spin's numbers.
    #[must_use]
    pub const fn spin(self) -> SpinTuning {
        self.spin
    }

    /// Fog's numbers.
    #[must_use]
    pub const fn fog(self) -> FogTuning {
        self.fog
    }

    /// Lens's numbers.
    #[must_use]
    pub const fn lens(self) -> LensTuning {
        self.lens
    }

    /// Blocks' numbers.
    #[must_use]
    pub const fn blocks(self) -> BlocksTuning {
        self.blocks
    }
}

impl ShrinkTuning {
    /// The factor on paddle height over the rally's returns.
    #[must_use]
    pub const fn height_factor(self) -> Ramp {
        self.height_factor
    }
}

impl WindTuning {
    fn validate(file: &file::WindFile) -> Result<Self, TuningError> {
        let max_drift = checked(
            "effects.wind.max_drift",
            Range::NonNegativeFinite,
            file.max_drift,
        )?;
        Ok(Self {
            drift: Ramp::new(file.starts_at_return, file.full_at_return, 0.0, max_drift)
                .map_err(TuningError::WindRamp)?,
            believed: PerStar::validate(
                &file.believed_share,
                [
                    "effects.wind.believed_share.one_star",
                    "effects.wind.believed_share.two_stars",
                    "effects.wind.believed_share.three_stars",
                ],
            )?,
        })
    }

    /// The drift over one full crossing, in arena units, over the rally's
    /// returns.
    #[must_use]
    pub const fn drift(self) -> Ramp {
        self.drift
    }

    /// The drift at full dial.
    #[must_use]
    pub fn max_drift(self) -> f32 {
        self.drift.at(u16::MAX)
    }

    /// The share of the true drift an AI of `difficulty` expects: 1 is the
    /// truth, less under-leads the ball, more over-leads it.
    #[must_use]
    pub const fn believed_share(self, difficulty: Difficulty) -> f32 {
        self.believed.by(difficulty)
    }
}

impl PerStar {
    /// The numbers in `file`, each blamed on its entry of `paths` if
    /// refused: one star's first, then two stars', then three stars'.
    fn validate(file: &file::PerStarFile, paths: [&'static str; 3]) -> Result<Self, TuningError> {
        let [one_star, two_stars, three_stars] = paths;
        let number = |path, number| checked(path, Range::NonNegativeFinite, number);
        Ok(Self {
            one_star: number(one_star, file.one_star)?,
            two_stars: number(two_stars, file.two_stars)?,
            three_stars: number(three_stars, file.three_stars)?,
        })
    }
}

impl PerStar<u16> {
    /// The whole numbers in `file`, as they are: none can be negative.
    const fn whole(file: &file::PerStarFile<u16>) -> Self {
        Self {
            one_star: file.one_star,
            two_stars: file.two_stars,
            three_stars: file.three_stars,
        }
    }
}

impl<T: Copy> PerStar<T> {
    /// The number for an AI of `difficulty`.
    const fn by(self, difficulty: Difficulty) -> T {
        match difficulty {
            Difficulty::OneStar => self.one_star,
            Difficulty::TwoStars => self.two_stars,
            Difficulty::ThreeStars => self.three_stars,
        }
    }
}

impl SpinTuning {
    fn validate(file: &file::SpinFile) -> Result<Self, TuningError> {
        let max_drift = checked(
            "effects.spin.max_drift",
            Range::NonNegativeFinite,
            file.max_drift,
        )?;
        Ok(Self {
            drift: Ramp::new(file.starts_at_return, file.full_at_return, 0.0, max_drift)
                .map_err(TuningError::SpinRamp)?,
            believed: PerStar::validate(
                &file.believed_share,
                [
                    "effects.spin.believed_share.one_star",
                    "effects.spin.believed_share.two_stars",
                    "effects.spin.believed_share.three_stars",
                ],
            )?,
        })
    }

    /// The drift over one full crossing from a moving-paddle hit, in arena
    /// units, over the rally's returns.
    #[must_use]
    pub const fn drift(self) -> Ramp {
        self.drift
    }

    /// The drift at full dial.
    #[must_use]
    pub fn max_drift(self) -> f32 {
        self.drift.at(u16::MAX)
    }

    /// The share of the true drift an AI of `difficulty` expects: 1 is the
    /// truth, less under-leads the ball, more over-leads it.
    #[must_use]
    pub const fn believed_share(self, difficulty: Difficulty) -> f32 {
        self.believed.by(difficulty)
    }
}

impl FogTuning {
    fn validate(file: &file::FogFile) -> Result<Self, TuningError> {
        let max_width = checked(
            "effects.fog.max_width",
            Range::NonNegativeFinite,
            file.max_width,
        )?;
        Ok(Self {
            width: Ramp::new(file.starts_at_return, file.full_at_return, 0.0, max_width)
                .map_err(TuningError::FogRamp)?,
            added_error: PerStar::validate(
                &file.added_error,
                [
                    "effects.fog.added_error.one_star",
                    "effects.fog.added_error.two_stars",
                    "effects.fog.added_error.three_stars",
                ],
            )?,
        })
    }

    /// The band's width, in arena units, over the rally's returns.
    #[must_use]
    pub const fn width(self) -> Ramp {
        self.width
    }

    /// The band's width at full dial.
    #[must_use]
    pub fn max_width(self) -> f32 {
        self.width.at(u16::MAX)
    }

    /// The error added to the aim of an AI of `difficulty` at full dial, in
    /// arena units.
    #[must_use]
    pub const fn added_error(self, difficulty: Difficulty) -> f32 {
        self.added_error.by(difficulty)
    }
}

impl LensTuning {
    /// The largest bend a lens may be tuned to. The strength of a lens's
    /// pull is sized by measure so that no crossing bends by more than the
    /// tuned angle, and the measure was taken up to here (M2 spec 2.6.5).
    pub const LARGEST_BEND_DEGREES: f32 = 30.0;

    fn validate(file: &file::LensFile) -> Result<Self, TuningError> {
        let max_bend_degrees = checked(
            "effects.lens.max_bend_degrees",
            Range::NonNegativeFinite,
            file.max_bend_degrees,
        )?;
        if max_bend_degrees > Self::LARGEST_BEND_DEGREES {
            return Err(TuningError::LensBendAboveTheLargest {
                max_bend_degrees,
                largest_degrees: Self::LARGEST_BEND_DEGREES,
            });
        }
        Ok(Self {
            count: ItemCount::new(file.starts_at_return, file.full_at_return, file.max_count)
                .map_err(TuningError::LensCount)?,
            radius: checked("effects.lens.radius", Range::PositiveFinite, file.radius)?,
            max_bend: max_bend_degrees.to_radians(),
            clear_run: checked(
                "effects.lens.clear_run",
                Range::NonNegativeFinite,
                file.clear_run,
            )?,
            unseen_flights: PerStar::whole(&file.unseen_flights),
            believed: PerStar::validate(
                &file.believed_share,
                [
                    "effects.lens.believed_share.one_star",
                    "effects.lens.believed_share.two_stars",
                    "effects.lens.believed_share.three_stars",
                ],
            )?,
        })
    }

    /// The lenses drawn, over the rally's returns. Each has a mirrored copy
    /// besides.
    #[must_use]
    pub const fn count(self) -> ItemCount {
        self.count
    }

    /// The radius of one lens, in arena units.
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.radius
    }

    /// The largest bend one lens gives any crossing, in radians.
    #[must_use]
    pub const fn max_bend(self) -> f32 {
        self.max_bend
    }

    /// The stretch before each paddle's contact line that no lens reaches
    /// into, in arena units.
    #[must_use]
    pub const fn clear_run(self) -> f32 {
        self.clear_run
    }

    /// The flights for which an AI of `difficulty` does not see a new lens.
    #[must_use]
    pub const fn unseen_flights(self, difficulty: Difficulty) -> u16 {
        self.unseen_flights.by(difficulty)
    }

    /// The share of a seen lens's pull an AI of `difficulty` expects: 1 is
    /// the truth, less under-bends the ball's path, more over-bends it.
    #[must_use]
    pub const fn believed_share(self, difficulty: Difficulty) -> f32 {
        self.believed.by(difficulty)
    }
}

impl BlocksTuning {
    fn validate(file: &file::BlocksFile, paddle: PaddleTuning) -> Result<Self, TuningError> {
        // Above nothing: a ball can shuttle between an upright block and
        // its copy and never leave (M2 spec, facts).
        let min_tilt_degrees = checked(
            "effects.blocks.min_tilt_degrees",
            Range::PositiveFinite,
            file.min_tilt_degrees,
        )?;
        let min_tilt = checked(
            "effects.blocks.min_tilt_degrees",
            Range::BelowRightAngle,
            min_tilt_degrees.to_radians(),
        )?;
        let max_tilt = checked(
            "effects.blocks.max_tilt_degrees",
            Range::BelowRightAngle,
            file.max_tilt_degrees.to_radians(),
        )?;
        if max_tilt < min_tilt {
            return Err(TuningError::BlocksTiltRangeReversed {
                min_tilt_degrees,
                max_tilt_degrees: file.max_tilt_degrees,
            });
        }
        Ok(Self {
            count: ItemCount::new(file.starts_at_return, file.full_at_return, file.max_count)
                .map_err(TuningError::BlocksCount)?,
            half_size: Vec2::new(paddle.thickness, paddle.height_default) / 2.0,
            min_tilt,
            max_tilt,
            clear_run: checked(
                "effects.blocks.clear_run",
                Range::NonNegativeFinite,
                file.clear_run,
            )?,
            unseen_flights: PerStar::whole(&file.unseen_flights),
            believed: PerStar::validate(
                &file.believed_share,
                [
                    "effects.blocks.believed_share.one_star",
                    "effects.blocks.believed_share.two_stars",
                    "effects.blocks.believed_share.three_stars",
                ],
            )?,
        })
    }

    /// The blocks drawn, over the rally's returns. Each has a mirrored copy
    /// besides.
    #[must_use]
    pub const fn count(self) -> ItemCount {
        self.count
    }

    /// How far a block reaches from its centre: half the paddle's thickness
    /// across it and half the default paddle's length along it.
    #[must_use]
    pub const fn half_size(self) -> Vec2 {
        self.half_size
    }

    /// The least a block is turned from upright, either way, in radians.
    #[must_use]
    pub const fn min_tilt(self) -> f32 {
        self.min_tilt
    }

    /// The most a block is turned from upright, either way, in radians.
    #[must_use]
    pub const fn max_tilt(self) -> f32 {
        self.max_tilt
    }

    /// The stretch before each paddle's contact line that no block, grown
    /// by the ball's radius, reaches into, in arena units: a ball that
    /// bounces off a block is at least this far from the line.
    #[must_use]
    pub const fn clear_run(self) -> f32 {
        self.clear_run
    }

    /// The flights for which an AI of `difficulty` does not see a new block.
    #[must_use]
    pub const fn unseen_flights(self, difficulty: Difficulty) -> u16 {
        self.unseen_flights.by(difficulty)
    }

    /// The share of a seen block's tilt an AI of `difficulty` believes: 1
    /// is the truth, less takes the block for more upright than it is.
    #[must_use]
    pub const fn believed_share(self, difficulty: Difficulty) -> f32 {
        self.believed.by(difficulty)
    }
}

impl MatchFlowTuning {
    fn validate(file: file::MatchFlowFile) -> Result<Self, TuningError> {
        let countdown = seconds("match_flow.countdown_seconds", file.countdown_seconds)?;
        let point_pause = seconds("match_flow.point_pause_seconds", file.point_pause_seconds)?;
        if file.points_to_win_options.is_empty() {
            return Err(TuningError::PointsToWinOptionsEmpty);
        }
        if !file
            .points_to_win_options
            .contains(&file.points_to_win_default)
        {
            return Err(TuningError::PointsToWinDefaultNotOffered {
                default: file.points_to_win_default,
            });
        }
        Ok(Self {
            countdown,
            point_pause,
            points_to_win_options: file.points_to_win_options,
            points_to_win_default: file.points_to_win_default,
        })
    }

    /// Time from serve cue to the ball moving.
    #[must_use]
    pub const fn countdown(&self) -> Duration {
        self.countdown
    }

    /// How long a scored point is shown before the next countdown starts.
    #[must_use]
    pub const fn point_pause(&self) -> Duration {
        self.point_pause
    }

    /// The points-to-win choices offered in the menu, non-empty.
    #[must_use]
    pub fn points_to_win_options(&self) -> &[NonZeroU8] {
        &self.points_to_win_options
    }

    /// The preselected choice, always one of the options.
    #[must_use]
    pub const fn points_to_win_default(&self) -> NonZeroU8 {
        self.points_to_win_default
    }
}

impl AiTuning {
    fn validate(file: &file::AiFile) -> Result<Self, TuningError> {
        Ok(Self {
            one_star: DifficultyTuning::validate(
                &file.one_star,
                "ai.one_star.reaction_delay_seconds",
                "ai.one_star.prediction_error",
            )?,
            two_stars: DifficultyTuning::validate(
                &file.two_stars,
                "ai.two_stars.reaction_delay_seconds",
                "ai.two_stars.prediction_error",
            )?,
            three_stars: DifficultyTuning::validate(
                &file.three_stars,
                "ai.three_stars.reaction_delay_seconds",
                "ai.three_stars.prediction_error",
            )?,
        })
    }

    /// The handicap for `difficulty`.
    #[must_use]
    pub const fn level(self, difficulty: Difficulty) -> DifficultyTuning {
        match difficulty {
            Difficulty::OneStar => self.one_star,
            Difficulty::TwoStars => self.two_stars,
            Difficulty::ThreeStars => self.three_stars,
        }
    }
}

impl DifficultyTuning {
    fn validate(
        file: &file::DifficultyFile,
        reaction_delay_path: &'static str,
        prediction_error_path: &'static str,
    ) -> Result<Self, TuningError> {
        Ok(Self {
            reaction_delay: seconds(reaction_delay_path, file.reaction_delay_seconds)?,
            prediction_error: checked(
                prediction_error_path,
                Range::NonNegativeFinite,
                file.prediction_error,
            )?,
        })
    }

    /// How long the AI waits after the ball turns toward it before moving.
    #[must_use]
    pub const fn reaction_delay(self) -> Duration {
        self.reaction_delay
    }

    /// Spread, in arena units, of the error added to the AI's predicted target.
    #[must_use]
    pub const fn prediction_error(self) -> f32 {
        self.prediction_error
    }
}

impl BindingsTuning {
    fn validate(file: &file::BindingsFile) -> Result<Self, TuningError> {
        let left = SeatKeys::from_file(&file.left);
        let right = SeatKeys::from_file(&file.right);
        let mut seen = HashSet::new();
        for key in left.all().into_iter().chain(right.all()) {
            if key == KeyCode::Escape {
                return Err(TuningError::ReservedKey { key });
            }
            if !seen.insert(key) {
                return Err(TuningError::DuplicateKey { key });
            }
        }
        Ok(Self { left, right })
    }

    /// The keys `seat` holds.
    #[must_use]
    pub const fn seat(self, seat: Seat) -> SeatKeys {
        match seat {
            Seat::Left => self.left,
            Seat::Right => self.right,
        }
    }
}

impl SeatKeys {
    const fn from_file(file: &file::SeatKeysFile) -> Self {
        Self {
            up: file.up,
            down: file.down,
            action: file.action,
            back: file.back,
        }
    }

    const fn all(self) -> [KeyCode; 4] {
        [self.up, self.down, self.action, self.back]
    }

    /// Moves the paddle up; navigates up in menus.
    #[must_use]
    pub const fn up(self) -> KeyCode {
        self.up
    }

    /// Moves the paddle down; navigates down in menus.
    #[must_use]
    pub const fn down(self) -> KeyCode {
        self.down
    }

    /// Confirms in menus; the in-play action key, unused until C2.
    #[must_use]
    pub const fn action(self) -> KeyCode {
        self.action
    }

    /// Backs out in menus; reserved in play.
    #[must_use]
    pub const fn back(self) -> KeyCode {
        self.back
    }
}

fn validate_palette(file: &[file::ColorFile]) -> Result<Vec<Srgba>, TuningError> {
    if file.is_empty() {
        return Err(TuningError::PaletteEmpty);
    }
    file.iter()
        .enumerate()
        .map(|(index, color)| {
            let channel = |channel: &'static str, value: f32| {
                Range::NonNegativeFinite.check(value).map_err(|refused| {
                    TuningError::PaletteChannel {
                        index,
                        channel,
                        refused,
                    }
                })
            };
            Ok(Srgba::rgb(
                channel("red", color.red)?,
                channel("green", color.green)?,
                channel("blue", color.blue)?,
            ))
        })
        .collect()
}

/// A non-negative finite number of seconds into a [`Duration`], blamed on `path` if refused.
fn seconds(path: &'static str, seconds: f32) -> Result<Duration, TuningError> {
    checked(path, Range::NonNegativeFinite, seconds).map(Duration::from_secs_f32)
}

/// One scalar through its kernel range, blamed on `path` if refused.
fn checked(path: &'static str, range: Range, value: f32) -> Result<f32, TuningError> {
    range
        .check(value)
        .map_err(|refused| TuningError::Field { path, refused })
}

/// A file angle in degrees into a kernel [`MaxDeflection`], blamed on `path` if refused.
fn deflection(path: &'static str, degrees: f32) -> Result<MaxDeflection, TuningError> {
    MaxDeflection::new(degrees.to_radians()).map_err(|refused| TuningError::Field { path, refused })
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_3;
    use std::time::Duration;

    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::AssetPlugin;
    use bevy::input::keyboard::KeyCode;

    use bevy::asset::AssetServer;
    use bevy::asset::io::embedded::{EmbeddedAssetRegistry, watched_path};

    use super::{EMBEDDED_RON, Tuning, TuningError, TuningPlugin, embedded_tuning_path};
    use std::num::{NonZeroU8, NonZeroU16};

    use crate::domain::{Difficulty, EffectId, Mode, Notice, Seat};
    use crate::kernel::geometry::InvalidLengthBounds;
    use crate::kernel::obstacle::BounceRule;
    use crate::kernel::ramp::{FullBeforeStart, InvalidItemCount};
    use crate::kernel::test_support::assert_near;
    use crate::kernel::{OutOfRange, Range};
    use crate::testing::{boot, frame, headless_app, record, recorded, run_until, take_recorded};

    const VALID: &str = r"(
        arena: (width: 160.0, height: 90.0),
        paddle: (height_min: 8.0, height_max: 30.0, height_default: 14.0, thickness: 2.0, speed: 90.0, inset: 4.0),
        ball: (
            radius: 1.0,
            serve_speed: 80.0,
            speed_cap: 250.0,
            flight_limit_seconds: 20.0,
            contacts_per_step: 4,
            obstacle_bounces_per_flight: 6,
            max_deflection_degrees: 60.0,
            serve_cone_degrees: 30.0,
        ),
        modes: (
            classic: (per_hit_increase: 7.0, always: [speed], bag: [], thresholds_percent: [], rotate_every_points: 1),
            chaos: (per_hit_increase: 3.0, always: [speed], bag: [shrink], thresholds_percent: [30, 70], rotate_every_points: 1),
        ),
        effects: (
            shrink: (starts_at_return: 3, full_at_return: 15),
            wind: (starts_at_return: 0, full_at_return: 12, max_drift: 24.0, believed_share: (one_star: 0.4, two_stars: 0.7, three_stars: 0.9)),
            spin: (starts_at_return: 0, full_at_return: 12, max_drift: 16.0, believed_share: (one_star: 0.3, two_stars: 0.6, three_stars: 0.85)),
            fog: (starts_at_return: 0, full_at_return: 8, max_width: 64.0, added_error: (one_star: 12.0, two_stars: 6.0, three_stars: 2.5)),
            lens: (
                starts_at_return: 1,
                full_at_return: 7,
                max_count: 3,
                radius: 8.0,
                max_bend_degrees: 15.0,
                clear_run: 24.0,
                unseen_flights: (one_star: 3, two_stars: 2, three_stars: 1),
                believed_share: (one_star: 0.4, two_stars: 0.7, three_stars: 0.9),
            ),
            blocks: (
                starts_at_return: 2,
                full_at_return: 8,
                max_count: 2,
                min_tilt_degrees: 10.0,
                max_tilt_degrees: 35.0,
                clear_run: 24.0,
                unseen_flights: (one_star: 3, two_stars: 2, three_stars: 1),
                believed_share: (one_star: 0.4, two_stars: 0.7, three_stars: 0.9),
            ),
        ),
        match_flow: (countdown_seconds: 3.0, point_pause_seconds: 0.8, points_to_win_options: [5, 10, 15, 21], points_to_win_default: 10),
        ai: (
            one_star: (reaction_delay_seconds: 0.35, prediction_error: 12.0),
            two_stars: (reaction_delay_seconds: 0.2, prediction_error: 6.0),
            three_stars: (reaction_delay_seconds: 0.0, prediction_error: 0.0),
        ),
        bindings: (
            left: (up: KeyW, down: KeyS, action: KeyD, back: KeyA),
            right: (up: ArrowUp, down: ArrowDown, action: ArrowRight, back: ArrowLeft),
        ),
        palette: [
            (red: 1.0, green: 0.3, blue: 0.2),
            (red: 0.2, green: 0.6, blue: 1.0),
        ],
    )";

    fn valid() -> Tuning {
        Tuning::from_ron(VALID).expect("VALID fixture passes validation")
    }

    /// `VALID` with one value swapped: the first `key: old` becomes `key: new`.
    /// A value ends at the first `,` or `)` outside any brackets it opens.
    fn with(key: &str, new_value: &str) -> String {
        let needle = format!("{key}: ");
        let Some(key_at) = VALID.find(&needle) else {
            panic!("no key {key} in VALID");
        };
        let start = key_at + needle.len();
        let mut depth = 0_u32;
        let end = VALID[start..]
            .char_indices()
            .find_map(|(i, c)| match c {
                '[' | '(' => {
                    depth += 1;
                    None
                }
                ']' | ')' if depth > 0 => {
                    depth -= 1;
                    None
                }
                ',' | ')' if depth == 0 => Some(start + i),
                _ => None,
            })
            .expect("every value in VALID ends with a comma or a closing paren");
        format!("{}{}{}", &VALID[..start], new_value, &VALID[end..])
    }

    /// Durations built from `f32` seconds land within a microsecond of the intent.
    fn assert_duration_near(actual: Duration, expected: Duration) {
        assert!(
            actual.abs_diff(expected) <= Duration::from_micros(1),
            "expected {expected:?}, got {actual:?}"
        );
    }

    /// Headless app with the asset system and the tuning plugin. Updates until
    /// the `Tuning` resource appears or the step budget runs out.
    fn load_through_assets() -> Option<Tuning> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TuningPlugin));
        for _ in 0..1_000 {
            app.update();
            if let Some(tuning) = app.world().get_resource::<Tuning>() {
                return Some(tuning.clone());
            }
        }
        None
    }

    /// What Bevy's embedded watcher does when the source file changes: new
    /// bytes into the embedded source, then a reload of the asset path.
    fn override_embedded_tuning(app: &mut App, ron: &str) {
        app.world()
            .resource::<EmbeddedAssetRegistry>()
            .insert_asset(
                watched_path(file!(), "tuning.ron"),
                &bevy::asset::embedded_path!("tuning.ron"),
                ron.as_bytes().to_vec(),
            );
        app.world()
            .resource::<AssetServer>()
            .reload(embedded_tuning_path());
    }

    fn ball_radius(app: &App) -> f32 {
        app.world().resource::<Tuning>().ball().radius().value()
    }

    #[test]
    fn a_valid_override_replaces_the_resource_on_reload() {
        let mut app = headless_app(1);
        boot(&mut app);
        assert_near(ball_radius(&app), 1.0, 1e-6);
        override_embedded_tuning(&mut app, &with("radius", "2.0"));
        run_until(
            &mut app,
            |app| (ball_radius(app) - 2.0).abs() < 1e-6,
            1_000,
            "the override never reached the resource",
        );
    }

    #[test]
    fn a_refused_override_keeps_the_last_good_values_and_notifies() {
        let mut app = headless_app(1);
        record::<Notice>(&mut app);
        boot(&mut app);
        let last_good = app.world().resource::<Tuning>().clone();
        override_embedded_tuning(&mut app, &with("radius", "-1.0"));
        run_until(
            &mut app,
            |app| !recorded::<Notice>(app).is_empty(),
            1_000,
            "no notice about the refused override",
        );
        for _ in 0..10 {
            frame(&mut app);
        }
        let notices = take_recorded::<Notice>(&mut app);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].text.contains("ball.radius"),
            "{}",
            notices[0].text
        );
        assert_eq!(*app.world().resource::<Tuning>(), last_good);
    }

    #[test]
    fn a_refusal_is_announced_again_once_a_reload_has_succeeded_in_between() {
        let mut app = headless_app(1);
        record::<Notice>(&mut app);
        boot(&mut app);
        let refused = with("radius", "-1.0");
        override_embedded_tuning(&mut app, &refused);
        run_until(
            &mut app,
            |app| !recorded::<Notice>(app).is_empty(),
            1_000,
            "no notice about the first refusal",
        );
        override_embedded_tuning(&mut app, &with("radius", "2.0"));
        run_until(
            &mut app,
            |app| (ball_radius(app) - 2.0).abs() < 1e-6,
            1_000,
            "the valid override never reached the resource",
        );
        take_recorded::<Notice>(&mut app);
        override_embedded_tuning(&mut app, &refused);
        run_until(
            &mut app,
            |app| !recorded::<Notice>(app).is_empty(),
            1_000,
            "no notice about the second refusal",
        );
        assert_near(ball_radius(&app), 2.0, 1e-6);
    }

    #[test]
    fn the_plugin_loads_the_embedded_file_through_the_asset_system_into_the_resource() {
        let Some(loaded) = load_through_assets() else {
            panic!("tuning resource never appeared");
        };
        let parsed =
            Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning file passes validation");
        assert_eq!(loaded, parsed);
    }

    #[test]
    fn the_embedded_tuning_file_parses_and_validates() {
        let tuning =
            Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning file passes validation");
        assert_near(tuning.arena().half_extents().x, 80.0, 1e-6);
        assert_eq!(tuning.palette().len(), 8);
        assert_eq!(tuning.match_flow().points_to_win_default().get(), 10);
    }

    #[test]
    fn valid_file_yields_kernel_types_with_degrees_turned_into_radians() {
        let tuning = valid();
        assert_near(tuning.arena().half_extents().y, 45.0, 1e-6);
        assert_near(tuning.ball().radius().value(), 1.0, 1e-6);
        let classic = tuning.mode(Mode::Classic).speed_rule();
        assert_near(classic.launch(80.0, classic.per_hit_increase()), 87.0, 1e-6);
        assert_near(tuning.ball().serve_speed(), 80.0, 1e-6);
        assert_near(tuning.ball().max_deflection().radians(), FRAC_PI_3, 1e-6);
        assert_near(tuning.ball().serve_cone().radians(), FRAC_PI_3 / 2.0, 1e-6);
        assert_near(tuning.paddle().height_bounds().clamp(100.0), 30.0, 1e-6);
        assert_near(tuning.paddle().height_default(), 14.0, 1e-6);
        assert_near(tuning.paddle().thickness(), 2.0, 1e-6);
        assert_near(tuning.paddle().speed(), 90.0, 1e-6);
        assert_near(tuning.paddle().inset(), 4.0, 1e-6);
    }

    #[test]
    fn match_flow_and_ai_sections_become_durations_and_per_difficulty_entries() {
        let tuning = valid();
        assert_duration_near(tuning.match_flow().countdown(), Duration::from_millis(3000));
        assert_duration_near(
            tuning.match_flow().point_pause(),
            Duration::from_millis(800),
        );
        let options: Vec<u8> = tuning
            .match_flow()
            .points_to_win_options()
            .iter()
            .map(|n| n.get())
            .collect();
        assert_eq!(options, [5, 10, 15, 21]);
        assert_eq!(tuning.match_flow().points_to_win_default().get(), 10);
        let one = tuning.ai().level(Difficulty::OneStar);
        assert_duration_near(one.reaction_delay(), Duration::from_millis(350));
        assert_near(one.prediction_error(), 12.0, 1e-6);
        let three = tuning.ai().level(Difficulty::ThreeStars);
        assert_eq!(three.reaction_delay(), Duration::ZERO);
        assert_near(three.prediction_error(), 0.0, 1e-6);
    }

    #[test]
    fn match_flow_rules_reject_empty_options_and_an_unoffered_default() {
        assert!(matches!(
            Tuning::from_ron(&with("points_to_win_options", "[]")),
            Err(TuningError::PointsToWinOptionsEmpty)
        ));
        // Zero is unrepresentable in the schema itself, so it fails at parse time.
        assert!(matches!(
            Tuning::from_ron(&with("points_to_win_options", "[0, 10]")),
            Err(TuningError::Parse(_))
        ));
        assert!(matches!(
            Tuning::from_ron(&with("points_to_win_default", "7")),
            Err(TuningError::PointsToWinDefaultNotOffered { default }) if default.get() == 7
        ));
        assert!(matches!(
            Tuning::from_ron(&with("countdown_seconds", "-1.0")),
            Err(TuningError::Field {
                path: "match_flow.countdown_seconds",
                ..
            })
        ));
        assert!(matches!(
            Tuning::from_ron(&with("point_pause_seconds", "-1.0")),
            Err(TuningError::Field {
                path: "match_flow.point_pause_seconds",
                ..
            })
        ));
        assert!(matches!(
            Tuning::from_ron(&with("reaction_delay_seconds", "-0.1")),
            Err(TuningError::Field {
                path: "ai.one_star.reaction_delay_seconds",
                ..
            })
        ));
    }

    #[test]
    fn bindings_are_key_codes_per_seat_and_palette_is_an_ordered_list_of_colors() {
        let tuning = valid();
        let left = tuning.bindings().seat(Seat::Left);
        assert_eq!(left.up(), KeyCode::KeyW);
        assert_eq!(left.down(), KeyCode::KeyS);
        assert_eq!(left.action(), KeyCode::KeyD);
        assert_eq!(left.back(), KeyCode::KeyA);
        assert_eq!(tuning.bindings().seat(Seat::Right).up(), KeyCode::ArrowUp);

        let palette = tuning.palette();
        assert_eq!(palette.len(), 2);
        assert_near(palette[1].blue, 1.0, 1e-6);
        assert_near(palette[1].alpha, 1.0, 1e-6);
    }

    #[test]
    fn a_key_bound_twice_anywhere_is_rejected_naming_the_key() {
        assert!(matches!(
            Tuning::from_ron(&with("down", "KeyW")),
            Err(TuningError::DuplicateKey { key: KeyCode::KeyW })
        ));
        assert!(matches!(
            Tuning::from_ron(&VALID.replace("action: ArrowRight", "action: KeyA")),
            Err(TuningError::DuplicateKey { key: KeyCode::KeyA })
        ));
    }

    #[test]
    fn palette_must_be_non_empty_with_finite_non_negative_channels() {
        assert!(matches!(
            Tuning::from_ron(&with("palette", "[]")),
            Err(TuningError::PaletteEmpty)
        ));
        assert!(matches!(
            Tuning::from_ron(&with("green", "-0.3")),
            Err(TuningError::PaletteChannel {
                index: 0,
                channel: "green",
                ..
            })
        ));
    }

    #[test]
    fn out_of_range_value_is_rejected_naming_its_path() {
        assert_eq!(
            Tuning::from_ron(&with("radius", "0.0")),
            Err(TuningError::Field {
                path: "ball.radius",
                refused: OutOfRange {
                    value: 0.0,
                    expected: Range::PositiveFinite,
                },
            })
        );
        assert!(matches!(
            Tuning::from_ron(&with("width", "-160.0")),
            Err(TuningError::Arena(_))
        ));
        assert!(matches!(
            Tuning::from_ron(&with("per_hit_increase", "-7.0")),
            Err(TuningError::BallSpeedRule(_))
        ));
        assert!(matches!(
            Tuning::from_ron(&with("max_deflection_degrees", "90.0")),
            Err(TuningError::Field {
                path: "ball.max_deflection_degrees",
                ..
            })
        ));
        assert!(matches!(
            Tuning::from_ron(&with("thickness", "0.0")),
            Err(TuningError::Field {
                path: "paddle.thickness",
                ..
            })
        ));
    }

    #[test]
    fn cross_field_rules_are_rejected_with_their_own_errors() {
        assert!(matches!(
            Tuning::from_ron(&with("serve_speed", "300.0")),
            Err(TuningError::ServeSpeedAboveCap { .. })
        ));
        assert!(matches!(
            Tuning::from_ron(&with("height_default", "40.0")),
            Err(TuningError::PaddleHeightDefaultOutsideBounds { .. })
        ));
        assert!(matches!(
            Tuning::from_ron(&with("height_max", "4.0")),
            Err(TuningError::PaddleHeightBounds(
                InvalidLengthBounds::MaxBelowMin { .. }
            ))
        ));
    }

    /// The refusal for `ron`, which must name every path in `paths`.
    fn refusal_naming(ron: &str, paths: &[&str]) -> TuningError {
        let Err(err) = Tuning::from_ron(ron) else {
            panic!("accepted a file that should be refused: {paths:?}");
        };
        let message = err.to_string();
        for path in paths {
            assert!(message.contains(path), "{message:?} does not name {path}");
        }
        err
    }

    /// Plan 3.17: each cross-section rule guards a precondition downstream.
    /// `VALID` is a 160 by 90 arena, ball radius 1, paddles 2 thick at inset 4.
    #[test]
    fn everything_must_fit_the_arena_and_each_refusal_names_its_values() {
        assert!(matches!(
            refusal_naming(
                &with("height_max", "95.0"),
                &["paddle.height_max", "arena.height"]
            ),
            TuningError::PaddleTallerThanArena { .. }
        ));
        assert!(matches!(
            refusal_naming(&with("radius", "45.0"), &["ball.radius", "arena.height"]),
            TuningError::BallTooBigForArena { .. }
        ));
        assert!(matches!(
            refusal_naming(
                &with("inset", "78.0"),
                &["paddle.inset", "paddle.thickness"]
            ),
            TuningError::PaddleReachesCentreLine { .. }
        ));
        // 88 units of playable height at 60 steps a second: 5,280 per second.
        assert!(matches!(
            refusal_naming(&with("speed_cap", "5280.0"), &["ball.speed_cap"]),
            TuningError::SpeedCapCrossesArenaInOneStep { .. }
        ));
        // A ball that fast crosses Blocks' clear run in a step as well (M2
        // spec 2.2.6): an arena wide enough for a clear run of 90 shows
        // this rule's own edge.
        let roomy = with("speed_cap", "5279.0")
            .replace("width: 160.0", "width: 400.0")
            .replace(
                "max_tilt_degrees: 35.0,\n                clear_run: 24.0",
                "max_tilt_degrees: 35.0,\n                clear_run: 90.0",
            );
        assert!(Tuning::from_ron(&roomy).is_ok());
        assert!(Tuning::from_ron(&with("height_max", "90.0")).is_ok());
    }

    #[test]
    fn escape_is_reserved_for_pause_and_cannot_be_bound_to_a_seat() {
        assert!(matches!(
            Tuning::from_ron(&with("back", "Escape")),
            Err(TuningError::ReservedKey {
                key: KeyCode::Escape
            })
        ));
    }

    #[test]
    fn unparseable_or_misspelled_file_is_a_parse_error() {
        assert!(matches!(
            Tuning::from_ron("(arena: ("),
            Err(TuningError::Parse(_))
        ));
        assert!(matches!(
            Tuning::from_ron(&VALID.replace("thickness", "thicknes")),
            Err(TuningError::Parse(_))
        ));
    }

    /// M2 spec 2.1.1: a mode is data. Classic is Speed alone with an empty
    /// bag; Chaos ramps more gently and schedules its bag.
    #[test]
    fn each_mode_reads_as_its_speed_rule_its_effects_and_its_schedule() {
        let tuning = valid();
        let classic = tuning.mode(Mode::Classic);
        assert_near(classic.speed_rule().per_hit_increase(), 7.0, 1e-6);
        assert_near(classic.speed_rule().cap(), 250.0, 1e-6);
        assert_eq!(classic.always(), [EffectId::Speed]);
        assert_eq!(classic.bag(), []);
        assert_eq!(classic.thresholds_percent(), [] as [u8; 0]);

        let cap = NonZeroU8::new(2).expect("two is non-zero");
        let chaos = tuning.mode(Mode::Chaos { cap });
        assert_near(chaos.speed_rule().per_hit_increase(), 3.0, 1e-6);
        assert_near(chaos.speed_rule().cap(), 250.0, 1e-6);
        assert_eq!(chaos.always(), [EffectId::Speed]);
        assert_eq!(chaos.bag(), [EffectId::Shrink]);
        assert_eq!(chaos.thresholds_percent(), [30, 70]);
        assert_eq!(chaos.rotate_every_points().get(), 1);
    }

    #[test]
    fn the_ball_carries_the_cap_every_mode_shares_and_the_flight_limit() {
        let tuning = valid();
        assert_near(tuning.ball().speed_cap(), 250.0, 1e-6);
        assert_near(tuning.ball().flight_limit_seconds(), 20.0, 1e-6);
        assert!(matches!(
            Tuning::from_ron(&with("flight_limit_seconds", "0.0")),
            Err(TuningError::Field {
                path: "ball.flight_limit_seconds",
                ..
            })
        ));
    }

    /// Shrink's dial runs from the default height to the minimum: as a
    /// factor, from one to 8 over 14.
    #[test]
    fn shrinks_dial_is_a_factor_from_one_to_the_minimum_over_the_default() {
        let factor = valid().effects().shrink().height_factor();
        assert_near(factor.at(3), 1.0, 1e-6);
        assert_near(factor.at(15), 8.0 / 14.0, 1e-6);
    }

    #[test]
    fn a_mode_naming_an_effect_twice_is_rejected_naming_the_mode_and_the_effect() {
        assert_eq!(
            Tuning::from_ron(&with("bag", "[speed]")),
            Err(TuningError::ModeNamesEffectTwice {
                mode: "classic",
                effect: EffectId::Speed,
            })
        );
        assert_eq!(
            Tuning::from_ron(&with("always", "[speed, speed]")),
            Err(TuningError::ModeNamesEffectTwice {
                mode: "classic",
                effect: EffectId::Speed,
            })
        );
    }

    #[test]
    fn an_effect_name_the_game_does_not_know_is_a_parse_error() {
        assert!(matches!(
            Tuning::from_ron(&with("bag", "[gravity_well]")),
            Err(TuningError::Parse(_))
        ));
    }

    #[test]
    fn thresholds_that_do_not_rise_or_pass_a_hundred_are_rejected() {
        for thresholds in ["[70, 30]", "[30, 30]", "[30, 101]"] {
            assert!(
                matches!(
                    Tuning::from_ron(&with("thresholds_percent", thresholds)),
                    Err(TuningError::ModeThresholdsDoNotRise {
                        mode: "classic",
                        ..
                    })
                ),
                "{thresholds}"
            );
        }
        assert!(Tuning::from_ron(&with("thresholds_percent", "[0, 100]")).is_ok());
    }

    /// M2 spec 2.6.0: Chaos ramps its speed more gently than Classic.
    #[test]
    fn a_chaos_ramp_not_below_classics_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with("per_hit_increase", "3.0")),
            Err(TuningError::ChaosRampNotBelowClassic {
                classic: 3.0,
                chaos: 3.0,
            })
        );
    }

    /// M2 spec 2.3.9's done-condition, through the file.
    #[test]
    fn a_shrink_dial_full_before_it_starts_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with("full_at_return", "1")),
            Err(TuningError::ShrinkRamp(FullBeforeStart {
                starts_at: 3,
                full_at: 1,
            }))
        );
    }

    /// `VALID` with one piece of Wind's section replaced.
    fn with_wind(old: &str, new: &str) -> String {
        let ron = VALID.replacen(old, new, 1);
        assert_ne!(ron, VALID, "{old} is not in VALID");
        ron
    }

    /// The distance the ball's centre travels in one full crossing: the
    /// arena's 160 less, on each side, the inset of 4, the paddle's
    /// thickness of 2, and the ball's radius of 1.
    #[test]
    fn the_crossing_width_is_the_distance_between_the_two_contact_lines() {
        assert_near(valid().crossing_width(), 146.0, 1e-6);
    }

    /// M2 spec 2.6.2: Wind's dial is the drift over one crossing, from
    /// nothing to the tuned maximum, and its flaw is tuned per star.
    #[test]
    fn winds_section_becomes_a_drift_dial_and_a_believed_share_per_star() {
        let wind = valid().effects().wind();
        assert_near(wind.drift().at(0), 0.0, 1e-6);
        assert_near(wind.drift().at(6), 12.0, 1e-6);
        assert_near(wind.drift().at(12), 24.0, 1e-6);
        assert_near(wind.drift().at(400), 24.0, 1e-6);
        assert_near(wind.believed_share(Difficulty::OneStar), 0.4, 1e-6);
        assert_near(wind.believed_share(Difficulty::TwoStars), 0.7, 1e-6);
        assert_near(wind.believed_share(Difficulty::ThreeStars), 0.9, 1e-6);
    }

    #[test]
    fn a_wind_dial_full_before_it_starts_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_wind("starts_at_return: 0", "starts_at_return: 13")),
            Err(TuningError::WindRamp(FullBeforeStart {
                starts_at: 13,
                full_at: 12,
            }))
        );
    }

    /// M2 spec 2.6.2: the maximum is under half the playable height, 44 of
    /// the 88 here, so no wind carries a ball from the middle to a wall in
    /// one crossing.
    #[test]
    fn a_wind_that_drifts_half_the_playable_height_or_more_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_wind("max_drift: 24.0", "max_drift: 44.0")),
            Err(TuningError::WindDriftNotUnderHalfThePlayableHeight {
                max_drift: 44.0,
                half_playable_height: 44.0,
            })
        );
        assert!(Tuning::from_ron(&with_wind("max_drift: 24.0", "max_drift: 43.5")).is_ok());
        assert!(Tuning::from_ron(&with_wind("max_drift: 24.0", "max_drift: 0.0")).is_ok());
    }

    #[test]
    fn a_negative_wind_number_is_rejected_by_its_path() {
        for (old, new, path) in [
            (
                "max_drift: 24.0",
                "max_drift: -1.0",
                "effects.wind.max_drift",
            ),
            (
                "one_star: 0.4",
                "one_star: -0.1",
                "effects.wind.believed_share.one_star",
            ),
            (
                "two_stars: 0.7",
                "two_stars: -0.1",
                "effects.wind.believed_share.two_stars",
            ),
            (
                "three_stars: 0.9",
                "three_stars: -0.1",
                "effects.wind.believed_share.three_stars",
            ),
        ] {
            assert!(
                matches!(
                    Tuning::from_ron(&with_wind(old, new)),
                    Err(TuningError::Field { path: refused, .. }) if refused == path
                ),
                "{new}"
            );
        }
        // More than the whole drift is a belief too: the AI over-leads.
        assert!(Tuning::from_ron(&with_wind("three_stars: 0.9", "three_stars: 1.3")).is_ok());
    }

    #[test]
    fn winds_refusals_read_as_one_sentence_naming_the_value() {
        let ramp = Tuning::from_ron(&with_wind("starts_at_return: 0", "starts_at_return: 13"))
            .expect_err("full first");
        assert_eq!(
            ramp.to_string(),
            "tuning effects.wind: a dial full at return 12 cannot start to move at return 13"
        );
        let drift = Tuning::from_ron(&with_wind("max_drift: 24.0", "max_drift: 50.0"))
            .expect_err("too far");
        assert_eq!(
            drift.to_string(),
            "tuning effects.wind.max_drift 50 is not under half the playable height 44"
        );
    }

    /// `VALID` with one piece of Spin's section replaced: the first `old`
    /// after the section's name, so Wind's like-named numbers stay.
    fn with_spin(old: &str, new: &str) -> String {
        let (before, section) = VALID
            .split_once("spin: (")
            .expect("VALID has a spin section");
        let replaced = section.replacen(old, new, 1);
        assert_ne!(replaced, section, "{old} is not in VALID's spin section");
        format!("{before}spin: ({replaced}")
    }

    /// M2 spec 2.6.4: Spin's dial is the drift over one crossing from a
    /// moving-paddle hit, from nothing to the tuned maximum, and its flaw
    /// is tuned per star.
    #[test]
    fn spins_section_becomes_a_drift_dial_and_a_believed_share_per_star() {
        let spin = valid().effects().spin();
        assert_near(spin.drift().at(0), 0.0, 1e-6);
        assert_near(spin.drift().at(6), 8.0, 1e-6);
        assert_near(spin.drift().at(12), 16.0, 1e-6);
        assert_near(spin.drift().at(400), 16.0, 1e-6);
        assert_near(spin.max_drift(), 16.0, 1e-6);
        assert_near(spin.believed_share(Difficulty::OneStar), 0.3, 1e-6);
        assert_near(spin.believed_share(Difficulty::TwoStars), 0.6, 1e-6);
        assert_near(spin.believed_share(Difficulty::ThreeStars), 0.85, 1e-6);
    }

    #[test]
    fn a_spin_dial_full_before_it_starts_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_spin("starts_at_return: 0", "starts_at_return: 13")),
            Err(TuningError::SpinRamp(FullBeforeStart {
                starts_at: 13,
                full_at: 12,
            }))
        );
    }

    /// As Wind's (M2 phase 6 report, ruling 6): the maximum is under half
    /// the playable height, 44 of the 88 here, so no spin alone carries a
    /// ball from the middle to a wall in one crossing.
    #[test]
    fn a_spin_that_drifts_half_the_playable_height_or_more_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_spin("max_drift: 16.0", "max_drift: 44.0")),
            Err(TuningError::SpinDriftNotUnderHalfThePlayableHeight {
                max_drift: 44.0,
                half_playable_height: 44.0,
            })
        );
        assert!(Tuning::from_ron(&with_spin("max_drift: 16.0", "max_drift: 43.5")).is_ok());
        assert!(Tuning::from_ron(&with_spin("max_drift: 16.0", "max_drift: 0.0")).is_ok());
    }

    #[test]
    fn a_negative_spin_number_is_rejected_by_its_path() {
        for (old, new, path) in [
            (
                "max_drift: 16.0",
                "max_drift: -1.0",
                "effects.spin.max_drift",
            ),
            (
                "one_star: 0.3",
                "one_star: -0.1",
                "effects.spin.believed_share.one_star",
            ),
            (
                "two_stars: 0.6",
                "two_stars: -0.1",
                "effects.spin.believed_share.two_stars",
            ),
            (
                "three_stars: 0.85",
                "three_stars: -0.1",
                "effects.spin.believed_share.three_stars",
            ),
        ] {
            assert!(
                matches!(
                    Tuning::from_ron(&with_spin(old, new)),
                    Err(TuningError::Field { path: refused, .. }) if refused == path
                ),
                "{new}"
            );
        }
        // More than the whole drift is a belief too, as for Wind.
        assert!(Tuning::from_ron(&with_spin("three_stars: 0.85", "three_stars: 1.3")).is_ok());
    }

    #[test]
    fn spins_refusals_read_as_one_sentence_naming_the_value() {
        let ramp = Tuning::from_ron(&with_spin("starts_at_return: 0", "starts_at_return: 13"))
            .expect_err("full first");
        assert_eq!(
            ramp.to_string(),
            "tuning effects.spin: a dial full at return 12 cannot start to move at return 13"
        );
        let drift = Tuning::from_ron(&with_spin("max_drift: 16.0", "max_drift: 50.0"))
            .expect_err("too far");
        assert_eq!(
            drift.to_string(),
            "tuning effects.spin.max_drift 50 is not under half the playable height 44"
        );
    }

    /// `VALID` with one piece of Fog's section replaced: the first `old`
    /// after the section's name, so the like-named numbers before it stay.
    fn with_fog(old: &str, new: &str) -> String {
        let (before, section) = VALID.split_once("fog: (").expect("VALID has a fog section");
        let replaced = section.replacen(old, new, 1);
        assert_ne!(replaced, section, "{old} is not in VALID's fog section");
        format!("{before}fog: ({replaced}")
    }

    /// M2 spec 2.6.3: Fog's dial is the width of its band, from nothing to
    /// the tuned maximum, and its flaw is an added error tuned per star.
    #[test]
    fn fogs_section_becomes_a_width_dial_and_an_added_error_per_star() {
        let fog = valid().effects().fog();
        assert_near(fog.width().at(0), 0.0, 1e-6);
        assert_near(fog.width().at(4), 32.0, 1e-6);
        assert_near(fog.width().at(8), 64.0, 1e-6);
        assert_near(fog.width().at(400), 64.0, 1e-6);
        assert_near(fog.max_width(), 64.0, 1e-6);
        assert_near(fog.added_error(Difficulty::OneStar), 12.0, 1e-6);
        assert_near(fog.added_error(Difficulty::TwoStars), 6.0, 1e-6);
        assert_near(fog.added_error(Difficulty::ThreeStars), 2.5, 1e-6);
    }

    #[test]
    fn a_fog_dial_full_before_it_starts_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_fog("starts_at_return: 0", "starts_at_return: 9")),
            Err(TuningError::FogRamp(FullBeforeStart {
                starts_at: 9,
                full_at: 8,
            }))
        );
    }

    /// M2 spec 2.6.3: the maximum leaves a visible run before each paddle.
    /// The band stays under half the crossing, 73 of the 146 here, so the
    /// two runs together are longer than the fog.
    #[test]
    fn a_fog_as_wide_as_half_the_crossing_or_wider_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_fog("max_width: 64.0", "max_width: 73.0")),
            Err(TuningError::FogNotUnderHalfTheCrossingWidth {
                max_width: 73.0,
                half_crossing_width: 73.0,
            })
        );
        assert!(Tuning::from_ron(&with_fog("max_width: 64.0", "max_width: 72.5")).is_ok());
        assert!(Tuning::from_ron(&with_fog("max_width: 64.0", "max_width: 0.0")).is_ok());
    }

    #[test]
    fn a_negative_fog_number_is_rejected_by_its_path() {
        for (old, new, path) in [
            (
                "max_width: 64.0",
                "max_width: -1.0",
                "effects.fog.max_width",
            ),
            (
                "one_star: 12.0",
                "one_star: -0.1",
                "effects.fog.added_error.one_star",
            ),
            (
                "two_stars: 6.0",
                "two_stars: -0.1",
                "effects.fog.added_error.two_stars",
            ),
            (
                "three_stars: 2.5",
                "three_stars: -0.1",
                "effects.fog.added_error.three_stars",
            ),
        ] {
            assert!(
                matches!(
                    Tuning::from_ron(&with_fog(old, new)),
                    Err(TuningError::Field { path: refused, .. }) if refused == path
                ),
                "{new}"
            );
        }
    }

    #[test]
    fn fogs_refusals_read_as_one_sentence_naming_the_value() {
        let ramp = Tuning::from_ron(&with_fog("starts_at_return: 0", "starts_at_return: 9"))
            .expect_err("full first");
        assert_eq!(
            ramp.to_string(),
            "tuning effects.fog: a dial full at return 8 cannot start to move at return 9"
        );
        let width = Tuning::from_ron(&with_fog("max_width: 64.0", "max_width: 80.0"))
            .expect_err("too wide");
        assert_eq!(
            width.to_string(),
            "tuning effects.fog.max_width 80 is not under half the crossing width 73"
        );
    }

    /// M2 spec 2.2.6 and 2.2.6.1: the obstacle channel's bounds are the
    /// ball's numbers. The contacts in a step and the bounces in a flight
    /// are tuned; the floor is the steepest a paddle can send the ball, and
    /// the fastest it leaves an obstacle is the speed cap.
    #[test]
    fn the_ball_bounces_off_obstacles_by_the_tuned_counts_and_no_steeper_than_a_paddle_sends_it() {
        let ball = valid().ball();
        assert_eq!(
            ball.bounce_rule(),
            BounceRule::new(
                NonZeroU8::new(4).expect("four is not zero"),
                ball.max_deflection()
            )
            .with_bounces_per_flight(NonZeroU16::new(6).expect("six is not zero"))
            .no_faster_than(250.0)
        );
        let fewer = Tuning::from_ron(&with("contacts_per_step", "2")).expect("two is valid");
        assert_eq!(fewer.ball().bounce_rule().contacts_per_step().get(), 2);
        let tighter =
            Tuning::from_ron(&with("obstacle_bounces_per_flight", "4")).expect("four is valid");
        assert_eq!(tighter.ball().bounce_rule().bounces_per_flight().get(), 4);
    }

    /// A step of no contact could bounce off nothing, and a flight of no
    /// bounce would fly through every block: each count is a whole number
    /// above zero, and the file's shape says so.
    #[test]
    fn a_ball_allowed_no_contact_in_a_step_or_no_bounce_in_a_flight_does_not_parse() {
        for key in ["contacts_per_step", "obstacle_bounces_per_flight"] {
            assert!(
                matches!(
                    Tuning::from_ron(&with(key, "0")),
                    Err(TuningError::Parse(_))
                ),
                "{key}"
            );
        }
    }

    /// `VALID` with one piece of Lens's section replaced: the first `old`
    /// after the section's name, so the like-named numbers before it stay.
    fn with_lens(old: &str, new: &str) -> String {
        in_lens_of(VALID, old, new)
    }

    /// `text` with the first `old` of its Lens section replaced by `new`.
    fn in_lens_of(text: &str, old: &str, new: &str) -> String {
        let (before, section) = text.split_once("lens: (").expect("a lens section");
        let replaced = section.replacen(old, new, 1);
        assert_ne!(replaced, section, "{old} is not in the lens section");
        format!("{before}lens: ({replaced}")
    }

    /// M2 spec 2.6.5: Lens's dial is a count of lenses, each of one size
    /// and one largest bend; its flaw is a lag in flights and a believed
    /// share of the pull, each tuned per star.
    #[test]
    fn lens_section_becomes_a_count_dial_a_lens_shape_and_a_flaw_per_star() {
        let lens = valid().effects().lens();
        let counts: Vec<u16> = (0..=8).map(|returns| lens.count().after(returns)).collect();
        assert_eq!(counts, [0, 0, 0, 1, 1, 2, 2, 3, 3]);
        assert_eq!(lens.count().full(), 3);
        assert_near(lens.radius(), 8.0, 1e-6);
        assert_near(lens.max_bend(), 15.0_f32.to_radians(), 1e-6);
        assert_near(lens.clear_run(), 24.0, 1e-6);
        assert_eq!(lens.unseen_flights(Difficulty::OneStar), 3);
        assert_eq!(lens.unseen_flights(Difficulty::TwoStars), 2);
        assert_eq!(lens.unseen_flights(Difficulty::ThreeStars), 1);
        assert_near(lens.believed_share(Difficulty::OneStar), 0.4, 1e-6);
        assert_near(lens.believed_share(Difficulty::TwoStars), 0.7, 1e-6);
        assert_near(lens.believed_share(Difficulty::ThreeStars), 0.9, 1e-6);
    }

    #[test]
    fn a_lens_count_full_before_it_starts_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_lens("starts_at_return: 1", "starts_at_return: 8")),
            Err(TuningError::LensCount(InvalidItemCount::FullBeforeStart(
                FullBeforeStart {
                    starts_at: 8,
                    full_at: 7,
                }
            )))
        );
    }

    /// M2 spec 2.3.9's done-condition: validation rejects an item effect
    /// that starts at hit zero.
    #[test]
    fn a_lens_count_that_starts_to_move_at_the_serve_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_lens("starts_at_return: 1", "starts_at_return: 0")),
            Err(TuningError::LensCount(InvalidItemCount::StartsAtTheServe))
        );
    }

    #[test]
    fn a_lens_number_out_of_its_range_is_rejected_by_its_path() {
        for (old, new, path) in [
            ("radius: 8.0", "radius: 0.0", "effects.lens.radius"),
            (
                "max_bend_degrees: 15.0",
                "max_bend_degrees: -1.0",
                "effects.lens.max_bend_degrees",
            ),
            (
                "clear_run: 24.0",
                "clear_run: -1.0",
                "effects.lens.clear_run",
            ),
            (
                "one_star: 0.4",
                "one_star: -0.1",
                "effects.lens.believed_share.one_star",
            ),
            (
                "two_stars: 0.7",
                "two_stars: -0.1",
                "effects.lens.believed_share.two_stars",
            ),
            (
                "three_stars: 0.9",
                "three_stars: -0.1",
                "effects.lens.believed_share.three_stars",
            ),
        ] {
            assert!(
                matches!(
                    Tuning::from_ron(&with_lens(old, new)),
                    Err(TuningError::Field { path: refused, .. }) if refused == path
                ),
                "{new}"
            );
        }
    }

    /// M2 spec 2.6.5: one lens bends no crossing more than the tuned angle.
    /// The pull is sized by measure, and the measure reaches thirty degrees:
    /// a larger bend is refused, since the bound would not hold for it.
    #[test]
    fn a_lens_bend_above_the_largest_its_pull_is_sized_for_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_lens(
                "max_bend_degrees: 15.0",
                "max_bend_degrees: 30.5"
            )),
            Err(TuningError::LensBendAboveTheLargest {
                max_bend_degrees: 30.5,
                largest_degrees: 30.0,
            })
        );
        assert!(
            Tuning::from_ron(&with_lens(
                "max_bend_degrees: 15.0",
                "max_bend_degrees: 30.0"
            ))
            .is_ok()
        );
        assert!(
            Tuning::from_ron(&with_lens(
                "max_bend_degrees: 15.0",
                "max_bend_degrees: 0.0"
            ))
            .is_ok()
        );
    }

    /// A lens and its copy sit on their own halves, clear of the paddle by
    /// the tuned run: a lens across, then the run, must fit between midfield
    /// and a paddle's contact line, 73 here.
    #[test]
    fn a_lens_with_no_room_on_its_half_of_the_crossing_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_lens("clear_run: 24.0", "clear_run: 57.5")),
            Err(TuningError::LensHasNoRoomOnItsHalf {
                radius: 8.0,
                clear_run: 57.5,
                half_crossing_width: 73.0,
            })
        );
        assert!(Tuning::from_ron(&with_lens("clear_run: 24.0", "clear_run: 57.0")).is_ok());
        assert!(Tuning::from_ron(&with_lens("clear_run: 24.0", "clear_run: 0.0")).is_ok());
    }

    /// A lens lies inside the band the ball's centre travels, so no ball
    /// bounces off a wall inside one. In an arena 60 high that band reaches
    /// 29 either side of the middle.
    #[test]
    fn a_lens_taller_than_the_playable_height_is_rejected() {
        let low = with("height", "60.0");
        let fits = in_lens_of(&low, "clear_run: 24.0", "clear_run: 0.0");
        assert_eq!(
            Tuning::from_ron(&in_lens_of(&fits, "radius: 8.0", "radius: 30.0")),
            Err(TuningError::LensTallerThanThePlayableHeight {
                radius: 30.0,
                half_playable_height: 29.0,
            })
        );
        assert!(Tuning::from_ron(&in_lens_of(&fits, "radius: 8.0", "radius: 29.0")).is_ok());
    }

    #[test]
    fn lens_refusals_read_as_one_sentence_naming_the_value() {
        let serve = Tuning::from_ron(&with_lens("starts_at_return: 1", "starts_at_return: 0"))
            .expect_err("starts at the serve");
        assert_eq!(
            serve.to_string(),
            "tuning effects.lens: items appear at a paddle return, so their count cannot start to move at return 0"
        );
        let room = Tuning::from_ron(&with_lens("clear_run: 24.0", "clear_run: 60.0"))
            .expect_err("no room");
        assert_eq!(
            room.to_string(),
            "tuning effects.lens.radius 8 and effects.lens.clear_run 60 leave a lens no room on its half of the crossing, 73 wide"
        );
        let bend = Tuning::from_ron(&with_lens(
            "max_bend_degrees: 15.0",
            "max_bend_degrees: 45.0",
        ))
        .expect_err("too large a bend");
        assert_eq!(
            bend.to_string(),
            "tuning effects.lens.max_bend_degrees 45 is above 30, the largest bend a lens's pull is sized for"
        );
        let tall = TuningError::LensTallerThanThePlayableHeight {
            radius: 30.0,
            half_playable_height: 29.0,
        };
        assert_eq!(
            tall.to_string(),
            "tuning effects.lens.radius 30 is above half the playable height 29"
        );
    }

    #[test]
    fn the_new_refusals_read_as_one_sentence_naming_the_value() {
        let twice = Tuning::from_ron(&with("bag", "[speed]")).expect_err("named twice");
        assert_eq!(
            twice.to_string(),
            "tuning modes.classic names the effect Speed more than once"
        );
        let ramp = Tuning::from_ron(&with("per_hit_increase", "3.0")).expect_err("not below");
        assert_eq!(
            ramp.to_string(),
            "tuning modes.chaos.per_hit_increase 3 is not below modes.classic.per_hit_increase 3"
        );
        let shrink = Tuning::from_ron(&with("full_at_return", "1")).expect_err("full first");
        assert_eq!(
            shrink.to_string(),
            "tuning effects.shrink: a dial full at return 1 cannot start to move at return 3"
        );
    }

    /// `VALID` with one piece of Blocks' section replaced: the first `old`
    /// after the section opens becomes `new`.
    fn with_blocks(old: &str, new: &str) -> String {
        let (before, section) = VALID.split_once("blocks: (").expect("a blocks section");
        let replaced = section.replacen(old, new, 1);
        assert_ne!(replaced, section, "{old} is not in the blocks section");
        format!("{before}blocks: ({replaced}")
    }

    /// M2 spec 2.6.6: Blocks' dial is a count of blocks, each the default
    /// paddle's length and the paddle's thickness, at a tilt from a tuned
    /// range; its flaw is a lag and a share per star.
    #[test]
    fn blocks_section_becomes_a_count_dial_a_block_shape_and_a_flaw_per_star() {
        let blocks = valid().effects().blocks();
        let counts: Vec<u16> = (0..=9)
            .map(|returns| blocks.count().after(returns))
            .collect();
        assert_eq!(counts, [0, 0, 0, 0, 0, 1, 1, 1, 2, 2]);
        assert_eq!(blocks.count().full(), 2);
        assert_near(blocks.half_size().x, 1.0, 1e-6);
        assert_near(blocks.half_size().y, 7.0, 1e-6);
        assert_near(blocks.min_tilt(), 10.0_f32.to_radians(), 1e-6);
        assert_near(blocks.max_tilt(), 35.0_f32.to_radians(), 1e-6);
        assert_near(blocks.clear_run(), 24.0, 1e-6);
        assert_eq!(blocks.unseen_flights(Difficulty::OneStar), 3);
        assert_eq!(blocks.unseen_flights(Difficulty::TwoStars), 2);
        assert_eq!(blocks.unseen_flights(Difficulty::ThreeStars), 1);
        assert_near(blocks.believed_share(Difficulty::OneStar), 0.4, 1e-6);
        assert_near(blocks.believed_share(Difficulty::TwoStars), 0.7, 1e-6);
        assert_near(blocks.believed_share(Difficulty::ThreeStars), 0.9, 1e-6);
    }

    /// M2 spec 2.6.6: a block is as long as the default paddle, whatever
    /// that is tuned to.
    #[test]
    fn a_block_is_the_default_paddles_length_and_the_paddles_thickness() {
        let longer = with("height_default", "20.0").replace("thickness: 2.0", "thickness: 3.0");
        let blocks = Tuning::from_ron(&longer)
            .expect("a longer, thicker paddle is valid")
            .effects()
            .blocks();
        assert_near(blocks.half_size().x, 1.5, 1e-6);
        assert_near(blocks.half_size().y, 10.0, 1e-6);
    }

    #[test]
    fn a_blocks_count_full_before_it_starts_or_starting_at_the_serve_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_blocks("starts_at_return: 2", "starts_at_return: 9")),
            Err(TuningError::BlocksCount(InvalidItemCount::FullBeforeStart(
                FullBeforeStart {
                    starts_at: 9,
                    full_at: 8,
                }
            )))
        );
        assert_eq!(
            Tuning::from_ron(&with_blocks("starts_at_return: 2", "starts_at_return: 0")),
            Err(TuningError::BlocksCount(InvalidItemCount::StartsAtTheServe))
        );
    }

    #[test]
    fn a_blocks_number_out_of_its_range_is_rejected_by_its_path() {
        for (old, new, path) in [
            // Upright is excluded: a ball can shuttle between an upright
            // block and its copy (M2 spec, facts).
            (
                "min_tilt_degrees: 10.0",
                "min_tilt_degrees: 0.0",
                "effects.blocks.min_tilt_degrees",
            ),
            (
                "max_tilt_degrees: 35.0",
                "max_tilt_degrees: 90.0",
                "effects.blocks.max_tilt_degrees",
            ),
            (
                "clear_run: 24.0",
                "clear_run: -1.0",
                "effects.blocks.clear_run",
            ),
            (
                "one_star: 0.4",
                "one_star: -0.1",
                "effects.blocks.believed_share.one_star",
            ),
            (
                "two_stars: 0.7",
                "two_stars: -0.1",
                "effects.blocks.believed_share.two_stars",
            ),
            (
                "three_stars: 0.9",
                "three_stars: -0.1",
                "effects.blocks.believed_share.three_stars",
            ),
        ] {
            assert!(
                matches!(
                    Tuning::from_ron(&with_blocks(old, new)),
                    Err(TuningError::Field { path: refused, .. }) if refused == path
                ),
                "{new} was not refused at {path}"
            );
        }
    }

    #[test]
    fn a_blocks_tilt_range_with_its_ends_reversed_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_blocks(
                "max_tilt_degrees: 35.0",
                "max_tilt_degrees: 9.0"
            )),
            Err(TuningError::BlocksTiltRangeReversed {
                min_tilt_degrees: 10.0,
                max_tilt_degrees: 9.0,
            })
        );
        assert!(
            Tuning::from_ron(&with_blocks(
                "max_tilt_degrees: 35.0",
                "max_tilt_degrees: 10.0"
            ))
            .is_ok()
        );
    }

    /// M2 spec 2.2.6: no step may hold both a block bounce and a paddle
    /// contact, and the step that crosses a contact line bounces off no
    /// block. A ball at the cap of 250 travels 4.17 units in a step.
    #[test]
    fn a_clear_run_a_ball_at_the_speed_cap_crosses_in_one_step_is_rejected() {
        assert_eq!(
            Tuning::from_ron(&with_blocks("clear_run: 24.0", "clear_run: 4.0")),
            Err(TuningError::BlocksClearRunWithinOneStep {
                clear_run: 4.0,
                step_travel: 250.0 / 60.0,
            })
        );
        assert!(Tuning::from_ron(&with_blocks("clear_run: 24.0", "clear_run: 4.2")).is_ok());
    }

    /// M2 spec 2.6.6: where a block's centre may lie, on the left half. A
    /// block reaches 7.07 from its centre at most, corner to centre. It
    /// keeps a ball's width off midfield, so a served ball touches no
    /// block and passes between a block and its copy; its grown shape stays
    /// the clear run short of the contact line, 73 from midfield; and it
    /// keeps a ball's width off each wall.
    #[test]
    fn a_blocks_centre_has_room_clear_of_midfield_the_paddle_and_the_walls() {
        let room = valid().block_room();
        let reach = 50.0_f32.sqrt();
        assert_near(room.nearest(), reach + 2.0, 1e-4);
        assert_near(room.farthest(), 73.0 - 24.0 - reach - 1.0, 1e-4);
        assert_near(room.highest(), 45.0 - 2.0 - reach, 1e-4);
    }

    #[test]
    fn blocks_with_no_room_on_their_half_of_the_crossing_are_rejected() {
        assert!(matches!(
            Tuning::from_ron(&with_blocks("clear_run: 24.0", "clear_run: 56.0")),
            Err(TuningError::BlocksHaveNoRoomOnTheirHalf {
                clear_run: 56.0,
                half_crossing_width: 73.0,
                ..
            })
        ));
        assert!(Tuning::from_ron(&with_blocks("clear_run: 24.0", "clear_run: 55.8")).is_ok());
    }

    /// In an arena 60 high a block may reach 28 from its centre and keep a
    /// ball's width to each wall. A paddle 56.2 long gives a block that
    /// reaches 28.12, and one 55.8 long a block that reaches 27.92. The
    /// clear run is short here, so that a block that long has room on its
    /// half and it is the walls that refuse it.
    #[test]
    fn a_block_too_long_to_stand_clear_of_both_walls_is_rejected() {
        let low = with_blocks("clear_run: 24.0", "clear_run: 4.2")
            .replace("height: 90.0", "height: 60.0")
            .replace("height_max: 30.0", "height_max: 58.0");
        let too_long = low.replace("height_default: 14.0", "height_default: 56.2");
        assert_eq!(
            Tuning::from_ron(&too_long),
            Err(TuningError::BlockHasNoRoomBetweenTheWalls {
                length: 56.2,
                arena_height: 60.0,
            })
        );
        let fits = low.replace("height_default: 14.0", "height_default: 55.8");
        assert!(Tuning::from_ron(&fits).is_ok());
    }

    #[test]
    fn blocks_refusals_read_as_one_sentence_naming_the_value() {
        let refused = |text: String| {
            Tuning::from_ron(&text)
                .expect_err("each of these is refused")
                .to_string()
        };
        assert_eq!(
            refused(with_blocks("starts_at_return: 2", "starts_at_return: 0")),
            "tuning effects.blocks: items appear at a paddle return, so their count cannot start to move at return 0"
        );
        assert_eq!(
            refused(with_blocks(
                "max_tilt_degrees: 35.0",
                "max_tilt_degrees: 9.0"
            )),
            "tuning effects.blocks.max_tilt_degrees 9 is below effects.blocks.min_tilt_degrees 10"
        );
        assert_eq!(
            refused(with_blocks("clear_run: 24.0", "clear_run: 4.0")),
            "tuning effects.blocks.clear_run 4 is not above 4.166666666666667, what a ball at the speed cap travels in one step"
        );
        assert_eq!(
            refused(with_blocks("clear_run: 24.0", "clear_run: 60.0")),
            "tuning effects.blocks.clear_run 60 leaves a block 14 long no room on its half of the crossing, 73 wide"
        );
        assert_eq!(
            TuningError::BlockHasNoRoomBetweenTheWalls {
                length: 86.0,
                arena_height: 90.0,
            }
            .to_string(),
            "tuning paddle.height_default 86 makes a block too long to stand a ball's width clear of both walls in an arena 90 high"
        );
    }
}
