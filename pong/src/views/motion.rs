//! Where bodies are drawn (spec 1.6.2): `Transform` interpolated between the
//! last two fixed-step samples of each body's centre, so motion is smooth at
//! any display rate. The picture runs one fixed step behind the simulation.
//!
//! Samples are taken on every fixed tick, including ticks in which the step
//! gate keeps everything still, so a stopped body is drawn still: once
//! nothing moves, the two samples agree after one tick. The core's
//! `StepStart` updates only on live steps and would replay the last step's
//! motion through every pause (plan 4.8, declined; phase 4 report).

use bevy::app::{App, FixedPostUpdate, Plugin, RunFixedMainLoop, RunFixedMainLoopSystems};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Added, With};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::Vec2;
use bevy::state::condition::in_state;
use bevy::time::{Fixed, Time};
use bevy::transform::components::Transform;
use pong_core::features::ball::{Ball, Position, Snap};
use pong_core::features::paddle::Paddle;
use pong_core::features::tuning::Tuning;
use pong_core::flow::states::AppState;

/// The once-a-frame view work after the fixed steps, in this order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewSystems {
    /// New bodies get their samples and visuals.
    Attach,
    /// Teleported bodies drop their old sample.
    Snap,
    /// `Transform` follows the samples.
    Interpolate,
}

/// Samples bodies every fixed tick and draws them between samples once a
/// frame, during a match, where the bodies exist.
pub struct MotionPlugin;

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            RunFixedMainLoop,
            (
                ViewSystems::Attach,
                ViewSystems::Snap,
                ViewSystems::Interpolate,
            )
                .chain()
                .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
                .run_if(in_state(AppState::InMatch)),
        )
        .add_systems(
            FixedPostUpdate,
            (sample_balls, sample_paddles).run_if(in_state(AppState::InMatch)),
        )
        .add_systems(
            RunFixedMainLoop,
            (
                (attach_ball_samples, attach_paddle_samples).in_set(ViewSystems::Attach),
                snap_teleported_balls.in_set(ViewSystems::Snap),
                interpolate.in_set(ViewSystems::Interpolate),
            ),
        );
    }
}

/// A body's centre at the last two fixed ticks, in arena units.
///
/// Invariant: `current` is the body's centre at the latest tick, or at the
/// latest snap if one came after it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Transform)]
pub struct Samples {
    previous: Vec2,
    current: Vec2,
}

impl Samples {
    /// Both samples at `centre`: drawn there until it moves.
    #[must_use]
    pub const fn at(centre: Vec2) -> Self {
        Self {
            previous: centre,
            current: centre,
        }
    }

    /// The centre at a new tick.
    pub const fn push(&mut self, centre: Vec2) {
        self.previous = self.current;
        self.current = centre;
    }

    /// Where to draw the body `fraction` of a step after the latest tick, in
    /// `0.0..1.0`: from the previous sample at zero toward the current one.
    #[must_use]
    pub fn at_fraction(self, fraction: f32) -> Vec2 {
        self.previous.lerp(self.current, fraction)
    }
}

fn paddle_centre(paddle: Paddle, tuning: &Tuning) -> Vec2 {
    paddle.rect(tuning).center()
}

fn attach_ball_samples(mut commands: Commands, balls: Query<(Entity, &Position), Added<Ball>>) {
    for (entity, position) in &balls {
        commands.entity(entity).insert(Samples::at(position.0));
    }
}

fn attach_paddle_samples(
    mut commands: Commands,
    tuning: Res<Tuning>,
    paddles: Query<(Entity, &Paddle), Added<Paddle>>,
) {
    for (entity, paddle) in &paddles {
        commands
            .entity(entity)
            .insert(Samples::at(paddle_centre(*paddle, &tuning)));
    }
}

fn sample_balls(mut balls: Query<(&Position, &mut Samples), With<Ball>>) {
    for (position, mut samples) in &mut balls {
        samples.push(position.0);
    }
}

fn sample_paddles(tuning: Res<Tuning>, mut paddles: Query<(&Paddle, &mut Samples)>) {
    for (paddle, mut samples) in &mut paddles {
        samples.push(paddle_centre(*paddle, &tuning));
    }
}

/// A cut, not a slide: a teleported ball (spawn, centre spot) is drawn where
/// it now is, even on a frame that runs no fixed tick, and held still there
/// until the next tick gives it a second sample. Consumes the core's [`Snap`],
/// as its contract asks of the view.
fn snap_teleported_balls(
    mut commands: Commands,
    mut balls: Query<(Entity, &Position, &mut Samples), With<Snap>>,
) {
    for (entity, position, mut samples) in &mut balls {
        *samples = Samples::at(position.0);
        commands.entity(entity).remove::<Snap>();
    }
}

/// Writes x and y only; z is the body's layer, set by its visuals.
fn interpolate(time: Res<Time<Fixed>>, mut bodies: Query<(&Samples, &mut Transform)>) {
    let fraction = time.overstep_fraction();
    for (samples, mut transform) in &mut bodies {
        let centre = samples.at_fraction(fraction);
        transform.translation.x = centre.x;
        transform.translation.y = centre.y;
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::Samples;

    #[test]
    fn a_push_moves_the_drawn_range_one_sample_on() {
        let mut samples = Samples::at(Vec2::ZERO);
        samples.push(Vec2::new(2.0, 0.0));
        assert_eq!(samples.at_fraction(0.0), Vec2::ZERO);
        assert_eq!(samples.at_fraction(0.5), Vec2::new(1.0, 0.0));
        samples.push(Vec2::new(4.0, 0.0));
        assert_eq!(samples.at_fraction(0.0), Vec2::new(2.0, 0.0));
    }

    #[test]
    fn two_equal_samples_draw_the_body_still_at_any_fraction() {
        let mut samples = Samples::at(Vec2::new(3.0, 1.0));
        samples.push(Vec2::new(3.0, 1.0));
        for fraction in [0.0, 0.3, 0.99] {
            assert_eq!(samples.at_fraction(fraction), Vec2::new(3.0, 1.0));
        }
    }
}
