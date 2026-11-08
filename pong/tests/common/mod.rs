//! Helpers shared by the shell's integration tests.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use pong_core::testing::frame;

/// Writes one key event as the window backend writes it; `InputPlugin`
/// turns it into `ButtonInput<KeyCode>` on the next frame.
pub fn key(app: &mut App, key_code: KeyCode, state: ButtonState) {
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
}

/// `key_code` pressed in one frame and released in the next.
pub fn tap(app: &mut App, key_code: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key(app, key_code, state);
        frame(app);
    }
}
