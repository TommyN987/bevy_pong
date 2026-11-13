//! Where Spin's view draws its mark (M2 spec 2.6.4, R1, R9, R11): one
//! arrowhead riding on the ball for the whole of a spun flight, on the side
//! the ball will curve to and pointing that way, larger as the dial rises.
//! Headless: the core harness plus the plugin that places the mark, which
//! touches only the mark's parent, `Transform`, and `Visibility`; how the
//! mark looks is judged by eye.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::math::{Vec2, Vec3};
use bevy::transform::components::Transform;
use pong::effects::spin::{SpinMark, SpinMarkPlugin};
use pong_core::domain::{Controller, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::ball::Ball;
use pong_core::features::effects::Sandbox;
use pong_core::features::seats::PaddleIntent;
use pong_core::flow::states::MatchState;
use pong_core::testing::{
    ball_state, boot, enter_match, frame, headless_app, match_state, place_ball, report_intent,
    run_until,
};

/// How far a paddle travels in one fixed step with its key held: 90 units
/// a second at 60 steps a second.
const STEP_TRAVEL: f32 = 1.5;

fn two_humans() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        players: SeatPlayers::GUESTS,
        points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        mode: Mode::Classic,
    }
}

fn until(app: &mut App, state: MatchState) {
    run_until(
        app,
        |app| match_state(app) == state,
        1_000,
        "the state never came",
    );
}

/// A match in its first countdown with the mark's plugin and nothing
/// forced: Classic, so no spin.
fn classic_app() -> App {
    let mut app = headless_app(1);
    app.add_plugins(SpinMarkPlugin);
    boot(&mut app);
    enter_match(&mut app, two_humans());
    frame(&mut app);
    app
}

/// A match in its first countdown with Spin alone in play and every flight
/// starting at `returns`: forced before the match is entered. With no Speed
/// beside it every return leaves at the serve speed.
fn spin_app(returns: u16) -> App {
    let mut app = headless_app(1);
    app.add_plugins(SpinMarkPlugin);
    boot(&mut app);
    let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
    sandbox.force_effects(&[EffectId::Spin]);
    sandbox.hold_returns(Some(returns));
    enter_match(&mut app, two_humans());
    frame(&mut app);
    assert_eq!(match_state(&app), MatchState::Countdown);
    app
}

/// The mark as placed.
#[derive(Debug, Clone, Copy)]
struct Placed {
    /// The entity it is a child of.
    on: Entity,
    /// Where its centre is beside its parent's, along y.
    beside_y: f32,
    /// Which way its tip points along y, and how tall it is.
    tip_y: f32,
    shows: bool,
}

/// The one mark of the match.
fn mark(app: &mut App) -> Placed {
    let mut query = app
        .world_mut()
        .query_filtered::<(&ChildOf, &Transform, &Visibility), With<SpinMark>>();
    let (child_of, transform, visibility) = query.single(app.world()).expect("exactly one mark");
    // The unit mark's tip is at +y; where the transform turns it.
    let tip = transform.rotation * (transform.scale * Vec3::Y);
    Placed {
        on: child_of.parent(),
        beside_y: transform.translation.y,
        tip_y: tip.y,
        shows: *visibility != Visibility::Hidden,
    }
}

fn the_ball(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Ball>>()
        .single(app.world())
        .expect("exactly one ball")
}

/// `1.0` for a paddle held up, `-1.0` for one held down, `0.0` for a still one.
fn sign_of(intent: PaddleIntent) -> f32 {
    match intent {
        PaddleIntent::Up => 1.0,
        PaddleIntent::Still => 0.0,
        PaddleIntent::Down => -1.0,
    }
}

/// Play begins, and the right paddle returns a ball level while the right
/// seat holds `intent`: the ball is sent at the height the paddle's centre
/// has in the step of the hit. Returns the height the ball left at.
fn return_off_the_right_paddle(app: &mut App, intent: PaddleIntent) -> f32 {
    until(app, MatchState::Playing);
    frame(app);
    let contact_y = sign_of(intent) * STEP_TRAVEL;
    report_intent(app, Seat::Right, intent);
    place_ball(app, Vec2::new(72.0, contact_y), Vec2::new(200.0, 0.0));
    frame(app);
    report_intent(app, Seat::Right, PaddleIntent::Still);
    assert!(
        ball_state(app).1.x < 0.0,
        "the right paddle returned the ball"
    );
    contact_y
}

/// The ball carries its mark from the start, out of sight: a countdown has
/// no flight, and a serve is not spun.
#[test]
fn the_ball_carries_one_mark_hidden_until_a_return_is_spun() {
    let mut app = spin_app(12);
    let placed = mark(&mut app);
    assert_eq!(placed.on, the_ball(&mut app), "the mark rides on the ball");
    assert!(!placed.shows, "in the countdown");
    until(&mut app, MatchState::Playing);
    for _ in 0..5 {
        frame(&mut app);
        assert!(!mark(&mut app).shows, "after the serve");
    }
}

