//! The developer overlay (spec 1.6.5, plan 10.1): F3 toggles it; it reads
//! the rates, the ball, the computer's aim, and the states. Headless: the core
//! harness plus `InputPlugin`, the screens, and the overlay.

mod common;

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::camera::visibility::Visibility;
use bevy::ecs::query::With;
use bevy::input::InputPlugin;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::ui::widget::Text;
use common::tap;
use pong::overlay::{OverlayPlugin, OverlayText};
use pong::ui::ScreensPlugin;
use pong_core::domain::{Controller, Difficulty, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::effects::{ActiveEffects, Sandbox};
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::MatchState;
use pong_core::testing::{
    boot, enter_match, frame, headless_app, match_state, place_ball, run_until, start_match,
};

fn overlay_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, ScreensPlugin, OverlayPlugin));
    boot(&mut app);
    frame(&mut app);
    app
}

fn overlay(app: &mut App) -> (Visibility, String) {
    let mut query = app
        .world_mut()
        .query_filtered::<(&Visibility, &Text), With<OverlayText>>();
    let (visibility, text) = query.single(app.world()).expect("one overlay");
    (*visibility, text.0.clone())
}

fn shown(app: &mut App) -> bool {
    overlay(app).0 != Visibility::Hidden
}

#[test]
fn f3_shows_and_hides_the_overlay_which_starts_hidden() {
    let mut app = overlay_app();
    assert!(!shown(&mut app), "hidden by default");
    tap(&mut app, KeyCode::F3);
    assert!(shown(&mut app));
    tap(&mut app, KeyCode::F3);
    assert!(!shown(&mut app));
}

#[test]
fn in_the_menu_the_overlay_reads_the_rates_and_the_screen() {
    let mut app = overlay_app();
    tap(&mut app, KeyCode::F3);
    for _ in 0..70 {
        frame(&mut app);
    }
    assert_eq!(
        overlay(&mut app).1,
        "fixed 60.0 Hz   frame 60 fps\nMenu / Main"
    );
}

#[test]
fn in_a_match_the_overlay_reads_the_ball_and_the_computers_aim() {
    let mut app = overlay_app();
    enter_match(
        &mut app,
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Ai(Difficulty::TwoStars),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("not zero"),
            mode: Mode::Classic,
        },
    );
    tap(&mut app, KeyCode::F3);
    let text = overlay(&mut app).1;
    let lines: Vec<&str> = text.lines().skip(1).collect();
    assert_eq!(
        lines,
        [
            "ball 0.0 u/s",
            "effects Speed",
            "returns 0",
            "AI right: centre",
            "sandbox keys: 1 Speed, 2 Shrink, 3 Wind, 4 Spin, [ ] returns, 0 release",
            "InMatch / Countdown"
        ]
    );
}

fn chaos() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        players: SeatPlayers::GUESTS,
        points_to_win: NonZeroU8::new(10).expect("not zero"),
        mode: Mode::Chaos {
            cap: NonZeroU8::MIN,
        },
    }
}

fn sandbox(app: &App) -> &Sandbox {
    app.world().resource::<Sandbox>()
}

/// M2 spec 2.9.1: a sandbox forces the active effects and the hit count
/// from the overlay.
#[test]
fn with_the_overlay_shown_the_number_keys_toggle_effects_and_the_brackets_hold_the_count() {
    let mut app = overlay_app();
    start_match(&mut app, chaos());
    tap(&mut app, KeyCode::F3);
    assert_eq!(sandbox(&app).forced_effects(), None);

    // Which bag effect the match starts with is the seeded draw's to say.
    // Its key, by its place among the effects, takes it out of what is in
    // play.
    let bag = app
        .world()
        .resource::<Tuning>()
        .mode(chaos().mode)
        .bag()
        .to_vec();
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let in_play = app.world().resource::<ActiveEffects>().ids().to_vec();
    let (key, scheduled) = digits
        .into_iter()
        .zip(EffectId::ALL)
        .find(|(_, effect)| bag.contains(effect) && in_play.contains(effect))
        .expect("Chaos has one bag effect in play");
    tap(&mut app, key);
    assert_eq!(
        sandbox(&app).forced_effects(),
        Some([EffectId::Speed].as_slice())
    );
    // It plays on until the point: the overlay reads both.
    let text = overlay(&mut app).1;
    assert!(
        text.contains(&format!("\neffects Speed, {scheduled:?}\n")),
        "{text}"
    );
    assert!(
        text.contains("\n  effects from the next point: Speed\n"),
        "{text}"
    );
    tap(&mut app, key);
    assert_eq!(
        sandbox(&app).forced_effects(),
        Some([EffectId::Speed, scheduled].as_slice())
    );

    // The count starts from the rally's own, zero here, and stops at zero.
    tap(&mut app, KeyCode::BracketRight);
    tap(&mut app, KeyCode::BracketRight);
    assert_eq!(sandbox(&app).held_returns(), Some(2));
    for _ in 0..3 {
        tap(&mut app, KeyCode::BracketLeft);
    }
    assert_eq!(sandbox(&app).held_returns(), Some(0));
    assert!(overlay(&mut app).1.contains("this match records nothing"));

    tap(&mut app, KeyCode::Digit0);
    assert_eq!(
        (sandbox(&app).forced_effects(), sandbox(&app).held_returns()),
        (None, None)
    );
}

