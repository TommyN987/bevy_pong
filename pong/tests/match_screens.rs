//! The match screens (spec 1.6.3, plans 8.1 and 8.3): the score over each
//! half, the pause screen, and the match over screen with what each profile
//! gained. Headless: the core harness plus `InputPlugin` and the shell's
//! screens, with keys written as the window backend writes them.

mod common;

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::color::Srgba;
use bevy::ecs::component::Component;
use bevy::ecs::message::Messages;
use bevy::ecs::query::With;
use bevy::input::InputPlugin;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::state::state::State;
use bevy::ui::widget::Text;
use common::tap;
use pong::ui::ScreensPlugin;
use pong::ui::hud::{CountdownText, ScoreText};
use pong::ui::match_over::{GainLine, WinnerHeading};
use pong::ui::menu::MenuScreen;
use pong::ui::pause::PauseRow;
use pong::ui::style::KeyHint;
use pong_core::domain::{
    BallHitPaddle, Controller, Difficulty, MatchConfig, Mode, Player, ProfileId, Seat, SeatPlayers,
};
use pong_core::features::persistence::model::{Profile, ProfileName, Profiles, Records};
use pong_core::features::rules::{MatchControl, Score};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::{AppState, MatchState};
use pong_core::kernel::geometry::Side;
use pong_core::testing::{
    app_state, boot, enter_match, frame, headless_app, match_state, place_ball, run_until,
    start_match,
};

const ZOE: ProfileId = ProfileId(1);
const MAX: ProfileId = ProfileId(2);
const LEFT_DOWN: KeyCode = KeyCode::KeyS;
const LEFT_CONFIRM: KeyCode = KeyCode::KeyD;

/// The menu, with Zoë and Max saved.
fn screens_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, ScreensPlugin));
    boot(&mut app);
    frame(&mut app);
    let profile = |id, name| {
        Profile::new(
            id,
            ProfileName::new(name).expect("valid"),
            Srgba::RED,
            Records::default(),
        )
        .expect("valid")
    };
    app.insert_resource(
        Profiles::new(vec![profile(ZOE, "Zoë"), profile(MAX, "Max")]).expect("distinct ids"),
    );
    app
}

fn humans(players: SeatPlayers, points: u8) -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        players,
        points_to_win: NonZeroU8::new(points).expect("not zero"),
        mode: Mode::Classic,
    }
}

fn zoe_and_max() -> SeatPlayers {
    SeatPlayers {
        left_seat: Player::Profile(ZOE),
        right_seat: Player::Profile(MAX),
    }
}

fn texts<C: Component>(app: &mut App) -> Vec<String> {
    let mut query = app.world_mut().query_filtered::<&Text, With<C>>();
    query.iter(app.world()).map(|text| text.0.clone()).collect()
}

fn count<C: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<C>>()
        .iter(app.world())
        .count()
}

fn score_text(app: &mut App, side: Side) -> String {
    let mut query = app.world_mut().query::<(&ScoreText, &Text)>();
    query
        .iter(app.world())
        .find(|(score, _)| score.0 == side)
        .map(|(_, text)| text.0.clone())
        .expect("a score for each side")
}

fn records(app: &App, id: ProfileId) -> Records {
    app.world()
        .resource::<Profiles>()
        .as_slice()
        .iter()
        .find(|profile| profile.id() == id)
        .expect("the profile")
        .records()
}

fn until(app: &mut App, state: MatchState) {
    run_until(
        app,
        |app| app_state(app) == AppState::InMatch && match_state(app) == state,
        1_000,
        "the match never reached the expected state",
    );
}

/// `scorer` takes the next point: the ball just past the other goal, flying on.
fn score_for(app: &mut App, scorer: Side) {
    until(app, MatchState::Playing);
    let arena = app.world().resource::<Tuning>().arena();
    let (x, vx) = match scorer {
        Side::Left => (arena.goal_x(Side::Right) + 2.0, 60.0),
        Side::Right => (arena.goal_x(Side::Left) - 2.0, -60.0),
    };
    place_ball(app, Vec2::new(x, 0.0), Vec2::new(vx, 0.0));
}

#[test]
fn the_score_shows_over_each_half_and_follows_play() {
    let mut app = screens_app();
    start_match(&mut app, humans(SeatPlayers::GUESTS, 10));
    frame(&mut app);
    assert_eq!(
        (
            score_text(&mut app, Side::Left),
            score_text(&mut app, Side::Right)
        ),
        ("0".to_owned(), "0".to_owned())
    );
    app.world_mut().resource_mut::<Score>().award(Side::Left);
    frame(&mut app);
    assert_eq!(
        (
            score_text(&mut app, Side::Left),
            score_text(&mut app, Side::Right)
        ),
        ("1".to_owned(), "0".to_owned())
    );
}

#[test]
fn escape_pauses_and_the_pause_screen_offers_play_and_menu() {
    let mut app = screens_app();
    enter_match(&mut app, humans(SeatPlayers::GUESTS, 10));
    assert_eq!(match_state(&app), MatchState::Countdown);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(match_state(&app), MatchState::Paused);
    assert_eq!(count::<PauseRow>(&mut app), 2);
    assert!(texts::<Text>(&mut app).contains(&"Play".to_owned()));
}

#[test]
fn escape_or_play_on_the_pause_screen_resumes_where_play_stopped() {
    let mut app = screens_app();
    enter_match(&mut app, humans(SeatPlayers::GUESTS, 10));
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, KeyCode::Escape);
    assert_eq!(match_state(&app), MatchState::Countdown);
    assert_eq!(count::<PauseRow>(&mut app), 0);

    tap(&mut app, KeyCode::Escape);
    tap(&mut app, KeyCode::ArrowRight);
    assert_eq!(match_state(&app), MatchState::Countdown);
}

