//! Ball reflection off paddles and walls, and the speed rule after a hit
//! (spec 1.2.4, 1.2.5, 1.2.7).

use bevy::math::Vec2;

use super::geometry::Side;
use super::{OutOfRange, Range};

/// Where a ball struck a paddle, as a fraction of the paddle's half-height.
///
/// Invariant: within `-1.0..=1.0`, zero at the paddle's center, positive toward
/// `+y`. A ball whose center passes the paddle's end (a corner clip) clamps to
/// the end. Precondition on [`HitOffset::new`]: `paddle_half_height` is
/// positive, which the paddle's clamped size setter (spec 1.4.3) guarantees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitOffset(f32);

impl HitOffset {
    /// Offset of a ball centered at `ball_y` on a paddle centered at
    /// `paddle_center_y` with the given half-height.
    #[must_use]
    pub fn new(ball_y: f32, paddle_center_y: f32, paddle_half_height: f32) -> Self {
        Self(((ball_y - paddle_center_y) / paddle_half_height).clamp(-1.0, 1.0))
    }

    /// The offset, within `-1.0..=1.0`.
    #[must_use]
    pub const fn value(self) -> f32 {
        self.0
    }
}

/// Largest angle from straight-across that a paddle can send the ball, in radians.
///
/// Invariant: within `0.0..FRAC_PI_2`. Zero makes every hit leave straight; a
/// right angle would send the ball along the paddle and is excluded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaxDeflection(f32);

impl MaxDeflection {
    /// # Errors
    /// `radians` is outside `0.0..FRAC_PI_2` or not a number.
    pub fn new(radians: f32) -> Result<Self, OutOfRange> {
        Range::BelowRightAngle.check(radians).map(Self)
    }

    /// The angle in radians.
    #[must_use]
    pub const fn radians(self) -> f32 {
        self.0
    }
}

/// Velocity of a ball leaving the paddle on `paddle` side.
///
/// The ball leaves toward the opposite side at angle `offset * max_angle` from
/// straight-across, toward the end it struck: a center hit leaves straight, an
/// end hit at `max_angle`, mirrored exactly between the two sides. The result
/// has length `speed`.
#[must_use]
pub fn paddle_reflection(
    paddle: Side,
    offset: HitOffset,
    speed: f32,
    max_angle: MaxDeflection,
) -> Vec2 {
    let (sin, cos) = (offset.value() * max_angle.radians()).sin_cos();
    Vec2::new(paddle.opposite().sign() * cos, sin) * speed
}

/// Velocity after a bounce off a flat surface with unit `normal`: the component
/// along the normal reverses, the rest is kept. Walls use this; paddles use
/// [`paddle_reflection`], which aims by hit offset instead.
#[must_use]
pub fn mirror(velocity: Vec2, normal: Vec2) -> Vec2 {
    velocity - 2.0 * velocity.dot(normal) * normal
}

/// Speed a ball leaves a paddle at: `serve_speed` plus the effects' summed
/// `gain`, never above `cap`. The one statement of the speed channel's bound
/// (ADR 0011); physics applies it with the cap every mode shares.
#[must_use]
pub fn launch_speed(serve_speed: f32, gain: f32, cap: f32) -> f32 {
    (serve_speed + gain).min(cap)
}

/// How ball speed changes over a rally (spec 1.2.5, 1.2.7): what each paddle
/// return adds, and the cap on the result. The Speed effect reads the
/// increase; the base applies the cap in [`SpeedRule::launch`] (M2 spec 2.0.4).
///
/// Invariants, enforced by [`SpeedRule::new`]: `per_hit_increase` is finite and
/// at least zero, so speed never falls; `cap` is positive and finite, so speed
/// is bounded after every hit and escalation never outruns the swept collision step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedRule {
    per_hit_increase: f32,
    cap: f32,
}

/// Why [`SpeedRule::new`] refused its values. As with
/// [`InvalidArena`](super::geometry::InvalidArena), the inner value is message, not `source`.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum InvalidSpeedRule {
    /// The per-hit increase was negative or not finite.
    #[error("per-hit speed increase {0}")]
    PerHitIncrease(OutOfRange),
    /// The cap was not positive and finite.
    #[error("speed cap {0}")]
    Cap(OutOfRange),
}

