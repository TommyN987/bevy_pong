//! Match over (spec 1.6.3): who won, and what each profile gained, as the
//! files now hold it. Confirm, by either seat, or Escape returns to the main
//! menu.
//!
//! What a profile gained is its records now against a copy taken on entering
//! the match, so the screen shows exactly what was recorded: nothing for a
//! Guest or the computer, nothing when one profile sat on both seats.
//! Persistence records the result in the frame `MatchEnded` is sent, before
//! the match enters `MatchOver`.

use bevy::app::{App, Plugin, Update};
use bevy::color::{Color, Srgba};
use bevy::ecs::component::Component;
use bevy::ecs::error::BevyError;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, Val};
use pong_core::domain::{Controller, MatchConfig, Player, ProfileId};
use pong_core::features::persistence::model::{Profile, Profiles, Records};
use pong_core::features::rules::Score;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;

use crate::ui::menu_input::{MenuDrivers, MenuKey, MenuPress, menu_press};
use crate::ui::style::font;

/// The match over screen.
///
/// Preconditions: Bevy's `InputPlugin` and the core plugin group.
pub struct MatchOverPlugin;

impl Plugin for MatchOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), keep_records_at_match_start)
            .add_systems(OnEnter(MatchState::MatchOver), show_match_over)
            .add_systems(
                Update,
                leave_finished_match.run_if(in_state(MatchState::MatchOver)),
            );
    }
}

/// The heading naming the winner.
#[derive(Component, Debug, Default)]
pub struct WinnerHeading;

/// One profile's gains.
#[derive(Component, Debug, Default)]
pub struct GainLine;

/// The profiles as they were on entering the match.
#[derive(Resource, Debug)]
struct RecordsAtMatchStart(Profiles);

fn keep_records_at_match_start(mut commands: Commands, profiles: Res<Profiles>) {
    commands.insert_resource(RecordsAtMatchStart(profiles.clone()));
}

fn find(profiles: &Profiles, id: ProfileId) -> Option<&Profile> {
    profiles
        .as_slice()
        .iter()
        .find(|profile| profile.id() == id)
}

/// The name a side goes by: its profile's, its seat's side for a Guest, or
/// the computer's stars.
fn side_name(side: Side, config: &MatchConfig, profiles: &Profiles) -> (String, Srgba) {
    match (config.controller(side), config.player_on(side)) {
        (Controller::Ai(difficulty), _) => {
            ("★".repeat(usize::from(difficulty.stars())), Srgba::WHITE)
        }
        (_, Some(Player::Profile(id))) => find(profiles, id).map_or_else(
            || (side_word(side).to_owned(), Srgba::WHITE),
            |profile| (profile.name().as_str().to_owned(), profile.color()),
        ),
        (Controller::Human(_), Some(Player::Guest) | None) => {
            (side_word(side).to_owned(), Srgba::WHITE)
        }
    }
}

const fn side_word(side: Side) -> &'static str {
    match side {
        Side::Left => "Left",
        Side::Right => "Right",
    }
}

/// What `name` gained from `before` to `after`, or `None` if nothing.
fn gain_line(name: &str, before: Records, after: Records) -> Option<String> {
    let count = |gained: u32, one: &str, many: &str| match gained {
        0 => None,
        1 => Some(format!("+1 {one}")),
        n => Some(format!("+{n} {many}")),
    };
    let parts: Vec<String> = [
        count(after.wins.saturating_sub(before.wins), "win", "wins"),
        count(after.losses.saturating_sub(before.losses), "loss", "losses"),
        (after.longest_rally > before.longest_rally)
            .then(|| format!("best rally {}", after.longest_rally)),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| format!("{name} {}", parts.join(", ")))
}

/// # Errors
/// `MatchOver` with no winner on the score: rules ended the match early.
fn show_match_over(
    mut commands: Commands,
    config: Res<MatchConfig>,
    score: Res<Score>,
    profiles: Res<Profiles>,
    at_start: Res<RecordsAtMatchStart>,
) -> Result<(), BevyError> {
    let winner = score
        .winner(config.points_to_win)
        .ok_or_else(|| BevyError::from("the match is over with no winner"))?;
    let (winner_name, winner_color) = side_name(winner, &config, &profiles);
    let mut ids: Vec<ProfileId> = [Side::Left, Side::Right]
        .into_iter()
        .filter_map(|side| match config.player_on(side) {
            Some(Player::Profile(id)) => Some(id),
            Some(Player::Guest) | None => None,
        })
        .collect();
    ids.dedup();
    let gains: Vec<(String, Srgba)> = ids
        .into_iter()
        .filter_map(|id| {
            let after = find(&profiles, id)?;
            let before = find(&at_start.0, id).map_or_else(Records::default, Profile::records);
            gain_line(after.name().as_str(), before, after.records())
                .map(|line| (line, after.color()))
        })
        .collect();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Vh(1.5),
                ..Node::default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            DespawnOnExit(MatchState::MatchOver),
        ))
        .with_children(|screen| {
            screen.spawn((
                WinnerHeading,
                Text::new(format!("{winner_name} wins!")),
                font(9.0),
                TextColor(Color::from(winner_color)),
            ));
            for (line, color) in gains {
                screen.spawn((
                    GainLine,
                    Text::new(line),
                    font(5.0),
                    TextColor(Color::from(color)),
                ));
            }
        });
    Ok(())
}

fn leave_finished_match(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<Tuning>,
    mut next_app: ResMut<NextState<AppState>>,
) {
    if let Some(MenuPress::Seat(_, MenuKey::Confirm) | MenuPress::Escape) =
        menu_press(&keys, tuning.bindings(), MenuDrivers::EitherSeat)
    {
        next_app.set(AppState::Menu);
    }
}
