//! The name screen (spec 1.6.3): a seat types a new profile's name. Seat
//! keys type letters here, so only Enter and Escape act.

use bevy::color::Color;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::error::BevyError;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::AutoFocus;
use bevy::state::state::{NextState, State};
use bevy::text::{
    EditableText, EditableTextFilter, FontSize, TextColor, TextCursorStyle, TextFont,
};
use bevy::ui::{BorderColor, Node, UiRect, Val};
use bevy::ui_widgets::TextInput;
use pong_core::features::persistence::model::ProfileName;

use super::{MatchDraft, MenuScreen};

/// Marks the name screen's text field.
#[derive(Component, Debug, Default)]
pub struct NameField;

/// Whether `c` may be typed into a name: a Latin letter, a digit, a space,
/// or a hyphen.
#[must_use]
pub fn name_char_allowed(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || c == ' '
        || c == '-'
        || (c.is_alphabetic() && ('\u{C0}'..='\u{24F}').contains(&c))
}

/// The name field, holding `typed` when the seat came back from the
/// colours. Bevy's `TextInput` edits it while it has focus; it takes at most
/// [`ProfileName::MAX_CHARS`] characters (spec 1.6.3).
pub(super) fn name_field(typed: Option<&ProfileName>) -> impl Bundle {
    (
        NameField,
        Node {
            min_width: Val::Vw(30.0),
            padding: UiRect::axes(Val::Vh(2.0), Val::Vh(0.5)),
            border: UiRect::all(Val::Vh(0.4)),
            ..Node::default()
        },
        BorderColor::all(Color::WHITE),
        TextFont {
            font_size: FontSize::Vh(6.0),
            ..TextFont::default()
        },
        TextColor(Color::WHITE),
        TextInput,
        EditableText {
            max_characters: Some(ProfileName::MAX_CHARS),
            ..EditableText::new(typed.map_or("", ProfileName::as_str))
        },
        EditableTextFilter::new(name_char_allowed),
        TextCursorStyle::default(),
        AutoFocus,
    )
}

/// Enter takes the typed name, trimmed, on to the colours if it is a valid
/// name; Escape backs out to the profile list.
///
/// # Errors
/// Enter on a name screen without exactly one name field.
pub(super) fn confirm_name(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<MenuScreen>>,
    fields: Query<&EditableText, With<NameField>>,
    mut draft: ResMut<MatchDraft>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
) -> Result<(), BevyError> {
    let screen = *screen.get();
    let MenuScreen::NameProfile(seat) = screen else {
        return Ok(());
    };
    if keys.just_pressed(KeyCode::Escape) {
        if let Some(before) = draft.screen_before(screen) {
            next_screen.set(before);
        }
        return Ok(());
    }
    if !keys.just_pressed(KeyCode::Enter) {
        return Ok(());
    }
    let typed = fields.single()?.value().to_string();
    if let Ok(name) = ProfileName::new(typed.trim()) {
        draft.new_name = Some(name);
        next_screen.set(MenuScreen::PickColor(seat));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::name_char_allowed;

    #[test]
    fn letters_with_their_accents_digits_spaces_and_hyphens_are_typed() {
        for c in ['Z', 'o', 'ë', 'Å', 'ß', 'ł', '7', ' ', '-'] {
            assert!(name_char_allowed(c), "{c:?}");
        }
    }

    /// Names keep to Latin letters, every one of which the game's font draws
    /// (`DejaVu Sans Bold`, checked up to U+024F); symbols are not names.
    #[test]
    fn symbols_controls_and_letters_outside_latin_are_refused() {
        for c in ['★', '\n', '\t', '.', '漢', 'Ж', '😀'] {
            assert!(!name_char_allowed(c), "{c:?}");
        }
    }
}
