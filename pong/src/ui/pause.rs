//! Pause (spec 1.6.3; ADR 0003): Escape pauses wherever the paddles move,
//! and the pause screen offers Play and Menu. Escape, the back key, or Play
//! resumes where play stopped; Menu leaves the match, recording nothing,
//! since no `MatchEnded` is sent. Either seat's keys drive it.
//!
//! The keys are read just before the fixed steps, as the focus-loss adapter
//! does, so rules acts on the request within the same frame.

use bevy::app::{App, Plugin, RunFixedMainLoop, RunFixedMainLoopSystems};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::common_conditions::input_just_pressed;
use bevy::input::keyboard::KeyCode;
use bevy::state::condition::in_state;
use bevy::state::state::{NextState, OnEnter};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, Val};
use pong_core::features::rules::MatchControl;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::{AppState, MatchState, paddles_move};

use crate::ui::menu_input::{MenuDrivers, MenuKey, MenuPress, menu_press};
use crate::ui::style::{cursor_frame, font, row};

/// Escape's pause, and the pause screen.
///
/// Preconditions: Bevy's `InputPlugin` and the core plugin group.
pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseCursor>()
            .add_systems(OnEnter(MatchState::Paused), show_pause_screen)
            .add_systems(
                RunFixedMainLoop,
                (
                    pause_on_escape
                        .run_if(paddles_move.and_then(input_just_pressed(KeyCode::Escape))),
                    navigate_pause.run_if(in_state(MatchState::Paused)),
                    mark_pause_cursor.run_if(resource_changed::<PauseCursor>),
                )
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            );
    }
}

/// A row of the pause screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PauseChoice {
    /// Back to the match.
    #[default]
    Resume,
    /// Out to the main menu.
    LeaveMatch,
}

/// One pause screen row.
#[derive(Component, Debug)]
pub struct PauseRow(pub PauseChoice);

/// The row under the cursor; on Play each time the screen opens.
#[derive(Resource, Debug, Default)]
struct PauseCursor(PauseChoice);

fn pause_on_escape(mut control: ResMut<MatchControl>) {
    control.request_pause();
}

fn show_pause_screen(mut commands: Commands, mut cursor: ResMut<PauseCursor>) {
    *cursor = PauseCursor::default();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Vh(1.0),
                ..Node::default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            DespawnOnExit(MatchState::Paused),
        ))
        .with_children(|screen| {
            screen.spawn((Text::new("Pause"), font(9.0), TextColor(Color::WHITE)));
            for (choice, label) in [
                (PauseChoice::Resume, "Play"),
                (PauseChoice::LeaveMatch, "Menu"),
            ] {
                screen.spawn((
                    PauseRow(choice),
                    row(
                        label.to_owned(),
                        Color::WHITE,
                        choice == PauseChoice::Resume,
                    ),
                ));
            }
        });
}

fn navigate_pause(
    keys: Res<ButtonInput<KeyCode>>,
    tuning: Res<Tuning>,
    mut cursor: ResMut<PauseCursor>,
    mut control: ResMut<MatchControl>,
    mut next_app: ResMut<NextState<AppState>>,
) {
    match menu_press(&keys, tuning.bindings(), MenuDrivers::EitherSeat) {
        None => {}
        Some(MenuPress::Seat(_, MenuKey::Up)) => cursor.0 = PauseChoice::Resume,
        Some(MenuPress::Seat(_, MenuKey::Down)) => cursor.0 = PauseChoice::LeaveMatch,
        Some(MenuPress::Seat(_, MenuKey::Back) | MenuPress::Escape) => control.request_resume(),
        Some(MenuPress::Seat(_, MenuKey::Confirm)) => match cursor.0 {
            PauseChoice::Resume => control.request_resume(),
            PauseChoice::LeaveMatch => next_app.set(AppState::Menu),
        },
    }
}

fn mark_pause_cursor(
    cursor: Res<PauseCursor>,
    mut rows: Query<(&PauseRow, &mut bevy::ui::BorderColor)>,
) {
    for (PauseRow(choice), mut border) in &mut rows {
        *border = cursor_frame(*choice == cursor.0);
    }
}
