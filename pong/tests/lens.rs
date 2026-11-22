//! Where Lens's view puts its lenses (M2 spec 2.6.5, 2.2.9, R1, R9, R11):
//! one on the floor for every lens the core's Lens pulls with, each where
//! the core has it, there from the frame its flight starts and gone at the
//! countdown. Headless: the core harness plus the plugins that place the
//! bodies, the lenses, and the fog's band, which touch only `Transform` and
//! `Visibility`; how a lens looks is judged by eye.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::math::Vec2;
use bevy::transform::components::Transform;
use pong::effects::fog::{FogBand, FogBandPlugin};
use pong::effects::lens::{APPEAR_SECONDS, FloorLens, FloorLensesPlugin};
use pong::views::motion::MotionPlugin;
use pong::views::trail::TrailPlugin;
use pong_core::domain::{Controller, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::ball::{Ball, Snap};
use pong_core::features::effects::lens::{Lens, PlacedLens};
use pong_core::features::effects::{ActiveEffects, Sandbox};
use pong_core::features::tuning::{EMBEDDED_RON, Tuning};
use pong_core::flow::FIXED_HZ;
use pong_core::flow::states::MatchState;
use pong_core::kernel::flight::FlightStart;
use pong_core::testing::{
    ball_state, boot, enter_match, frame, headless_app, leave_match, match_state, place_ball,
    run_until,
};

/// A held return count at which every dial is full, whatever return the
/// tuning has it full at.
const FULL_DIAL: u16 = u16::MAX;

fn tuning() -> Tuning {
    Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
}

/// The first return at which the embedded dial holds `count` drawn lenses.
/// The dial's numbers are the tuning's to change; these tests hold the floor
/// to whatever it says.
fn the_return_that_brings(count: u16) -> u16 {
    (0..=u16::MAX)
        .find(|&returns| tuning().effects().lens().count().after(returns) >= count)
        .expect("the embedded dial reaches that count")
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

/// A match from `seed` in its first countdown, with the plugins that draw
/// the ball, its trail, the lenses, and the fog's band. With `forced`,
/// exactly those effects are in play and every flight starts at that return
/// count: forced before the match is entered. Without, the match is Classic
/// as the game plays it, so no lens.
fn app_from(seed: u64, forced: Option<(&[EffectId], u16)>) -> App {
    let mut app = headless_app(seed);
    app.add_plugins((MotionPlugin, TrailPlugin, FogBandPlugin, FloorLensesPlugin));
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

/// Lens alone in play, every flight starting at `returns`. With no Speed
/// beside it the ball flies at the serve speed.
fn app_among_lenses(returns: u16) -> App {
    app_from(1, Some((&[EffectId::Lens], returns)))
}

/// The lenses the core's Lens pulls with in the rally `app` is in, during a
/// flight that began at `returns`.
fn the_cores_lenses(app: &App, returns: u16) -> Vec<PlacedLens> {
    let rally = app.world().resource::<ActiveEffects>().rally_seed();
    Lens::new(&tuning(), rally).placed(&FlightStart {
        returns,
        hit: None,
        returns_held: false,
    })
}

/// One lens as drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drawn {
    entity: Entity,
    at: Vec2,
    size: Vec2,
    hidden: bool,
}

/// The lenses on the floor as drawn, in the order the core has them.
fn drawn_lenses(app: &mut App) -> Vec<Drawn> {
    let mut query = app
        .world_mut()
        .query::<(Entity, &FloorLens, &Transform, Option<&Visibility>)>();
    let mut lenses: Vec<(usize, Drawn)> = query
        .iter(app.world())
        .map(|(entity, lens, transform, visibility)| {
            let drawn = Drawn {
                entity,
                at: transform.translation.truncate(),
                size: transform.scale.truncate(),
                hidden: visibility == Some(&Visibility::Hidden),
            };
            (lens.index(), drawn)
        })
        .collect();
    lenses.sort_by_key(|&(index, _)| index);
    for (place, (index, _)) in lenses.iter().enumerate() {
        assert_eq!(*index, place, "the floor's lenses are numbered from zero");
    }
    lenses.into_iter().map(|(_, drawn)| drawn).collect()
}

/// Frames in which a new lens has grown to its size, and one to spare.
fn frames_to_appear() -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a quarter second of frames is a small positive number"
    )]
    let frames = (f64::from(APPEAR_SECONDS) * FIXED_HZ).ceil() as u32;
    frames + 1
}

/// Play begins and its first flight with it: frames until that flight's
/// lenses lie on the floor, new and small. A serve is launched a step or
/// two into play.
fn until_the_lenses_lie(app: &mut App) -> Vec<Drawn> {
    until(app, MatchState::Playing);
    for _ in 0..5 {
        let lenses = drawn_lenses(app);
        if !lenses.is_empty() {
            return lenses;
        }
        frame(app);
    }
    panic!("play began and no lens came with its first flight");
}

/// [`until_the_lenses_lie`], then frames until each has grown to its size.
fn until_the_lenses_have_grown(app: &mut App) {
    until_the_lenses_lie(app);
    for _ in 0..frames_to_appear() {
        frame(app);
    }
}

