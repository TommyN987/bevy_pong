//! Temporary dev entry (plans 4.5, 5.2): stands in for the menus until plan
//! 8.3, which removes this module. Reaching the menu starts a match to ten at
//! once: two humans, or with `--vs <stars>` on the command line
//! (`just run --vs 3`) the left seat against the computer. Escape pauses,
//! resumes, and starts the next match from `MatchOver`; the score goes to the
//! log, as there is no score display yet, and so does every `Notice`, as
//! there is no notice line yet.
//!
//! Escape is the key the spec gives pause and back-out (1.6.1 keys), so the
//! habit carries over to the real screens.

use std::num::NonZeroU8;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::message::MessageReader;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyCode;
use bevy::log::{info, warn};
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter, State};
use pong_core::domain::{Controller, Difficulty, MatchConfig, Notice, Seat};
use pong_core::features::rules::{MatchControl, Score};
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;

/// Skips the menu into [`config`](Self::config)'s match and gives Escape its
/// match duties.
///
/// Preconditions: Bevy's `InputPlugin` (in `DefaultPlugins`) and the core
/// plugin group.
pub struct DevEntryPlugin {
    /// The match every visit to the menu starts.
    pub config: MatchConfig,
}

impl Plugin for DevEntryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DevMatch(self.config))
            .add_systems(OnEnter(AppState::Menu), start_dev_match)
            .add_systems(
                Update,
                (
                    escape.run_if(
                        in_state(AppState::InMatch).and_then(input_just_pressed(KeyCode::Escape)),
                    ),
                    log_score
                        .run_if(in_state(AppState::InMatch).and_then(resource_changed::<Score>)),
                    log_notices,
                ),
            );
    }
}

/// The match the dev entry starts: both seats human, to ten.
#[must_use]
pub fn two_humans_to_ten() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        points_to_win: NonZeroU8::MIN.saturating_add(9),
    }
}

/// The match [`DevEntryPlugin`] was given.
#[derive(Resource)]
struct DevMatch(MatchConfig);

/// What the menu will do once a player picks: install the match and enter it.
fn start_dev_match(
    mut commands: Commands,
    dev_match: Res<DevMatch>,
    mut next: ResMut<NextState<AppState>>,
) {
    commands.insert_resource(dev_match.0);
    next.set(AppState::InMatch);
}

/// Pause and resume ask rules through `MatchControl` (ADR 0003); leaving a
/// finished match is the shell's, and the menu then starts the next one.
fn escape(
    state: Res<State<MatchState>>,
    mut control: ResMut<MatchControl>,
    mut next: ResMut<NextState<AppState>>,
) {
    match state.get() {
        MatchState::Countdown | MatchState::Playing | MatchState::PointScored => {
            control.request_pause();
        }
        MatchState::Paused => control.request_resume(),
        MatchState::MatchOver => next.set(AppState::Menu),
    }
}

/// Why the dev entry's arguments were refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DevArgsError {
    /// `--vs` came last, with no star count after it.
    #[error("`--vs` needs the computer's stars: 1, 2, or 3")]
    MissingStars,
    /// The word after `--vs` is not 1, 2, or 3.
    #[error("`{0}` is not a star count; the computer plays at 1, 2, or 3 stars")]
    UnknownStars(String),
    /// A word the dev entry does not know.
    #[error(
        "unknown argument `{0}`; the dev entry takes nothing, or `--vs 1`, `--vs 2`, or `--vs 3`"
    )]
    UnknownArgument(String),
}

/// The dev entry's match from the command-line words after the program name:
/// none for [`two_humans_to_ten`]; `--vs <stars>` for the left seat against
/// the computer on the right at 1, 2, or 3 stars, still to ten.
///
/// # Errors
/// Any word but `--vs` and its star count; see [`DevArgsError`].
pub fn dev_match(args: &[String]) -> Result<MatchConfig, DevArgsError> {
    let mut config = two_humans_to_ten();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        if word != "--vs" {
            return Err(DevArgsError::UnknownArgument(word.clone()));
        }
        let stars = words.next().ok_or(DevArgsError::MissingStars)?;
        config.right = Controller::Ai(difficulty_with(stars)?);
    }
    Ok(config)
}

/// The difficulty the menu would show as `stars` stars.
fn difficulty_with(stars: &str) -> Result<Difficulty, DevArgsError> {
    match stars {
        "1" => Ok(Difficulty::OneStar),
        "2" => Ok(Difficulty::TwoStars),
        "3" => Ok(Difficulty::ThreeStars),
        _ => Err(DevArgsError::UnknownStars(stars.to_owned())),
    }
}

/// Until the notice line (plan 8.1), each `Notice` goes to the log, so a
/// developer sees what the player would be told.
fn log_notices(mut notices: MessageReader<Notice>) {
    for notice in notices.read() {
        warn!("notice: {}", notice.text);
    }
}

fn log_score(score: Res<Score>) {
    info!(
        "score {} : {}",
        score.side(Side::Left),
        score.side(Side::Right)
    );
}
