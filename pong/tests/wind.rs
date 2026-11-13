//! Where Wind's view draws its arrows (M2 spec 2.6.2, R1, R11): small
//! arrowheads across the arena that point and drift the way the wind blows,
//! from the countdown on, faster as the dial rises. Headless: the core
//! harness plus the plugin that places the arrows, which touches only
//! `Transform` and `Visibility`; how an arrow looks is judged by eye.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::camera::visibility::Visibility;
use bevy::math::{Vec2, Vec3};
use bevy::transform::components::Transform;
use pong::effects::wind::{WindArrow, WindArrowsPlugin};
use pong_core::domain::{Controller, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::effects::wind::WindDirection;
use pong_core::features::effects::{ActiveEffects, Sandbox};
use pong_core::features::rules::MatchControl;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::MatchState;
use pong_core::testing::{
    ball_state, boot, enter_match, frame, headless_app, match_state, place_ball, run_until,
};

fn two_humans() -> MatchConfig {
    MatchConfig {
        left: Controller::Human(Seat::Left),
        right: Controller::Human(Seat::Right),
        players: SeatPlayers::GUESTS,
        points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        mode: Mode::Classic,
    }
}

/// A match in its first countdown, from `seed`, with the arrows' plugin and
/// nothing forced: Classic, so no wind.
fn calm_app(seed: u64) -> App {
    let mut app = headless_app(seed);
    app.add_plugins(WindArrowsPlugin);
    boot(&mut app);
    enter_match(&mut app, two_humans());
    frame(&mut app);
    app
}

/// A match in its first countdown, from `seed`, with Wind alone in play and
/// every flight starting at `returns`: forced before the match is entered.
fn windy_app(seed: u64, returns: u16) -> App {
    let mut app = headless_app(seed);
    app.add_plugins(WindArrowsPlugin);
    boot(&mut app);
    let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
    sandbox.force_effects(&[EffectId::Wind]);
    sandbox.hold_returns(Some(returns));
    enter_match(&mut app, two_humans());
    frame(&mut app);
    assert_eq!(match_state(&app), MatchState::Countdown);
    app
}

fn until(app: &mut App, state: MatchState) {
    run_until(
        app,
        |app| match_state(app) == state,
        1_000,
        "the state never came",
    );
}

/// One arrow as drawn: where, which way its tip points along y, and whether
/// it shows.
#[derive(Debug, Clone, Copy)]
struct Drawn {
    at: Vec2,
    tip_y: f32,
    shows: bool,
}

/// Every arrow, in a stable order.
fn arrows(app: &mut App) -> Vec<Drawn> {
    let mut query = app
        .world_mut()
        .query::<(&WindArrow, &Transform, &Visibility)>();
    let mut drawn: Vec<(u8, Drawn)> = query
        .iter(app.world())
        .map(|(arrow, transform, visibility)| {
            // The unit arrow's tip is at +y; where the transform turns it.
            let tip = transform.rotation * (transform.scale * Vec3::Y);
            (
                arrow.index(),
                Drawn {
                    at: transform.translation.truncate(),
                    tip_y: tip.y,
                    shows: *visibility != Visibility::Hidden,
                },
            )
        })
        .collect();
    drawn.sort_by_key(|(index, _)| *index);
    drawn.into_iter().map(|(_, arrow)| arrow).collect()
}

/// How far each arrow moved along y over one frame, leaving out the arrows
/// that wrapped from one wall to the other in it.
fn drift_in_one_frame(app: &mut App) -> Vec<f32> {
    let before = arrows(app);
    frame(app);
    let after = arrows(app);
    before
        .iter()
        .zip(&after)
        .map(|(before, after)| after.at.y - before.at.y)
        .filter(|moved| moved.abs() < 20.0)
        .collect()
}

fn shown_direction(app: &App) -> WindDirection {
    WindDirection::of(app.world().resource::<ActiveEffects>().rally_seed())
}

/// R1 needs the arrows only while there is a wind to explain.
#[test]
fn without_wind_in_play_no_arrow_shows() {
    let mut app = calm_app(1);
    let drawn = arrows(&mut app);
    assert!(drawn.len() >= 8, "only {} arrows", drawn.len());
    assert!(drawn.iter().all(|arrow| !arrow.shows), "{drawn:?}");
    until(&mut app, MatchState::Playing);
    frame(&mut app);
    assert!(arrows(&mut app).iter().all(|arrow| !arrow.shows));
}

/// M2 spec 2.6.2: the direction is shown in the countdown. R11: nothing
/// flashes; an arrow that shows keeps showing, through the serve and play.
#[test]
fn with_wind_in_play_every_arrow_shows_from_the_countdown_on_and_never_blinks() {
    let mut app = windy_app(1, 12);
    let mut frames_in_play = 0;
    for frames in 0..400 {
        let drawn = arrows(&mut app);
        assert!(
            drawn.iter().all(|arrow| arrow.shows),
            "an arrow hid in frame {frames}, in {:?}",
            match_state(&app)
        );
        frames_in_play += u32::from(match_state(&app) == MatchState::Playing);
        frame(&mut app);
    }
    assert!(frames_in_play > 0, "the run never left the countdown");
}

/// R1, end to end: the cause is on screen before the consequence. In the
/// countdown the arrows point and drift one way; that is the way the
/// rally's seed shows; and once play begins, that is the way a level ball
/// is carried. Over a few seeds both directions come up.
#[test]
fn in_the_countdown_the_arrows_point_and_drift_the_way_the_ball_is_then_carried() {
    let mut seen = Vec::new();
    for seed in 1..=6 {
        let mut app = windy_app(seed, 12);
        let shown = shown_direction(&app);
        let drawn = arrows(&mut app);
        assert!(
            drawn.iter().all(|arrow| arrow.tip_y * shown.sign() > 0.0),
            "seed {seed}: the arrows do not point {shown:?}: {drawn:?}"
        );
        let drift = drift_in_one_frame(&mut app);
        assert!(
            !drift.is_empty() && drift.iter().all(|moved| moved * shown.sign() > 0.0),
            "seed {seed}: the arrows do not drift {shown:?}: {drift:?}"
        );

        until(&mut app, MatchState::Playing);
        frame(&mut app);
        place_ball(&mut app, Vec2::new(-60.0, 0.0), Vec2::new(150.0, 0.0));
        for _ in 0..20 {
            frame(&mut app);
        }
        let carried = ball_state(&mut app).0.y;
        assert!(
            carried * shown.sign() > 1.0,
            "seed {seed}: shown {shown:?}, and the ball was carried to {carried}"
        );
        seen.push(shown);
    }
    assert!(seen.contains(&WindDirection::Up) && seen.contains(&WindDirection::Down));
}

/// The next rally's wind is on screen before its countdown ends, after
/// every point: the arrows follow the rally's seed, not the first one.
#[test]
fn after_each_point_the_arrows_turn_to_the_next_rallys_wind() {
    let mut app = windy_app(1, 12);
    for point in 0..6 {
        until(&mut app, MatchState::Playing);
        // Out over the right goal line, above the paddle.
        place_ball(&mut app, Vec2::new(70.0, 30.0), Vec2::new(250.0, 0.0));
        until(&mut app, MatchState::Countdown);
        frame(&mut app);
        let shown = shown_direction(&app);
        let drift = drift_in_one_frame(&mut app);
        assert!(
            arrows(&mut app)
                .iter()
                .all(|arrow| arrow.tip_y * shown.sign() > 0.0)
                && drift.iter().all(|moved| moved * shown.sign() > 0.0),
            "after point {point} the arrows do not show {shown:?}"
        );
    }
}

/// R9 on screen: the dial is seen. The arrows drift at a breeze while the
/// dial is at zero, as in every countdown, and faster once it is full.
#[test]
fn the_arrows_drift_faster_at_full_dial_than_in_the_countdown() {
    let mut app = windy_app(1, 12);
    let speed = |drift: Vec<f32>| drift.iter().map(|moved| moved.abs()).sum::<f32>() / 10.0;
    let in_countdown = {
        let drift = drift_in_one_frame(&mut app);
        assert!(drift.len() >= 10, "{drift:?}");
        speed(drift[..10].to_vec())
    };
    until(&mut app, MatchState::Playing);
    for _ in 0..3 {
        frame(&mut app);
    }
    let at_full_dial = {
        let drift = drift_in_one_frame(&mut app);
        assert!(drift.len() >= 10, "{drift:?}");
        speed(drift[..10].to_vec())
    };
    assert!(
        in_countdown > 0.0,
        "the arrows stand still in the countdown"
    );
    assert!(
        at_full_dial > 2.0 * in_countdown,
        "{at_full_dial} a frame at full dial, {in_countdown} in the countdown"
    );
}

/// As the ball's view: while play is paused nothing on the field moves.
#[test]
fn while_play_is_paused_the_arrows_hold_still() {
    let mut app = windy_app(1, 12);
    until(&mut app, MatchState::Playing);
    app.world_mut()
        .resource_mut::<MatchControl>()
        .request_pause();
    until(&mut app, MatchState::Paused);
    frame(&mut app);
    let drift = drift_in_one_frame(&mut app);
    assert!(
        drift.iter().all(|moved| moved.abs() < 1e-6),
        "the arrows drifted through a pause: {drift:?}"
    );
    assert!(arrows(&mut app).iter().all(|arrow| arrow.shows));
}

#[test]
fn the_arrows_stay_inside_the_arena_however_long_the_wind_blows() {
    let mut app = windy_app(1, 12);
    let half = app.world().resource::<Tuning>().arena().half_extents();
    until(&mut app, MatchState::Playing);
    for _ in 0..600 {
        frame(&mut app);
        for arrow in arrows(&mut app) {
            assert!(
                arrow.at.x.abs() <= half.x && arrow.at.y.abs() <= half.y,
                "an arrow left the arena: {arrow:?}"
            );
        }
    }
}
