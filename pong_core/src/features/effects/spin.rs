//! Spin (M2 spec 2.6.4): a return from a moving paddle curves the way the
//! paddle moved, by a drift that grows with the rally. A return from a still
//! paddle flies straight, so the hitter chooses among three outcomes at
//! every hit.

use std::time::Duration;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::features::tuning::{SpinTuning, Tuning};
use crate::kernel::flight::{Contribution, Drift, Field, FlightStart, StepMotion};

/// Curves a return the way its paddle moved in the step of the hit, by a
/// drift per crossing that rides the rally's return count.
///
/// The dial is mild at zero (R9), and the pull is lateral only, so it never
/// changes how long a crossing takes (R4). Spin chooses nothing per rally:
/// the hitter chooses at every hit, by moving the paddle or holding it
/// still (R2, R10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spin {
    tuning: SpinTuning,
    crossing_width: f32,
}

impl Spin {
    /// Spin at `tuning`'s dial.
    #[must_use]
    pub fn new(tuning: &Tuning) -> Self {
        Self {
            tuning: tuning.effects().spin(),
            crossing_width: tuning.crossing_width(),
        }
    }

    /// How far the flight that began at `start` is curved over one full
    /// crossing, in arena units: the dial's amount, positive toward `+y`
    /// from a paddle that moved up, negative from one that moved down.
    /// Zero from a still paddle and from a serve, which no paddle touched.
    ///
    /// A pure function of `start`, so the shell can show the curve the
    /// physics then flies.
    #[must_use]
    pub fn drift(&self, start: &FlightStart) -> f32 {
        let amount = self.tuning.drift().at(start.returns);
        match start.hit.map(|hit| hit.paddle_motion) {
            Some(StepMotion::Up) => amount,
            Some(StepMotion::Down) => -amount,
            Some(StepMotion::Still) | None => 0.0,
        }
    }

    /// Spin's share of a flight it curves by `drift` units over a crossing.
    /// No drift, no field.
    fn curving(&self, drift: f32) -> Contribution {
        let field = (drift.abs() > 0.0).then(|| {
            let pull = Drift::new(drift, self.crossing_width);
            Box::new(pull) as Box<dyn Field>
        });
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field,
        }
    }
}

impl Effect for Spin {
    fn id(&self) -> EffectId {
        EffectId::Spin
    }

    fn contribute(&self, start: &FlightStart) -> Contribution {
        self.curving(self.drift(start))
    }

