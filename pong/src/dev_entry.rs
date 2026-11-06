//! Temporary dev entry (plan 4.5): stands in for the menus until plan 8.3,
//! which removes this module. Reaching the menu starts a two-human match to
//! ten at once; Escape pauses, resumes, and starts the next match from
//! `MatchOver`; the score goes to the log, as there is no score display yet.
//!
//! Escape is the key the spec gives pause and back-out (1.6.1 keys), so the
//! habit carries over to the real screens.

use std::num::NonZeroU8;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyCode;
use bevy::log::info;
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter, State};
use pong_core::domain::{Controller, MatchConfig, Seat};
use pong_core::features::rules::{MatchControl, Score};
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;

/// Skips the menu into a two-human match and gives Escape its match duties.
///
/// Preconditions: Bevy's `InputPlugin` (in `DefaultPlugins`) and the core
/// plugin group.
pub struct DevEntryPlugin;

impl Plugin for DevEntryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), start_two_human_match)
            .add_systems(
                Update,
                (
                    escape.run_if(
                        in_state(AppState::InMatch).and_then(input_just_pressed(KeyCode::Escape)),
                    ),
                    log_score
                        .run_if(in_state(AppState::InMatch).and_then(resource_changed::<Score>)),
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

/// What the menu will do once a player picks: install the match and enter it.
fn start_two_human_match(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    commands.insert_resource(two_humans_to_ten());
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

fn log_score(score: Res<Score>) {
    info!(
        "score {} : {}",
        score.side(Side::Left),
        score.side(Side::Right)
    );
}
