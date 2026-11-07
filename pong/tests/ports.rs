//! The shell's ports (spec 1.6.1, plan 4.1) driven through Bevy's real input
//! pipeline: `KeyboardInput` and `WindowFocused` messages in, paddles and match
//! state out. Headless: the core harness plus `InputPlugin` and the
//! `WindowFocused` message, which `DefaultPlugins` provides in the game.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::{ButtonState, InputPlugin};
use bevy::window::WindowFocused;
use pong::ports::PortsPlugin;
use pong_core::domain::{Controller, MatchConfig, Seat};
use pong_core::features::paddle::Paddle;
use pong_core::features::tuning::{SeatKeys, Tuning};
use pong_core::flow::states::MatchState;
use pong_core::kernel::geometry::Side;
use pong_core::testing::{
    FrameLength, boot, enter_match, fixed_timestep, frame, headless_app, match_state,
    set_frame_length, start_match,
};

fn two_humans() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
    }
}

/// The core harness with the shell's ports and the Bevy pieces they read.
fn shell_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins((InputPlugin, PortsPlugin))
        .add_message::<WindowFocused>();
    boot(&mut app);
    app
}

fn keys(app: &App, seat: Seat) -> SeatKeys {
    app.world().resource::<Tuning>().bindings().seat(seat)
}

/// A key event as the window backend writes it.
fn key(app: &mut App, key_code: KeyCode, state: ButtonState) {
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
}

fn focus(app: &mut App, focused: bool) {
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: Entity::PLACEHOLDER,
            focused,
        });
}

fn paddle_y(app: &mut App, side: Side) -> f32 {
    let mut paddles = app.world_mut().query::<&Paddle>();
    paddles
        .iter(app.world())
        .find(|paddle| paddle.side() == side)
        .expect("a paddle on each side")
        .center_y()
}

/// How far a paddle moves in one fixed step.
fn step_travel(app: &App) -> f32 {
    app.world().resource::<Tuning>().paddle().speed() * fixed_timestep(app).as_secs_f32()
}

#[track_caller]
fn assert_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 1e-4,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn a_press_released_within_one_frame_moves_the_paddle_exactly_once() {
    // One step per frame, and half a step, where the tap may fall on a frame
    // that runs no step and the core's latch carries it to the next.
    for length in [FrameLength::Steps(1), FrameLength::HalfStep] {
        let mut app = shell_app();
        start_match(&mut app, two_humans());
        set_frame_length(&mut app, length);
        let up = keys(&app, Seat::Left).up();
        key(&mut app, up, ButtonState::Pressed);
        key(&mut app, up, ButtonState::Released);
        for _ in 0..6 {
            frame(&mut app);
        }
        assert_near(paddle_y(&mut app, Side::Left), step_travel(&app));
        assert_near(paddle_y(&mut app, Side::Right), 0.0);
    }
}

#[test]
fn a_held_key_moves_the_paddle_on_every_fixed_step() {
    let mut app = shell_app();
    start_match(&mut app, two_humans());
    set_frame_length(&mut app, FrameLength::Steps(2));
    let down = keys(&app, Seat::Right).down();
    key(&mut app, down, ButtonState::Pressed);
    for _ in 0..3 {
        frame(&mut app);
    }
    assert_near(paddle_y(&mut app, Side::Right), -6.0 * step_travel(&app));
}

#[test]
fn focus_loss_in_play_stops_the_paddles_that_frame_and_pauses() {
    let mut app = shell_app();
    start_match(&mut app, two_humans());
    let up = keys(&app, Seat::Left).up();
    key(&mut app, up, ButtonState::Pressed);
    frame(&mut app);
    let moved = paddle_y(&mut app, Side::Left);
    assert_near(moved, step_travel(&app));

    // The key stays pressed in `ButtonInput`, as in the game until Bevy
    // releases it a frame later; the clear after the report still wins.
    focus(&mut app, false);
    frame(&mut app);
    assert_near(paddle_y(&mut app, Side::Left), moved);
    frame(&mut app);
    assert_eq!(match_state(&app), MatchState::Paused);
    assert_near(paddle_y(&mut app, Side::Left), moved);
}

#[test]
fn focus_loss_outside_play_does_not_pause() {
    let mut app = shell_app();
    enter_match(&mut app, two_humans());
    assert_eq!(match_state(&app), MatchState::Countdown);
    focus(&mut app, false);
    frame(&mut app);
    frame(&mut app);
    assert_eq!(match_state(&app), MatchState::Countdown);
}

#[test]
fn focus_lost_and_regained_within_one_frame_changes_nothing() {
    let mut app = shell_app();
    start_match(&mut app, two_humans());
    let up = keys(&app, Seat::Left).up();
    key(&mut app, up, ButtonState::Pressed);
    focus(&mut app, false);
    focus(&mut app, true);
    frame(&mut app);
    frame(&mut app);
    assert_eq!(match_state(&app), MatchState::Playing);
    assert_near(paddle_y(&mut app, Side::Left), 2.0 * step_travel(&app));
}
