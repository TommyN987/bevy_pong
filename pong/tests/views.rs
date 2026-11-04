//! Where the views draw bodies (spec 1.6.2, plan 4.2): `Transform` between the
//! last two fixed-step samples, still while play is stopped, cut on a
//! teleport. Headless: the core harness plus the motion plugin, which touches
//! only `Transform`; how bodies look is judged by eye.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::math::Vec2;
use bevy::state::state::NextState;
use bevy::transform::components::Transform;
use pong::views::motion::MotionPlugin;
use pong_core::domain::{Controller, MatchConfig, Seat};
use pong_core::features::ball::{Ball, Snap};
use pong_core::features::paddle::Paddle;
use pong_core::features::rules::MatchControl;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::MatchState;
use pong_core::testing::{
    FrameLength, ball_state, boot, frame, headless_app, match_state, place_ball, set_frame_length,
    start_match,
};

fn two_humans() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
    }
}

/// A match in play with the motion plugin, the ball placed clear of both
/// paddles and walls, moving one arena unit per fixed step along +x.
///
/// The first serve has already moved the ball when play begins, so placing it
/// is a teleport and carries `Snap`, as the core's own teleports do; without
/// it the view would slide from the served path for the first frames. After a
/// snap the ball is drawn still until the next fixed tick gives it a second
/// sample, so two whole-step frames pass before frames shrink to `length`.
fn playing_app(length: FrameLength) -> App {
    let mut app = headless_app(1);
    app.add_plugins(MotionPlugin);
    boot(&mut app);
    start_match(&mut app, two_humans());
    place_ball(&mut app, Vec2::new(0.0, 30.0), Vec2::new(60.0, 0.0));
    let ball = app
        .world_mut()
        .query_filtered::<Entity, With<Ball>>()
        .single(app.world())
        .expect("one ball");
    app.world_mut().entity_mut(ball).insert(Snap);
    frame(&mut app);
    frame(&mut app);
    set_frame_length(&mut app, length);
    app
}

fn drawn_ball(app: &mut App) -> Vec2 {
    app.world_mut()
        .query_filtered::<&Transform, With<Ball>>()
        .single(app.world())
        .expect("one drawn ball")
        .translation
        .truncate()
}

#[track_caller]
fn assert_vec_near(actual: Vec2, expected: Vec2) {
    assert!(
        (actual - expected).abs().max_element() <= 1e-4,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn a_moving_ball_is_drawn_further_on_every_frame_even_without_a_fixed_step() {
    let mut app = playing_app(FrameLength::HalfStep);
    let mut drawn = drawn_ball(&mut app);
    let mut frames_without_a_step = 0;
    for _ in 0..20 {
        let simulated = ball_state(&mut app).0;
        frame(&mut app);
        if ball_state(&mut app).0 == simulated {
            frames_without_a_step += 1;
        }
        let now = drawn_ball(&mut app);
        assert!(now.x > drawn.x, "drawn ball stalled at {now}");
        let position = ball_state(&mut app).0;
        assert!(
            now.x <= position.x && now.x >= position.x - 1.0 - 1e-4,
            "drawn at {now}, more than one step behind {position}"
        );
        drawn = now;
    }
    assert!(
        frames_without_a_step >= 5,
        "half-step frames ran a step each"
    );
}

#[test]
fn while_play_is_paused_the_drawn_ball_holds_still_where_it_is() {
    let mut app = playing_app(FrameLength::HalfStep);
    app.world_mut()
        .resource_mut::<MatchControl>()
        .request_pause();
    for _ in 0..3 {
        frame(&mut app);
    }
    assert_eq!(match_state(&app), MatchState::Paused);
    let position = ball_state(&mut app).0;
    for _ in 0..10 {
        frame(&mut app);
        assert_vec_near(drawn_ball(&mut app), position);
    }
}

/// A countdown puts the ball back on the centre spot with `Snap`. Run once
/// on each parity of the half-step alternation, so the teleport falls once on
/// a frame that runs no fixed step, where only the snap prevents a slide.
#[test]
fn a_teleported_ball_is_drawn_at_once_where_it_now_is() {
    for offset in 0..2 {
        let mut app = playing_app(FrameLength::HalfStep);
        for _ in 0..offset {
            frame(&mut app);
        }
        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        frame(&mut app);
        assert_vec_near(drawn_ball(&mut app), Vec2::ZERO);
        let snapped = app
            .world_mut()
            .query_filtered::<(), (With<Ball>, With<Snap>)>()
            .iter(app.world())
            .count();
        assert_eq!(snapped, 0, "the view consumes Snap");
    }
}

#[test]
fn a_paddle_is_drawn_at_the_centre_of_its_rectangle() {
    let mut app = playing_app(FrameLength::Steps(1));
    frame(&mut app);
    let tuning = app.world().resource::<Tuning>().clone();
    let mut paddles = app.world_mut().query::<(&Paddle, &Transform)>();
    let mut count = 0;
    for (paddle, transform) in paddles.iter(app.world()) {
        assert_vec_near(
            transform.translation.truncate(),
            paddle.rect(&tuning).center(),
        );
        count += 1;
    }
    assert_eq!(count, 2);
}
