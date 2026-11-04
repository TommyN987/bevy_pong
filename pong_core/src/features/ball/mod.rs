//! Ball (spec 1.4.2): its components, its lifetime over a match, and the serve.
//! Movement and collision belong to physics.

use bevy::app::{App, FixedUpdate, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec2;
use bevy::state::state::OnEnter;
use bevy::state::state_scoped::DespawnOnExit;

use crate::domain::{Random, Serve};
use crate::features::tuning::Tuning;
use crate::flow::phases::FixedStepPhase;
use crate::flow::states::{AppState, MatchState};

/// Spawns the ball for the length of a match, puts it back on the centre spot
/// at every countdown, and launches it on [`Serve`].
pub struct BallPlugin;

impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Serve>()
            .add_systems(OnEnter(AppState::InMatch), spawn_ball)
            .add_systems(OnEnter(MatchState::Countdown), place_ball_on_centre_spot)
            .add_systems(FixedUpdate, serve_on_request.in_set(FixedStepPhase::Intent));
    }
}

/// The one ball of a match.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ball;

/// Center of a body in arena units. The shell interpolates its `Transform`
/// from this between fixed steps.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Position(pub Vec2);

/// Change of position per second.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity(pub Vec2);

/// The body teleported: the view must not interpolate across the jump.
/// Set by the core when the ball spawns and when it returns to the centre spot
/// for a countdown; removed by the view after one frame.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snap;

fn spawn_ball(mut commands: Commands) {
    commands.spawn((
        Ball,
        Position::default(),
        Velocity::default(),
        Snap,
        DespawnOnExit(AppState::InMatch),
    ));
}

/// Every countdown starts with the ball at rest on the centre spot (ADR 0003),
/// wherever the last point left it. The first countdown of a match finds it
/// there already: the spawn's commands apply before this schedule runs.
fn place_ball_on_centre_spot(
    mut commands: Commands,
    mut balls: Query<(Entity, &mut Position, &mut Velocity), With<Ball>>,
) {
    for (entity, mut position, mut velocity) in &mut balls {
        position.0 = Vec2::ZERO;
        velocity.0 = Vec2::ZERO;
        commands.entity(entity).insert(Snap);
    }
}

/// Launches the ball toward the requested side at a random angle inside the
/// serve cone, from the seeded source.
///
/// Precondition: the ball rests on the centre spot, where the countdown that
/// precedes every serve put it.
fn serve_on_request(
    mut serves: MessageReader<Serve>,
    mut random: ResMut<Random>,
    tuning: Res<Tuning>,
    mut balls: Query<&mut Velocity, With<Ball>>,
) {
    let Some(serve) = serves.read().last() else {
        return;
    };
    let ball = tuning.ball();
    let cone = ball.serve_cone().radians();
    let angle = random.next_f32_in(-cone, cone);
    let (sin, cos) = angle.sin_cos();
    let launch = Vec2::new(serve.toward.sign() * cos, sin) * ball.serve_speed();
    for mut velocity in &mut balls {
        velocity.0 = launch;
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use bevy::app::App;
    use bevy::ecs::entity::Entity;
    use bevy::ecs::message::Messages;
    use bevy::ecs::query::With;
    use bevy::math::Vec2;
    use bevy::state::state::NextState;

    use super::{Ball, Position, Snap, Velocity};
    use crate::domain::{Controller, MatchConfig, Seat, Serve};
    use crate::features::tuning::Tuning;
    use crate::flow::states::MatchState;
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::{assert_near, assert_vec_near};
    use crate::testing::{
        boot, enter_match, frame, headless_app, match_state, place_ball, start_match,
    };

    fn two_humans() -> MatchConfig {
        MatchConfig {
            left: Controller::Human(Seat::Left),
            right: Controller::Human(Seat::Right),
            points_to_win: NonZeroU8::new(10).expect("ten is non-zero"),
        }
    }

    fn ball(app: &mut App) -> (Position, Velocity, bool) {
        let mut query = app
            .world_mut()
            .query::<(&Position, &Velocity, Option<&Snap>, &Ball)>();
        let mut balls = query.iter(app.world());
        let (position, velocity, snap, _) = balls.next().expect("a ball exists");
        assert!(balls.next().is_none(), "more than one ball");
        (*position, *velocity, snap.is_some())
    }

    fn serve(app: &mut App, toward: Side) {
        app.world_mut()
            .resource_mut::<Messages<Serve>>()
            .write(Serve { toward });
        frame(app);
    }

    #[test]
    fn entering_a_match_spawns_one_ball_at_rest_in_the_center() {
        let mut app = headless_app(1);
        boot(&mut app);
        enter_match(&mut app, two_humans());
        let (position, velocity, _) = ball(&mut app);
        assert_vec_near(position.0, Vec2::ZERO, 1e-6);
        assert_vec_near(velocity.0, Vec2::ZERO, 1e-6);
    }

    /// ADR 0003: a countdown after a point starts with the ball at rest on the
    /// centre spot, and the view is told not to interpolate the jump.
    #[test]
    fn entering_a_countdown_puts_the_ball_at_rest_on_the_centre_spot_and_snaps() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        place_ball(&mut app, Vec2::new(83.0, -20.0), Vec2::new(90.0, 15.0));
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<Ball>>()
            .single(app.world())
            .expect("one ball");
        app.world_mut().entity_mut(entity).remove::<Snap>();
        app.world_mut()
            .resource_mut::<NextState<MatchState>>()
            .set(MatchState::Countdown);
        frame(&mut app);
        assert_eq!(match_state(&app), MatchState::Countdown);
        let (position, velocity, snapped) = ball(&mut app);
        assert_vec_near(position.0, Vec2::ZERO, 1e-6);
        assert_vec_near(velocity.0, Vec2::ZERO, 1e-6);
        assert!(snapped);
    }

    #[test]
    fn a_serve_launches_toward_the_given_side_at_serve_speed_within_the_cone() {
        let mut app = headless_app(1);
        boot(&mut app);
        start_match(&mut app, two_humans());
        let ball_tuning = app.world().resource::<Tuning>().ball();
        serve(&mut app, Side::Right);
        let (_, velocity, _) = ball(&mut app);
        assert!(velocity.0.x > 0.0, "{velocity:?}");
        assert_near(velocity.0.length(), ball_tuning.serve_speed(), 1e-3);
        let angle = velocity.0.y.atan2(velocity.0.x).abs();
        assert!(
            angle <= ball_tuning.serve_cone().radians() + 1e-6,
            "{angle}"
        );

        serve(&mut app, Side::Left);
        let (_, velocity, _) = ball(&mut app);
        assert!(velocity.0.x < 0.0, "{velocity:?}");
    }

    #[test]
    fn the_same_seed_reproduces_the_same_serve_angle() {
        let mut first = headless_app(7);
        boot(&mut first);
        start_match(&mut first, two_humans());
        serve(&mut first, Side::Right);
        let mut second = headless_app(7);
        boot(&mut second);
        start_match(&mut second, two_humans());
        serve(&mut second, Side::Right);
        assert_eq!(ball(&mut first).1, ball(&mut second).1);
        let mut other = headless_app(8);
        boot(&mut other);
        start_match(&mut other, two_humans());
        serve(&mut other, Side::Right);
        assert_ne!(ball(&mut first).1, ball(&mut other).1);
    }
}
