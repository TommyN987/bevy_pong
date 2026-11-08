//! The notice line (spec 1.7.3): what the player is told, in one line at
//! the foot of the menu. It keeps every `Notice` since the last match start,
//! since two can come at once (one per save file at boot), and clears when a
//! match starts.

use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::children;
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{JustifyContent, Node, PositionType, Val};
use pong_core::domain::Notice;
use pong_core::flow::states::AppState;

use crate::ui::style::font;

/// Collects every `Notice`, shows them on the menu, and forgets them when a
/// match starts.
///
/// Precondition: the core plugin group (`AppState`, `Notice`).
pub struct NoticeLinePlugin;

impl Plugin for NoticeLinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NoticeLine>()
            .add_systems(OnEnter(AppState::Menu), spawn_notice_line)
            .add_systems(OnEnter(AppState::InMatch), clear_notices)
            .add_systems(
                Update,
                (
                    collect_notices,
                    show_notices.run_if(resource_changed::<NoticeLine>),
                )
                    .chain(),
            );
    }
}

/// Every `Notice` since the last match start, oldest first.
#[derive(Resource, Debug, Default)]
pub struct NoticeLine(Vec<String>);

impl NoticeLine {
    /// The notices as one line; empty if there are none.
    #[must_use]
    pub fn text(&self) -> String {
        self.0.join(" ")
    }
}

/// Marks the text that shows the notice line.
#[derive(Component, Debug, Default)]
pub struct NoticeText;

fn collect_notices(mut notices: MessageReader<Notice>, mut line: ResMut<NoticeLine>) {
    for notice in notices.read() {
        line.0.push(notice.text.clone());
    }
}

fn clear_notices(mut line: ResMut<NoticeLine>) {
    line.0.clear();
}

/// A soft yellow: news, not an alarm.
const NOTICE_COLOR: Color = Color::srgb(1.0, 0.85, 0.4);

fn spawn_notice_line(mut commands: Commands, line: Res<NoticeLine>) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Vh(4.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..Node::default()
        },
        DespawnOnExit(AppState::Menu),
        children![(
            NoticeText,
            Text::new(line.text()),
            font(3.0),
            TextColor(NOTICE_COLOR),
        )],
    ));
}

fn show_notices(line: Res<NoticeLine>, mut texts: Query<&mut Text, With<NoticeText>>) {
    for mut text in &mut texts {
        text.0 = line.text();
    }
}
