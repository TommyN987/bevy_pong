//! Focus loss (spec 1.6.1, ADR 0003): a window that loses focus forgets every
//! seat intent, and asks the core to pause while the paddles move: in play
//! and in the waits between rallies (ADR 0004), so no countdown runs out and
//! no serve flies while nobody is looking.

use bevy::ecs::message::MessageReader;
use bevy::ecs::system::{Res, ResMut};
use bevy::state::state::State;
use bevy::window::WindowFocused;
use pong_core::features::rules::MatchControl;
use pong_core::features::seats::SeatIntents;
use pong_core::flow::states::{MatchState, paddles_move};

/// Acts on the frame's latest focus change only, so focus lost and regained
/// within one frame changes nothing.
///
/// Clears the intents in every state: a key held as focus left must not move a
/// paddle once play resumes. Bevy releases the keys of `ButtonInput` one frame
/// after focus loss, so this frame's report still carries them; this system
/// runs after the keyboard adapter and its clear is the last write the fixed
/// steps see. Asks to pause only while the paddles move; rules decides the
/// transition, and resuming a countdown restarts its full timer (ADR 0003).
pub(super) fn pause_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    match_state: Option<Res<State<MatchState>>>,
    mut seats: ResMut<SeatIntents>,
    mut control: ResMut<MatchControl>,
) {
    let lost = focus.read().last().is_some_and(|change| !change.focused);
    if !lost {
        return;
    }
    seats.clear();
    if paddles_move(match_state) {
        control.request_pause();
    }
}
