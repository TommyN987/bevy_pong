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
use pong_core::domain::{Controller, Difficulty, MatchConfig, Seat, SeatPlayers};
use pong_core::testing::{boot, enter_match, frame, headless_app};

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
        },
    );
    tap(&mut app, KeyCode::F3);
    let text = overlay(&mut app).1;
    let lines: Vec<&str> = text.lines().skip(1).collect();
    assert_eq!(
        lines,
        ["ball 0.0 u/s", "AI right: centre", "InMatch / Countdown"]
    );
}