/// R1, end to end: the cause is on screen before the consequence. From the
/// frame of a moving-paddle return the mark shows on the side the paddle
/// moved to and points that way; and that is the way the ball, which left
/// level, is then carried.
#[test]
fn off_a_moving_paddle_the_mark_shows_and_points_the_way_the_ball_then_curves() {
    for intent in [PaddleIntent::Up, PaddleIntent::Down] {
        let toward = sign_of(intent);
        let mut app = spin_app(12);
        let left_at = return_off_the_right_paddle(&mut app, intent);
        let placed = mark(&mut app);
        assert!(placed.shows, "{intent:?}: no mark on a spun ball");
        assert!(
            placed.tip_y * toward > 0.0 && placed.beside_y * toward > 0.0,
            "{intent:?}: the mark does not point that way: {placed:?}"
        );
        for _ in 0..40 {
            frame(&mut app);
        }
        let carried = ball_state(&mut app).0.y - left_at;
        assert!(
            carried * toward > 1.0,
            "{intent:?}: the mark points {}, and the ball was carried {carried}",
            placed.tip_y
        );
    }
}

/// A still paddle's return flies straight: there is nothing to show.
#[test]
fn off_a_still_paddle_no_mark_shows() {
    let mut app = spin_app(12);
    return_off_the_right_paddle(&mut app, PaddleIntent::Still);
    for _ in 0..10 {
        assert!(!mark(&mut app).shows);
        frame(&mut app);
    }
}

/// R9: mild at zero. With the dial at zero no return is spun, so none is
/// marked.
#[test]
fn at_dial_zero_no_mark_shows_off_a_moving_paddle() {
    let mut app = spin_app(0);
    return_off_the_right_paddle(&mut app, PaddleIntent::Up);
    assert!(!mark(&mut app).shows);
}

/// R1 needs the mark only while there is a spin to explain.
#[test]
fn without_spin_in_play_no_mark_shows_off_a_moving_paddle() {
    let mut app = classic_app();
    return_off_the_right_paddle(&mut app, PaddleIntent::Up);
    assert!(!mark(&mut app).shows);
}

/// R9 on screen: the dial is seen. The mark is larger at full dial than
/// early in the rally, in each direction of its size.
#[test]
fn the_mark_is_larger_at_full_dial_than_early_in_the_rally() {
    let size_at = |returns: u16| {
        let mut app = spin_app(returns);
        return_off_the_right_paddle(&mut app, PaddleIntent::Up);
        let placed = mark(&mut app);
        assert!(placed.shows, "at {returns} returns");
        placed.tip_y
    };
    let (early, full, past_full) = (size_at(1), size_at(12), size_at(40));
    assert!(early > 0.0 && full > 1.5 * early, "{early} then {full}");
    assert!(
        (past_full - full).abs() < 1e-5,
        "the dial is full by 12: {full} then {past_full}"
    );
}

/// R11: nothing flashes. The mark changes only where a flight starts or a
/// rally ends: it shows, pointing one way, in every frame of the spun
/// flight, and is gone by the next countdown.
#[test]
fn the_mark_holds_for_the_whole_flight_and_goes_with_the_rally() {
    let mut app = spin_app(12);
    return_off_the_right_paddle(&mut app, PaddleIntent::Down);
    let mut frames = 0;
    while ball_state(&mut app).0.x > -60.0 {
        let placed = mark(&mut app);
        assert!(
            placed.shows && placed.tip_y < 0.0,
            "frame {frames} of the flight: {placed:?}"
        );
        frame(&mut app);
        frames += 1;
        assert!(frames < 300, "the ball never crossed");
    }
    assert!(frames > 60, "the flight lasted only {frames} frames");

    until(&mut app, MatchState::Countdown);
    frame(&mut app);
    assert!(!mark(&mut app).shows, "in the next countdown");
}

/// The mark lives and dies with its ball: no mark is left behind by a
/// match, and a second match has one again.
#[test]
fn leaving_the_match_takes_the_mark_with_the_ball() {
    let mut app = spin_app(12);
    let marks = |app: &mut App| {
        app.world_mut()
            .query_filtered::<Entity, With<SpinMark>>()
            .iter(app.world())
            .count()
    };
    assert_eq!(marks(&mut app), 1);
    pong_core::testing::leave_match(&mut app);
    assert_eq!(marks(&mut app), 0);
    enter_match(&mut app, two_humans());
    frame(&mut app);
    assert_eq!(marks(&mut app), 1);
}
