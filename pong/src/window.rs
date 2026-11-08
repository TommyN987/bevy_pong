//! The game window (spec 1.6.6, plan 4.3): titled, borderless fullscreen,
//! no pointer over it, and the mode the settings hold (plan 8.1). Alt+Enter
//! toggles the saved mode, so a toggle lasts into the next start.
//!
//! The window opens before `Boot` loads the settings, so it opens in the mode
//! [`opening_mode`] reads from the settings file. Switching it in the first
//! frames instead can be lost: on Wayland the compositor confirms the opening
//! fullscreen only later, and Bevy skips a change to the mode the window
//! reports, so the request to leave fullscreen is dropped
//! (`bevy_winit` 0.20.1, `system.rs:368-371`).

use bevy::app::{App, Plugin, Update};
use bevy::ecs::query::With;
use bevy::ecs::schedule::common_conditions::{resource_exists, resource_exists_and_changed};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyCode;
use bevy::window::{CursorOptions, MonitorSelection, PrimaryWindow, Window, WindowMode};
use pong_core::features::persistence::document::decode_settings;
use pong_core::features::persistence::io::SavePaths;
use pong_core::features::persistence::load::peek;
use pong_core::features::persistence::model::{Settings, WindowMode as SavedWindowMode};

/// The game's name, on the window and the title screens (`docs/design.md`).
pub const TITLE: &str = "Rusty Pong";

/// The application ID on Wayland and `WM_CLASS` on X11, which compositors
/// match window rules against. Fixed at window creation.
pub const APP_ID: &str = "bevy_pong";

/// The primary window as the game opens it, in `mode`.
///
/// `Primary`, not `Current`: at creation the window is on no monitor yet, so
/// `Current` finds none and Bevy warns on every start. On Wayland, which has
/// no primary monitor, both leave the choice to the compositor.
#[must_use]
pub fn primary_window(mode: SavedWindowMode) -> Window {
    Window {
        title: TITLE.to_owned(),
        name: Some(APP_ID.to_owned()),
        mode: match mode {
            SavedWindowMode::BorderlessFullscreen => {
                WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
            }
            SavedWindowMode::Windowed => WindowMode::Windowed,
        },
        ..Window::default()
    }
}

/// The mode the settings file holds, read before the app is built; the
/// first start's mode if there is no file, it cannot be used, or there is
/// nowhere to save. Reads only: `Boot` applies the load policy later.
#[must_use]
pub fn opening_mode(paths: Option<&SavePaths>) -> SavedWindowMode {
    paths
        .and_then(|paths| peek(&paths.settings_file(), decode_settings))
        .unwrap_or_default()
        .window_mode
}

/// The game is played on the keyboard, so the pointer is hidden. It is
/// hidden only while over the window, so it stays usable elsewhere.
#[must_use]
pub fn primary_cursor_options() -> CursorOptions {
    CursorOptions {
        visible: false,
        ..CursorOptions::default()
    }
}

/// The saved mode Alt+Enter switches to.
#[must_use]
pub const fn toggled(mode: SavedWindowMode) -> SavedWindowMode {
    match mode {
        SavedWindowMode::BorderlessFullscreen => SavedWindowMode::Windowed,
        SavedWindowMode::Windowed => SavedWindowMode::BorderlessFullscreen,
    }
}

/// The mode a window in `current` takes to show `saved`, or `None` if it
/// already does. Any fullscreen counts as fullscreen, on whichever monitor;
/// a window goes fullscreen on the monitor it is on.
#[must_use]
pub const fn mode_to_apply(saved: SavedWindowMode, current: WindowMode) -> Option<WindowMode> {
    match (saved, current) {
        (SavedWindowMode::Windowed, WindowMode::Windowed)
        | (
            SavedWindowMode::BorderlessFullscreen,
            WindowMode::BorderlessFullscreen(_) | WindowMode::Fullscreen(..),
        ) => None,
        (
            SavedWindowMode::Windowed,
            WindowMode::BorderlessFullscreen(_) | WindowMode::Fullscreen(..),
        ) => Some(WindowMode::Windowed),
        (SavedWindowMode::BorderlessFullscreen, WindowMode::Windowed) => {
            Some(WindowMode::BorderlessFullscreen(MonitorSelection::Current))
        }
    }
}

