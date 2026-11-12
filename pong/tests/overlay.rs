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
use bevy::ui::widget::Text;
use common::tap;
use pong::overlay::{OverlayPlugin, OverlayText};
use pong::ui::ScreensPlugin;
use pong_core::domain::{Controller, Difficulty, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::effects::Sandbox;
use pong_core::testing::{boot, enter_match, frame, headless_app, start_match};

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
            "sandbox keys: 1-2 effect, [ ] returns, 0 release",
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

    // Shrink is the second effect: key 2 takes it out of what is in play.
    tap(&mut app, KeyCode::Digit2);
    assert_eq!(
        sandbox(&app).forced_effects(),
        Some([EffectId::Speed].as_slice())
    );
    tap(&mut app, KeyCode::Digit2);
    assert_eq!(
        sandbox(&app).forced_effects(),
        Some([EffectId::Speed, EffectId::Shrink].as_slice())
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
