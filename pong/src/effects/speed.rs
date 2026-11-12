//! Speed's shell half (M2 spec 2.6.0): declared empty. The ball is seen to
//! fly faster, and Classic has always played so without a word.

use super::ShellHalf;

/// No icon, no cue, no view of its own.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: None,
        join_cue: None,
        view: None,
    }
}
