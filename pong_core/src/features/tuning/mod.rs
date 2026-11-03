//! Tuning (spec 1.4.8): one embedded RON file, validated at load into the
//! [`Tuning`] resource. Every range check goes through a kernel constructor,
//! so this module owns no range table of its own.

mod file;

use std::collections::HashSet;
use std::num::NonZeroU8;
use std::time::Duration;

use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::io::Reader;
use bevy::asset::{
    Asset, AssetApp, AssetLoader, AssetServer, Assets, Handle, LoadContext, LoadState,
    embedded_asset, load_embedded_asset,
};
use bevy::color::Srgba;
use bevy::ecs::error::BevyError;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::{not, resource_exists};
use bevy::ecs::system::{Commands, Res};
use bevy::input::keyboard::KeyCode;
use bevy::reflect::TypePath;

use crate::domain::{Difficulty, Seat};
use crate::kernel::geometry::{Arena, InvalidArena, InvalidLengthBounds, LengthBounds, Radius};
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
            .add_systems(Startup, begin_loading)
            .add_systems(
                Update,
                promote_when_loaded.run_if(not(resource_exists::<Tuning>)),
            );
    }
}

/// The in-flight or completed load of the tuning asset.
#[derive(Resource)]
struct TuningHandle(Handle<Tuning>);

fn begin_loading(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(TuningHandle(load_embedded_asset!(
        &*asset_server,
        "tuning.ron"
    )));
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
    match_flow: MatchFlowTuning,
    ai: AiTuning,
    bindings: BindingsTuning,
    palette: Vec<Srgba>,
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
    speed_rule: SpeedRule,
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
}

impl Tuning {
    /// Parses and validates RON text.
    ///
    /// # Errors
    /// The first problem found, naming the value to fix.
    pub fn from_ron(text: &str) -> Result<Self, TuningError> {
        let file: TuningFile = ron::from_str(text)?;
        Ok(Self {
            arena: Arena::new(file.arena.width, file.arena.height)?,
            paddle: PaddleTuning::validate(&file.paddle)?,
            ball: BallTuning::validate(&file.ball)?,
            match_flow: MatchFlowTuning::validate(file.match_flow)?,
            ai: AiTuning::validate(&file.ai)?,
            bindings: BindingsTuning::validate(&file.bindings)?,
            palette: validate_palette(&file.palette)?,
        })
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
        let speed_rule = SpeedRule::new(file.per_hit_increase, file.speed_cap)?;
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
            speed_rule,
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

    /// Per-hit increase and cap.
    #[must_use]
    pub const fn speed_rule(self) -> SpeedRule {
        self.speed_rule
    }

    /// Largest angle from straight-across a paddle hit can produce.
    #[must_use]
    pub const fn max_deflection(self) -> MaxDeflection {
        self.max_deflection
    }

    /// Largest angle from straight-across a serve can take; the serve samples within it.
    #[must_use]
    pub const fn serve_cone(self) -> MaxDeflection {
        self.serve_cone
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

    use super::{EMBEDDED_RON, Tuning, TuningError, TuningPlugin};
    use crate::domain::{Difficulty, Seat};
    use crate::kernel::geometry::InvalidLengthBounds;
    use crate::kernel::test_support::assert_near;
    use crate::kernel::{OutOfRange, Range};

    const VALID: &str = r"(
        arena: (width: 160.0, height: 90.0),
        paddle: (height_min: 8.0, height_max: 30.0, height_default: 14.0, thickness: 2.0, speed: 90.0, inset: 4.0),
        ball: (
            radius: 1.0,
            serve_speed: 80.0,
            speed_cap: 250.0,
            per_hit_increase: 7.0,
            max_deflection_degrees: 60.0,
            serve_cone_degrees: 30.0,
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
        assert_near(tuning.ball().speed_rule().after_hit(80.0), 87.0, 1e-6);
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
}
