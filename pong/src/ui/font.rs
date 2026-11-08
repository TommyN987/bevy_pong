//! The game's font: `DejaVu Sans Bold` 2.37, embedded, in place of Bevy's
//! built-in one, which draws ASCII only. It draws ★, the arrows, and every
//! Latin letter, so names such as "Zoë" show. Licence: `fonts/DejaVu-LICENSE`.

use bevy::app::{App, Plugin, Startup};
use bevy::asset::{AssetId, Assets};
use bevy::ecs::error::BevyError;
use bevy::ecs::system::ResMut;
use bevy::text::Font;

/// `DejaVu Sans Bold`, as shipped.
pub const GAME_FONT: &[u8] = include_bytes!("fonts/DejaVuSans-Bold.ttf");

/// Puts [`GAME_FONT`] in the default font's slot at startup, so every
/// `TextFont::default()` uses it, the name field's included.
///
/// Bevy registers a font with its text layout the first time the font's id
/// appears, in the first frame's `PostUpdate`; the slot is filled in
/// `Startup`, before that, and never again.
///
/// Precondition: Bevy's `TextPlugin` (in `DefaultPlugins`) is added; without
/// it there are no fonts and this does nothing.
pub struct GameFontPlugin;

impl Plugin for GameFontPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, install_game_font);
    }
}

fn install_game_font(fonts: Option<ResMut<Assets<Font>>>) -> Result<(), BevyError> {
    if let Some(mut fonts) = fonts {
        fonts.insert(AssetId::default(), Font::from_bytes(GAME_FONT.to_vec()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetId, Assets};
    use bevy::text::Font;
    use pong_core::testing::{frame, headless_app};

    use super::{GAME_FONT, GameFontPlugin};

    #[test]
    fn the_game_font_is_a_truetype_file() {
        assert_eq!(GAME_FONT.get(..4), Some([0, 1, 0, 0].as_slice()));
    }

    /// Every text without a font of its own uses the default font's slot.
    #[test]
    fn the_game_font_takes_the_default_fonts_place() {
        let mut app = headless_app(1);
        app.init_asset::<Font>().add_plugins(GameFontPlugin);
        frame(&mut app);
        assert!(
            app.world()
                .resource::<Assets<Font>>()
                .get(AssetId::default())
                .is_some()
        );
    }
}
