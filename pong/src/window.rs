//! The game window (spec 1.6.6, plan 4.3): titled, borderless fullscreen,
//! no pointer over it, and Alt+Enter between fullscreen and a window. The
//! mode comes from settings once they exist (plan 7.x); until then every
//! start is fullscreen and a toggle lasts until the game quits.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::query::With;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Query, Res};
use bevy::input::ButtonInput;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyCode;
use bevy::window::{CursorOptions, MonitorSelection, PrimaryWindow, Window, WindowMode};

/// The window's title. A placeholder until the game has a name.
pub const TITLE: &str = "bevy_pong";

/// The application ID on Wayland and `WM_CLASS` on X11, which compositors
/// match window rules against. Fixed at window creation.
pub const APP_ID: &str = "bevy_pong";

/// The primary window as the game opens it.
///
/// `Primary`, not `Current`: at creation the window is on no monitor yet, so
/// `Current` finds none and Bevy warns on every start. On Wayland, which has
/// no primary monitor, both leave the choice to the compositor.
#[must_use]
pub fn primary_window() -> Window {
    Window {
        title: TITLE.to_owned(),
        name: Some(APP_ID.to_owned()),
        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
        ..Window::default()
    }
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

/// The mode Alt+Enter switches to: a window from either fullscreen, and
/// borderless fullscreen on the monitor the window is on from a window.
#[must_use]
pub const fn toggled(mode: WindowMode) -> WindowMode {
    match mode {
        WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        WindowMode::BorderlessFullscreen(_) | WindowMode::Fullscreen(..) => WindowMode::Windowed,
    }
}

/// Alt+Enter toggles the primary window between fullscreen and windowed
/// (spec 1.6.6 MAY), in any state.
///
/// Preconditions: Bevy's `InputPlugin` and `WindowPlugin`, both in
/// `DefaultPlugins`.
pub struct WindowModePlugin;

impl Plugin for WindowModePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            toggle_window_mode.run_if(input_just_pressed(KeyCode::Enter).and_then(alt_held)),
        );
    }
}

fn alt_held(keys: Res<ButtonInput<KeyCode>>) -> bool {
    keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
}

fn toggle_window_mode(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in &mut windows {
        window.mode = toggled(window.mode);
    }
}

#[cfg(test)]
mod tests {
    use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};

    use super::{APP_ID, TITLE, primary_cursor_options, primary_window, toggled};

    #[test]
    fn the_first_start_is_borderless_fullscreen_titled_and_without_a_pointer() {
        let window = primary_window();
        assert_eq!(
            window.mode,
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        );
        assert_eq!(window.title, TITLE);
        assert_eq!(window.name.as_deref(), Some(APP_ID));
        assert!(!primary_cursor_options().visible);
    }

    #[test]
    fn every_fullscreen_toggles_to_a_window_and_a_window_to_fullscreen_where_it_is() {
        let fullscreens = [
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            WindowMode::Fullscreen(MonitorSelection::Index(1), VideoModeSelection::Current),
        ];
        for mode in fullscreens {
            assert_eq!(toggled(mode), WindowMode::Windowed, "{mode:?}");
        }
        assert_eq!(
            toggled(WindowMode::Windowed),
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        );
    }
}
