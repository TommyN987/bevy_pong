//! Physics (spec 1.4.4): thin systems over the kernel. Integrate moves the ball
//! between the walls; collide sweeps it against the paddles. The AI predicts
//! with the same kernel functions these systems call.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::message::MessageWriter;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::math::Vec2;
use bevy::time::{Fixed, Time};

use crate::app::phases::FixedStepPhase;
use crate::domain::{BallHitPaddle, BallHitWall};
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::paddle::Paddle;
use crate::features::tuning::Tuning;
use crate::kernel::collide::{Face, Impact, swept_circle_rect};
use crate::kernel::integrate::{Motion, advance_between_walls};
use crate::kernel::reflect::{HitOffset, mirror, paddle_reflection};

/// Moves the ball in the integrate phase and resolves paddle contact in the
/// collide phase. Emits [`BallHitWall`] and [`BallHitPaddle`].
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BallHitWall>()
            .add_message::<BallHitPaddle>()
            .init_resource::<BallForce>()
            .init_resource::<BallStepStart>()
            .add_systems(
                FixedUpdate,
                (
                    integrate_ball.in_set(FixedStepPhase::Integrate),
                    collide_ball_with_paddles.in_set(FixedStepPhase::Collide),
                ),
            );
    }
}

/// Force on the ball this step, in units per second squared (unit mass).
/// Effects add to it in the effects phase; integration spends it and zeroes it.
/// Nothing adds to it in M1 (spec 1.2.8).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct BallForce(pub Vec2);

/// Where the ball was before this step's integration, so the collide phase can
/// sweep the path it took. `None` before the first step of a match.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
struct BallStepStart(Option<Motion>);

fn integrate_ball(
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    mut force: ResMut<BallForce>,
    mut step_start: ResMut<BallStepStart>,
    mut walls: MessageWriter<BallHitWall>,
    mut balls: Query<(&mut Position, &mut Velocity), With<Ball>>,
) {
    for (mut position, mut velocity) in &mut balls {
        let motion = Motion {
            position: position.0,
            velocity: velocity.0,
        };
        step_start.0 = Some(motion);
        let step = advance_between_walls(
            tuning.arena(),
            tuning.ball().radius(),
            motion,
            force.0,
            time.delta_secs(),
        );
        position.0 = step.motion.position;
        velocity.0 = step.motion.velocity;
        if step.bounced {
            walls.write(BallHitWall);
        }
    }
    force.0 = Vec2::ZERO;
}

