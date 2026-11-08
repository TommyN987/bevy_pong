//! Where the ball's trail is drawn (spec 1.6.2 SHOULD): [`GHOSTS`] ghosts of
//! the ball at its last few fixed ticks. The ball is drawn between its last
//! two samples ([`super::motion`]); ghost `back` is drawn the same way,
//! `back` ticks further into the past, so the trail is as smooth as the ball
//! at any display rate. How a ghost looks is [`super::ball`]'s.

use bevy::app::{App, FixedPostUpdate, Plugin, RunFixedMainLoop};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Added, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::Vec2;
use bevy::state::condition::in_state;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::time::{Fixed, Time};
use bevy::transform::components::Transform;
use pong_core::features::ball::{Ball, Position, Snap};
use pong_core::flow::states::AppState;

use super::motion::ViewSystems;

/// Ghosts behind each ball: at 60 ticks a second, the last 0.13 s.
pub const GHOSTS: u8 = 8;

/// Samples kept: the ball's two, then one more per ghost.
const KEPT: usize = GHOSTS as usize + 2;

/// A ball's centre at its last ticks, newest first, in arena units.
///
/// Invariant: entry 0 is the centre at the latest tick, or at the latest snap
/// if one came after it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Trail([Vec2; KEPT]);

impl Trail {
    /// Every sample at `centre`: the ghosts sit on the ball until it moves.
    #[must_use]
    pub const fn at(centre: Vec2) -> Self {
        Self([centre; KEPT])
    }

    /// The centre at a new tick; the oldest is dropped.
    pub fn push(&mut self, centre: Vec2) {
        self.0.rotate_right(1);
        self.0[0] = centre;
    }

    /// Where ghost `back` (1 to [`GHOSTS`]) is drawn `fraction` of a step
    /// after the latest tick: between the samples `back + 1` and `back` ticks
    /// old, as the ball is between 1 and 0.
    #[must_use]
    pub fn ghost_at(&self, back: u8, fraction: f32) -> Vec2 {
        let back = usize::from(back.clamp(1, GHOSTS));
        self.0[back + 1].lerp(self.0[back], fraction)
    }
}

/// One ghost of `ball`, `back` ticks behind it; it lives for the match, as
/// its ball does.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailGhost {
    ball: Entity,
    back: u8,
}

impl TrailGhost {
    /// How many ticks behind its ball, 1 to [`GHOSTS`].
    #[must_use]
    pub const fn back(self) -> u8 {
        self.back
    }
}

/// Gives each ball its trail of ghosts during a match and draws them behind
/// it.
///
/// Precondition: [`super::motion::MotionPlugin`], whose sets order the work.
pub struct TrailPlugin;

impl Plugin for TrailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            sample_trails.run_if(in_state(AppState::InMatch)),
        )
        .add_systems(
            RunFixedMainLoop,
            (
                attach_trails.in_set(ViewSystems::Attach),
                snap_trails.in_set(ViewSystems::Snap),
                place_ghosts.in_set(ViewSystems::Interpolate),
            ),
        );
    }
}

fn attach_trails(mut commands: Commands, balls: Query<(Entity, &Position), Added<Ball>>) {
    for (ball, position) in &balls {
        commands.entity(ball).insert(Trail::at(position.0));
        for back in 1..=GHOSTS {
            commands.spawn((
                TrailGhost { ball, back },
                Transform::from_translation(position.0.extend(0.0)),
                DespawnOnExit(AppState::InMatch),
            ));
        }
    }
}

fn sample_trails(mut balls: Query<(&Position, &mut Trail), With<Ball>>) {
    for (position, mut trail) in &mut balls {
        trail.push(position.0);
    }
}

/// A cut for the trail too: every ghost onto the teleported ball. `Snap` is
/// read here and consumed by [`super::motion`] in the same set.
fn snap_trails(mut balls: Query<(&Position, &mut Trail), With<Snap>>) {
    for (position, mut trail) in &mut balls {
        *trail = Trail::at(position.0);
    }
}

/// Writes x and y only; z and size are the ghost's look.
fn place_ghosts(
    time: Res<Time<Fixed>>,
    trails: Query<&Trail>,
    mut ghosts: Query<(&TrailGhost, &mut Transform)>,
) {
    let fraction = time.overstep_fraction();
    for (ghost, mut transform) in &mut ghosts {
        if let Ok(trail) = trails.get(ghost.ball) {
            let centre = trail.ghost_at(ghost.back, fraction);
            transform.translation.x = centre.x;
            transform.translation.y = centre.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::{GHOSTS, Trail};

    #[test]
    fn a_ghost_is_drawn_between_the_two_samples_its_number_of_ticks_back() {
        let mut trail = Trail::at(Vec2::ZERO);
        for x in 1..=12_u8 {
            trail.push(Vec2::new(f32::from(x), 0.0));
        }
        // Newest 12, so the ball is drawn from 11 toward 12.
        assert_eq!(trail.ghost_at(1, 0.0), Vec2::new(10.0, 0.0));
        assert_eq!(trail.ghost_at(1, 0.5), Vec2::new(10.5, 0.0));
        assert_eq!(
            trail.ghost_at(GHOSTS, 0.0),
            Vec2::new(11.0 - f32::from(GHOSTS), 0.0)
        );
    }

    #[test]
    fn a_trail_at_one_spot_draws_every_ghost_there() {
        let trail = Trail::at(Vec2::new(3.0, -2.0));
        for back in 1..=GHOSTS {
            assert_eq!(trail.ghost_at(back, 0.7), Vec2::new(3.0, -2.0));
        }
    }
}
