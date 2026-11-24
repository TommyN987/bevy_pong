//! Blocks' shell half (M2 spec 2.6.6): declared empty until its view, cue,
//! and icon are built (M2 plan 9.3, E.3). Blocks is in no mode's bag until
//! then, so no player meets a block that is not drawn.

use super::ShellHalf;

/// No icon, no cue, no view of its own yet.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: None,
        join_cue: None,
        view: None,
    }
}
