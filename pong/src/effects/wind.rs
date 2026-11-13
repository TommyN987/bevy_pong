//! Wind's shell half (M2 spec 2.6.2): declared empty until its icon, cue,
//! and view are built (M2 plan 5.2, E.3). Wind is in no mode's bag until
//! then, so the players are told nothing about an effect they cannot meet.

use super::ShellHalf;

/// No icon, no cue, no view yet.
#[must_use]
pub const fn shell_half() -> ShellHalf {
    ShellHalf {
        icon: None,
        join_cue: None,
        view: None,
    }
}
