//! Shrink's shell half (M2 spec 2.6.1): both paddles shorten as the rally
//! grows.

use bevy::color::Color;

use super::{Icon, ShellHalf};
use crate::audio::synth::{BRASS, phrase, tone};

/// Two arrowheads pressing toward each other: something squeezed shorter.
const ICON: Icon = Icon {
    glyph: "▼\n▲",
    color: Color::srgb(1.0, 0.6, 0.1),
};

/// Shrink's icon and cue.
///
/// - Cue: three falling notes, each shorter in pitch than the last.
/// - View: none of its own. The paddle view draws each paddle's rectangle
///   every frame, so a shorter paddle is already a shorter bar (R1).
#[must_use]
pub fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: Some(ICON),
        join_cue: Some(phrase(
            &[
                tone(&[659.25], 0.09),
                tone(&[523.25], 0.09),
                tone(&[392.0], 0.22),
            ],
            BRASS,
        )),
        view: None,
    }
}
