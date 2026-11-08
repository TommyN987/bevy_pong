//! Screens (spec 1.6.3): one per state, built from Bevy UI nodes on the
//! game's camera (docs/architecture.md, rule 8).

pub mod boot_screen;
pub mod font;
pub mod menu;
pub mod menu_input;
pub mod notice_line;

use bevy::app::{App, Plugin};

/// Every screen: the game's font, the boot screen, the menus, and the notice
/// line.
///
/// Preconditions: `DefaultPlugins` and the core plugin group.
pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            font::GameFontPlugin,
            boot_screen::BootScreenPlugin,
            menu::MenuPlugin,
            notice_line::NoticeLinePlugin,
        ));
    }
}
