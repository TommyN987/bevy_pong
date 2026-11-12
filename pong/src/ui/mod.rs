//! Screens (spec 1.6.3): one per state, built from Bevy UI nodes on the
//! game's camera (docs/architecture.md, rule 8).

pub mod boot_screen;
pub mod effect_hud;
pub mod font;
pub mod hud;
pub mod match_over;
pub mod menu;
pub mod menu_input;
pub mod notice_line;
pub mod pause;
pub mod style;

use bevy::app::{App, Plugin};
use pong_core::domain::Mode;

/// A mode's name as the screens show it (M2 spec 2.8.1): `Classic`, or
/// `Chaos` and a dot per effect its cap allows at once.
#[must_use]
pub fn mode_label(mode: Mode) -> String {
    match mode {
        Mode::Classic => "Classic".to_owned(),
        Mode::Chaos { cap } => format!("Chaos {}", "●".repeat(usize::from(cap.get()))),
    }
}

/// Every screen: the game's font, the boot screen, the menus, the notice
/// line, the match HUD and its effects, pause, and match over.
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
            hud::HudPlugin,
            effect_hud::EffectHudPlugin,
            pause::PausePlugin,
            match_over::MatchOverPlugin,
        ));
    }
}
