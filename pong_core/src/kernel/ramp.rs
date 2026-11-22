//! The curve every effect's dial rides (M2 spec 2.3.9): from one value to
//! another over a span of the rally's returns. It yields a number
//! ([`Ramp`]) or a whole count of items ([`ItemCount`]).

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

/// How many items of an item effect are on the field, as a function of the
/// rally's return count: the curve's whole-count form (M2 spec 2.3.9). None
/// until `starts_at` returns, then one more each time the dial reaches the
/// next whole number, then `full` from `full_at` returns on.
///
/// Invariants, enforced by [`ItemCount::new`]: `1 <= starts_at <= full_at`.
/// So no item is on the field at a serve, and the count never falls as the
/// rally grows: an item, once there, stays until the point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemCount {
    starts_at: u16,
    full_at: u16,
    full: u16,
}

/// Why [`ItemCount::new`] refused its returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidItemCount {
    /// The count would be full before it starts to move.
    #[error(transparent)]
    FullBeforeStart(#[from] FullBeforeStart),
    /// The count would start to move at the serve, which no paddle returned.
    #[error("items appear at a paddle return, so their count cannot start to move at return 0")]
    StartsAtTheServe,
}

impl ItemCount {
    /// A count from none to `full` items, rising over the returns
    /// `starts_at..=full_at`.
    ///
    /// # Errors
    /// `starts_at` is zero, or `full_at` is before `starts_at`.
    pub const fn new(starts_at: u16, full_at: u16, full: u16) -> Result<Self, InvalidItemCount> {
        if starts_at == 0 {
            return Err(InvalidItemCount::StartsAtTheServe);
        }
        if full_at < starts_at {
            return Err(InvalidItemCount::FullBeforeStart(FullBeforeStart {
                starts_at,
                full_at,
            }));
        }
        Ok(Self {
            starts_at,
            full_at,
            full,
        })
    }

    /// The items on the field after `returns` returns of the rally.
    ///
    /// Postcondition: none at zero returns, never fewer than after fewer
    /// returns, and at most [`full`](Self::full). Reckoned in whole numbers:
    /// the whole part of the linear dial, exactly.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the quotient is under `full`, which is a u16"
    )]
    pub const fn after(self, returns: u16) -> u16 {
        if returns >= self.full_at {
            return self.full;
        }
        if returns <= self.starts_at {
            return 0;
        }
        // Strictly between the two, so the span is at least two returns and
        // the quotient is under `full`: it fits the `u16` it came from.
        let done = (returns - self.starts_at) as u32;
        let span = (self.full_at - self.starts_at) as u32;
        let whole = self.full as u32 * done / span;
        whole as u16
    }

    /// The items on the field at full dial.
    #[must_use]
    pub const fn full(self) -> u16 {
        self.full
    }
}

#[cfg(test)]
mod tests {
    use super::{FullBeforeStart, InvalidItemCount, ItemCount, Ramp};
    use crate::kernel::test_support::assert_near;

    fn items() -> ItemCount {
        ItemCount::new(1, 7, 3).expect("one is before seven and after the serve")
    }

    /// M2 spec 2.3.9, the whole-count form: the count is the whole part of
    /// the dial, so an item arrives at the first return the dial reaches it.
    #[test]
    fn an_item_arrives_at_the_first_return_the_dial_reaches_its_number() {
        let counts: Vec<u16> = (0..=8).map(|returns| items().after(returns)).collect();
        assert_eq!(counts, [0, 0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn the_count_stays_full_from_its_full_return_on() {
        assert_eq!(items().after(7), 3);
        assert_eq!(items().after(u16::MAX), 3);
        assert_eq!(items().full(), 3);
    }

    /// Thirds are not exact in floating point: the count is reckoned in
    /// whole numbers, so no item arrives a return late for a rounding.
    #[test]
    fn the_count_is_exact_where_the_dial_divides_unevenly() {
        let sevenths = ItemCount::new(1, 8, 7).expect("one is before eight");
        let counts: Vec<u16> = (1..=8).map(|returns| sevenths.after(returns)).collect();
        assert_eq!(counts, [0, 1, 2, 3, 4, 5, 6, 7]);
        // One item a return over the widest span there is: no overflow.
        let widest = ItemCount::new(1, u16::MAX, u16::MAX - 1).expect("one is before the last");
        assert_eq!(widest.after(u16::MAX - 1), u16::MAX - 2);
    }

    #[test]
    fn equal_returns_bring_every_item_at_once() {
        let step = ItemCount::new(5, 5, 2).expect("equal returns are allowed");
        assert_eq!(step.after(4), 0);
        assert_eq!(step.after(5), 2);
    }

    /// M2 spec 2.3.9's done-condition, the item rule: an item appears at a
    /// paddle return, so a serve flies over an empty field.
    #[test]
    fn a_count_that_starts_to_move_at_the_serve_is_refused() {
        assert_eq!(
            ItemCount::new(0, 4, 3),
            Err(InvalidItemCount::StartsAtTheServe)
        );
        assert_eq!(
            InvalidItemCount::StartsAtTheServe.to_string(),
            "items appear at a paddle return, so their count cannot start to move at return 0"
        );
    }

    #[test]
    fn a_count_full_before_it_starts_is_refused_as_any_dial_is() {
        let refused = FullBeforeStart {
            starts_at: 10,
            full_at: 4,
        };
        assert_eq!(
            ItemCount::new(10, 4, 3),
            Err(InvalidItemCount::FullBeforeStart(refused))
        );
        assert_eq!(
            InvalidItemCount::FullBeforeStart(refused).to_string(),
            "a dial full at return 4 cannot start to move at return 10"
        );
    }

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
