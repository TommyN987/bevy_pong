//! Kinematic integration (spec 1.2.2).

use bevy::math::Vec2;

use super::geometry::{Arena, Radius};
use super::reflect::mirror;

/// Position and velocity of a moving point, in arena units and units per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// Center of the body.
    pub position: Vec2,
    /// Change of position per second.
    pub velocity: Vec2,
}

/// Advances `motion` by `dt` seconds under `force`, semi-implicit Euler.
///
/// The ball has unit mass, so `force` is also the acceleration. The caller
/// asks the ball's flight for it ([`Flight::force`](super::flight::Flight::force)).
/// Velocity updates first and the new velocity moves the position, which is
/// stable for the oscillating forces that later effects add.
///
/// This is the one integrator: the physics step and the AI's prediction
/// (spec 1.2.6) both call it, which is the fairness guarantee of spec 1.2.
#[must_use]
pub fn integrate(motion: Motion, force: Vec2, dt: f32) -> Motion {
    let velocity = motion.velocity + force * dt;
    Motion {
        position: motion.position + velocity * dt,
        velocity,
    }
}

/// Result of [`advance_between_walls`]: where the ball is now and whether it
/// bounced off a wall during the step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallStep {
    /// Position and velocity after the step.
    pub motion: Motion,
    /// Whether the step crossed a wall and was folded back inside.
    pub bounced: bool,
}

/// One fixed step of a ball of `radius` inside `arena`: [`integrate`], with
/// the vertical speed bounded, then a bounce off the top or bottom wall if
/// the step crossed one.
///
/// The ball's edge, not its center, touches the wall, so the center turns at
/// `half_height - radius`. The overshoot past the wall is folded back inside
/// and the vertical velocity reverses, which is where the ball would be had it
/// bounced mid-step. Goal lines are not walls: the ball flies through them and
/// the rules decide the point.
///
/// The bound (M2 spec 2.2.2) sits here, on the channel, and in no effect: a
/// single fold undoes at most the playable height, so the step's vertical
/// travel is cut to it, whatever force or entry speed asked for more. The
/// horizontal speed is not bounded here; fields are lateral, and the launch
/// speed has its own cap ([`super::reflect::SpeedRule`]).
///
/// Precondition (the caller's): `motion` is inside the walls and `dt` is
/// not negative.
/// Postcondition: the ball ends inside the walls, to rounding, under any
/// `force`; its vertical speed is at most the playable height per `dt`.
/// Tuning validation keeps every launch under that bound, so a launch is
/// never cut (plan 3.17). Prediction (spec 1.2.6) and the physics step share
/// this function, so both see the same bound.
#[must_use]
pub fn advance_between_walls(
    arena: Arena,
    radius: Radius,
    motion: Motion,
    force: Vec2,
    dt: f32,
) -> WallStep {
    let limit = arena.center_limit_y(radius);
    let stepped = within_one_fold(motion, integrate(motion, force, dt), 2.0 * limit, dt);
    if stepped.position.y > limit {
        return WallStep {
            motion: fold_at_wall(stepped, limit),
            bounced: true,
        };
    }
    if stepped.position.y < -limit {
        return WallStep {
            motion: fold_at_wall(stepped, -limit),
            bounced: true,
        };
    }
    WallStep {
        motion: stepped,
        bounced: false,
    }
}

/// `stepped`, the step of `dt` seconds from `before`, with its vertical
/// travel cut to `playable_height`: the vertical speed is bounded to that
/// height per `dt`, and the position follows the bounded speed. A step of no
/// length travels nowhere and bounds nothing.
fn within_one_fold(before: Motion, stepped: Motion, playable_height: f32, dt: f32) -> Motion {
    let fastest = playable_height / dt;
    if stepped.velocity.y.abs() <= fastest {
        return stepped;
    }
    let vertical = stepped.velocity.y.clamp(-fastest, fastest);
    Motion {
        position: Vec2::new(stepped.position.x, before.position.y + vertical * dt),
        velocity: Vec2::new(stepped.velocity.x, vertical),
    }
}

