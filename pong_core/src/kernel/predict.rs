//! Ball crossing prediction for the AI (spec 1.2.6).

use super::flight::Flight;
use super::geometry::{Arena, Radius};
use super::integrate::{Motion, advance_between_walls};

/// Steps the prediction will take before giving up. At sixty steps a second this
/// is over twenty minutes of flight; a ball that slow is a simulation fault, not
/// a rally.
const MAX_STEPS: u32 = 100_000;

/// Y coordinate at which a ball of `radius` in `flight` first reaches
/// `target_x`, or `None` if it never will.
///
/// The ball is stepped with the same [`advance_between_walls`] the physics step
/// uses, at the same `dt`, under the same `flight` (M2 spec 2.2.1), so the AI
/// sees exactly what the ball will do and nothing more. Wall bounces and the
/// flight's forces are modeled; paddles are not. The flight's clock is read
/// ahead, not advanced. The crossing inside the final step is interpolated,
/// so the answer does not snap to a step boundary.
///
/// `None` when the ball is stationary along x, moving away from `target_x`, or
/// still short of it after [`MAX_STEPS`]. A ball already on the line crosses now.
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
    let mut motion = start;
    for step in 0..MAX_STEPS {
        let force = flight.force_ahead(motion, step, dt);
        let next = advance_between_walls(arena, radius, motion, force, dt).motion;
        let remaining = target_x - motion.position.x;
        let travelled = next.position.x - motion.position.x;
        if travelled.abs() >= remaining.abs() {
            let fraction = remaining / travelled;
            // The integrator moves the ball with the velocity the force has
            // already changed (semi-implicit Euler), so the crossing does too.
            let stepped_velocity_y = motion.velocity.y + force.y * dt;
            let unbounced_y = motion.position.y + stepped_velocity_y * dt * fraction;
            return Some(fold_once(unbounced_y, limit));
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
    use bevy::math::Vec2;

    use super::crossing_y;
    use crate::kernel::flight::{Contribution, Field, Flight, FlightStart};
    use crate::kernel::integrate::{Motion, advance_between_walls};
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
            }),
        )
    }

    /// Steps the ball as the physics step does, ticking the flight, until it
    /// passes `target_x`; the crossing is read off the last step's straight line.
    fn simulated_crossing(start: Motion, mut flight: Flight, target_x: f32) -> f32 {
        let mut motion = start;
        for _ in 0..10_000 {
            let force = flight.force(motion, DT);
            let next = advance_between_walls(standard_arena(), unit_radius(), motion, force, DT);
            flight.tick();
            if next.motion.position.x >= target_x {
                assert!(!next.bounced, "the scripted course crosses clear of a wall");
                let fraction =
                    (target_x - motion.position.x) / (next.motion.position.x - motion.position.x);
                return motion.position.y + fraction * (next.motion.position.y - motion.position.y);
            }
            motion = next.motion;
        }
        panic!("ball never reached x = {target_x}");
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