impl SpeedRule {
    /// Rule adding `per_hit_increase` units per second on every hit, never above `cap`.
    ///
    /// # Errors
    /// A value outside its range, naming which one.
    pub fn new(per_hit_increase: f32, cap: f32) -> Result<Self, InvalidSpeedRule> {
        let per_hit_increase = Range::NonNegativeFinite
            .check(per_hit_increase)
            .map_err(InvalidSpeedRule::PerHitIncrease)?;
        let cap = Range::PositiveFinite
            .check(cap)
            .map_err(InvalidSpeedRule::Cap)?;
        Ok(Self {
            per_hit_increase,
            cap,
        })
    }

    /// Speed a ball leaves a paddle at: `serve_speed` plus the effects' summed
    /// `gain`, never above `cap`. The cap is the base's bound on the speed
    /// channel (ADR 0011): whatever the effects add, the swept collision step
    /// and the single wall fold stay valid.
    #[must_use]
    pub fn launch(self, serve_speed: f32, gain: f32) -> f32 {
        launch_speed(serve_speed, gain, self.cap)
    }

    /// Units per second each paddle return adds, before the cap.
    #[must_use]
    pub const fn per_hit_increase(self) -> f32 {
        self.per_hit_increase
    }

    /// The highest speed a hit can produce.
    #[must_use]
    pub const fn cap(self) -> f32 {
        self.cap
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_3};

    use bevy::math::Vec2;

    use super::{HitOffset, InvalidSpeedRule, MaxDeflection, SpeedRule, mirror, paddle_reflection};
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::{assert_near, assert_vec_near};
    use crate::kernel::{OutOfRange, Range};

    const SPEED: f32 = 100.0;

    fn max_angle() -> MaxDeflection {
        MaxDeflection::new(FRAC_PI_3).expect("sixty degrees is below a right angle")
    }

    /// Offset of a ball at `ball_y` on a paddle centered at zero with half-height 6.
    fn offset(ball_y: f32) -> HitOffset {
        HitOffset::new(ball_y, 0.0, 6.0)
    }

    #[test]
    fn hit_offset_is_the_fraction_of_half_height_clamped_to_the_ends() {
        assert_near(offset(0.0).value(), 0.0, 1e-6);
        assert_near(offset(3.0).value(), 0.5, 1e-6);
        assert_near(offset(-6.0).value(), -1.0, 1e-6);
        assert_near(offset(9.0).value(), 1.0, 1e-6);
        assert_near(HitOffset::new(13.0, 10.0, 6.0).value(), 0.5, 1e-6);
    }

    #[test]
    fn max_deflection_accepts_zero_up_to_but_excluding_a_right_angle() {
        assert!(MaxDeflection::new(0.0).is_ok());
        assert!(MaxDeflection::new(1.5).is_ok());
        assert_eq!(
            MaxDeflection::new(FRAC_PI_2),
            Err(OutOfRange {
                value: FRAC_PI_2,
                expected: Range::BelowRightAngle,
            })
        );
        assert!(MaxDeflection::new(-0.1).is_err());
        assert!(MaxDeflection::new(f32::NAN).is_err());
        assert_near(max_angle().radians(), FRAC_PI_3, 1e-6);
    }

    #[test]
    fn center_hit_leaves_straight_away_from_the_paddle() {
        let out = paddle_reflection(Side::Right, offset(0.0), SPEED, max_angle());
        assert_vec_near(out, Vec2::new(-SPEED, 0.0), 1e-4);
    }

    #[test]
    fn end_hit_leaves_at_the_maximum_angle_toward_the_end_struck() {
        let out = paddle_reflection(Side::Right, offset(6.0), SPEED, max_angle());
        let expected = Vec2::new(-SPEED * FRAC_PI_3.cos(), SPEED * FRAC_PI_3.sin());
        assert_vec_near(out, expected, 1e-4);
    }

    #[test]
    fn both_sides_mirror_each_other_and_offset_sign_picks_up_or_down() {
        let off_right = paddle_reflection(Side::Right, offset(3.0), SPEED, max_angle());
        let off_left = paddle_reflection(Side::Left, offset(3.0), SPEED, max_angle());
        assert_vec_near(off_left, Vec2::new(-off_right.x, off_right.y), 1e-4);

        let off_left_low = paddle_reflection(Side::Left, offset(-3.0), SPEED, max_angle());
        assert_vec_near(off_left_low, Vec2::new(off_left.x, -off_left.y), 1e-4);
    }

