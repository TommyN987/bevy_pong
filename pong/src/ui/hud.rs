//! The match HUD (spec 1.6.3): each side's score over its half, for the
//! whole match, and during the countdown its whole seconds left in the
//! middle (plan 8.4).

use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::state::condition::in_state;
use bevy::state::state::{OnEnter, State};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{JustifyContent, Node, PositionType, Val};
use pong_core::features::rules::{Score, StateTimer};
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;

use crate::ui::style::font;

/// Shows the score and the countdown on entering a match and keeps them
/// current.
///
/// Precondition: the core plugin group (`AppState`, `Score`).
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), show_hud)
            .add_systems(
                Update,
                (
                    show_score
                        .run_if(in_state(AppState::InMatch).and_then(resource_changed::<Score>)),
                    show_countdown.run_if(in_state(AppState::InMatch)),
                ),
            );
    }
}

/// The countdown's whole seconds left; empty outside `Countdown`.
#[derive(Component, Debug, Default)]
pub struct CountdownText;

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
            hud.spawn((
                CountdownText,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Vh(40.0),
                    ..Node::default()
                },
                Text::default(),
                font(16.0),
                TextColor(Color::WHITE),
            ));
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

/// The countdown in whole seconds, rounded up: 3, 2, 1, then nothing. Writes
/// only when the text changes.
fn show_countdown(
    state: Res<State<MatchState>>,
    timer: Option<Res<StateTimer>>,
    mut texts: Query<&mut Text, With<CountdownText>>,
) {
    let shown = match (state.get(), timer) {
        (MatchState::Countdown, Some(timer)) => {
            timer.time_left().as_millis().div_ceil(1000).to_string()
        }
        _ => String::new(),
    };
    for mut text in &mut texts {
        if text.0 != shown {
            text.0.clone_from(&shown);
        }
    }
}

fn show_score(score: Res<Score>, mut texts: Query<(&ScoreText, &mut Text)>) {
    for (ScoreText(side), mut text) in &mut texts {
        text.0 = score.side(*side).to_string();
    }
}
