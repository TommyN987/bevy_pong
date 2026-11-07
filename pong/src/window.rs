//! The game window (spec 1.6.6, plan 4.3): titled, borderless fullscreen.
//! The mode comes from settings once they exist (plan 7.x); until then every
//! start is fullscreen.

use bevy::window::{MonitorSelection, Window, WindowMode};

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

#[cfg(test)]
mod tests {
    use bevy::window::{MonitorSelection, WindowMode};

    use super::{APP_ID, TITLE, primary_window};

    #[test]
    fn the_first_start_is_borderless_fullscreen_and_titled() {
        let window = primary_window();
        assert_eq!(
            window.mode,
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        );
        assert_eq!(window.title, TITLE);
        assert_eq!(window.name.as_deref(), Some(APP_ID));
    }
}