fn assert_near(actual: Vec2, expected: Vec2, what: &str) {
    assert!(
        (actual - expected).length() < 1e-4,
        "{what}: expected {expected}, got {actual}"
    );
}

/// Puts the ball at `position` moving at `velocity`: a teleport, so it
/// carries `Snap`, as the core's own teleports do, and is drawn where it
/// now is.
fn teleport_the_ball(app: &mut App, position: Vec2, velocity: Vec2) {
    place_ball(app, position, velocity);
    let ball = app
        .world_mut()
        .query_filtered::<Entity, With<Ball>>()
        .single(app.world())
        .expect("exactly one ball");
    app.world_mut().entity_mut(ball).insert(Snap);
}

/// Whether the fog hides the ball as drawn.
fn the_ball_is_hidden(app: &mut App) -> bool {
    let mut query = app
        .world_mut()
        .query_filtered::<Option<&Visibility>, With<Ball>>();
    query.single(app.world()).expect("exactly one ball") == Some(&Visibility::Hidden)
}

/// R9: absent at zero, and a countdown has no flight. The floor is empty
/// until a flight has lenses.
#[test]
fn the_floor_is_empty_in_the_countdown_and_under_a_flight_with_no_lens() {
    let mut waiting = app_among_lenses(FULL_DIAL);
    assert!(drawn_lenses(&mut waiting).is_empty(), "in the countdown");

    let mut app = app_among_lenses(0);
    until(&mut app, MatchState::Playing);
    for _ in 0..30 {
        frame(&mut app);
        assert!(drawn_lenses(&mut app).is_empty(), "with the dial at zero");
    }
}

/// M2 spec 2.6.5 and R1: what is drawn is what pulls. Every lens of the
/// flight is on the floor, a drawn lens and its mirrored copy alike, each
/// where the core has it and as large.
#[test]
fn the_floor_shows_every_lens_of_the_flight_where_the_core_has_it() {
    let mut app = app_among_lenses(FULL_DIAL);
    until_the_lenses_have_grown(&mut app);
    let expected = the_cores_lenses(&app, FULL_DIAL);
    assert!(!expected.is_empty(), "the embedded dial brings no lens");
    let drawn = drawn_lenses(&mut app);
    assert_eq!(drawn.len(), expected.len());
    for (drawn, lens) in drawn.iter().zip(&expected) {
        assert_near(drawn.at, lens.centre, "where it lies");
        assert_near(drawn.size, Vec2::splat(lens.radius), "how large it is");
        assert!(!drawn.hidden);
    }
    for pair in drawn.chunks(2) {
        assert_near(
            pair[1].at,
            Vec2::new(-pair[0].at.x, pair[0].at.y),
            "the copy, mirrored across midfield",
        );
    }
}

/// R1, as the game plays it: the cause is on screen before the consequence.
/// The flight before a lens is due crosses an empty floor. The return that
/// brings the lens puts it on the floor, with its copy, in the frame of the
/// hit, while the ball is still at the paddle.
#[test]
fn a_lens_comes_with_the_return_that_brings_it_before_the_ball_reaches_it() {
    let first_arrival = the_return_that_brings(1);
    let mut app = app_among_lenses(first_arrival - 1);
    until(&mut app, MatchState::Playing);
    for _ in 0..5 {
        frame(&mut app);
        assert!(drawn_lenses(&mut app).is_empty(), "a return too soon");
    }
    // The rally counts again, on from the held count: the next return is
    // the one that brings the first lens.
    app.world_mut().resource_mut::<Sandbox>().hold_returns(None);
    teleport_the_ball(&mut app, Vec2::new(72.0, 0.0), Vec2::new(200.0, 0.0));
    frame(&mut app);
    let (position, velocity) = ball_state(&mut app);
    assert!(velocity.x < 0.0, "the right paddle returned the ball");
    assert!(position.x > 60.0, "the ball is still at the paddle");
    let expected = the_cores_lenses(&app, first_arrival);
    assert_eq!(expected.len(), 2, "one lens and its copy");
    let drawn = drawn_lenses(&mut app);
    assert_eq!(drawn.len(), 2);
    for (drawn, lens) in drawn.iter().zip(&expected) {
        assert_near(drawn.at, lens.centre, "where it lies");
        assert!(
            lens.centre.x + lens.radius < position.x,
            "the lens at {lens:?} is not ahead of the ball at {position}"
        );
    }
}

/// R11: a lens arrives once and calmly. It grows from small to its size
/// within the time it takes to appear, never shrinking on the way, and then
/// holds its size and its place.
#[test]
fn a_new_lens_grows_to_its_size_without_shrinking_and_holds_it() {
    let mut app = app_among_lenses(FULL_DIAL);
    let radius = tuning().effects().lens().radius();
    let first = until_the_lenses_lie(&mut app);
    assert!(
        first.iter().all(|lens| lens.size.x < radius / 2.0),
        "a new lens starts small: {first:?}"
    );
    let mut before = first;
    for _ in 0..frames_to_appear() {
        frame(&mut app);
        let now = drawn_lenses(&mut app);
        for (now, before) in now.iter().zip(&before) {
            assert_eq!(now.entity, before.entity, "the same lens, not a new one");
            assert!(now.size.x >= before.size.x, "{before:?} then {now:?}");
            assert!(now.size.x <= radius + 1e-4, "{now:?} is over its size");
            assert_near(now.at, before.at, "where it lies");
            assert!((now.size.x - now.size.y).abs() < 1e-6, "{now:?} is round");
        }
        before = now;
    }
    let grown = before;
    for _ in 0..30 {
        frame(&mut app);
        assert_eq!(drawn_lenses(&mut app), grown, "a grown lens holds");
    }
}

