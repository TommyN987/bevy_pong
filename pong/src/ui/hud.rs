//! The match HUD (spec 1.6.3): each side's score over its half, for the
//! whole match.

use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::state::condition::in_state;
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{JustifyContent, Node, PositionType, Val};
use pong_core::features::rules::Score;
use pong_core::flow::states::AppState;
use pong_core::kernel::geometry::Side;

use crate::ui::style::font;

/// Shows the score on entering a match and keeps it current.
///
/// Precondition: the core plugin group (`AppState`, `Score`).
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), show_hud)
            .add_systems(
                Update,
                show_score.run_if(in_state(AppState::InMatch).and_then(resource_changed::<Score>)),
            );
    }
}

/// The score of one side.
#[derive(Component, Debug)]
pub struct ScoreText(pub Side);

fn show_hud(mut commands: Commands, score: Res<Score>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Vh(3.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceAround,
                ..Node::default()
            },
            DespawnOnExit(AppState::InMatch),
        ))
        .with_children(|hud| {
            for side in [Side::Left, Side::Right] {
                hud.spawn((
                    ScoreText(side),
                    Text::new(score.side(side).to_string()),
                    font(10.0),
                    TextColor(Color::WHITE),
                ));
            }
        });
}

fn show_score(score: Res<Score>, mut texts: Query<(&ScoreText, &mut Text)>) {
    for (ScoreText(side), mut text) in &mut texts {
        text.0 = score.side(*side).to_string();
    }
}
