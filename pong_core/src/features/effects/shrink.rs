//! Shrink (M2 spec 2.6.1): both paddles shorten as the rally grows, from the
//! default height toward the minimum, and are whole again for the next rally.

use std::time::Duration;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::features::tuning::Tuning;
use crate::kernel::flight::{Contribution, FlightStart};
use crate::kernel::ramp::Ramp;

/// Scales the height of both paddles by a factor that rides the rally's
/// return count.
///
/// The dial is mild at zero (R9): no rally starts with short paddles. The
/// base applies the factor through the paddle's clamped setter, so no dial
/// can take a paddle below its tuned minimum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shrink {
    height_factor: Ramp,
}

impl Shrink {
    /// Shrink at `tuning`'s dial.
    #[must_use]
    pub const fn new(tuning: &Tuning) -> Self {
        Self {
            height_factor: tuning.effects().shrink().height_factor(),
        }
    }
}

impl Effect for Shrink {
    fn id(&self) -> EffectId {
        EffectId::Shrink
    }

    fn contribute(&self, start: &FlightStart) -> Contribution {
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: self.height_factor.at(start.returns),
            field: None,
        }
    }

    /// Inherent (M2 spec 2.6.1): a shorter paddle forgives less of the
    /// star's own aim error, and every AI sees the paddles as they are.
    fn flaw(&self, start: &FlightStart, _difficulty: Difficulty) -> Flaw {
        Flaw {
            added_error: 0.0,
            added_delay: Duration::ZERO,
            believed: self.contribute(start),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Shrink;
    use crate::domain::Difficulty;
    use crate::domain::EffectId;
    use crate::features::effects::Effect;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::kernel::flight::FlightStart;
    use crate::kernel::test_support::assert_near;

    fn shrink() -> Shrink {
        Shrink::new(&Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid"))
    }

    fn factor_after(returns: u16) -> f32 {
        shrink()
            .contribute(&FlightStart { returns, hit: None })
            .paddle_height_factor
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(shrink().id(), EffectId::Shrink);
    }

    /// R9: mild at zero. The embedded dial starts to move after return 3.
    #[test]
    fn a_rally_starts_with_whole_paddles() {
        assert_near(factor_after(0), 1.0, 1e-6);
        assert_near(factor_after(3), 1.0, 1e-6);
    }

    /// The embedded dial is full at return 15: height 8 of a default 14.
    #[test]
    fn at_full_dial_the_factor_is_the_minimum_over_the_default_height() {
        assert_near(factor_after(15), 8.0 / 14.0, 1e-6);
        assert_near(factor_after(400), 8.0 / 14.0, 1e-6);
    }

    /// R3's shape: the dial never eases as the rally grows.
    #[test]
    fn the_factor_never_rises_as_the_rally_grows() {
        let factors: Vec<f32> = (0..30).map(factor_after).collect();
        for pair in factors.windows(2) {
            assert!(pair[1] <= pair[0], "the paddles grew: {pair:?}");
        }
    }

    /// M2 spec 2.6.1: Shrink's flaw is inherent in the smaller paddle. No
    /// AI is told anything but the truth about it.
    #[test]
    fn it_adds_no_flaw_and_is_believed_as_it_is() {
        for difficulty in [
            Difficulty::OneStar,
            Difficulty::TwoStars,
            Difficulty::ThreeStars,
        ] {
            let flaw = shrink().flaw(
                &FlightStart {
                    returns: 15,
                    hit: None,
                },
                difficulty,
            );
            assert_near(flaw.added_error, 0.0, 1e-6);
            assert_eq!(flaw.added_delay, Duration::ZERO);
            assert_near(flaw.believed.speed_gain, 0.0, 1e-6);
            assert_near(flaw.believed.paddle_height_factor, 8.0 / 14.0, 1e-6);
            assert!(flaw.believed.field.is_none());
        }
    }

    #[test]
    fn it_touches_neither_the_speed_nor_the_flight_path() {
        let contribution = shrink().contribute(&FlightStart {
            returns: 9,
            hit: None,
        });
        assert_near(contribution.speed_gain, 0.0, 1e-6);
        assert!(contribution.field.is_none());
    }
}