/// Plan 8.3: after a focus loss the pause screen resumes play.
#[test]
fn after_a_focus_loss_the_pause_screen_resumes_play() {
    let mut app = screens_app();
    start_match(&mut app, humans(SeatPlayers::GUESTS, 10));
    until(&mut app, MatchState::Playing);
    app.world_mut()
        .resource_mut::<MatchControl>()
        .request_pause();
    until(&mut app, MatchState::Paused);
    tap(&mut app, LEFT_CONFIRM);
    assert_eq!(match_state(&app), MatchState::Playing);
}

#[test]
fn menu_on_the_pause_screen_leaves_the_match_and_records_nothing() {
    let mut app = screens_app();
    start_match(&mut app, humans(zoe_and_max(), 1));
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, LEFT_DOWN);
    tap(&mut app, LEFT_CONFIRM);
    assert_eq!(app_state(&app), AppState::Menu);
    assert_eq!(
        *app.world().resource::<State<MenuScreen>>().get(),
        MenuScreen::Main
    );
    assert_eq!(records(&app, ZOE), Records::default());
    assert_eq!(records(&app, MAX), Records::default());
    assert_eq!(
        count::<ScoreText>(&mut app),
        0,
        "the score outlived the match"
    );
}

#[test]
fn the_pause_cursor_starts_on_play_every_time() {
    let mut app = screens_app();
    start_match(&mut app, humans(SeatPlayers::GUESTS, 10));
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, LEFT_DOWN);
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, KeyCode::Escape);
    tap(&mut app, LEFT_CONFIRM);
    assert_eq!(
        app_state(&app),
        AppState::InMatch,
        "Menu was under the cursor"
    );
}

/// Spec 1.6.3: match over with winner and records delta.
#[test]
fn match_over_names_the_winner_and_what_each_profile_gained() {
    let mut app = screens_app();
    start_match(&mut app, humans(zoe_and_max(), 1));
    let mut hits = app.world_mut().resource_mut::<Messages<BallHitPaddle>>();
    for _ in 0..3 {
        hits.write(BallHitPaddle { side: Side::Left });
    }
    score_for(&mut app, Side::Left);
    until(&mut app, MatchState::MatchOver);
    frame(&mut app);
    assert_eq!(texts::<WinnerHeading>(&mut app), ["Zoë wins!"]);
    assert_eq!(
        texts::<GainLine>(&mut app),
        ["Zoë +1 win, best rally 3", "Max +1 loss, best rally 3"]
    );
}

#[test]
fn a_guest_or_the_computer_is_named_by_side_or_stars_and_gains_nothing() {
    let mut app = screens_app();
    let guest_and_max = SeatPlayers {
        left_seat: Player::Guest,
        right_seat: Player::Profile(MAX),
    };
    start_match(&mut app, humans(guest_and_max, 1));
    score_for(&mut app, Side::Left);
    until(&mut app, MatchState::MatchOver);
    frame(&mut app);
    assert_eq!(texts::<WinnerHeading>(&mut app), ["Left wins!"]);
    assert_eq!(texts::<GainLine>(&mut app), ["Max +1 loss"]);
    tap(&mut app, KeyCode::Escape);

    let zoe_vs_computer = MatchConfig {
        right: Controller::Ai(Difficulty::TwoStars),
        ..humans(zoe_and_max(), 1)
    };
    start_match(&mut app, zoe_vs_computer);
    score_for(&mut app, Side::Right);
    until(&mut app, MatchState::MatchOver);
    frame(&mut app);
    assert_eq!(texts::<WinnerHeading>(&mut app), ["★★ wins!"]);
    assert_eq!(texts::<GainLine>(&mut app), ["Zoë +1 loss"]);
}

#[test]
fn confirm_or_escape_after_the_match_returns_to_the_main_menu() {
    for key in [LEFT_CONFIRM, KeyCode::ArrowRight, KeyCode::Escape] {
        let mut app = screens_app();
        start_match(&mut app, humans(SeatPlayers::GUESTS, 1));
        score_for(&mut app, Side::Left);
        until(&mut app, MatchState::MatchOver);
        tap(&mut app, key);
        assert_eq!(app_state(&app), AppState::Menu, "{key:?}");
        assert_eq!(count::<WinnerHeading>(&mut app), 0, "{key:?}");
    }
}

/// Plan 8.4 (R3): match over says how to go on, with either seat's key.
#[test]
fn match_over_hints_at_both_seats_confirm_keys() {
    let mut app = screens_app();
    start_match(&mut app, humans(SeatPlayers::GUESTS, 1));
    score_for(&mut app, Side::Left);
    until(&mut app, MatchState::MatchOver);
    frame(&mut app);
    assert_eq!(texts::<KeyHint>(&mut app), ["D✓ →✓"]);
}

/// Plan 8.4 (R7): the wait before a serve counts down in whole seconds.
#[test]
fn the_countdown_shows_its_seconds_and_goes_when_play_begins() {
    let mut app = screens_app();
    enter_match(&mut app, humans(SeatPlayers::GUESTS, 10));
    frame(&mut app);
    let countdown = app.world().resource::<Tuning>().match_flow().countdown();
    let seconds = countdown.as_millis().div_ceil(1000).to_string();
    assert_eq!(texts::<CountdownText>(&mut app), [seconds]);
    until(&mut app, MatchState::Playing);
    frame(&mut app);
    assert_eq!(texts::<CountdownText>(&mut app), [""]);
}