/// The primary window shows the mode `Settings` holds (spec 1.6.6), from
/// the end of `Boot` on; Alt+Enter toggles that mode (spec 1.6.6 MAY), in
/// any state, and the core saves it like any setting.
///
/// Preconditions: Bevy's `InputPlugin` and `WindowPlugin`, both in
/// `DefaultPlugins`, and the core plugin group.
pub struct WindowModePlugin;

impl Plugin for WindowModePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                toggle_window_mode.run_if(
                    resource_exists::<Settings>
                        .and_then(input_just_pressed(KeyCode::Enter))
                        .and_then(alt_held),
                ),
                show_window_mode.run_if(resource_exists_and_changed::<Settings>),
            )
                .chain(),
        );
    }
}

fn alt_held(keys: Res<ButtonInput<KeyCode>>) -> bool {
    keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
}

fn toggle_window_mode(mut settings: ResMut<Settings>) {
    settings.window_mode = toggled(settings.window_mode);
}

fn show_window_mode(settings: Res<Settings>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        if let Some(mode) = mode_to_apply(settings.window_mode, window.mode) {
            window.mode = mode;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};

    use pong_core::features::persistence::model::WindowMode as SavedWindowMode;

    use pong_core::features::persistence::model::Settings;

    use super::{APP_ID, TITLE, mode_to_apply, primary_cursor_options, primary_window, toggled};

    #[test]
    fn the_first_start_is_borderless_fullscreen_titled_and_without_a_pointer() {
        let window = primary_window(Settings::default().window_mode);
        assert_eq!(
            window.mode,
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        );
        assert_eq!(window.title, TITLE);
        assert_eq!(window.name.as_deref(), Some(APP_ID));
        assert!(!primary_cursor_options().visible);
    }

    #[test]
    fn a_saved_window_opens_as_a_window() {
        assert_eq!(
            primary_window(SavedWindowMode::Windowed).mode,
            WindowMode::Windowed
        );
    }

    #[test]
    fn a_saved_window_takes_any_fullscreen_to_a_window_and_leaves_a_window_alone() {
        let fullscreens = [
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            WindowMode::Fullscreen(MonitorSelection::Index(1), VideoModeSelection::Current),
        ];
        for mode in fullscreens {
            assert_eq!(
                mode_to_apply(SavedWindowMode::Windowed, mode),
                Some(WindowMode::Windowed),
                "{mode:?}"
            );
        }
        assert_eq!(
            mode_to_apply(SavedWindowMode::Windowed, WindowMode::Windowed),
            None
        );
    }

    /// A fullscreen window already is what the setting asks for, on
    /// whichever monitor it is.
    #[test]
    fn a_saved_fullscreen_takes_a_window_fullscreen_where_it_is_and_leaves_a_fullscreen_alone() {
        assert_eq!(
            mode_to_apply(SavedWindowMode::BorderlessFullscreen, WindowMode::Windowed),
            Some(WindowMode::BorderlessFullscreen(MonitorSelection::Current))
        );
        for mode in [
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            WindowMode::Fullscreen(MonitorSelection::Index(1), VideoModeSelection::Current),
        ] {
            assert_eq!(
                mode_to_apply(SavedWindowMode::BorderlessFullscreen, mode),
                None,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn the_saved_mode_toggles_between_fullscreen_and_a_window() {
        assert_eq!(
            toggled(SavedWindowMode::BorderlessFullscreen),
            SavedWindowMode::Windowed
        );
        assert_eq!(
            toggled(SavedWindowMode::Windowed),
            SavedWindowMode::BorderlessFullscreen
        );
    }
}
