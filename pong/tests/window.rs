//! The window mode from settings (spec 1.6.6) and Alt+Enter (its MAY)
//! through Bevy's real input pipeline: key events and saved settings in, the
//! primary window's mode out. Headless: the core harness plus `InputPlugin`
//! and a primary window entity, which no backend opens here.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::{ButtonState, InputPlugin};
use bevy::window::{MonitorSelection, PrimaryWindow, Window, WindowMode};
use pong::window::{WindowModePlugin, opening_mode, primary_window};
use pong_core::features::persistence::document::encode_settings;
use pong_core::features::persistence::model::{Settings, WindowMode as SavedWindowMode};
use pong_core::testing::{TempSaveDir, boot, frame, headless_app};

fn window_app() -> (App, Entity) {
    window_app_with(|_| {})
}

/// The app booted with `before_boot` run first, as a previous run's files
/// would be there.
fn window_app_with(before_boot: impl FnOnce(&App)) -> (App, Entity) {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, WindowModePlugin));
    let window = app
        .world_mut()
        .spawn((
            primary_window(SavedWindowMode::BorderlessFullscreen),
            PrimaryWindow,
        ))
        .id();
    before_boot(&app);
    boot(&mut app);
    frame(&mut app);
    (app, window)
}

fn saved_mode(app: &App) -> SavedWindowMode {
    app.world().resource::<Settings>().window_mode
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
fn alt_enter_toggles_the_saved_mode_and_the_window_follows() {
    for alt in [KeyCode::AltLeft, KeyCode::AltRight] {
        let (mut app, window) = window_app();
        key(&mut app, alt, ButtonState::Pressed);
        tap_enter(&mut app);
        assert_eq!(mode(&app, window), WindowMode::Windowed, "{alt:?}");
        assert_eq!(saved_mode(&app), SavedWindowMode::Windowed, "{alt:?}");
        tap_enter(&mut app);
        assert_eq!(
            mode(&app, window),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            "{alt:?}"
        );
        assert_eq!(
            saved_mode(&app),
            SavedWindowMode::BorderlessFullscreen,
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

/// Spec 1.6.6: after toggling, windowed on the next start. The window opens
/// in the saved mode: switching it in the first frames can be lost, since on
/// Wayland the compositor confirms the opening fullscreen only later, and
/// Bevy skips a change to a mode the window already reports.
#[test]
fn a_window_saved_by_the_last_run_opens_as_a_window() {
    let temp = TempSaveDir::new();
    let paths = temp.save_paths();
    let windowed = Settings {
        window_mode: SavedWindowMode::Windowed,
        ..Settings::default()
    };
    let file = paths.settings_file();
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("writable");
    std::fs::write(file, encode_settings(windowed).expect("encodes")).expect("writable");
    assert_eq!(opening_mode(Some(&paths)), SavedWindowMode::Windowed);
    assert_eq!(
        primary_window(opening_mode(Some(&paths))).mode,
        WindowMode::Windowed
    );
}

#[test]
fn without_saved_settings_the_window_opens_fullscreen() {
    let temp = TempSaveDir::new();
    assert_eq!(
        opening_mode(Some(&temp.save_paths())),
        SavedWindowMode::BorderlessFullscreen
    );
    assert_eq!(opening_mode(None), SavedWindowMode::BorderlessFullscreen);
}

/// The first start opens fullscreen on the primary monitor; the saved
/// fullscreen does not move it.
#[test]
fn a_saved_fullscreen_leaves_the_window_where_it_opened() {
    let (app, window) = window_app();
    assert_eq!(
        mode(&app, window),
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    );
}

#[test]
fn a_mode_picked_in_settings_reaches_the_window() {
    let (mut app, window) = window_app();
    app.world_mut().resource_mut::<Settings>().window_mode = SavedWindowMode::Windowed;
    frame(&mut app);
    assert_eq!(mode(&app, window), WindowMode::Windowed);
}
