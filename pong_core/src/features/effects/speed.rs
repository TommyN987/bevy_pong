//! Speed (M2 spec 2.6.0): the ball leaves each paddle return faster. Classic
//! Pong's one effect, and the rally's escalator there.

use super::Effect;
use crate::domain::EffectId;
use crate::kernel::flight::{Contribution, FlightStart};
use crate::kernel::reflect::SpeedRule;

/// Adds a fixed gain to the launch speed for every return of the rally.
///
/// The gain depends on the return count alone, never on how fast the ball
/// happens to fly (M2 spec 2.0.4): a force that speeds the ball up in mid-air
/// does not feed the ramp. The base adds the serve speed and applies the cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speed {
    per_return: f32,
}

impl Speed {
    /// Speed at `rule`'s per-hit increase: the rule of the mode played.
    #[must_use]
    pub const fn new(rule: SpeedRule) -> Self {
        Self {
            per_return: rule.per_hit_increase(),
        }
    }
}

impl Effect for Speed {
    fn id(&self) -> EffectId {
        EffectId::Speed
    }

    fn contribute(&self, start: &FlightStart) -> Contribution {
        Contribution {
            speed_gain: self.per_return * f32::from(start.returns),
            paddle_height_factor: 1.0,
            field: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Speed;
    use crate::domain::EffectId;
    use crate::domain::Mode;
    use crate::features::effects::Effect;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::kernel::flight::{FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::assert_near;

    fn speed() -> Speed {
        let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
        Speed::new(tuning.mode(Mode::Classic).speed_rule())
    }

    fn after(returns: u16) -> FlightStart {
        FlightStart {
            returns,
            hit: Some(Hit {
                side: Side::Left,
                paddle_motion: StepMotion::Still,
            }),
        }
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(speed().id(), EffectId::Speed);
    }

    #[test]
    fn the_serve_gains_nothing_and_each_return_adds_the_tuned_increase() {
        assert_near(
            speed().contribute(&FlightStart::SERVE).speed_gain,
            0.0,
            1e-6,
        );
        assert_near(speed().contribute(&after(1)).speed_gain, 7.0, 1e-6);
        assert_near(speed().contribute(&after(10)).speed_gain, 70.0, 1e-4);
    }

    #[test]
    fn it_pushes_no_field() {
        assert!(speed().contribute(&after(3)).field.is_none());
    }
}
