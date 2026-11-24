//! Ball crossing prediction for the AI (spec 1.2.6).

use super::flight::Flight;
use super::geometry::{Arena, Radius};
use super::integrate::Motion;
use super::obstacle::advance_among_obstacles;

/// Steps the prediction will take before giving up. At sixty steps a second this
/// is over twenty minutes of flight; a ball that slow is a simulation fault, not
/// a rally.
const MAX_STEPS: u32 = 100_000;

/// Y coordinate at which a ball of `radius` in `flight` first reaches
/// `target_x`, or `None` if it never will.
///
/// The ball is stepped with the same [`advance_among_obstacles`] the physics
/// step uses, at the same `dt`, under the same `flight` (M2 spec 2.2.1,
/// 2.2.6), so the AI sees exactly what the ball will do and nothing more.
/// Wall bounces, the flight's forces, and bounces off its obstacles are
/// modeled; paddles are not. The flight's clock is read ahead, not advanced,
/// and its bounce budget is spent ahead, not from it (M2 spec 2.2.6.1).
/// The crossing inside the final step is interpolated, so the answer does
/// not snap to a step boundary.
///
/// `None` when the ball is stationary along x, moving away from `target_x`,
/// turned back by an obstacle and out past the goal line on the far side, or
/// still short of the target after [`MAX_STEPS`]. A ball already on the line
/// crosses now. A ball that starts moving away has no crossing even where an
/// obstacle would turn it round: ask again once it has turned.
///
/// Precondition (the caller's): `target_x` is inside the arena, and no
/// obstacle of `flight` is within a step's travel of it, so that the step
/// that crosses it bounces off none. Whoever places obstacles keeps them
/// clear of the lines a crossing is asked for.
#[must_use]
pub fn crossing_y(
    arena: Arena,
    radius: Radius,
    start: Motion,
    flight: &Flight,
    target_x: f32,
    dt: f32,
) -> Option<f32> {
    let distance = target_x - start.position.x;
    if distance == 0.0 {
        return Some(start.position.y);
    }
    if distance * start.velocity.x <= 0.0 {
        return None;
    }

    let limit = arena.center_limit_y(radius);
    let far_goal_x = -distance.signum() * arena.half_extents().x;
    let mut motion = start;
    let mut obstacles = flight.obstacles_in_flight();
    for step in 0..MAX_STEPS {
        let force = flight.force_ahead(motion, step, dt);
        let stepped = advance_among_obstacles(arena, radius, motion, force, dt, obstacles);
        obstacles = obstacles.after(stepped.obstacle_bounces);
        let next = stepped.motion;
        let remaining = target_x - motion.position.x;
        let travelled = next.position.x - motion.position.x;
        if travelled.abs() >= remaining.abs() {
            let fraction = remaining / travelled;
            // The wall step moved the ball with the velocity the force had
            // already changed and the bound had cut (semi-implicit Euler, M2
            // spec 2.2.2), so the crossing does too: that velocity is the
            // step's own, turned back if the step bounced.
            let stepped_velocity_y = if stepped.wall_bounces > 0 {
                -next.velocity.y
            } else {
                next.velocity.y
            };
            let unbounced_y = motion.position.y + stepped_velocity_y * dt * fraction;
            return Some(fold_once(unbounced_y, limit));
        }
        if (next.position.x - far_goal_x) * far_goal_x > 0.0 {
            return None;
        }
        motion = next;
    }
    None
}

