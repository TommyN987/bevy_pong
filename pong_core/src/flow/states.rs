//! App and match states (spec 1.4.5, 1.4.10; docs/architecture.md "States").
//! The app roof gates `Boot`; the rules feature drives `MatchState`.

use bevy::state::state::{States, SubStates};

/// Top-level flow: `Boot` waits for everything the menu needs, then `Menu`,
/// then `InMatch`.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Waiting for tuning, settings, and profiles. Nothing is shown yet.
    #[default]
    Boot,
    /// Menus: mode, profiles, settings.
    Menu,
    /// A match is in progress.
    InMatch,
}

/// Where a match is in its flow. Exists only while [`AppState::InMatch`].
///
/// The rules feature makes every transition (ADR 0003). `Countdown` and
/// `PointScored` are timed from tuning and advance by themselves. `Playing` is
/// the only state in which the fixed-step phases run. `Paused` is entered and
/// left at the shell's request through the rules feature's `MatchControl`
/// port, and resume returns to the substate paused from. `MatchOver` is final;
/// the shell returns to the menu from it.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::InMatch)]
pub enum MatchState {
    /// The serve cue is running; nothing moves.
    #[default]
    Countdown,
    /// The ball is live.
    Playing,
    /// A point was just scored; the score is shown.
    PointScored,
    /// Play is suspended at the shell's request.
    Paused,
    /// A side reached the target.
    MatchOver,
}
