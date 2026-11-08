//! The look the screens share: text sized to the viewport (docs/architecture.md
//! "Presentation"), and list rows framed white while the cursor is on them.

use bevy::color::Color;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::children;
use bevy::ecs::component::Component;
use bevy::text::{FontSize, TextColor, TextFont};
use bevy::ui::widget::Text;
use bevy::ui::{BorderColor, Node, UiRect, Val};

/// Hints and cues, under the white of what can be picked.
pub const DIM: Color = Color::srgb(0.6, 0.6, 0.7);

/// The game's font at `viewport_percent` of the viewport's height.
#[must_use]
pub fn font(viewport_percent: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Vh(viewport_percent),
        ..TextFont::default()
    }
}

/// Marks a hint: which key does what, in small dim text (plan 8.4).
#[derive(Component, Debug, Default)]
pub struct KeyHint;

/// A hint line: `text` small and dim.
#[must_use]
pub fn hint(text: &str) -> impl Bundle {
    (KeyHint, Text::new(text), font(4.0), TextColor(DIM))
}

/// A row's frame: white while the cursor is on it, none otherwise.
#[must_use]
pub fn cursor_frame(under_cursor: bool) -> BorderColor {
    BorderColor::all(if under_cursor {
        Color::WHITE
    } else {
        Color::NONE
    })
}

/// One list row: `text` in `color`, framed if `under_cursor`.
#[must_use]
pub fn row(text: String, color: Color, under_cursor: bool) -> impl Bundle {
    (
        Node {
            padding: UiRect::axes(Val::Vh(2.0), Val::Vh(0.5)),
            border: UiRect::all(Val::Vh(0.4)),
            ..Node::default()
        },
        cursor_frame(under_cursor),
        children![(Text::new(text), font(6.0), TextColor(color))],
    )
}
