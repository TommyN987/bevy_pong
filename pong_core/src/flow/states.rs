//! App and match states (spec 1.4.5, 1.4.10; docs/architecture.md "States").
//! The app roof gates `Boot`; the rules feature drives `MatchState`.

use bevy::ecs::system::Res;
use bevy::state::state::{State, States, SubStates};

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
/// `PointScored` are timed from tuning and advance by themselves. Paddles move
/// in those two and in `Playing` ([`MatchState::paddles_move`]); the ball is in
/// play only in `Playing` (ADR 0004). `Paused` is entered and
/// left at the shell's request through the rules feature's `MatchControl`
/// port, and resume returns to the substate paused from. `MatchOver` is final;
/// the shell returns to the menu from it.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::InMatch)]
pub enum MatchState {
    /// The serve cue is running; the ball waits on the centre spot and the
    /// paddles move, so players line up for the serve.
    #[default]
    Countdown,
    /// The ball is live.
    Playing,
    /// A point was just scored; the score is shown, the ball waits where it
    /// went out, and the paddles move.
    PointScored,
    /// Play is suspended at the shell's request.
    Paused,
    /// A side reached the target.
    MatchOver,
}

impl MatchState {
    /// Whether the paddles move in this substate: in play and in the waits
    /// between rallies, not while paused or after the match (ADR 0004).
    #[must_use]
    pub const fn paddles_move(self) -> bool {
        match self {
            Self::Countdown | Self::Playing | Self::PointScored => true,
            Self::Paused | Self::MatchOver => false,
        }
    }
}

/// Run condition: a match is on and its substate lets the paddles move. False
/// outside a match, where the substate does not exist. The shell's keyboard
/// adapter reports intents under it.
#[must_use]
pub fn paddles_move(state: Option<Res<State<MatchState>>>) -> bool {
    state.is_some_and(|state| state.get().paddles_move())
}
