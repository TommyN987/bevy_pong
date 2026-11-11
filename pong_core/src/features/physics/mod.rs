//! Physics (spec 1.4.4): thin systems over the kernel. Integrate moves the ball
//! between the walls under its flight's forces; collide sweeps it against the
//! paddles, and a paddle return starts the ball's next flight (M2 spec 2.2.4).
//! The AI predicts with the same kernel functions these systems call.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res};
use bevy::math::Vec2;
use bevy::time::{Fixed, Time};

use crate::domain::{BallHitPaddle, BallHitWall};
use crate::features::ball::{Ball, Position, Velocity};
use crate::features::effects::{ActiveEffects, BallFlight};
use crate::features::paddle::Paddle;
use crate::features::tuning::Tuning;
use crate::flow::phases::{BallInPlay, FixedStepPhase};
use crate::kernel::collide::{Face, Impact, swept_circle_rect};
use crate::kernel::flight::{FlightStart, Hit};
use crate::kernel::integrate::{Motion, advance_between_walls};
use crate::kernel::reflect::{HitOffset, mirror, paddle_reflection};

/// Moves every ball in the integrate phase and resolves paddle contact in the
/// collide phase. Emits [`BallHitWall`] and [`BallHitPaddle`].
///
/// Makes [`StepStart`] a required component of [`Ball`], so the ball feature
/// spawns balls without naming it. Precondition (Bevy's): no `Ball` exists
/// before this plugin builds; the ball feature spawns in systems.
///
/// Reads the ball's [`BallFlight`] and the [`ActiveEffects`], which the
/// effects feature provides.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BallHitWall>()
            .add_message::<BallHitPaddle>()
            .register_required_components::<Ball, StepStart>()
            .add_systems(
                FixedUpdate,
                (
                    integrate_ball.in_set(FixedStepPhase::Integrate),
                    collide_ball_with_paddles.in_set(FixedStepPhase::Collide),
                )
                    .in_set(BallInPlay),
            );
    }
}

/// Where a ball's centre was when the current fixed step began: the start of
/// the path the collide phase sweeps, and the earlier end of a view's
/// interpolation (plan 4.8). Written by integration on every step before
/// anything reads it.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct StepStart(Vec2);

impl StepStart {
    /// The centre at the start of the step.
    #[must_use]
    pub const fn position(self) -> Vec2 {
        self.0
    }
}

fn integrate_ball(
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    mut walls: MessageWriter<BallHitWall>,
    mut balls: Query<
        (
            &mut Position,
            &mut Velocity,
            &mut BallFlight,
            &mut StepStart,
        ),
        With<Ball>,
    >,
) {
    let dt = time.delta_secs();
    for (mut position, mut velocity, mut flight, mut step_start) in &mut balls {
        step_start.0 = position.0;
        let motion = Motion {
            position: position.0,
            velocity: velocity.0,
        };
        let force = flight.flight().force(motion, dt);
        let step = advance_between_walls(tuning.arena(), tuning.ball().radius(), motion, force, dt);
        flight.tick();
        position.0 = step.motion.position;
        velocity.0 = step.motion.velocity;
        if step.bounced {
            walls.write(BallHitWall);
        }
    }
}