    /// The second form alone (M2 spec 2.3.12): an AI commits its aim when
    /// the ball turns toward it, having read its star's share of the true
    /// spin, in the true direction, so its aim is off by the rest. "Reads
    /// the spin late" (M2 spec 2.6.4) is this, not an added delay: a delay
    /// costs an AI nothing while the ball is slow. A miss made this way is
    /// the hitter's to earn, by spinning the ball; nothing is left to a
    /// draw.
    fn flaw(&self, start: &FlightStart, difficulty: Difficulty) -> Flaw {
        let share = self.tuning.believed_share(difficulty);
        Flaw {
            added_error: 0.0,
            added_delay: Duration::ZERO,
            believed: self.curving(share * self.drift(start)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::math::Vec2;

    use super::Spin;
    use crate::domain::{Difficulty, EffectId};
    use crate::features::effects::Effect;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::kernel::flight::{Contribution, Flight, FlightStart, Hit, StepMotion};
    use crate::kernel::geometry::Side;
    use crate::kernel::integrate::{Motion, advance_between_walls};
    use crate::kernel::predict::crossing_y;
    use crate::kernel::test_support::assert_near;

    const DT: f32 = 1.0 / 60.0;
    const STARS: [Difficulty; 3] = [
        Difficulty::OneStar,
        Difficulty::TwoStars,
        Difficulty::ThreeStars,
    ];
    const MOVING: [(StepMotion, f32); 2] = [(StepMotion::Up, 1.0), (StepMotion::Down, -1.0)];

    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn spin() -> Spin {
        Spin::new(&tuning())
    }

    /// The rally's `returns`-th return, by the left paddle moving as
    /// `paddle_motion`.
    fn returned(returns: u16, paddle_motion: StepMotion) -> FlightStart {
        FlightStart {
            returns,
            hit: Some(Hit {
                side: Side::Left,
                paddle_motion,
            }),
        }
    }

    /// The flight one contribution makes, begun at the serve.
    fn flight_of(contribution: Contribution) -> Flight {
        Flight::start(FlightStart::SERVE, [contribution])
    }

    /// A ball crossing level at the cap, in the middle of the arena.
    fn level_ball() -> Motion {
        Motion {
            position: Vec2::ZERO,
            velocity: Vec2::new(250.0, 0.0),
        }
    }

    /// The pull on a level ball at the cap, in the flight that began at `start`.
    fn pull(start: FlightStart) -> f32 {
        flight_of(spin().contribute(&start))
            .force(level_ball(), DT)
            .y
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(spin().id(), EffectId::Spin);
    }

    /// M2 spec 2.6.4's three outcomes: the ball curves the way the paddle
    /// moved, by the same amount either way, whichever paddle returned it;
    /// a still paddle's return is not curved.
    #[test]
    fn a_moving_paddles_return_curves_the_way_the_paddle_moved_and_a_still_ones_does_not() {
        for returns in [1, 6, 12, 40] {
            let up = pull(returned(returns, StepMotion::Up));
            let down = pull(returned(returns, StepMotion::Down));
            assert!(up > 0.0, "after {returns} returns: {up}");
            assert_near(down, -up, 1e-4);
            let still = spin().contribute(&returned(returns, StepMotion::Still));
            assert!(still.field.is_none(), "after {returns} returns");

            let by_the_right_paddle = FlightStart {
                returns,
                hit: Some(Hit {
                    side: Side::Right,
                    paddle_motion: StepMotion::Up,
                }),
            };
            assert_near(pull(by_the_right_paddle), up, 1e-6);
        }
    }

    /// No paddle touched a served ball: it is never spun, at any dial.
    #[test]
    fn a_serve_is_not_spun() {
        for returns in [0, 12] {
            let serve = FlightStart { returns, hit: None };
            assert!(spin().contribute(&serve).field.is_none());
            assert_near(spin().drift(&serve), 0.0, 1e-6);
        }
    }

    /// R9 and M2 spec 2.6.4: mild at zero. The embedded dial starts at
    /// nothing, so at a count of zero no paddle motion curves the ball.
    #[test]
    fn at_dial_zero_no_return_is_spun() {
        for (paddle_motion, _) in MOVING {
            let start = returned(0, paddle_motion);
            assert!(spin().contribute(&start).field.is_none());
        }
    }

    /// The drift a flight is spun with is what the shell shows and what the
    /// field carries: signed toward the paddle's motion, the dial's amount.
    #[test]
    fn the_drift_of_a_flight_is_the_dials_amount_signed_the_way_the_paddle_moved() {
        for (paddle_motion, sign) in MOVING {
            assert_near(spin().drift(&returned(6, paddle_motion)), sign * 8.0, 1e-5);
            assert_near(
                spin().drift(&returned(12, paddle_motion)),
                sign * 16.0,
                1e-5,
            );
            assert_near(
                spin().drift(&returned(99, paddle_motion)),
                sign * 16.0,
                1e-5,
            );
        }
        assert_near(spin().drift(&returned(12, StepMotion::Still)), 0.0, 1e-6);
    }

    /// R3's shape, and R9's grades: the curve never eases as the rally
    /// grows, and is the same from the return the dial is full at.
    #[test]
    fn the_curve_never_eases_as_the_rally_grows() {
        let pulls: Vec<f32> = (0..30)
            .map(|returns| pull(returned(returns, StepMotion::Up)))
            .collect();
        for pair in pulls.windows(2) {
            assert!(pair[1] >= pair[0], "the spin eased: {pair:?}");
        }
        assert!(pulls[12] > pulls[6] && pulls[6] > pulls[1]);
        assert_near(pulls[29], pulls[12], 1e-4);
    }

    #[test]
    fn it_touches_neither_the_speed_nor_the_paddles() {
        for paddle_motion in [StepMotion::Up, StepMotion::Still, StepMotion::Down] {
            let contribution = spin().contribute(&returned(12, paddle_motion));
            assert_near(contribution.speed_gain, 0.0, 1e-6);
            assert_near(contribution.paddle_height_factor, 1.0, 1e-6);
        }
    }

    /// Where a ball that leaves one contact line level, at `start_y` and
    /// `speed`, crosses the other, under `flight`.
    fn crossing_from(start_y: f32, speed: f32, flight: &Flight) -> f32 {
        let tuning = tuning();
        let half = tuning.crossing_width() / 2.0;
        let start = Motion {
            position: Vec2::new(-half, start_y),
            velocity: Vec2::new(speed, 0.0),
        };
        crossing_y(
            tuning.arena(),
            tuning.ball().radius(),
            start,
            flight,
            half,
            DT,
        )
        .expect("the ball flies at the far contact line")
    }

    /// M2 spec 2.6.4: the dial is the drift over one crossing. At full dial
    /// the embedded spin carries a ball 16 units over one crossing, at the
    /// serve speed as at the cap, apart from the integrator's budget of one
    /// step's share of the drift.
    #[test]
    fn the_drift_at_the_far_paddle_is_the_dial_at_the_serve_speed_and_at_the_cap() {
        let tuning = tuning();
        for (paddle_motion, sign) in MOVING {
            let flight = flight_of(spin().contribute(&returned(12, paddle_motion)));
            for speed in [tuning.ball().serve_speed(), tuning.ball().speed_cap()] {
                // From 8 short of the middle, clear of both walls.
                let start_y = -8.0 * sign;
                let drift = sign * (crossing_from(start_y, speed, &flight) - start_y);
                let steps = tuning.crossing_width() / (speed * DT);
                let budget = 16.0 / steps;
                assert!(
                    (16.0 - 0.01..=16.0 + budget + 0.01).contains(&drift),
                    "at {speed} the ball curved {drift}, not 16 to {}",
                    16.0 + budget
                );
            }
        }
    }

    /// R4: spin is lateral. It never changes how fast the ball crosses, so
    /// no spin can stall a rally: a crossing takes the time it would take
    /// unspun, walls and all.
    #[test]
    fn spin_never_changes_the_balls_horizontal_speed() {
        let tuning = tuning();
        let mut flight = flight_of(spin().contribute(&returned(12, StepMotion::Down)));
        let mut motion = Motion {
            position: Vec2::new(-70.0, 30.0),
            velocity: Vec2::new(80.0, -60.0),
        };
        let mut bounces = 0;
        for _ in 0..105 {
            let force = flight.force(motion, DT);
            let step =
                advance_between_walls(tuning.arena(), tuning.ball().radius(), motion, force, DT);
            flight.tick();
            motion = step.motion;
            bounces += u32::from(step.bounced);
            assert_near(motion.velocity.x, 80.0, 1e-6);
        }
        // 105 steps at 80 units/s: the 140 units from -70 to 70.
        assert_near(motion.position.x, 70.0, 1e-2);
        assert!(bounces >= 1, "the course was to take the ball into a wall");
    }

    /// M2 spec 2.3.12, second form (M2 phase 6 report, ruling 2): an AI of
    /// each star has read its share of the true spin, in the true
    /// direction, when it commits its aim. Nothing is added to its error or
    /// its delay: the spin read short is the whole flaw.
    #[test]
    fn each_star_believes_its_share_of_the_true_spin_and_is_charged_nothing_else() {
        let tuning = tuning();
        for (paddle_motion, _) in MOVING {
            let start = returned(12, paddle_motion);
            let truth = pull(start);
            for difficulty in STARS {
                let flaw = spin().flaw(&start, difficulty);
                assert_near(flaw.added_error, 0.0, 1e-6);
                assert_eq!(flaw.added_delay, Duration::ZERO);
                assert_near(flaw.believed.speed_gain, 0.0, 1e-6);
                assert_near(flaw.believed.paddle_height_factor, 1.0, 1e-6);
                let believed = flight_of(flaw.believed).force(level_ball(), DT).y;
                let share = tuning.effects().spin().believed_share(difficulty);
                assert_near(believed, share * truth, 1e-3);
            }
        }
    }

    /// More stars, a truer reading: the embedded shares rise with the stars
    /// and none is the whole truth.
    #[test]
    fn the_embedded_shares_rise_with_the_stars_and_stay_short_of_the_truth() {
        let spin = tuning().effects().spin();
        let shares = STARS.map(|difficulty| spin.believed_share(difficulty));
        assert!(shares[0] < shares[1] && shares[1] < shares[2], "{shares:?}");
        assert!(shares[2] < 1.0, "{shares:?}");
    }

    /// Where there is no spin there is nothing to misread: a still paddle's
    /// return, a serve, and a return at dial zero.
    #[test]
    fn of_a_flight_that_is_not_spun_every_star_believes_in_no_spin() {
        let unspun = [
            returned(12, StepMotion::Still),
            FlightStart::SERVE,
            returned(0, StepMotion::Up),
        ];
        for start in unspun {
            for difficulty in STARS {
                let flaw = spin().flaw(&start, difficulty);
                assert!(flaw.believed.field.is_none(), "{start:?}");
            }
        }
    }
}
