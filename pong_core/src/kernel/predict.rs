//! Ball crossing prediction for the AI (spec 1.2.6).

use bevy::math::Vec2;

use super::geometry::{Arena, Radius};
use super::integrate::{Motion, advance_between_walls};

/// Steps the prediction will take before giving up. At sixty steps a second this
/// is over twenty minutes of flight; a ball that slow is a simulation fault, not
/// a rally.
const MAX_STEPS: u32 = 100_000;

/// Y coordinate at which a free-flying ball of `radius` first reaches `target_x`,
/// or `None` if it never will.
///
/// The ball is stepped with the same [`advance_between_walls`] the physics step
/// uses, at the same `dt`, so the AI sees exactly what the ball will do and
/// nothing more. Only wall bounces are modeled: no paddles, no forces
/// (spec 1.2.8). The crossing inside the final step is interpolated, so the
/// answer does not snap to a step boundary.
///
/// `None` when the ball is stationary along x, moving away from `target_x`, or
/// still short of it after [`MAX_STEPS`]. A ball already on the line crosses now.
#[must_use]
pub fn crossing_y(
    arena: Arena,
    radius: Radius,
    start: Motion,
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
    for _ in 0..MAX_STEPS {
        let next = advance_between_walls(arena, radius, motion, Vec2::ZERO, dt).motion;
        let remaining = target_x - motion.position.x;
        let travelled = next.position.x - motion.position.x;
        if travelled.abs() >= remaining.abs() {
            let fraction = remaining / travelled;
            let unbounced_y = motion.position.y + motion.velocity.y * dt * fraction;
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
    use crate::kernel::integrate::Motion;
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

        let Some(y) = crossing_y(standard_arena(), unit_radius(), start, target_x, DT) else {
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
        let Some(y) = crossing_y(standard_arena(), unit_radius(), start, 30.5, DT) else {
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
        assert!(crossing_y(standard_arena(), unit_radius(), start, 70.0, DT).is_none());
    }

    #[test]
    fn ball_with_no_horizontal_speed_has_no_crossing() {
        let start = Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(0.0, 30.0),
        };
        assert!(crossing_y(standard_arena(), unit_radius(), start, 70.0, DT).is_none());
    }

    #[test]
    fn ball_already_on_the_target_line_crosses_now() {
        let start = Motion {
            position: Vec2::new(70.0, 12.0),
            velocity: Vec2::new(60.0, 30.0),
        };
        let Some(y) = crossing_y(standard_arena(), unit_radius(), start, 70.0, DT) else {
            panic!("ball on the line reported no crossing");
        };
        assert_near(y, 12.0, 1e-6);
    }
}