/// M2 spec 2.6.5 and 2.4.3: a lens stays until the point. The floor holds
/// through the rally's flights, and is empty again by the next countdown.
#[test]
fn the_lenses_hold_through_the_rally_and_go_at_the_countdown() {
    let mut app = app_among_lenses(FULL_DIAL);
    until_the_lenses_have_grown(&mut app);
    let floor = drawn_lenses(&mut app);
    assert!(!floor.is_empty(), "the embedded dial brings no lens");
    // A return starts a second flight of the same rally.
    teleport_the_ball(&mut app, Vec2::new(72.0, 0.0), Vec2::new(200.0, 0.0));
    frame(&mut app);
    assert!(ball_state(&mut app).1.x < 0.0, "the right paddle returned");
    for _ in 0..20 {
        frame(&mut app);
        assert_eq!(drawn_lenses(&mut app), floor, "in the rally's next flight");
    }
    // Past the left paddle, which waits at the centre: a point.
    teleport_the_ball(&mut app, Vec2::new(-60.0, 40.0), Vec2::new(-200.0, 0.0));
    until(&mut app, MatchState::Countdown);
    frame(&mut app);
    assert!(drawn_lenses(&mut app).is_empty(), "in the next countdown");
}

/// A match from the first seed whose rally lays a lens wholly inside the
/// widest fog, with Fog and Lens in play at full dial.
fn app_with_a_lens_in_the_fog() -> (App, f32) {
    let numbers = tuning().effects();
    let half_band = numbers.fog().max_width() / 2.0;
    let reach = numbers.lens().radius();
    (1..200)
        .map(|seed| app_from(seed, Some((&[EffectId::Fog, EffectId::Lens], FULL_DIAL))))
        .find(|app| {
            the_cores_lenses(app, FULL_DIAL)
                .iter()
                .any(|lens| lens.centre.x.abs() + reach < half_band)
        })
        .map(|app| (app, half_band))
        .expect("some rally of two hundred lays a lens inside the fog")
}

/// M2 spec 2.2.9's done-condition, the lens's half (M2 plan 8.1): fog hides
/// the ball only. A lens inside the fog band stays visible, in every frame
/// the ball is hidden there.
#[test]
fn a_lens_inside_the_fog_band_stays_visible_while_the_ball_there_is_hidden() {
    let (mut app, half_band) = app_with_a_lens_in_the_fog();
    until(&mut app, MatchState::Playing);
    frame(&mut app);
    let band = {
        let mut bands = app.world_mut().query::<&FogBand>();
        bands.single(app.world()).expect("exactly one band").width()
    };
    assert!((band / 2.0 - half_band).abs() < 1e-4, "the fog is full");
    teleport_the_ball(&mut app, Vec2::new(-60.0, 0.0), Vec2::new(80.0, 0.0));
    let mut frames_hidden = 0;
    let mut frames = 0;
    while ball_state(&mut app).0.x < 60.0 {
        frame(&mut app);
        frames += 1;
        assert!(frames < 300, "the ball never crossed");
        if !the_ball_is_hidden(&mut app) {
            continue;
        }
        frames_hidden += 1;
        let inside: Vec<Drawn> = drawn_lenses(&mut app)
            .into_iter()
            .filter(|lens| lens.at.x.abs() < half_band)
            .collect();
        assert!(!inside.is_empty(), "no lens in the band");
        assert!(
            inside.iter().all(|lens| !lens.hidden),
            "a lens in the band is hidden: {inside:?}"
        );
    }
    assert!(
        frames_hidden > 30,
        "the ball was hidden {frames_hidden} frames"
    );
}

/// The lenses are Lens's: in a match without it the floor stays empty.
#[test]
fn without_lens_in_play_the_floor_stays_empty() {
    let mut app = app_from(1, None);
    until(&mut app, MatchState::Playing);
    for _ in 0..30 {
        frame(&mut app);
        let floor = drawn_lenses(&mut app);
        assert!(floor.is_empty(), "without Lens: {floor:?}");
    }
}

/// The lenses live with their match: none is left behind.
#[test]
fn leaving_the_match_takes_the_lenses_with_it() {
    let mut app = app_among_lenses(FULL_DIAL);
    until_the_lenses_have_grown(&mut app);
    assert!(
        !drawn_lenses(&mut app).is_empty(),
        "the embedded dial brings no lens"
    );
    leave_match(&mut app);
    let left_behind = drawn_lenses(&mut app);
    assert!(left_behind.is_empty(), "{left_behind:?}");
}
