//! Where Blocks' view stands its blocks (M2 spec 2.6.6, 2.2.6.1, R1, R9,
//! R11): one on the floor for every block the ball bounces off, each where
//! the core has it, as large and as tilted, there at its full size from
//! the frame its flight starts and gone at the countdown; and shown as
//! spent while the flight has spent its bounces. Headless: the core harness
//! plus the plugins that place the bodies and the blocks, which touch only
//! `Transform`; how a block looks is judged by eye.

use std::num::NonZeroU8;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::math::{Quat, Vec2};
use bevy::transform::components::Transform;
use pong::effects::blocks::{APPEAR_SECONDS, FloorBlock, FloorBlocksPlugin};
use pong::views::motion::MotionPlugin;
use pong::views::trail::TrailPlugin;
use pong_core::domain::{Controller, EffectId, MatchConfig, Mode, Seat, SeatPlayers};
use pong_core::features::ball::{Ball, Snap};
use pong_core::features::effects::blocks::Blocks;
use pong_core::features::effects::{ActiveEffects, BallFlight, Sandbox};
use pong_core::features::tuning::{EMBEDDED_RON, Tuning};
use pong_core::flow::FIXED_HZ;
use pong_core::flow::states::MatchState;
use pong_core::kernel::flight::FlightStart;
use pong_core::kernel::obstacle::Obstacle;
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

/// The first return at which the embedded dial holds `count` drawn blocks.
/// The dial's numbers are the tuning's to change; these tests hold the
/// floor to whatever it says.
fn the_return_that_brings(count: u16) -> u16 {
    (0..=u16::MAX)
        .find(|&returns| tuning().effects().blocks().count().after(returns) >= count)
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
/// the ball, its trail, and the blocks. With `forced`, exactly those
/// effects are in play and every flight starts at that return count:
/// forced before the match is entered. Without, the match is Classic as
/// the game plays it, so no block.
fn app_from(seed: u64, forced: Option<(&[EffectId], u16)>) -> App {
    let mut app = headless_app(seed);
    app.add_plugins((MotionPlugin, TrailPlugin, FloorBlocksPlugin));
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

/// Blocks alone in play, every flight starting at `returns`. With no Speed
/// beside it the ball flies at the serve speed.
fn app_among_blocks(returns: u16) -> App {
    app_from(1, Some((&[EffectId::Blocks], returns)))
}

/// The blocks the ball bounces off in the rally `app` is in, during a
/// flight that began at `returns`.
fn the_cores_blocks(app: &App, returns: u16) -> Vec<Obstacle> {
    let rally = app.world().resource::<ActiveEffects>().rally_seed();
    Blocks::new(&tuning(), rally).placed(&FlightStart {
        returns,
        hit: None,
        returns_held: false,
    })
}

/// One block as drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drawn {
    entity: Entity,
    at: Vec2,
    size: Vec2,
    turned: Quat,
    /// How new it shows: one in its first frame, nothing once settled.
    fresh: f32,
    spent: bool,
}

/// The blocks on the floor as drawn, in the order the core has them.
fn drawn_blocks(app: &mut App) -> Vec<Drawn> {
    let mut query = app.world_mut().query::<(Entity, &FloorBlock, &Transform)>();
    let mut blocks: Vec<(usize, Drawn)> = query
        .iter(app.world())
        .map(|(entity, block, transform)| {
            let drawn = Drawn {
                entity,
                at: transform.translation.truncate(),
                size: transform.scale.truncate(),
                turned: transform.rotation,
                fresh: block.freshness(),
                spent: block.spent(),
            };
            (block.index(), drawn)
        })
        .collect();
    blocks.sort_by_key(|&(index, _)| index);
    for (place, (index, _)) in blocks.iter().enumerate() {
        assert_eq!(*index, place, "the floor's blocks are numbered from zero");
    }
    blocks.into_iter().map(|(_, drawn)| drawn).collect()
}

/// Frames in which a new block has settled, and one to spare.
fn frames_to_settle() -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a quarter second of frames is a small positive number"
    )]
    let frames = (f64::from(APPEAR_SECONDS) * FIXED_HZ).ceil() as u32;
    frames + 1
}