    #[test]
    fn wall_mirror_flips_only_the_component_along_the_normal() {
        assert_vec_near(
            mirror(Vec2::new(30.0, -40.0), Vec2::Y),
            Vec2::new(30.0, 40.0),
            1e-6,
        );
        assert_vec_near(
            mirror(Vec2::new(30.0, 40.0), Vec2::NEG_Y),
            Vec2::new(30.0, -40.0),
            1e-6,
        );
        assert_vec_near(
            mirror(Vec2::new(30.0, 40.0), Vec2::NEG_X),
            Vec2::new(-30.0, 40.0),
            1e-6,
        );
    }

    const CAP: f32 = 250.0;

    fn speed_rule(per_hit_increase: f32) -> SpeedRule {
        SpeedRule::new(per_hit_increase, CAP)
            .expect("a non-negative increase and a positive cap form a valid rule")
    }

    #[test]
    fn speed_rule_errors_read_as_one_sentence_naming_the_field() {
        let Err(increase) = SpeedRule::new(-1.0, CAP) else {
            panic!("negative increase accepted");
        };
        assert_eq!(
            increase.to_string(),
            "per-hit speed increase -1 is out of range: expected zero or a positive finite number"
        );
        let Err(cap) = SpeedRule::new(0.0, 0.0) else {
            panic!("zero cap accepted");
        };
        assert_eq!(
            cap.to_string(),
            "speed cap 0 is out of range: expected a positive finite number"
        );
    }

    #[test]
    fn speed_rule_rejects_negative_increase_and_non_positive_cap_naming_the_field() {
        assert_eq!(
            SpeedRule::new(-1.0, CAP),
            Err(InvalidSpeedRule::PerHitIncrease(OutOfRange {
                value: -1.0,
                expected: Range::NonNegativeFinite,
            }))
        );
        assert_eq!(
            SpeedRule::new(0.0, 0.0),
            Err(InvalidSpeedRule::Cap(OutOfRange {
                value: 0.0,
                expected: Range::PositiveFinite,
            }))
        );
        assert!(matches!(
            SpeedRule::new(f32::NAN, CAP),
            Err(InvalidSpeedRule::PerHitIncrease(_))
        ));
        assert!(matches!(
            SpeedRule::new(1.0, f32::INFINITY),
            Err(InvalidSpeedRule::Cap(_))
        ));
        assert!(SpeedRule::new(0.0, CAP).is_ok());
    }

    #[test]
    fn a_thousand_reflections_never_exceed_the_cap() {
        let rule = speed_rule(7.0);
        let mut velocity = Vec2::new(80.0, 0.0);
        for hit in 0..1000 {
            let paddle = if hit % 2 == 0 {
                Side::Right
            } else {
                Side::Left
            };
            let returns = f32::from(u16::try_from(hit + 1).expect("a thousand fits"));
            let speed = rule.launch(80.0, rule.per_hit_increase() * returns);
            velocity = paddle_reflection(paddle, offset(2.0), speed, max_angle());
            assert!(
                velocity.length() <= CAP + 1e-3,
                "hit {hit}: {}",
                velocity.length()
            );
        }
        assert_near(velocity.length(), CAP, 1e-3);
    }

    #[test]
    fn zero_increase_leaves_speed_unchanged() {
        let rule = speed_rule(0.0);
        assert_near(rule.launch(80.0, rule.per_hit_increase()), 80.0, 1e-6);
        assert_near(rule.launch(CAP, rule.per_hit_increase()), CAP, 1e-6);
    }

    #[test]
    fn positive_increase_is_monotone_until_the_cap_then_flat() {
        let rule = speed_rule(60.0);
        let speeds: Vec<f32> = (1..=6_u16)
            .map(|returns| rule.launch(80.0, rule.per_hit_increase() * f32::from(returns)))
            .collect();
        for pair in speeds.windows(2) {
            assert!(pair[1] >= pair[0], "speed fell: {pair:?}");
        }
        // From 80: 140, 200, 250 (capped), 250, 250, 250.
        assert_near(speeds[1], 200.0, 1e-6);
        assert_near(speeds[2], CAP, 1e-6);
        assert_near(speeds[5], CAP, 1e-6);
    }
}