/// Reflects a y that overshot one wall back inside `-limit..=limit`.
fn fold_once(y: f32, limit: f32) -> f32 {
    if y > limit {
        2.0 * limit - y
    } else if y < -limit {
        -2.0 * limit - y
    } else {
        y
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU8, NonZeroU16};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use bevy::math::Vec2;

    use super::crossing_y;
    use crate::kernel::flight::{Contribution, Field, Flight, FlightStart};
    use crate::kernel::integrate::{Motion, advance_between_walls};
    use crate::kernel::obstacle::{BounceRule, Obstacle, advance_among_obstacles};
    use crate::kernel::reflect::MaxDeflection;
    use crate::kernel::test_support::{assert_near, standard_arena, unit_radius};

    const DT: f32 = 1.0 / 60.0;
    /// The ball's center bounces at ±44.
    const LIMIT: f32 = 44.0;

    /// Analytic zigzag: an unbounded straight-line y folded into `-limit..=limit`
    /// as a triangle wave. Independent of the kernel's stepping.
    fn fold_analytic(y: f32, limit: f32) -> f32 {
        let period = 4.0 * limit;
        let phase = (y + limit).rem_euclid(period);
        if phase <= 2.0 * limit {
            phase - limit
        } else {
            3.0 * limit - phase
        }
    }

    #[test]
    fn prediction_matches_the_analytic_zigzag_within_one_unit_after_two_bounces() {
        let start = Motion {
            position: Vec2::new(-40.0, 10.0),
            velocity: Vec2::new(60.0, 90.0),
        };
        let target_x = 70.0;
        // t = 110 / 60 s; unbounded y = 10 + 90 t = 175, which crosses both walls.
        let expected = fold_analytic(175.0, LIMIT);
        assert_near(expected, -1.0, 1e-4);

        let Some(y) = crossing_y(
            standard_arena(),
            unit_radius(),
            start,
            &Flight::default(),
            target_x,
            DT,
        ) else {
            panic!("ball moving toward the target never crossed it");
        };
        assert_near(y, expected, 1.0);
    }

    #[test]
    fn crossing_between_two_steps_is_interpolated_not_rounded_to_a_step() {
        let start = Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(60.0, 30.0),
        };
        // x = 30.5 is reached at t = 30.5 / 60 s, half a step past step 30.
        let Some(y) = crossing_y(
            standard_arena(),
            unit_radius(),
            start,
            &Flight::default(),
            30.5,
            DT,
        ) else {
            panic!("ball never crossed");
        };
        assert_near(y, 15.25, 1e-3);
    }

    #[test]
    fn ball_moving_away_from_the_target_has_no_crossing() {
        let start = Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(-60.0, 30.0),
        };
        assert!(
            crossing_y(
                standard_arena(),
                unit_radius(),
                start,
                &Flight::default(),
                70.0,
                DT
            )
            .is_none()
        );
    }

    #[test]
    fn ball_with_no_horizontal_speed_has_no_crossing() {
        let start = Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(0.0, 30.0),
        };
        assert!(
            crossing_y(
                standard_arena(),
                unit_radius(),
                start,
                &Flight::default(),
                70.0,
                DT
            )
            .is_none()
        );
    }

    #[test]
    fn ball_already_on_the_target_line_crosses_now() {
        let start = Motion {
            position: Vec2::new(70.0, 12.0),
            velocity: Vec2::new(60.0, 30.0),
        };
        let Some(y) = crossing_y(
            standard_arena(),
            unit_radius(),
            start,
            &Flight::default(),
            70.0,
            DT,
        ) else {
            panic!("ball on the line reported no crossing");
        };
        assert_near(y, 12.0, 1e-6);
    }

    /// A constant push along y.
    #[derive(Debug)]
    struct Steady(f32);

    impl Field for Steady {
        fn lateral(&self, _motion: Motion, _seconds_in_flight: f32) -> f32 {
            self.0
        }
    }

    /// A push that swings with time in flight and leans on the ball's speed.
    #[derive(Debug)]
    struct Swinging;

    impl Field for Swinging {
        fn lateral(&self, motion: Motion, seconds_in_flight: f32) -> f32 {
            40.0 * (3.0 * seconds_in_flight).sin() - 0.1 * motion.velocity.y
        }
    }

    /// Two fields, in the order they are contributed.
    type Pair = [Box<dyn Field>; 2];

    fn flight_of(fields: Pair) -> Flight {
        Flight::start(
            FlightStart::SERVE,
            fields.map(|field| Contribution {
                speed_gain: 0.0,
                paddle_height_factor: 1.0,
                field: Some(field),
                obstacles: Vec::new(),
            }),
        )
    }

    /// Steps the ball as the physics step does, ticking the flight, until it
    /// passes `target_x`; the crossing is read off the last step's straight line.
    fn simulated_crossing(start: Motion, flight: Flight, target_x: f32) -> f32 {
        simulated_flight(start, flight, target_x).0
    }

    /// [`simulated_crossing`], and the bounces off obstacles on the way.
    fn simulated_flight(start: Motion, mut flight: Flight, target_x: f32) -> (f32, u32) {
        let mut motion = start;
        let mut bounces = 0;
        for _ in 0..10_000 {
            let force = flight.force(motion, DT);
            let next = advance_among_obstacles(
                standard_arena(),
                unit_radius(),
                motion,
                force,
                DT,
                flight.obstacles_in_flight(),
            );
            flight.tick();
            flight.bounced(next.obstacle_bounces);
            if next.motion.position.x >= target_x {
                assert_eq!(
                    (next.wall_bounces, next.obstacle_bounces),
                    (0, 0),
                    "the scripted course crosses clear of walls and obstacles"
                );
                let fraction =
                    (target_x - motion.position.x) / (next.motion.position.x - motion.position.x);
                let y = motion.position.y + fraction * (next.motion.position.y - motion.position.y);
                return (y, bounces);
            }
            bounces += u32::from(next.obstacle_bounces);
            motion = next.motion;
        }
        panic!("ball never reached x = {target_x}");
    }

    /// A flight among `obstacles`, bouncing by four contacts a step and no
    /// steeper than sixty degrees, under `fields`.
    fn flight_among(obstacles: Vec<Obstacle>, fields: Vec<Box<dyn Field>>) -> Flight {
        let rule = BounceRule::new(
            NonZeroU8::new(4).expect("four is not zero"),
            MaxDeflection::new(60.0_f32.to_radians()).expect("sixty degrees is valid"),
        );
        let standing = Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: None,
            obstacles,
        };
        let pushing = fields.into_iter().map(|field| Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: Some(field),
            obstacles: Vec::new(),
        });
        Flight::start(FlightStart::SERVE, std::iter::once(standing).chain(pushing))
            .bouncing_by(rule)
    }

    /// A block the size of the default paddle.
    fn block(centre: Vec2, tilt: f32) -> Obstacle {
        Obstacle::new(centre, Vec2::new(1.0, 7.0), tilt)
    }

    /// M2 spec 2.2.6, third done-condition: prediction equals simulation
    /// through two bounces. The ball is turned back by a block on the right
    /// and turned again by one on the left, with no force and under a
    /// steady push, and then crosses the target.
    #[test]
    fn prediction_equals_simulation_through_two_bounces() {
        let start = Motion {
            position: Vec2::new(-20.0, 0.0),
            velocity: Vec2::new(80.0, 0.0),
        };
        let blocks = || {
            vec![
                block(Vec2::new(30.0, 0.0), 0.1),
                block(Vec2::new(-40.0, -13.8), 0.1),
            ]
        };
        let pushed = || -> Vec<Box<dyn Field>> { vec![Box::new(Steady(-3.0))] };
        let mut crossings = Vec::new();
        for fields in [Vec::new, pushed] {
            let Some(predicted) = crossing_y(
                standard_arena(),
                unit_radius(),
                start,
                &flight_among(blocks(), fields()),
                73.0,
                DT,
            ) else {
                panic!("the ball was predicted never to cross");
            };
            let (simulated, bounces) =
                simulated_flight(start, flight_among(blocks(), fields()), 73.0);
            assert_eq!(bounces, 2);
            assert_near(predicted, simulated, 1e-3);
            crossings.push(predicted);
        }
        // The push moves the crossing: the two courses are not one.
        assert!((crossings[0] - crossings[1]).abs() > 1.0, "{crossings:?}");
    }

    /// M2 spec 2.2.6.1: the prediction spends the flight's budget as the
    /// simulation does. With one bounce to a flight the ball is turned back
    /// by the block on the right and flies through the one on the left,
    /// out by the far goal; and once the flight has made its one bounce,
    /// the ball flies through both and crosses on its straight line.
    #[test]
    fn prediction_spends_the_flights_bounce_budget_as_the_simulation_does() {
        let start = Motion {
            position: Vec2::new(-20.0, 0.0),
            velocity: Vec2::new(80.0, 0.0),
        };
        let one_bounce = || {
            let flight = flight_among(
                vec![
                    block(Vec2::new(30.0, 0.0), 0.1),
                    block(Vec2::new(-40.0, -13.8), 0.1),
                ],
                Vec::new(),
            );
            let rule = flight
                .bounce_rule()
                .with_bounces_per_flight(NonZeroU16::MIN);
            flight.bouncing_by(rule)
        };
        let predict =
            |flight: &Flight| crossing_y(standard_arena(), unit_radius(), start, flight, 73.0, DT);
        assert_eq!(predict(&one_bounce()), None);

        let mut spent = one_bounce();
        spent.bounced(1);
        let predicted = predict(&spent).expect("a spent flight flies straight through");
        assert_near(predicted, 0.0, 1e-4);
        let (simulated, bounces) = simulated_flight(start, spent, 73.0);
        assert_eq!(bounces, 0);
        assert_near(predicted, simulated, 1e-4);
    }

    /// A field that pushes nothing and counts how often it is asked: once
    /// a step.
    #[derive(Debug)]
    struct Counting(Arc<AtomicU32>);

    impl Field for Counting {
        fn lateral(&self, _motion: Motion, _seconds_in_flight: f32) -> f32 {
            self.0.fetch_add(1, Ordering::Relaxed);
            0.0
        }
    }

    /// A block can turn the ball back for good. The prediction follows it
    /// to the far goal line and gives up there, a second or two of flight
    /// on, and not after its twenty minutes.
    #[test]
    fn a_ball_a_block_turns_back_for_good_has_no_crossing_and_is_given_up_at_the_far_goal() {
        let start = Motion {
            position: Vec2::new(-20.0, 0.0),
            velocity: Vec2::new(80.0, 0.0),
        };
        let steps = Arc::new(AtomicU32::new(0));
        let flight = flight_among(
            vec![block(Vec2::new(30.0, 0.0), 0.1)],
            vec![Box::new(Counting(Arc::clone(&steps)))],
        );
        assert_eq!(
            crossing_y(standard_arena(), unit_radius(), start, &flight, 73.0, DT),
            None
        );
        // 48 units to the block and 108 back to the goal line, at 80 a second.
        let stepped = steps.load(Ordering::Relaxed);
        assert!((100..200).contains(&stepped), "{stepped} steps");
    }

    /// M2 spec 2.2.2: the bound on vertical speed is the wall step's, so the
    /// prediction sees it in its last, partial step as in every whole one.
    ///
    /// Pushed far past the bound, the ball flies 88 units a step at one unit
    /// along x: from 40 up past the wall at 44 and folded to -40, then up to
    /// 48 and folded to 40, then as the first step again. Half way through
    /// the third step it has gone 44 up from 40 and is folded to 4; at the
    /// step's end it is at -40.
    #[test]
    fn at_the_vertical_bound_the_prediction_is_still_the_simulation() {
        let start = Motion {
            position: Vec2::new(0.0, 40.0),
            velocity: Vec2::new(60.0, 0.0),
        };
        let flight = flight_of([Box::new(Steady(1.0e6)), Box::new(Steady(0.0))]);
        let crossing = |target_x| {
            crossing_y(
                standard_arena(),
                unit_radius(),
                start,
                &flight,
                target_x,
                DT,
            )
            .expect("the ball flies toward the target")
        };
        assert_near(crossing(2.5), 4.0, 1e-2);
        assert_near(crossing(3.0), -40.0, 1e-2);
    }

    /// M2 plan 1.1: under a composed flight the prediction is the simulation,
    /// whichever order the fields were contributed in.
    #[test]
    fn prediction_equals_simulation_under_two_fields_in_either_order() {
        let start = Motion {
            position: Vec2::new(-60.0, -20.0),
            velocity: Vec2::new(70.0, 10.0),
        };
        let target_x = 65.0;
        let orders: [fn() -> Pair; 2] = [
            || [Box::new(Steady(15.0)), Box::new(Swinging)],
            || [Box::new(Swinging), Box::new(Steady(15.0))],
        ];
        let mut crossings = Vec::new();
        for order in orders {
            let Some(predicted) = crossing_y(
                standard_arena(),
                unit_radius(),
                start,
                &flight_of(order()),
                target_x,
                DT,
            ) else {
                panic!("ball moving toward the target never crossed it");
            };
            let simulated = simulated_crossing(start, flight_of(order()), target_x);
            assert_near(predicted, simulated, 1e-3);
            crossings.push(predicted);
        }
        assert_near(crossings[0], crossings[1], 1e-3);
        // The fields bend the course: unforced, the ball would cross at -2.14.
        assert!((crossings[0] + 2.14).abs() > 5.0, "{crossings:?}");
    }

    /// Prediction reads the flight's clock ahead from where it stands, so a
    /// prediction made mid-flight agrees with one made at its start.
    #[test]
    fn a_prediction_made_mid_flight_continues_the_flights_clock() {
        let start = Motion {
            position: Vec2::new(-60.0, -20.0),
            velocity: Vec2::new(70.0, 10.0),
        };
        let fields = || -> Pair { [Box::new(Steady(15.0)), Box::new(Swinging)] };
        let Some(from_start) = crossing_y(
            standard_arena(),
            unit_radius(),
            start,
            &flight_of(fields()),
            65.0,
            DT,
        ) else {
            panic!("no crossing from the start");
        };
        let mut flight = flight_of(fields());
        let mut motion = start;
        for _ in 0..40 {
            let force = flight.force(motion, DT);
            motion =
                advance_between_walls(standard_arena(), unit_radius(), motion, force, DT).motion;
            flight.tick();
        }
        let Some(mid_flight) =
            crossing_y(standard_arena(), unit_radius(), motion, &flight, 65.0, DT)
        else {
            panic!("no crossing from mid-flight");
        };
        assert_near(mid_flight, from_start, 1e-3);
    }
}
