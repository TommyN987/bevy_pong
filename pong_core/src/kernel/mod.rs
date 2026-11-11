//! ECS-free math floor: pure functions over plain data and `bevy::math` types.
//!
//! Nothing here imports the rest of the crate (docs/architecture.md, rule 1 and 2).
//! Physics systems wrap these functions; the AI calls them directly, which is the
//! fairness guarantee of spec 1.2.

use std::f32::consts::FRAC_PI_2;
use std::fmt;

pub mod collide;
pub mod flight;
pub mod geometry;
pub mod integrate;
pub mod predict;
pub mod ramp;
pub mod random;
pub mod reflect;

#[cfg(test)]
pub(crate) mod test_support;

/// The set of values a kernel constructor accepts. Each variant owns both its
/// test and its wording, so the two cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Range {
    /// Finite and strictly above zero: lengths, speeds, caps.
    PositiveFinite,
    /// Finite and zero or above: increments that may be switched off.
    NonNegativeFinite,
    /// Radians in `0.0..FRAC_PI_2`.
    BelowRightAngle,
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PositiveFinite => "a positive finite number",
            Self::NonNegativeFinite => "zero or a positive finite number",
            Self::BelowRightAngle => {
                "an angle in radians from zero up to, excluding, a right angle"
            }
        })
    }
}

impl Range {
    /// Passes `value` through when it lies in this range.
    ///
    /// # Errors
    /// The value, with this range as what was expected.
    pub fn check(self, value: f32) -> Result<f32, OutOfRange> {
        let accepted = match self {
            Self::PositiveFinite => value.is_finite() && value > 0.0,
            Self::NonNegativeFinite => value.is_finite() && value >= 0.0,
            Self::BelowRightAngle => (0.0..FRAC_PI_2).contains(&value),
        };
        if accepted {
            Ok(value)
        } else {
            Err(OutOfRange {
                value,
                expected: self,
            })
        }
    }
}

/// A number refused by a kernel constructor: the value and the range it had to be in.
///
/// Kernel constructors are the single source of range truth for tuning values
/// (spec 1.4.8); tuning validation reports these rather than keeping its own table.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("{value} is out of range: expected {expected}")]
pub struct OutOfRange {
    /// The refused value.
    pub value: f32,
    /// The range it had to be in.
    pub expected: Range,
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::{OutOfRange, Range};

    #[test]
    fn each_range_accepts_its_values_and_refuses_the_rest() {
        assert_eq!(Range::PositiveFinite.check(0.5), Ok(0.5));
        assert!(Range::PositiveFinite.check(0.0).is_err());
        assert!(Range::PositiveFinite.check(f32::INFINITY).is_err());

        assert_eq!(Range::NonNegativeFinite.check(0.0), Ok(0.0));
        assert!(Range::NonNegativeFinite.check(-0.1).is_err());
        assert!(Range::NonNegativeFinite.check(f32::NAN).is_err());

        assert_eq!(Range::BelowRightAngle.check(0.0), Ok(0.0));
        assert!(Range::BelowRightAngle.check(FRAC_PI_2).is_err());
        assert!(Range::BelowRightAngle.check(-0.1).is_err());
    }

    #[test]
    fn a_refusal_carries_the_value_and_the_range_and_reads_as_one_sentence() {
        let err = Range::NonNegativeFinite.check(-1.0);
        assert_eq!(
            err,
            Err(OutOfRange {
                value: -1.0,
                expected: Range::NonNegativeFinite,
            })
        );
        assert_eq!(
            Range::PositiveFinite.to_string(),
            "a positive finite number"
        );
        assert_eq!(
            Range::NonNegativeFinite.to_string(),
            "zero or a positive finite number"
        );
        assert_eq!(
            Range::BelowRightAngle.to_string(),
            "an angle in radians from zero up to, excluding, a right angle"
        );
    }
}
