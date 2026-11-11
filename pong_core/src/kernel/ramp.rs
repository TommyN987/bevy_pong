//! The curve every effect's dial rides (M2 spec 2.3.9): from one value to
//! another over a span of the rally's returns.

/// A dial's value as a function of the rally's return count: `from` until
/// `starts_at` returns, then linear, then `to` from `full_at` returns on.
///
/// Invariant, enforced by [`Ramp::new`]: `starts_at <= full_at`. When the two
/// are equal the dial steps from `from` to `to` at that return.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    starts_at: u16,
    full_at: u16,
    from: f32,
    to: f32,
}

/// Why [`Ramp::new`] refused its returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a dial full at return {full_at} cannot start to move at return {starts_at}")]
pub struct FullBeforeStart {
    /// The return the dial was to start moving at.
    pub starts_at: u16,
    /// The return the dial was to be full at.
    pub full_at: u16,
}

impl Ramp {
    /// A ramp from `from` to `to`, moving over the returns `starts_at..=full_at`.
    ///
    /// # Errors
    /// `full_at` is before `starts_at`.
    pub const fn new(
        starts_at: u16,
        full_at: u16,
        from: f32,
        to: f32,
    ) -> Result<Self, FullBeforeStart> {
        if full_at < starts_at {
            return Err(FullBeforeStart { starts_at, full_at });
        }
        Ok(Self {
            starts_at,
            full_at,
            from,
            to,
        })
    }

    /// The dial's value after `returns` returns of the rally.
    #[must_use]
    pub fn at(self, returns: u16) -> f32 {
        if returns >= self.full_at {
            return self.to;
        }
        if returns <= self.starts_at {
            return self.from;
        }
        // Strictly between the two, so the span is at least two returns.
        let done = f32::from(returns - self.starts_at) / f32::from(self.full_at - self.starts_at);
        self.from + (self.to - self.from) * done
    }
}

#[cfg(test)]
mod tests {
    use super::{FullBeforeStart, Ramp};
    use crate::kernel::test_support::assert_near;

    fn ramp() -> Ramp {
        Ramp::new(3, 13, 14.0, 8.0).expect("three is before thirteen")
    }

    #[test]
    fn the_dial_rests_at_its_start_value_until_it_starts_to_move() {
        assert_near(ramp().at(0), 14.0, 1e-6);
        assert_near(ramp().at(3), 14.0, 1e-6);
    }

    #[test]
    fn the_dial_moves_evenly_between_its_two_returns() {
        assert_near(ramp().at(4), 13.4, 1e-5);
        assert_near(ramp().at(8), 11.0, 1e-5);
        assert_near(ramp().at(12), 8.6, 1e-5);
    }

    #[test]
    fn the_dial_stays_full_from_its_full_return_on() {
        assert_near(ramp().at(13), 8.0, 1e-6);
        assert_near(ramp().at(u16::MAX), 8.0, 1e-6);
    }

    #[test]
    fn equal_returns_make_a_step() {
        let step = Ramp::new(5, 5, 1.0, 0.5).expect("equal returns are allowed");
        assert_near(step.at(4), 1.0, 1e-6);
        assert_near(step.at(5), 0.5, 1e-6);
    }

    /// M2 spec 2.3.9's done-condition.
    #[test]
    fn a_full_return_before_the_start_return_is_refused() {
        assert_eq!(
            Ramp::new(10, 4, 1.0, 0.5),
            Err(FullBeforeStart {
                starts_at: 10,
                full_at: 4
            })
        );
        assert_eq!(
            FullBeforeStart {
                starts_at: 10,
                full_at: 4
            }
            .to_string(),
            "a dial full at return 4 cannot start to move at return 10"
        );
    }
}
