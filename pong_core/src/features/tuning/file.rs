//! On-disk shape of the tuning RON file: plain numbers, no invariants.
//! Validation into kernel types happens in [`super::Tuning::from_ron`].
//! Angles are written in degrees because people edit this file.

use std::num::NonZeroU8;

use bevy::input::keyboard::KeyCode;
use serde::Deserialize;

use crate::domain::EffectId;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TuningFile {
    pub(super) arena: ArenaFile,
    pub(super) paddle: PaddleFile,
    pub(super) ball: BallFile,
    pub(super) modes: ModesFile,
    pub(super) effects: EffectsFile,
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
    /// Distance from the goal line to the paddle's back face.
    pub(super) inset: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BallFile {
    pub(super) radius: f32,
    pub(super) serve_speed: f32,
    pub(super) speed_cap: f32,
    /// The longest a flight may last before the sanity phase calls it a fault.
    pub(super) flight_limit_seconds: f32,
    pub(super) max_deflection_degrees: f32,
    pub(super) serve_cone_degrees: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModesFile {
    pub(super) classic: ModeFile,
    pub(super) chaos: ModeFile,
}

/// One mode: its speed ramp, its effects, and its schedule.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeFile {
    pub(super) per_hit_increase: f32,
    /// In play from the first serve to the last.
    pub(super) always: Vec<EffectId>,
    /// Joining and rotating as the match goes on.
    pub(super) bag: Vec<EffectId>,
    /// Leader's progress toward the points target, in whole percent, at which
    /// one more bag effect joins. Whole numbers, so a boundary is exact.
    pub(super) thresholds_percent: Vec<u8>,
    /// Points scored between rotations of the bag.
    pub(super) rotate_every_points: NonZeroU8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EffectsFile {
    pub(super) shrink: ShrinkFile,
    pub(super) wind: WindFile,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShrinkFile {
    pub(super) starts_at_return: u16,
    pub(super) full_at_return: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WindFile {
    pub(super) starts_at_return: u16,
    pub(super) full_at_return: u16,
    /// Drift over one full crossing at full dial, in arena units.
    pub(super) max_drift: f32,
    pub(super) believed_share: BelievedShareFile,
}

/// The share of an effect's true strength an AI of each star expects.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BelievedShareFile {
    pub(super) one_star: f32,
    pub(super) two_stars: f32,
    pub(super) three_stars: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MatchFlowFile {
    pub(super) countdown_seconds: f32,
    /// How long the scored point stays on screen before the next countdown.
    pub(super) point_pause_seconds: f32,
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
