//! The effects on the HUD (M2 spec 2.7): an icon per effect in play, the one
//! joining shown large during the countdown, and the rally's return count.
//!
//! The row follows state, the large icon follows an event. Which icons are in
//! the row is read from [`ActiveEffects`] every frame, so it cannot drift
//! from the core. That an effect has *just joined* is not state the core
//! keeps: [`EffectStarted`] says it once, and [`JoiningIcon`] remembers it
//! until play begins.
//!
//! Every icon is spawned once, with the HUD, and laid out or not by its
//! `Display`, as the menu's rows are: an icon keeps its place in the row, and
//! nothing is spawned or despawned mid-match. Nothing here names an effect:
//! icons come from each effect's shell half (M2 spec 2.3.8).

use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::state::condition::in_state;
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, Display, FlexDirection, JustifyContent, Node, PositionType, Val};
use pong_core::domain::{EffectId, EffectStarted, MatchConfig};
use pong_core::features::ball::Ball;
use pong_core::features::effects::{ActiveEffects, BallFlight};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::{AppState, MatchState};

use crate::effects::shell_half;
use crate::ui::style::{DIM, font};

/// Shows the effects in play and the return count in a mode that schedules
/// effects; nothing in one that does not, so Classic's HUD is M1's.
///
/// Precondition: the core plugin group (`ActiveEffects`, `EffectStarted`,
/// `Tuning`, the states).
pub struct EffectHudPlugin;

impl Plugin for EffectHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), show_effect_hud)
            .add_systems(OnEnter(MatchState::Playing), settle_the_joining_icon)
            .add_systems(
                Update,
                (
                    show_the_joining_effect,
                    lay_out_the_icons_in_play,
                    show_return_count,
                )
                    .chain()
                    .run_if(in_state(AppState::InMatch)),
            );
    }
}

/// One effect's place in the icon row; laid out only while the effect is in
/// play and not shown large.
#[derive(Component, Debug)]
pub struct EffectIcon(pub EffectId);

/// The effect that joined for the rally about to start, shown large until
/// play begins; none otherwise.
#[derive(Component, Debug, Default)]
pub struct JoiningIcon(pub Option<EffectId>);

/// The rally's return count.
#[derive(Component, Debug, Default)]
pub struct ReturnCountText;

/// Spawns the row, the count, and the large icon's place, all empty, if the
/// match's mode schedules any effect. The mode is asked for its data, not
/// its name: a mode with a bag gets the HUD.
fn show_effect_hud(mut commands: Commands, config: Res<MatchConfig>, tuning: Res<Tuning>) {
    if tuning.mode(config.mode).bag().is_empty() {
        return;
    }
    let across_the_top = |top| Node {
        position_type: PositionType::Absolute,
        top: Val::Vh(top),
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        ..Node::default()
    };
    commands
        .spawn((across_the_top(3.0), DespawnOnExit(AppState::InMatch)))
        .with_children(|hud| {
            hud.spawn(Node {
                justify_content: JustifyContent::Center,
                column_gap: Val::Vh(3.0),
                // The row keeps its height while every icon is out of it, so
                // the count below does not jump when one comes in.
                min_height: Val::Vh(9.0),
                ..Node::default()
            })
            .with_children(|row| {
                for id in EffectId::ALL {
                    let Some(icon) = shell_half(id).icon else {
                        continue;
                    };
                    row.spawn((
                        EffectIcon(id),
                        Node {
                            display: Display::None,
                            ..Node::default()
                        },
                        Text::new(icon.glyph),
                        font(3.5),
                        TextColor(icon.color),
                    ));
                }
            });
            hud.spawn((ReturnCountText, Text::new("0"), font(5.0), TextColor(DIM)));
        });
    // Under the countdown's number, which sits at 40 % of the height.
    commands
        .spawn((across_the_top(60.0), DespawnOnExit(AppState::InMatch)))
        .with_children(|place| {
            place.spawn((
                JoiningIcon::default(),
                Text::default(),
                font(9.0),
                TextColor(Color::WHITE),
            ));
        });
}

/// An effect joined: its icon shows large from this frame. The core writes
/// [`EffectStarted`] on entering the countdown and this reads it later in
/// the same frame, so the icon is there from the countdown's first frame.
fn show_the_joining_effect(
    mut started: MessageReader<EffectStarted>,
    mut places: Query<(&mut JoiningIcon, &mut Text, &mut TextColor)>,
) {
    let Some(&EffectStarted { effect }) = started.read().last() else {
        return;
    };
    let Some(icon) = shell_half(effect).icon else {
        return;
    };
    for (mut joining, mut text, mut color) in &mut places {
        joining.0 = Some(effect);
        icon.glyph.clone_into(&mut text.0);
        color.0 = icon.color;
    }
}

/// Play begins: the large icon goes, and its effect takes its place in the
/// row. Resuming a paused rally enters `Playing` again and finds none.
fn settle_the_joining_icon(mut places: Query<(&mut JoiningIcon, &mut Text)>) {
    for (mut joining, mut text) in &mut places {
        if joining.0.take().is_some() {
            text.0.clear();
        }
    }
}

/// Lays out the icon of each effect in play, but for the one shown large.
/// Writes a `Node` only when its display changes, since any write lays it
/// out again.
fn lay_out_the_icons_in_play(
    active: Res<ActiveEffects>,
    places: Query<&JoiningIcon>,
    mut icons: Query<(&EffectIcon, &mut Node)>,
) {
    let joining = places.iter().find_map(|place| place.0);
    for (icon, mut node) in &mut icons {
        let display = if active.ids().contains(&icon.0) && joining != Some(icon.0) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

/// The ball's return count: the number every effect's dial rides (M2 spec
/// 2.2.4, amended), zero from the countdown to the first return. Writes only
/// when the text changes.
fn show_return_count(
    flights: Query<&BallFlight, With<Ball>>,
    mut texts: Query<&mut Text, With<ReturnCountText>>,
) {
    let Some(flight) = flights.iter().next() else {
        return;
    };
    let shown = flight.flight().returns().to_string();
    for mut text in &mut texts {
        if text.0 != shown {
            text.0.clone_from(&shown);
        }
    }
}
