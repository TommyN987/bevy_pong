//! The temporary dev entry (plans 4.5, 5.2): from the menu straight into a
//! two-human match or one against the computer, and Escape's duties in it.
//! Headless: the core harness plus `InputPlugin`, with keys written as the
//! window backend writes them.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::{ButtonState, InputPlugin};
use bevy::state::state::NextState;
use pong::dev_entry::{DevArgsError, DevEntryPlugin, dev_match, two_humans_to_ten};
use pong_core::domain::{Controller, Difficulty, MatchConfig, Seat, SeatPlayers};
use pong_core::features::rules::Score;
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;
use pong_core::testing::{app_state, boot, frame, headless_app, match_state, run_until};

fn dev_app() -> App {
    dev_app_playing(two_humans_to_ten())
}

fn dev_app_playing(config: MatchConfig) -> App {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, DevEntryPlugin { config }));
    app
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|&word| word.to_owned()).collect()
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
            players: SeatPlayers::GUESTS,
            points_to_win: config.points_to_win,
        }
    );
    assert_eq!(config.points_to_win.get(), 10);
}

#[test]
fn no_arguments_start_two_humans_to_ten() {
    assert_eq!(dev_match(&args(&[])), Ok(two_humans_to_ten()));
}

#[test]
fn vs_and_a_star_count_put_the_left_seat_against_the_computer_on_the_right() {
    for (stars, difficulty) in [
        ("1", Difficulty::OneStar),
        ("2", Difficulty::TwoStars),
        ("3", Difficulty::ThreeStars),
    ] {
        assert_eq!(
            dev_match(&args(&["--vs", stars])),
            Ok(MatchConfig {
                right: Controller::Ai(difficulty),
                ..two_humans_to_ten()
            }),
            "--vs {stars}"
        );
    }
}

#[test]
fn vs_without_a_star_count_is_refused() {
    assert_eq!(dev_match(&args(&["--vs"])), Err(DevArgsError::MissingStars));
}

#[test]
fn a_star_count_other_than_one_two_or_three_is_refused() {
    for stars in ["0", "4", "three"] {
        assert_eq!(
            dev_match(&args(&["--vs", stars])),
            Err(DevArgsError::UnknownStars(stars.to_owned()))
        );
    }
}

#[test]
fn an_unknown_argument_is_refused() {
    assert_eq!(
        dev_match(&args(&["--versus", "3"])),
        Err(DevArgsError::UnknownArgument("--versus".to_owned()))
    );
    assert_eq!(
        dev_match(&args(&["--vs", "3", "extra"])),
        Err(DevArgsError::UnknownArgument("extra".to_owned()))
    );
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

#[test]
fn reaching_the_menu_starts_the_match_the_plugin_was_given() {
    let vs_three_stars = MatchConfig {
        right: Controller::Ai(Difficulty::ThreeStars),
        ..two_humans_to_ten()
    };
    let mut app = dev_app_playing(vs_three_stars);
    boot(&mut app);
    frame(&mut app);
    assert_eq!(app_state(&app), AppState::InMatch);
    assert_eq!(*app.world().resource::<MatchConfig>(), vs_three_stars);
}
