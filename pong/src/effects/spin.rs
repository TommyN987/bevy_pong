//! Spin's shell half (M2 spec 2.6.4): declared empty until its icon, cue,
//! and view are built (M2 phase 6, step 3). Spin is in no mode's bag until
//! then, so no player meets it without them.

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
