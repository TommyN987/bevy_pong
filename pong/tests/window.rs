//! Alt+Enter (spec 1.6.6 MAY) through Bevy's real input pipeline: key events
//! in, the primary window's mode out. Headless: the core harness plus
//! `InputPlugin` and a primary window entity, which no backend opens here.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::{ButtonState, InputPlugin};
use bevy::window::{MonitorSelection, PrimaryWindow, Window, WindowMode};
use pong::window::{WindowModePlugin, primary_window};
use pong_core::testing::{frame, headless_app};

fn window_app() -> (App, Entity) {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, WindowModePlugin));
    let window = app
        .world_mut()
        .spawn((primary_window(), PrimaryWindow))
        .id();
    (app, window)
}

fn key(app: &mut App, key_code: KeyCode, state: ButtonState) {
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

/// Enter pressed in one frame and released in the next.
fn tap_enter(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key(app, KeyCode::Enter, state);
        frame(app);
    }
}

fn mode(app: &App, window: Entity) -> WindowMode {
    app.world()
        .get::<Window>(window)
        .expect("the primary window exists")
        .mode
}

#[test]
fn alt_enter_toggles_to_a_window_and_back_to_fullscreen() {
    for alt in [KeyCode::AltLeft, KeyCode::AltRight] {
        let (mut app, window) = window_app();
        key(&mut app, alt, ButtonState::Pressed);
        tap_enter(&mut app);
        assert_eq!(mode(&app, window), WindowMode::Windowed, "{alt:?}");
        tap_enter(&mut app);
        assert_eq!(
            mode(&app, window),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            "{alt:?}"
        );
    }
}

#[test]
fn enter_without_alt_leaves_the_mode() {
    let (mut app, window) = window_app();
    let before = mode(&app, window);
    tap_enter(&mut app);
    assert_eq!(mode(&app, window), before);
}

#[test]
fn alt_released_before_enter_leaves_the_mode() {
    let (mut app, window) = window_app();
    let before = mode(&app, window);
    key(&mut app, KeyCode::AltLeft, ButtonState::Pressed);
    frame(&mut app);
    key(&mut app, KeyCode::AltLeft, ButtonState::Released);
    tap_enter(&mut app);
    assert_eq!(mode(&app, window), before);
}