/// A key with no effect behind it does nothing.
#[test]
fn a_number_key_past_the_last_effect_forces_nothing() {
    let mut app = overlay_app();
    start_match(&mut app, chaos());
    tap(&mut app, KeyCode::F3);
    tap(&mut app, KeyCode::Digit9);
    assert_eq!(*sandbox(&app), Sandbox::default());
}

/// The sandbox is the overlay's: hidden, its keys are not the game's.
#[test]
fn the_sandbox_keys_do_nothing_while_the_overlay_is_hidden_or_outside_a_match() {
    let mut app = overlay_app();
    tap(&mut app, KeyCode::F3);
    tap(&mut app, KeyCode::Digit2);
    assert_eq!(*sandbox(&app), Sandbox::default(), "in the menu");
    tap(&mut app, KeyCode::F3);

    start_match(&mut app, chaos());
    for key in [KeyCode::Digit2, KeyCode::BracketRight, KeyCode::Digit0] {
        tap(&mut app, key);
    }
    assert_eq!(*sandbox(&app), Sandbox::default(), "hidden");
}

/// A count is first held from the rally's own, wherever the rally stands:
/// one above three returns is four, not one.
#[test]
fn the_first_bracket_holds_the_count_one_from_the_rallys_own() {
    let mut app = overlay_app();
    start_match(&mut app, chaos());
    tap(&mut app, KeyCode::F3);
    for _ in 0..3 {
        // Into the right paddle's face in the middle of one step.
        place_ball(&mut app, Vec2::new(72.0, 2.0), Vec2::new(200.0, 0.0));
        frame(&mut app);
    }
    assert!(overlay(&mut app).1.contains("\nreturns 3\n"));
    tap(&mut app, KeyCode::BracketRight);
    assert_eq!(sandbox(&app).held_returns(), Some(4));
}

/// The user's ruling after the phase 3 and 4 review: nothing stays forced
/// out of sight. Hiding the overlay lets go of the effects and the count;
/// the match it touched still records nothing.
#[test]
fn hiding_the_overlay_releases_the_sandbox() {
    let mut app = overlay_app();
    start_match(&mut app, chaos());
    tap(&mut app, KeyCode::F3);
    tap(&mut app, KeyCode::Digit2);
    tap(&mut app, KeyCode::BracketRight);
    assert!(sandbox(&app).is_forcing());

    tap(&mut app, KeyCode::F3);
    assert!(!shown(&mut app));
    assert_eq!(
        (sandbox(&app).forced_effects(), sandbox(&app).held_returns()),
        (None, None)
    );
    assert!(sandbox(&app).touched_match());
}

/// A match that is over was recorded, or skipped, when it ended: the keys
/// do nothing there, and the overlay does not offer them.
#[test]
fn once_the_match_is_over_the_sandbox_keys_do_nothing_and_are_not_offered() {
    let mut app = overlay_app();
    let to_one_point = MatchConfig {
        points_to_win: NonZeroU8::MIN,
        ..chaos()
    };
    start_match(&mut app, to_one_point);
    tap(&mut app, KeyCode::F3);
    assert!(overlay(&mut app).1.contains("\nsandbox keys: "));

    // Out over the right goal line, above the paddle: the match's one point.
    place_ball(&mut app, Vec2::new(70.0, 30.0), Vec2::new(250.0, 0.0));
    run_until(
        &mut app,
        |app| match_state(app) == MatchState::MatchOver,
        1_000,
        "the match never ended",
    );
    frame(&mut app);
    for key in [KeyCode::Digit2, KeyCode::BracketRight] {
        tap(&mut app, key);
    }
    assert_eq!(*sandbox(&app), Sandbox::default());
    let text = overlay(&mut app).1;
    assert!(!text.contains("sandbox"), "{text}");
}