/// Sweeps the ball along the path it took this step against every paddle and
/// resolves the earliest contact: a face hit reflects by hit offset and speeds
/// up; an end hit mirrors the vertical velocity. The remainder of the step is
/// travelled along the new velocity through the same wall-bounded step.
///
/// The sweep uses the straight line from the step's start to its end. A step
/// that also bounced off a wall is swept along the folded line, which is wrong
/// only for a ball striking a paddle end in the same step as a wall; the
/// sanity phase catches anything that escapes.
fn collide_ball_with_paddles(
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    step_start: Res<BallStepStart>,
    paddles: Query<&Paddle>,
    mut hits: MessageWriter<BallHitPaddle>,
    mut walls: MessageWriter<BallHitWall>,
    mut balls: Query<(&mut Position, &mut Velocity), With<Ball>>,
) {
    let Some(start) = step_start.0 else {
        return;
    };
    let arena = tuning.arena();
    let ball = tuning.ball();
    for (mut position, mut velocity) in &mut balls {
        let displacement = position.0 - start.position;
        let mut earliest: Option<(Impact, Paddle)> = None;
        for paddle in &paddles {
            if let Some(impact) = swept_circle_rect(
                start.position,
                ball.radius(),
                displacement,
                paddle.rect(arena),
            ) && earliest.is_none_or(|(first, _)| impact.toi < first.toi)
            {
                earliest = Some((impact, *paddle));
            }
        }
        let Some((impact, paddle)) = earliest else {
            continue;
        };
        let contact = start.position + displacement * impact.toi;
        let reflected = match impact.face {
            Face::Left | Face::Right => {
                let offset = HitOffset::new(contact.y, paddle.center_y(), paddle.height() / 2.0);
                let speed = ball.speed_rule().after_hit(velocity.0.length());
                paddle_reflection(paddle.side(), offset, speed, ball.max_deflection())
            }
            Face::Top | Face::Bottom => mirror(velocity.0, impact.face.normal()),
        };
        // The rest of the step continues from the contact along the new
        // velocity, between the walls like any other movement.
        let remainder = advance_between_walls(
            arena,
            ball.radius(),
            Motion {
                position: contact,
                velocity: reflected,
            },
            Vec2::ZERO,
            time.delta_secs() * (1.0 - impact.toi),
        );
        position.0 = remainder.motion.position;
        velocity.0 = remainder.motion.velocity;
        hits.write(BallHitPaddle {
            side: paddle.side(),
        });
        if remainder.bounced {
            walls.write(BallHitWall);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::ecs::query::With;
    use bevy::math::Vec2;
    use bevy::time::{Fixed, Time};

    use crate::domain::{BallHitPaddle, BallHitWall, Controller, MatchConfig, Seat};
    use crate::features::ball::{Ball, Position};
    use crate::features::paddle::Paddle;
    use crate::features::tuning::Tuning;
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::kernel::predict::crossing_y;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        ball_state, boot, headless_app, place_ball, record, start_match, step, take_recorded,
    };

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn match_app() -> App {
        let mut app = headless_app(1);
        record::<BallHitWall>(&mut app);
        record::<BallHitPaddle>(&mut app);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app
    }

    #[test]
    fn a_step_into_the_wall_folds_the_ball_back_and_emits_ball_hit_wall() {
        let mut app = match_app();
        place_ball(&mut app, Vec2::new(0.0, 43.0), Vec2::new(60.0, 120.0));
        step(&mut app);
        let (position, velocity) = ball_state(&mut app);
        assert_near(position.y, 43.0, 1e-3);
        assert_near(velocity.y, -120.0, 1e-6);
        assert_eq!(take_recorded::<BallHitWall>(&mut app), vec![BallHitWall]);
        assert_eq!(
            take_recorded::<BallHitPaddle>(&mut app),
            Vec::<BallHitPaddle>::new()
        );
    }

    #[test]
    fn a_ball_faster_than_the_paddle_is_thick_still_hits_the_face_speeds_up_and_deflects() {
        let mut app = match_app();
        let tuning = app.world().resource::<Tuning>().clone();
        // Right paddle face at x = 74, ball radius 1: contact at 73. From 72 at
        // 200 per second the step travels 3.33, more than the paddle's thickness.
        place_ball(&mut app, Vec2::new(72.0, 2.0), Vec2::new(200.0, 0.0));
        step(&mut app);
        let (position, velocity) = ball_state(&mut app);
        assert!(position.x < 73.0, "{position:?}");
        assert!(velocity.x < 0.0 && velocity.y > 0.0, "{velocity:?}");
        assert_near(
            velocity.length(),
            tuning.ball().speed_rule().after_hit(200.0),
            1e-3,
        );
        assert_eq!(
            take_recorded::<BallHitPaddle>(&mut app),
            vec![BallHitPaddle { side: Side::Right }]
        );
    }

    /// Both paddles track the ball's height through the paddle's own clamped
    /// API, so the rally lasts however fast the ball gets. Intent-driven
    /// movement is covered by the paddle tests.
    fn track_the_ball(app: &mut App) {
        let (ball, _) = ball_state(app);
        let arena = app.world().resource::<Tuning>().arena();
        let mut paddles = app.world_mut().query::<&mut Paddle>();
        for mut paddle in paddles.iter_mut(app.world_mut()) {
            let dy = ball.y - paddle.center_y();
            paddle.move_by(dy, arena);
        }
    }

    #[test]
    fn a_headless_rally_alternates_paddle_hits_with_wall_bounces_only_between_them() {
        let mut app = match_app();
        let arena = app.world().resource::<Tuning>().arena();
        // Steep enough to meet a wall before the first paddle: 73 units across at
        // 80 per second climbs 55, and the ball's center turns at 44.
        place_ball(&mut app, Vec2::ZERO, Vec2::new(80.0, 60.0));
        let mut paddle_hits = Vec::new();
        let mut wall_bounces_between = Vec::new();
        let mut walls_since_last_paddle = 0;
        for _ in 0..3_000 {
            track_the_ball(&mut app);
            step(&mut app);
            walls_since_last_paddle += take_recorded::<BallHitWall>(&mut app).len();
            for hit in take_recorded::<BallHitPaddle>(&mut app) {
                paddle_hits.push(hit.side);
                wall_bounces_between.push(walls_since_last_paddle);
                walls_since_last_paddle = 0;
            }
            let (position, _) = ball_state(&mut app);
            assert!(
                position.y.abs() <= arena.half_extents().y,
                "ball left the arena: {position:?}"
            );
            if paddle_hits.len() >= 8 {
                break;
            }
        }
        assert!(
            paddle_hits.len() >= 8,
            "rally died after {} hits",
            paddle_hits.len()
        );
        assert_eq!(paddle_hits[0], Side::Right);
        for pair in paddle_hits.windows(2) {
            assert_ne!(
                pair[0], pair[1],
                "two hits in a row on the same side: {paddle_hits:?}"
            );
        }
        assert!(
            wall_bounces_between.iter().any(|&n| n > 0),
            "no wall bounce in the whole rally"
        );
    }

    #[test]
    fn speed_rises_by_the_tuned_increase_on_every_paddle_hit_until_the_cap() {
        let mut app = match_app();
        let rule = app.world().resource::<Tuning>().ball().speed_rule();
        place_ball(&mut app, Vec2::ZERO, Vec2::new(80.0, 0.0));
        let mut expected = 80.0;
        for _ in 0..3_000 {
            step(&mut app);
            for _ in take_recorded::<BallHitPaddle>(&mut app) {
                expected = rule.after_hit(expected);
                assert_near(ball_state(&mut app).1.length(), expected, 1e-2);
            }
            if expected >= 250.0 {
                return;
            }
        }
        panic!("cap never reached; speed {expected}");
    }

    #[test]
    fn the_prediction_equals_the_simulation_within_one_unit() {
        let mut app = match_app();
        let tuning = app.world().resource::<Tuning>().clone();
        let start = Motion {
            position: Vec2::new(-40.0, 10.0),
            velocity: Vec2::new(60.0, 90.0),
        };
        place_ball(&mut app, start.position, start.velocity);
        // The crossing at 70 comes before the right paddle's face at 73.
        let target_x = 70.0;
        let dt = app
            .world()
            .resource::<Time<Fixed>>()
            .timestep()
            .as_secs_f32();
        let predicted = crossing_y(tuning.arena(), tuning.ball().radius(), start, target_x, dt)
            .expect("the ball flies toward the target");
        let mut before = start.position;
        for _ in 0..1_000 {
            step(&mut app);
            let (after, _) = ball_state(&mut app);
            if after.x >= target_x {
                let fraction = (target_x - before.x) / (after.x - before.x);
                let simulated = before.y + fraction * (after.y - before.y);
                assert_near(simulated, predicted, 1.0);
                return;
            }
            before = after;
        }
        panic!("ball never reached x = {target_x}");
    }

    #[test]
    fn a_match_has_exactly_one_ball_and_the_query_filter_is_honest() {
        let mut app = match_app();
        let count = app
            .world_mut()
            .query_filtered::<&Position, With<Ball>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }
}
