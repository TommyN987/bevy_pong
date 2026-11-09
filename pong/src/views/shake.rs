//! Screen shake on a point (spec 1.6.2 MAY): the camera wobbles for a moment
//! and comes to rest exactly at the centre. Two sines of different
//! frequencies, both starting at zero, under an envelope that falls to zero:
//! the same wobble every point, with no randomness.

use std::f32::consts::TAU;
use std::time::Duration;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageReader;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res};
use bevy::math::Vec2;
use bevy::time::Time;
use bevy::transform::components::Transform;
use pong_core::domain::PointScored;

/// How long a shake lasts.
pub const SHAKE_FOR: Duration = Duration::from_millis(300);
/// The farthest the camera moves, in arena units.
pub const SHAKE_REACH: f32 = 1.5;

/// The wobble's frequencies across and up, in hertz: unequal, so it is not a
/// line.
const WOBBLE_HZ: Vec2 = Vec2::new(23.0, 17.0);

/// Shakes this entity's `Transform` (the camera's) after each point. Writes
/// x and y only.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shake {
    /// Time since the last point; `None` before the first one.
    since: Option<Duration>,
}

/// The camera's offset `elapsed` after a point: zero at the point, zero from
/// [`SHAKE_FOR`] on, within [`SHAKE_REACH`] between.
#[must_use]
pub fn shake_offset(elapsed: Duration) -> Vec2 {
    if elapsed >= SHAKE_FOR {
        return Vec2::ZERO;
    }
    let t = elapsed.as_secs_f32();
    let left = 1.0 - t / SHAKE_FOR.as_secs_f32();
    let phase = TAU * WOBBLE_HZ * t;
    Vec2::new(phase.x.sin(), phase.y.sin()) * (SHAKE_REACH * left * left)
}

/// Starts a shake on every `PointScored` and moves each [`Shake`] entity.
///
/// Precondition: the core plugin group, for `PointScored`.
pub struct ShakePlugin;

impl Plugin for ShakePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (start_shakes, shake).chain());
    }
}

fn start_shakes(mut points: MessageReader<PointScored>, mut shakes: Query<&mut Shake>) {
    if points.read().count() == 0 {
        return;
    }
    for mut shake in &mut shakes {
        shake.since = Some(Duration::ZERO);
    }
}

/// The camera rests at the centre, and nothing else moves it: the offset is
/// its x and y. Once a shake is over, the last write puts it back at zero.
fn shake(time: Res<Time>, mut shakes: Query<(&mut Shake, &mut Transform)>) {
    for (mut shake, mut transform) in &mut shakes {
        let Some(since) = shake.since else {
            continue;
        };
        let since = since + time.delta();
        let offset = shake_offset(since);
        transform.translation.x = offset.x;
        transform.translation.y = offset.y;
        shake.since = (since < SHAKE_FOR).then_some(since);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::math::Vec2;

    use super::{SHAKE_FOR, SHAKE_REACH, shake_offset};

    fn reach_over(from: Duration, to: Duration) -> f32 {
        (0..100)
            .map(|i| from + to.saturating_sub(from) * i / 100)
            .map(|t| shake_offset(t).length())
            .fold(0.0, f32::max)
    }

    #[test]
    fn a_shake_starts_and_ends_at_the_centre() {
        assert_eq!(shake_offset(Duration::ZERO), Vec2::ZERO);
        for after in [SHAKE_FOR, SHAKE_FOR * 2, Duration::from_secs(60)] {
            assert_eq!(shake_offset(after), Vec2::ZERO, "{after:?}");
        }
    }

    #[test]
    fn a_shake_moves_the_camera_within_its_reach_and_dies_down() {
        let half = SHAKE_FOR / 2;
        let early = reach_over(Duration::ZERO, half);
        let late = reach_over(half, SHAKE_FOR);
        assert!(early > SHAKE_REACH / 3.0, "it barely moves: {early}");
        assert!(early <= SHAKE_REACH * 2_f32.sqrt(), "too far: {early}");
        assert!(
            late < early / 2.0,
            "it does not die down: {early} then {late}"
        );
    }
}
