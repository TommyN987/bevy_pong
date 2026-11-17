//! Where Fog's view puts its band and what the band hides (M2 spec 2.6.3,
//! 2.2.9, R1, R9, R11): one band centred on midfield, as wide as the
//! flight's fog, and inside it the ball and its trail hidden and nothing
//! else. Headless: the core harness plus the plugins that place bodies and
//! the band, which touch only `Transform` and `Visibility`; how the fog
//! looks is judged by eye.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::math::Vec2;
use bevy::transform::components::Transform;
use pong::effects::fog::{FogBand, FogBandPlugin};
use pong::effects::spin::{SpinMark, SpinMarkPlugin};
use pong::effects::wind::{WindArrow, WindArrowsPlugin};
use pong::views::motion::MotionPlugin;
use pong::views::trail::{GHOSTS, TrailGhost, TrailPlugin};
use pong_core::domain::{Controller, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::ball::{Ball, Snap};
use pong_core::features::effects::Sandbox;
use pong_core::features::seats::PaddleIntent;
use pong_core::features::tuning::{EMBEDDED_RON, Tuning};
use pong_core::flow::states::MatchState;
use pong_core::testing::{
    ball_state, boot, enter_match, frame, headless_app, leave_match, match_state, place_ball,
    report_intent, run_until,
};

/// The embedded arena's height.
const ARENA_HEIGHT: f32 = 90.0;

/// A held return count at which every dial is full, whatever return the
/// tuning has it full at.
const FULL_DIAL: u16 = u16::MAX;

/// How wide the embedded tuning makes the fog after `returns` returns. The
/// dial's numbers are the tuning's to change; these tests hold the band to
/// whatever it says.
fn fog_width_after(returns: u16) -> f32 {
    Tuning::from_ron(EMBEDDED_RON)
        .expect("the embedded tuning is valid")
        .effects()
        .fog()
        .width()
        .at(returns)
}

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

/// A match in its first countdown with the plugins that draw the ball, its
/// trail, and the band, and whatever `more` adds. With `forced`, exactly
/// those effects are in play and every flight starts at that return count:
/// forced before the match is entered. Without, the match is Classic as the
/// game plays it, so no fog.
fn app_with(forced: Option<(&[EffectId], u16)>, more: impl FnOnce(&mut App)) -> App {
    let mut app = headless_app(1);
    app.add_plugins((MotionPlugin, TrailPlugin, FogBandPlugin));
    more(&mut app);
    boot(&mut app);
    if let Some((effects, returns)) = forced {
        let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
        sandbox.force_effects(effects);
        sandbox.hold_returns(Some(returns));
    }
    enter_match(&mut app, two_humans());
    frame(&mut app);
    assert_eq!(match_state(&app), MatchState::Countdown);
    app
}

/// Fog alone in play, every flight starting at `returns`. With no Speed
/// beside it the ball flies at the serve speed.
fn foggy_app(returns: u16) -> App {
    app_with(Some((&[EffectId::Fog], returns)), |_| {})
}

/// The band as placed.
#[derive(Debug, Clone, Copy)]
struct Band {
    width: f32,
    at: Vec2,
    drawn_size: Vec2,
    shows: bool,
}

/// The one band of the match.
fn band(app: &mut App) -> Band {
    let mut query = app
        .world_mut()
        .query::<(&FogBand, &Transform, &Visibility)>();
    let (band, transform, visibility) = query.single(app.world()).expect("exactly one band");
    Band {
        width: band.width(),
        at: transform.translation.truncate(),
        drawn_size: transform.scale.truncate(),
        shows: *visibility != Visibility::Hidden,
    }
}

/// One of the bodies the fog may hide, as drawn.
#[derive(Debug, Clone, Copy)]
struct Body {
    x: f32,
    hidden: bool,
}

fn body(transform: &Transform, visibility: Option<&Visibility>) -> Body {
    Body {
        x: transform.translation.x,
        hidden: visibility == Some(&Visibility::Hidden),
    }
}

fn the_ball(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Ball>>()
        .single(app.world())
        .expect("exactly one ball")
}

fn drawn_ball(app: &mut App) -> Body {
    let mut query = app
        .world_mut()
        .query_filtered::<(&Transform, Option<&Visibility>), With<Ball>>();
    let (transform, visibility) = query.single(app.world()).expect("exactly one ball");
    body(transform, visibility)
}

/// The ball's ghosts as drawn, nearest the ball first.
fn drawn_ghosts(app: &mut App) -> Vec<Body> {
    let mut query = app
        .world_mut()
        .query::<(&TrailGhost, &Transform, Option<&Visibility>)>();
    let mut ghosts: Vec<(u8, Body)> = query
        .iter(app.world())
        .map(|(ghost, transform, visibility)| (ghost.back(), body(transform, visibility)))
        .collect();
    ghosts.sort_by_key(|&(back, _)| back);
    assert_eq!(ghosts.len(), usize::from(GHOSTS));
    ghosts.into_iter().map(|(_, body)| body).collect()
}

/// The ball, then its ghosts nearest first.
fn ball_and_trail(app: &mut App) -> Vec<Body> {
    let mut bodies = vec![drawn_ball(app)];
    bodies.extend(drawn_ghosts(app));
    bodies
}

/// Puts the ball at `position` moving at `velocity`: a teleport, so it
/// carries `Snap`, as the core's own teleports do, and is drawn where it
/// now is.
fn teleport_the_ball(app: &mut App, position: Vec2, velocity: Vec2) {
    place_ball(app, position, velocity);
    let ball = the_ball(app);
    app.world_mut().entity_mut(ball).insert(Snap);
}

/// Play begins, and the ball is put left of the band, level at `y` and
/// moving right at the serve speed. It flies in the serve's flight, whose
/// return count the sandbox holds.
fn send_the_ball_across(app: &mut App, y: f32) {
    until(app, MatchState::Playing);
    frame(app);
    teleport_the_ball(app, Vec2::new(-60.0, y), Vec2::new(80.0, 0.0));
    frame(app);
}

/// Runs frames until the ball, sent across, is right of `x`, calling
/// `each_frame` after each.
fn fly_until_right_of(app: &mut App, x: f32, mut each_frame: impl FnMut(&mut App)) {
    let mut frames = 0;
    while ball_state(app).0.x < x {
        frame(app);
        each_frame(app);
        frames += 1;
        assert!(frames < 300, "the ball never crossed");
    }
}

/// A body within this of the band's edge is on neither side for a test.
const AT_THE_EDGE: f32 = 1e-3;

/// A match has one band at midfield, out of sight until a flight has fog:
/// a countdown has no flight.
#[test]
fn the_match_has_one_band_at_midfield_hidden_in_the_countdown() {
    let mut app = foggy_app(FULL_DIAL);
    let band = band(&mut app);
    assert_eq!(band.at, Vec2::ZERO);
    assert!(!band.shows, "{band:?}");
}

/// M2 spec 2.6.3's dial, and R9 on screen: the band is as wide as the
/// flight's fog, from nothing to the tuned maximum, and as tall as the
/// arena.
#[test]
fn the_band_is_as_wide_as_the_flights_fog_and_as_tall_as_the_arena() {
    let widths = [1, 4, 12, FULL_DIAL].map(|returns| (returns, fog_width_after(returns)));
    assert!(
        widths
            .windows(2)
            .all(|pair| pair[0].1 > 0.0 && pair[1].1 > pair[0].1),
        "the embedded dial does not grow over these counts: {widths:?}"
    );
    for (returns, width) in widths {
        let mut app = foggy_app(returns);
        send_the_ball_across(&mut app, 10.0);
        let band = band(&mut app);
        assert!(band.shows, "at {returns} returns: {band:?}");
        assert!(
            (band.width - width).abs() < 1e-4,
            "at {returns} returns: {band:?}"
        );
        assert!(
            (band.drawn_size - Vec2::new(width, ARENA_HEIGHT)).length() < 1e-4,
            "at {returns} returns: {band:?}"
        );
        assert_eq!(band.at, Vec2::ZERO, "at {returns} returns");
    }
}

/// M2 spec 2.6.3's done-condition: the ball and its trail are absent inside
/// the band. A body whose drawn centre is inside the band is hidden, and
/// one outside it shows: the ball and each ghost on its own, so the trail
/// follows the ball in and out. The glow is the ball's own light, so a
/// hidden ball has none.
///
/// R11: over one crossing the ball changes twice, in and out.
#[test]
fn inside_the_band_the_ball_and_its_trail_are_hidden_and_outside_it_they_show() {
    let mut app = foggy_app(FULL_DIAL);
    send_the_ball_across(&mut app, 10.0);
    let half_width = band(&mut app).width / 2.0;
    let bodies = usize::from(GHOSTS) + 1;
    let mut seen_hidden = vec![false; bodies];
    let mut seen_shown_after = vec![false; bodies];
    let mut ball_was_hidden = false;
    let mut ball_changes = 0;
    fly_until_right_of(&mut app, 60.0, |app| {
        let drawn = ball_and_trail(app);
        for (index, body) in drawn.iter().enumerate() {
            let from_the_edge = body.x.abs() - half_width;
            if from_the_edge.abs() < AT_THE_EDGE {
                continue;
            }
            assert_eq!(
                body.hidden,
                from_the_edge < 0.0,
                "body {index}, drawn at {}",
                body.x
            );
            seen_hidden[index] |= body.hidden;
            seen_shown_after[index] |= seen_hidden[index] && !body.hidden;
        }
        ball_changes += usize::from(drawn[0].hidden != ball_was_hidden);
        ball_was_hidden = drawn[0].hidden;
    });
    assert!(seen_hidden.iter().all(|&seen| seen), "{seen_hidden:?}");
    assert!(
        seen_shown_after.iter().all(|&seen| seen),
        "{seen_shown_after:?}"
    );
    assert_eq!(ball_changes, 2, "the ball went in once and came out once");
}

/// M2 spec 2.2.9: fog hides the ball only. With Wind in play beside it,
/// the arrows inside the band keep showing while the ball is hidden there,
/// and so does anything else drawn inside it that is neither the ball nor
/// its trail.
#[test]
fn inside_the_band_winds_arrows_and_whatever_else_is_not_the_ball_stay_visible() {
    let mut app = app_with(Some((&[EffectId::Wind, EffectId::Fog], FULL_DIAL)), |app| {
        app.add_plugins(WindArrowsPlugin);
    });
    let furniture = app
        .world_mut()
        .spawn((Transform::from_xyz(5.0, 5.0, 1.0), Visibility::Inherited))
        .id();
    send_the_ball_across(&mut app, 0.0);
    let half_width = band(&mut app).width / 2.0;
    let mut frames_hidden = 0;
    fly_until_right_of(&mut app, 60.0, |app| {
        if !drawn_ball(app).hidden {
            return;
        }
        frames_hidden += 1;
        let mut arrows = app
            .world_mut()
            .query::<(&WindArrow, &Transform, &Visibility)>();
        let inside: Vec<Visibility> = arrows
            .iter(app.world())
            .filter(|(_, transform, _)| transform.translation.x.abs() < half_width)
            .map(|(_, _, visibility)| *visibility)
            .collect();
        assert!(
            inside.len() >= 4,
            "only {} arrows in the band",
            inside.len()
        );
        assert!(
            inside.iter().all(|&shown| shown != Visibility::Hidden),
            "an arrow in the band is hidden: {inside:?}"
        );
        assert_eq!(
            app.world().get::<Visibility>(furniture),
            Some(&Visibility::Inherited)
        );
    });
    assert!(
        frames_hidden > 30,
        "the ball was hidden {frames_hidden} frames"
    );
}

/// Spin's mark is the ball's child and inherits its visibility, so the fog
/// that hides the ball hides the mark, and the mark's own view goes on
/// asking only whether the flight is spun.
#[test]
fn in_the_band_a_spun_balls_mark_is_left_to_inherit_the_balls_hiding() {
    let mut app = app_with(Some((&[EffectId::Spin, EffectId::Fog], FULL_DIAL)), |app| {
        app.add_plugins(SpinMarkPlugin);
    });
    until(&mut app, MatchState::Playing);
    frame(&mut app);
    // A level ball met by the right paddle as it moves up: a spun return.
    report_intent(&mut app, Seat::Right, PaddleIntent::Up);
    teleport_the_ball(&mut app, Vec2::new(72.0, 1.5), Vec2::new(200.0, 0.0));
    frame(&mut app);
    report_intent(&mut app, Seat::Right, PaddleIntent::Still);
    assert!(
        ball_state(&mut app).1.x < 0.0,
        "the right paddle returned the ball"
    );
    let mut frames = 0;
    while drawn_ball(&mut app).x > 20.0 {
        frame(&mut app);
        frames += 1;
        assert!(frames < 300, "the ball never reached the band");
    }
    assert!(drawn_ball(&mut app).hidden, "the ball is in the band");
    let ball = the_ball(&mut app);
    let mut marks = app
        .world_mut()
        .query_filtered::<(&ChildOf, &Visibility), With<SpinMark>>();
    let (child_of, visibility) = marks.single(app.world()).expect("exactly one mark");
    assert_eq!(child_of.parent(), ball);
    assert_eq!(*visibility, Visibility::Inherited);
}

/// R9: mild at zero. With the dial at zero there is no band, and the ball
/// crosses midfield in plain sight.
#[test]
fn at_dial_zero_no_band_shows_and_the_ball_crosses_in_plain_sight() {
    let mut app = foggy_app(0);
    send_the_ball_across(&mut app, 10.0);
    fly_until_right_of(&mut app, 60.0, |app| {
        assert!(!band(app).shows);
        assert!(ball_and_trail(app).iter().all(|body| !body.hidden));
    });
}

/// The band is Fog's: in a match without it, nothing shows and nothing is
/// hidden.
#[test]
fn without_fog_in_play_no_band_shows_and_nothing_is_hidden() {
    let mut app = app_with(None, |_| {});
    send_the_ball_across(&mut app, 10.0);
    fly_until_right_of(&mut app, 60.0, |app| {
        assert!(!band(app).shows);
        assert!(ball_and_trail(app).iter().all(|body| !body.hidden));
    });
}

/// R1, as the game plays it: the cause is on screen before the consequence.
/// A serve crosses with no band. The first return brings one, in the frame
/// of the hit, while the ball is still at the paddle and in sight.
#[test]
fn the_band_comes_with_the_first_return_before_the_ball_reaches_it() {
    let mut app = app_with(Some((&[EffectId::Fog], 0)), |_| {});
    app.world_mut().resource_mut::<Sandbox>().hold_returns(None);
    until(&mut app, MatchState::Playing);
    for _ in 0..5 {
        frame(&mut app);
        assert!(!band(&mut app).shows, "a serve has no fog");
    }
    teleport_the_ball(&mut app, Vec2::new(72.0, 0.0), Vec2::new(200.0, 0.0));
    frame(&mut app);
    assert!(
        ball_state(&mut app).1.x < 0.0,
        "the right paddle returned the ball"
    );
    let band = band(&mut app);
    let after_one_return = fog_width_after(1);
    assert!(
        after_one_return > 0.0,
        "the embedded dial has not moved by then"
    );
    assert!(band.shows, "{band:?}");
    assert!((band.width - after_one_return).abs() < 1e-4, "{band:?}");
    let ball = drawn_ball(&mut app);
    assert!(!ball.hidden && ball.x > 60.0, "{ball:?}");
}

/// M2 spec 2.4.3 and R11: the band changes only where a flight starts or a
/// rally ends. It shows, as wide, in every frame of a flight, and is gone
/// by the next countdown, where the ball waits on the centre spot in plain
/// sight.
#[test]
fn the_band_holds_for_the_whole_flight_and_goes_with_the_rally() {
    let mut app = foggy_app(FULL_DIAL);
    let full_width = fog_width_after(FULL_DIAL);
    send_the_ball_across(&mut app, 30.0);
    let mut frames = 0;
    fly_until_right_of(&mut app, 60.0, |app| {
        let band = band(app);
        assert!(
            band.shows && (band.width - full_width).abs() < 1e-4,
            "frame {frames} of the flight: {band:?}"
        );
        frames += 1;
    });
    assert!(frames > 60, "the flight lasted only {frames} frames");

    // At 30 the ball passes over the right paddle, which waits at the centre.
    until(&mut app, MatchState::Countdown);
    frame(&mut app);
    assert!(!band(&mut app).shows, "in the next countdown");
    assert!(
        ball_and_trail(&mut app)
            .iter()
            .all(|body| !body.hidden && body.x.abs() < 1e-4),
        "the ball waits on the centre spot in plain sight"
    );
}

/// The band lives for the match: none is left behind, and a second match
/// has one again.
#[test]
fn leaving_the_match_takes_the_band_with_it() {
    let mut app = foggy_app(FULL_DIAL);
    let bands = |app: &mut App| {
        app.world_mut()
            .query_filtered::<Entity, With<FogBand>>()
            .iter(app.world())
            .count()
    };
    assert_eq!(bands(&mut app), 1);
    leave_match(&mut app);
    assert_eq!(bands(&mut app), 0);
    enter_match(&mut app, two_humans());
    frame(&mut app);
    assert_eq!(bands(&mut app), 1);
}
