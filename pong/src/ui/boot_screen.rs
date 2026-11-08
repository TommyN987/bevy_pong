//! The boot screen (spec 1.6.3): the game's title while `Boot` waits for
//! tuning, settings, and profiles.

use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::children;
use bevy::ecs::component::Component;
use bevy::ecs::system::Commands;
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, JustifyContent, Node, Val};
use pong_core::flow::states::AppState;

use crate::ui::style::font;
use crate::window::TITLE;

/// Shows the title on entering `Boot`; it goes when `Boot` does.
///
/// Precondition: the core plugin group (`AppState`).
pub struct BootScreenPlugin;

impl Plugin for BootScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Boot), show_boot_screen);
    }
}

/// Marks the boot screen's outermost node.
#[derive(Component, Debug, Default)]
pub struct BootScreen;

fn show_boot_screen(mut commands: Commands) {
    commands.spawn((
        BootScreen,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..Node::default()
        },
        DespawnOnExit(AppState::Boot),
        children![(Text::new(TITLE), font(9.0), TextColor(Color::WHITE),)],
    ));
}
