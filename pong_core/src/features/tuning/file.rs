//! On-disk shape of the tuning RON file: plain numbers, no invariants.
//! Validation into kernel types happens in [`super::Tuning::from_ron`].
//! Angles are written in degrees because people edit this file.

use std::num::NonZeroU8;

use bevy::input::keyboard::KeyCode;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TuningFile {
    pub(super) arena: ArenaFile,
    pub(super) paddle: PaddleFile,
    pub(super) ball: BallFile,
    pub(super) match_flow: MatchFlowFile,
    pub(super) ai: AiFile,
    pub(super) bindings: BindingsFile,
    pub(super) palette: Vec<ColorFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArenaFile {
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PaddleFile {
    pub(super) height_min: f32,
    pub(super) height_max: f32,
    pub(super) height_default: f32,
    pub(super) thickness: f32,
    pub(super) speed: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BallFile {
    pub(super) radius: f32,
    pub(super) serve_speed: f32,
    pub(super) speed_cap: f32,
    pub(super) per_hit_increase: f32,
    pub(super) max_deflection_degrees: f32,
    pub(super) serve_cone_degrees: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MatchFlowFile {
    pub(super) countdown_seconds: f32,
    /// Zero is unrepresentable: a match to zero points would end before it starts.
    pub(super) points_to_win_options: Vec<NonZeroU8>,
    pub(super) points_to_win_default: NonZeroU8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AiFile {
    pub(super) one_star: DifficultyFile,
    pub(super) two_stars: DifficultyFile,
    pub(super) three_stars: DifficultyFile,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DifficultyFile {
    pub(super) reaction_delay_seconds: f32,
    pub(super) prediction_error: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BindingsFile {
    pub(super) left: SeatKeysFile,
    pub(super) right: SeatKeysFile,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeatKeysFile {
    pub(super) up: KeyCode,
    pub(super) down: KeyCode,
    pub(super) action: KeyCode,
    pub(super) back: KeyCode,
}

/// Opaque sRGB; channels above one are allowed for bloom.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ColorFile {
    pub(super) red: f32,
    pub(super) green: f32,
    pub(super) blue: f32,
}