/// Sweeps each ball along the path it took this step against every paddle and
/// resolves the earliest contact. A face hit is a return: it starts the
/// ball's next flight, asking each effect in play once, and reflects by hit
/// offset at the speed the new flight gives. An end hit mirrors the vertical
/// velocity and starts no flight: the ball flies on toward the goal. The
/// remainder of the step is travelled along the new velocity through the same
/// wall-bounded step.
///
/// The sweep uses the straight line from the step's start to its end. A step
/// that also bounced off a wall is swept along the folded line, which is wrong
/// only for a ball striking a paddle end in the same step as a wall; the
/// sanity phase catches anything that escapes.
fn collide_ball_with_paddles(
    tuning: Res<Tuning>,
    time: Res<Time<Fixed>>,
    active: Res<ActiveEffects>,
    paddles: Query<&Paddle>,
    mut hits: MessageWriter<BallHitPaddle>,
    mut walls: MessageWriter<BallHitWall>,
    mut balls: Query<(&StepStart, &mut Position, &mut Velocity, &mut BallFlight), With<Ball>>,
) {
    let arena = tuning.arena();
    let ball = tuning.ball();
    for (step_start, mut position, mut velocity, mut flight) in &mut balls {
        let start = step_start.position();
        let displacement = position.0 - start;
        let mut earliest: Option<(Impact, Paddle)> = None;
        for paddle in &paddles {
            if let Some(impact) =
                swept_circle_rect(start, ball.radius(), displacement, paddle.rect(&tuning))
                && earliest.is_none_or(|(first, _)| impact.toi < first.toi)
            {
                earliest = Some((impact, *paddle));
            }
        }
        let Some((impact, paddle)) = earliest else {
            continue;
        };
        let contact = start + displacement * impact.toi;
        let reflected = match impact.face {
            Face::Left | Face::Right => {
                let returns = flight.flight().returns().saturating_add(1);
                flight.begin(active.start_flight(FlightStart {
                    returns,
                    hit: Some(Hit {
                        side: paddle.side(),
                        paddle_motion: paddle.step_motion(),
                    }),
                }));
                let offset = HitOffset::new(contact.y, paddle.center_y(), paddle.height() / 2.0);
                let speed = ball
                    .speed_rule()
                    .launch(ball.serve_speed(), flight.flight().speed_gain());
                paddle_reflection(paddle.side(), offset, speed, ball.max_deflection())
            }
            Face::Top | Face::Bottom => mirror(velocity.0, impact.face.normal()),
        };
        // The rest of the step continues from the contact along the new
        // velocity, between the walls and under the flight like any other
        // movement.
        let leaving = Motion {
            position: contact,
            velocity: reflected,
        };
        let remainder = advance_between_walls(
            arena,
            ball.radius(),
            leaving,
            flight.flight().force(leaving, time.delta_secs()),
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

    use bevy::app::{App, PluginGroupBuilder};
    use bevy::ecs::entity::Entity;
    use bevy::ecs::query::With;
    use bevy::math::Vec2;
    use bevy::time::{Fixed, Time};

    use crate::domain::{BallHitPaddle, BallHitWall, Controller, MatchConfig, Seat, SeatPlayers};
    use crate::features::ball::{Ball, Position, Velocity};
    use crate::features::paddle::Paddle;
    use crate::features::sanity::SanityPlugin;
    use crate::features::tuning::Tuning;
    use crate::kernel::flight::Flight;
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::Motion;
    use crate::kernel::predict::crossing_y;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{
        ball_state, boot, frame, headless_app_with, place_ball, record, start_match, take_recorded,
    };

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            players: SeatPlayers::GUESTS,
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn match_app() -> App {
        match_app_with(|core| core)
    }

    fn match_app_with(adjust: impl FnOnce(PluginGroupBuilder) -> PluginGroupBuilder) -> App {
        let mut app = headless_app_with(1, adjust);
        record::<BallHitWall>(&mut app);
        record::<BallHitPaddle>(&mut app);
        boot(&mut app);
        start_match(&mut app, two_humans());
        app
    }

    /// Into the right paddle within one step.
    const INTO_RIGHT: (Vec2, Vec2) = (Vec2::new(72.0, 2.0), Vec2::new(200.0, 0.0));
    /// Into the left paddle within one step, low.
    const INTO_LEFT: (Vec2, Vec2) = (Vec2::new(-72.0, -3.0), Vec2::new(-200.0, 0.0));

    /// Where a lone ball launched from `(position, velocity)` is after one step.
    fn alone_after_one_step((position, velocity): (Vec2, Vec2)) -> (Vec2, Vec2) {
        let mut app = match_app();
        place_ball(&mut app, position, velocity);
        frame(&mut app);
        ball_state(&mut app)
    }

    fn state_of(app: &App, ball: Entity) -> (Vec2, Vec2) {
        let entity = app.world().entity(ball);
        let position = entity.get::<Position>().expect("a ball has a position");
        let velocity = entity.get::<Velocity>().expect("a ball has a velocity");
        (position.0, velocity.0)
    }

    /// Plan 3.15: step state lives on each ball, so two balls in one step are
    /// each swept from their own start, exactly as if alone.
    #[test]
    fn two_balls_in_one_step_each_collide_exactly_as_if_alone() {
        // M1 plays with one ball, and the sanity phase faults on a second
        // (plan 6.1); this test is about step state, so it runs without it.
        let mut app = match_app_with(PluginGroupBuilder::disable::<SanityPlugin>);
        place_ball(&mut app, INTO_RIGHT.0, INTO_RIGHT.1);
        let first = app
            .world_mut()
            .query_filtered::<Entity, With<Ball>>()
            .single(app.world())
            .expect("the match ball");
        let second = app
            .world_mut()
            .spawn((Ball, Position(INTO_LEFT.0), Velocity(INTO_LEFT.1)))
            .id();
        frame(&mut app);
        assert_eq!(state_of(&app, first), alone_after_one_step(INTO_RIGHT));
        assert_eq!(state_of(&app, second), alone_after_one_step(INTO_LEFT));
        let mut sides: Vec<Side> = take_recorded::<BallHitPaddle>(&mut app)
            .into_iter()
            .map(|hit| hit.side)
            .collect();
        sides.sort_by_key(|side| *side == Side::Right);
        assert_eq!(sides, [Side::Left, Side::Right]);
    }

    #[test]
    fn a_step_into_the_wall_folds_the_ball_back_and_emits_ball_hit_wall() {
        let mut app = match_app();
        place_ball(&mut app, Vec2::new(0.0, 43.0), Vec2::new(60.0, 120.0));
        frame(&mut app);
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
    fn a_ball_faster_than_the_paddle_is_thick_still_hits_the_face_and_deflects() {
        let mut app = match_app();
        let tuning = app.world().resource::<Tuning>().clone();
        // Right paddle face at x = 74, ball radius 1: contact at 73. From 72 at
        // 200 per second the step travels 3.33, more than the paddle's thickness.
        place_ball(&mut app, Vec2::new(72.0, 2.0), Vec2::new(200.0, 0.0));
        frame(&mut app);
        let (position, velocity) = ball_state(&mut app);
        assert!(position.x < 73.0, "{position:?}");
        assert!(velocity.x < 0.0 && velocity.y > 0.0, "{velocity:?}");
        // M2 spec 2.0.4: the launch speed follows the rally's return count,
        // here the first, whatever speed the ball arrived at.
        let shape = tuning.ball();
        let first_return = shape
            .speed_rule()
            .launch(shape.serve_speed(), shape.speed_rule().per_hit_increase());
        assert_near(velocity.length(), first_return, 1e-3);
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
            frame(&mut app);
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
        let mut returns = 0_u16;
        for _ in 0..3_000 {
            frame(&mut app);
            for _ in take_recorded::<BallHitPaddle>(&mut app) {
                returns += 1;
                expected = rule.launch(80.0, rule.per_hit_increase() * f32::from(returns));
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
        let predicted = crossing_y(
            tuning.arena(),
            tuning.ball().radius(),
            start,
            &Flight::default(),
            target_x,
            dt,
        )
        .expect("the ball flies toward the target");
        let mut before = start.position;
        for _ in 0..1_000 {
            frame(&mut app);
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