/// Play begins and its first flight with it: frames until that flight's
/// blocks stand on the floor. A serve is launched a step or two into play.
fn until_the_blocks_stand(app: &mut App) -> Vec<Drawn> {
    until(app, MatchState::Playing);
    for _ in 0..5 {
        let blocks = drawn_blocks(app);
        if !blocks.is_empty() {
            return blocks;
        }
        frame(app);
    }
    panic!("play began and no block came with its first flight");
}

/// [`until_the_blocks_stand`], then frames until each has settled.
fn until_the_blocks_have_settled(app: &mut App) {
    until_the_blocks_stand(app);
    for _ in 0..frames_to_settle() {
        frame(app);
    }
}

fn assert_near(actual: Vec2, expected: Vec2, what: &str) {
    assert!(
        (actual - expected).length() < 1e-4,
        "{what}: expected {expected}, got {actual}"
    );
}

/// Asserts `drawn` shows `block`: on its centre, at its full size, turned
/// as it is tilted.
fn assert_shows(drawn: &Drawn, block: &Obstacle) {
    assert_near(drawn.at, block.centre(), "where it stands");
    assert_near(drawn.size, 2.0 * block.half_size(), "how large it is");
    let off = drawn
        .turned
        .angle_between(Quat::from_rotation_z(block.tilt()));
    assert!(off < 1e-3, "turned {off} radians off its tilt");
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

/// R9: absent at zero, and a countdown has no flight. The floor is empty
/// until a flight has blocks.
#[test]
fn the_floor_is_empty_in_the_countdown_and_under_a_flight_with_no_block() {
    let mut waiting = app_among_blocks(FULL_DIAL);
    assert!(drawn_blocks(&mut waiting).is_empty(), "in the countdown");

    let mut app = app_among_blocks(0);
    until(&mut app, MatchState::Playing);
    for _ in 0..30 {
        frame(&mut app);
        assert!(drawn_blocks(&mut app).is_empty(), "with the dial at zero");
    }
}

/// M2 spec 2.6.6 and R1: what is drawn is what the ball bounces off.
/// Every block of the flight is on the floor, a drawn block and its
/// mirrored copy alike, each where the core has it, as large, and as
/// tilted.
#[test]
fn the_floor_shows_every_block_of_the_flight_as_the_core_has_it() {
    let mut app = app_among_blocks(FULL_DIAL);
    until_the_blocks_have_settled(&mut app);
    let expected = the_cores_blocks(&app, FULL_DIAL);
    assert!(!expected.is_empty(), "the embedded dial brings no block");
    let drawn = drawn_blocks(&mut app);
    assert_eq!(drawn.len(), expected.len());
    for (drawn, block) in drawn.iter().zip(&expected) {
        assert_shows(drawn, block);
        assert!(!drawn.spent);
    }
    for pair in drawn.chunks(2) {
        assert_near(
            pair[1].at,
            Vec2::new(-pair[0].at.x, pair[0].at.y),
            "the copy, mirrored across midfield",
        );
    }
}

/// R1, as the game plays it: the cause is on screen before the
/// consequence. The flight before a block is due crosses an empty floor.
/// The return that brings the block puts it on the floor, with its copy,
/// in the frame of the hit, while the ball is still at the paddle: at its
/// full size, since it is solid from that step on.
#[test]
fn a_block_comes_whole_with_the_return_that_brings_it_before_the_ball_reaches_it() {
    let first_arrival = the_return_that_brings(1);
    let mut app = app_among_blocks(first_arrival - 1);
    until(&mut app, MatchState::Playing);
    for _ in 0..5 {
        frame(&mut app);
        assert!(drawn_blocks(&mut app).is_empty(), "a return too soon");
    }
    // The rally counts again, on from the held count: the next return is
    // the one that brings the first block.
    app.world_mut().resource_mut::<Sandbox>().hold_returns(None);
    teleport_the_ball(&mut app, Vec2::new(72.0, 0.0), Vec2::new(200.0, 0.0));
    frame(&mut app);
    let (position, velocity) = ball_state(&mut app);
    assert!(velocity.x < 0.0, "the right paddle returned the ball");
    assert!(position.x > 60.0, "the ball is still at the paddle");
    let expected = the_cores_blocks(&app, first_arrival);
    assert_eq!(expected.len(), 2, "one block and its copy");
    let drawn = drawn_blocks(&mut app);
    assert_eq!(drawn.len(), 2);
    for (drawn, block) in drawn.iter().zip(&expected) {
        assert_shows(drawn, block);
        assert!(drawn.fresh > 0.9, "a new block shows new: {drawn:?}");
        assert!(
            block.centre().x + block.half_size().length() < position.x,
            "the block at {block:?} is not ahead of the ball at {position}"
        );
    }
}

/// R11: a block arrives once and calmly. It stands at its size, its place,
/// and its tilt from its first frame; what shows it new fades within the
/// time it takes to appear, never returning on the way, and then is gone
/// for good.
#[test]
fn a_new_block_settles_once_and_keeps_its_size_its_place_and_its_tilt() {
    let mut app = app_among_blocks(FULL_DIAL);
    let first = until_the_blocks_stand(&mut app);
    assert!(
        first.iter().all(|block| block.fresh > 0.9),
        "a new block shows new: {first:?}"
    );
    let mut before = first;
    for _ in 0..frames_to_settle() {
        frame(&mut app);
        let now = drawn_blocks(&mut app);
        for (now, before) in now.iter().zip(&before) {
            assert_eq!(now.entity, before.entity, "the same block, not a new one");
            assert!(now.fresh <= before.fresh, "{before:?} then {now:?}");
            assert!(now.fresh >= 0.0, "{now:?}");
            assert_near(now.at, before.at, "where it stands");
            assert_near(now.size, before.size, "how large it is");
            assert!(now.turned.angle_between(before.turned) < 1e-4, "{now:?}");
        }
        before = now;
    }
    let settled = before;
    assert!(
        settled.iter().all(|block| block.fresh.abs() < 1e-6),
        "{settled:?}"
    );
    for _ in 0..30 {
        frame(&mut app);
        assert_eq!(drawn_blocks(&mut app), settled, "a settled block holds");
    }
}

/// M2 spec 2.6.6 and 2.4.3: a block stays until the point. The floor
/// holds through the rally's flights, and is empty again by the next
/// countdown.
#[test]
fn the_blocks_hold_through_the_rally_and_go_at_the_countdown() {
    let mut app = app_among_blocks(FULL_DIAL);
    until_the_blocks_have_settled(&mut app);
    let floor = drawn_blocks(&mut app);
    assert!(!floor.is_empty(), "the embedded dial brings no block");
    // A return starts a second flight of the same rally.
    teleport_the_ball(&mut app, Vec2::new(72.0, 0.0), Vec2::new(200.0, 0.0));
    frame(&mut app);
    assert!(ball_state(&mut app).1.x < 0.0, "the right paddle returned");
    for _ in 0..5 {
        frame(&mut app);
        assert_eq!(drawn_blocks(&mut app), floor, "in the rally's next flight");
    }
    // Past the left paddle, which waits at the centre: a point.
    teleport_the_ball(&mut app, Vec2::new(-60.0, 40.0), Vec2::new(-200.0, 0.0));
    until(&mut app, MatchState::Countdown);
    frame(&mut app);
    assert!(drawn_blocks(&mut app).is_empty(), "in the next countdown");
}

/// The point pause shows the rally that was just played: its flight is
/// still on the ball, and its blocks stand where they stood, in every
/// frame until the countdown. The next rally's seed is drawn at the point,
/// and its blocks are not this floor's.
#[test]
fn through_the_point_pause_the_blocks_stay_where_the_rally_stood_them() {
    let mut app = app_among_blocks(FULL_DIAL);
    until_the_blocks_have_settled(&mut app);
    let floor = drawn_blocks(&mut app);
    let rally = app.world().resource::<ActiveEffects>().rally_seed();
    // Past the left paddle, which waits at the centre: a point.
    teleport_the_ball(&mut app, Vec2::new(-60.0, 40.0), Vec2::new(-200.0, 0.0));
    until(&mut app, MatchState::PointScored);
    assert_ne!(
        app.world().resource::<ActiveEffects>().rally_seed(),
        rally,
        "the point draws the next rally's seed"
    );
    let mut frames = 0;
    while match_state(&app) == MatchState::PointScored {
        assert_eq!(drawn_blocks(&mut app), floor, "frame {frames} of the pause");
        frame(&mut app);
        frames += 1;
        assert!(frames < 300, "the point pause never ended");
    }
    assert!(frames > 10, "the point pause lasted only {frames} frames");
}

/// Counts bounces off blocks on the ball's flight, as physics counts
/// them, until it has made `total`. The served ball may have made some
/// already.
fn count_bounces_up_to(app: &mut App, total: u16) {
    let mut flights = app
        .world_mut()
        .query_filtered::<&mut BallFlight, With<Ball>>();
    let mut flight = flights
        .single_mut(app.world_mut())
        .expect("exactly one ball");
    let made = flight.flight().obstacle_bounces();
    assert!(made <= total, "the flight has made {made} bounces already");
    for _ in made..total {
        flight.bounced(1);
    }
}

/// M2 spec 2.2.6.1 and R1: a flight that has spent its bounces flies
/// through the blocks, and the floor says so. Every block shows spent from
/// the frame the last bounce is counted, one bounce short of it none does,
/// and the next return, with a new flight's bounces to spend, shows them
/// solid again.
#[test]
fn while_the_flights_bounces_are_spent_every_block_shows_it_until_the_next_return() {
    let budget = tuning().ball().bounce_rule().bounces_per_flight().get();
    let mut app = app_among_blocks(FULL_DIAL);
    until_the_blocks_have_settled(&mut app);
    // Still, at the centre spot, which no block reaches: nothing bounces.
    teleport_the_ball(&mut app, Vec2::ZERO, Vec2::ZERO);
    count_bounces_up_to(&mut app, budget - 1);
    frame(&mut app);
    let floor = drawn_blocks(&mut app);
    assert!(!floor.is_empty(), "the embedded dial brings no block");
    assert!(floor.iter().all(|block| !block.spent), "{floor:?}");

    count_bounces_up_to(&mut app, budget);
    frame(&mut app);
    let spent = drawn_blocks(&mut app);
    assert!(spent.iter().all(|block| block.spent), "{spent:?}");
    for (spent, solid) in spent.iter().zip(&floor) {
        assert_eq!(spent.entity, solid.entity, "the same block, shown spent");
        assert_near(spent.at, solid.at, "where it stands");
    }

    teleport_the_ball(&mut app, Vec2::new(72.0, 0.0), Vec2::new(200.0, 0.0));
    frame(&mut app);
    assert!(ball_state(&mut app).1.x < 0.0, "the right paddle returned");
    let again = drawn_blocks(&mut app);
    assert!(again.iter().all(|block| !block.spent), "{again:?}");
}

/// The blocks are Blocks': in a match without it the floor stays empty.
#[test]
fn without_blocks_in_play_the_floor_stays_empty() {
    let mut app = app_from(1, None);
    until(&mut app, MatchState::Playing);
    for _ in 0..30 {
        frame(&mut app);
        let floor = drawn_blocks(&mut app);
        assert!(floor.is_empty(), "without Blocks: {floor:?}");
    }
}

/// The blocks live with their match: none is left behind.
#[test]
fn leaving_the_match_takes_the_blocks_with_it() {
    let mut app = app_among_blocks(FULL_DIAL);
    until_the_blocks_have_settled(&mut app);
    assert!(
        !drawn_blocks(&mut app).is_empty(),
        "the embedded dial brings no block"
    );
    leave_match(&mut app);
    let left_behind = drawn_blocks(&mut app);
    assert!(left_behind.is_empty(), "{left_behind:?}");
}