/// Reflects an overshoot past the horizontal line `y = wall_y` back inside.
fn fold_at_wall(motion: Motion, wall_y: f32) -> Motion {
    Motion {
        position: Vec2::new(motion.position.x, 2.0 * wall_y - motion.position.y),
        velocity: mirror(motion.velocity, Vec2::Y),
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::{Motion, advance_between_walls, integrate};
    use crate::kernel::test_support::{assert_near, assert_vec_near, standard_arena, unit_radius};

    const DT: f32 = 1.0 / 60.0;
    const STEPS: u32 = 60;

    fn run(start: Motion, force: Vec2) -> Motion {
        (0..STEPS).fold(start, |m, _| integrate(m, force, DT))
    }

    #[test]
    fn zero_force_follows_the_straight_line_position_plus_velocity_times_time() {
        let start = Motion {
            position: Vec2::new(-10.0, 5.0),
            velocity: Vec2::new(60.0, -30.0),
        };
        let end = run(start, Vec2::ZERO);
        assert_vec_near(end.position, Vec2::new(50.0, -25.0), 1e-3);
        assert_vec_near(end.velocity, start.velocity, 1e-6);
    }

    #[test]
    fn constant_force_matches_half_a_t_squared_within_one_step_of_drift() {
        let start = Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(10.0, 0.0),
        };
        let force = Vec2::new(0.0, -10.0);
        let end = run(start, force);
        // Analytic: p = v0 t + a t² / 2 over t = 1 s. Semi-implicit Euler lands
        // a further a·t·dt/2 along the force, which is the drift budget here.
        let drift = force.length() * DT / 2.0;
        assert_vec_near(end.position, Vec2::new(10.0, -5.0), drift + 1e-3);
        assert_vec_near(end.velocity, Vec2::new(10.0, -10.0), 1e-4);
    }

    #[test]
    fn step_away_from_the_walls_equals_plain_integration() {
        let start = Motion {
            position: Vec2::new(5.0, 5.0),
            velocity: Vec2::new(60.0, 60.0),
        };
        let step = advance_between_walls(standard_arena(), unit_radius(), start, Vec2::ZERO, DT);
        assert_eq!(step.motion, integrate(start, Vec2::ZERO, DT));
        assert!(!step.bounced);
    }

    #[test]
    fn step_into_the_top_wall_folds_position_back_and_flips_vertical_velocity() {
        // The ball's edge may reach y = 44; its center starts at 43 and would land at 45.
        let start = Motion {
            position: Vec2::new(0.0, 43.0),
            velocity: Vec2::new(60.0, 120.0),
        };
        let step = advance_between_walls(standard_arena(), unit_radius(), start, Vec2::ZERO, DT);
        assert_vec_near(step.motion.position, Vec2::new(1.0, 43.0), 1e-4);
        assert_vec_near(step.motion.velocity, Vec2::new(60.0, -120.0), 1e-6);
        assert!(step.bounced);
    }

    /// M2 spec 2.2.2: the bound sits on the channel. A force that would
    /// carry the ball farther than one fold undoes is cut to the playable
    /// height per step, 88 units here, so the step still ends inside.
    #[test]
    fn a_step_driven_past_the_playable_height_is_cut_to_one_fold_and_lands_inside() {
        let limit = standard_arena().center_limit_y(unit_radius());
        // Unbounded, the force gives 16,666 units/s upward: 277 units in the
        // step, three playable heights.
        let force = Vec2::new(0.0, 1.0e6);
        for start_y in [-limit, -10.0, 40.0, limit] {
            let start = Motion {
                position: Vec2::new(0.0, start_y),
                velocity: Vec2::new(60.0, 0.0),
            };
            let step = advance_between_walls(standard_arena(), unit_radius(), start, force, DT);
            assert!(
                step.motion.position.y.abs() <= limit + 1e-3,
                "from {start_y} the ball ended outside: {}",
                step.motion.position.y
            );
            // 88 units a step is 5,280 units/s.
            assert_near(step.motion.velocity.y.abs(), 5_280.0, 0.01);
            assert_near(step.motion.velocity.x, 60.0, 1e-6);
        }
    }

    /// The bound is on the ball's vertical speed whatever its cause: a ball
    /// that enters the step too fast is cut as one that was pushed.
    #[test]
    fn a_ball_already_faster_than_one_fold_undoes_is_cut_the_same() {
        let limit = standard_arena().center_limit_y(unit_radius());
        let start = Motion {
            position: Vec2::new(0.0, 40.0),
            velocity: Vec2::new(60.0, -20_000.0),
        };
        let step = advance_between_walls(standard_arena(), unit_radius(), start, Vec2::ZERO, DT);
        // 40 down by 88 is -48: four past the wall at -44, folded to -40.
        assert_near(step.motion.position.y, -40.0, 1e-2);
        assert!(step.motion.position.y.abs() <= limit);
        assert_near(step.motion.velocity.y, 5_280.0, 0.01);
        assert!(step.bounced);
    }

    /// A step of no length bounds nothing: the remainder of a step that
    /// ended at a paddle may be zero seconds long.
    #[test]
    fn a_step_of_no_length_leaves_the_motion_as_it_was() {
        let start = Motion {
            position: Vec2::new(3.0, 40.0),
            velocity: Vec2::new(60.0, 9_000.0),
        };
        let step = advance_between_walls(standard_arena(), unit_radius(), start, Vec2::ZERO, 0.0);
        assert_eq!(step.motion, start);
        assert!(!step.bounced);
    }

    #[test]
    fn step_into_the_bottom_wall_mirrors_the_top_case() {
        let start = Motion {
            position: Vec2::new(0.0, -43.0),
            velocity: Vec2::new(60.0, -120.0),
        };
        let step = advance_between_walls(standard_arena(), unit_radius(), start, Vec2::ZERO, DT);
        assert_vec_near(step.motion.position, Vec2::new(1.0, -43.0), 1e-4);
        assert_vec_near(step.motion.velocity, Vec2::new(60.0, 120.0), 1e-6);
        assert!(step.bounced);
    }
}
