//! The temporary dev entry (plan 4.5): from the menu straight into a
//! two-human match, and Escape's duties in it. Headless: the core harness plus
//! `InputPlugin`, with keys written as the window backend writes them.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::{ButtonState, InputPlugin};
use bevy::state::state::NextState;
use pong::dev_entry::{DevEntryPlugin, two_humans_to_ten};
use pong_core::domain::{Controller, MatchConfig, Seat};
use pong_core::features::rules::Score;
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;
use pong_core::testing::{app_state, boot, frame, headless_app, match_state, run_until};

fn dev_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, DevEntryPlugin));
    app
}

/// Escape pressed in one frame and released in the next.
fn tap_escape(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                key_code: KeyCode::Escape,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        frame(app);
    }
}

fn until_match_state(app: &mut App, state: MatchState) {
    run_until(
        app,
        |app| app_state(app) == AppState::InMatch && match_state(app) == state,
        1_000,
        "the match never reached the expected state",
    );
}

#[test]
fn the_dev_match_is_two_humans_to_ten() {
    let config = two_humans_to_ten();
    assert_eq!(
        config,
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            points_to_win: config.points_to_win,
        }
    );
    assert_eq!(config.points_to_win.get(), 10);
}

/// The menu hands over to the match one frame after it is entered.
#[test]
fn reaching_the_menu_starts_the_dev_match() {
    let mut app = dev_app();
    boot(&mut app);
    frame(&mut app);
    assert_eq!(app_state(&app), AppState::InMatch);
    assert_eq!(*app.world().resource::<MatchConfig>(), two_humans_to_ten());
}

#[test]
fn escape_pauses_play_and_escape_again_resumes_it() {
    let mut app = dev_app();
    until_match_state(&mut app, MatchState::Playing);
    tap_escape(&mut app);
    until_match_state(&mut app, MatchState::Paused);
    tap_escape(&mut app);
    until_match_state(&mut app, MatchState::Playing);
}

#[test]
fn escape_after_the_match_is_over_starts_a_fresh_match() {
    let mut app = dev_app();
    until_match_state(&mut app, MatchState::Playing);
    app.world_mut().resource_mut::<Score>().award(Side::Left);
    // Rules ends a match only at ten points; the test skips the rally.
    app.world_mut()
        .resource_mut::<NextState<MatchState>>()
        .set(MatchState::MatchOver);
    until_match_state(&mut app, MatchState::MatchOver);
    tap_escape(&mut app);
    until_match_state(&mut app, MatchState::Countdown);
    assert_eq!(*app.world().resource::<Score>(), Score::default());
}
