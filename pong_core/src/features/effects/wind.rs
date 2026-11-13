//! Wind (M2 spec 2.6.2): a steady pull on the ball, up or down, that grows
//! with the rally. Its direction is chosen once per rally, from the rally's
//! seed, so it can be shown in the countdown before it blows.

use std::time::Duration;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::features::tuning::{Tuning, WindTuning};
use crate::kernel::flight::{Contribution, Drift, Field, FlightStart};
use crate::kernel::random::RallySeed;

/// Which way the wind blows for one rally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindDirection {
    /// Toward `+y`.
    Up,
    /// Toward `-y`.
    Down,
}

impl WindDirection {
    /// The direction of the rally `seed` belongs to: its lowest bit, so
    /// either is as likely. A pure function, so the countdown can show the
    /// direction the physics will then use.
    #[must_use]
    pub const fn of(seed: RallySeed) -> Self {
        if seed.value().is_multiple_of(2) {
            Self::Up
        } else {
            Self::Down
        }
    }

    /// `1.0` for up, `-1.0` for down.
    #[must_use]
    pub const fn sign(self) -> f32 {
        match self {
            Self::Up => 1.0,
            Self::Down => -1.0,
        }
    }
}

/// Pulls the ball up or down for a whole rally, by a drift per crossing
/// that rides the rally's return count.
///
/// The dial is mild at zero (R9): a serve flies straight. The pull is
/// lateral only, so it never changes how long a crossing takes (R4).
///
/// Invariant: built for one rally; the direction is that rally's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wind {
    tuning: WindTuning,
    direction: WindDirection,
    crossing_width: f32,
}

impl Wind {
    /// Wind at `tuning`'s dial, blowing the way `rally` chose.
    #[must_use]
    pub fn new(tuning: &Tuning, rally: RallySeed) -> Self {
        Self {
            tuning: tuning.effects().wind(),
            direction: WindDirection::of(rally),
            crossing_width: tuning.crossing_width(),
        }
    }

    /// The wind's share of a flight when it carries the ball `drift` units
    /// over a crossing, the way this rally blows. No drift, no field.
    fn pulling(&self, drift: f32) -> Contribution {
        let field = (drift > 0.0).then(|| {
            let pull = Drift::new(self.direction.sign() * drift, self.crossing_width);
            Box::new(pull) as Box<dyn Field>
        });
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field,
        }
    }
}

impl Effect for Wind {
    fn id(&self) -> EffectId {
        EffectId::Wind
    }

    fn contribute(&self, start: &FlightStart) -> Contribution {
        self.pulling(self.tuning.drift().at(start.returns))
    }

    /// The second form alone (M2 spec 2.3.12): an AI expects its star's
    /// share of the true drift, in the true direction, so its aim is off by
    /// the rest and its error grows with the wind (M2 spec 2.6.2). A miss
    /// made this way is the hitter's to earn, by placing the ball where the
    /// misjudged drift carries it past; nothing is left to a draw.
    fn flaw(&self, start: &FlightStart, difficulty: Difficulty) -> Flaw {
        let share = self.tuning.believed_share(difficulty);
        Flaw {
            added_error: 0.0,
            added_delay: Duration::ZERO,
            believed: self.pulling(share * self.tuning.drift().at(start.returns)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::math::Vec2;

    use super::{Wind, WindDirection};
    use crate::domain::{Difficulty, EffectId};
    use crate::features::effects::Effect;
    use crate::features::tuning::{EMBEDDED_RON, Tuning};
    use crate::kernel::flight::{Contribution, Flight, FlightStart};
    use crate::kernel::integrate::{Motion, advance_between_walls};
    use crate::kernel::predict::crossing_y;
    use crate::kernel::random::RallySeed;
    use crate::kernel::test_support::assert_near;

    const DT: f32 = 1.0 / 60.0;
    /// A seed whose rally blows up, and one whose rally blows down.
    const UP: RallySeed = RallySeed::new(6);
    const DOWN: RallySeed = RallySeed::new(7);
    const STARS: [Difficulty; 3] = [
        Difficulty::OneStar,
        Difficulty::TwoStars,
        Difficulty::ThreeStars,
    ];

    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn wind(rally: RallySeed) -> Wind {
        Wind::new(&tuning(), rally)
    }

    fn after(returns: u16) -> FlightStart {
        FlightStart { returns, hit: None }
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

    /// The pull on a level ball at the cap, under the wind of `rally` after
    /// `returns` returns.
    fn pull(rally: RallySeed, returns: u16) -> f32 {
        flight_of(wind(rally).contribute(&after(returns)))
            .force(level_ball(), DT)
            .y
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(wind(UP).id(), EffectId::Wind);
    }

    /// The direction is the seed's to say, and both are said: an even seed
    /// blows up, an odd one down.
    #[test]
    fn a_rallys_seed_chooses_the_direction() {
        assert_eq!(WindDirection::of(UP), WindDirection::Up);
        assert_eq!(WindDirection::of(DOWN), WindDirection::Down);
        assert!(WindDirection::Up.sign() > 0.0);
        assert!(WindDirection::Down.sign() < 0.0);
    }

    /// One rally, one direction: every flight of a rally is pulled the way
    /// its seed chose, and by the same amount either way.
    #[test]
    fn the_wind_pulls_the_way_the_rallys_seed_chose_at_every_return() {
        for returns in [1, 6, 12, 40] {
            let (up, down) = (pull(UP, returns), pull(DOWN, returns));
            assert!(up > 0.0, "after {returns} returns: {up}");
            assert_near(down, -up, 1e-4);
        }
    }

    /// R9: mild at zero. The embedded dial starts at nothing, so a serve
    /// flies straight.
    #[test]
    fn a_rally_starts_with_no_wind() {
        for rally in [UP, DOWN] {
            let serve = wind(rally).contribute(&FlightStart::SERVE);
            assert!(serve.field.is_none());
        }
    }

    /// R3's shape, and R9's grades: the pull never eases as the rally grows,
    /// and is the same from the return the dial is full at.
    #[test]
    fn the_pull_never_eases_as_the_rally_grows() {
        let pulls: Vec<f32> = (0..30).map(|returns| pull(UP, returns)).collect();
        for pair in pulls.windows(2) {
            assert!(pair[1] >= pair[0], "the wind eased: {pair:?}");
        }
        assert!(pulls[12] > pulls[6] && pulls[6] > pulls[1]);
        assert_near(pulls[29], pulls[12], 1e-4);
    }

    #[test]
    fn it_touches_neither_the_speed_nor_the_paddles() {
        let contribution = wind(UP).contribute(&after(12));
        assert_near(contribution.speed_gain, 0.0, 1e-6);
        assert_near(contribution.paddle_height_factor, 1.0, 1e-6);
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

    /// M2 spec 2.6.2's done-condition: the drift at the paddle matches the
    /// dial at two ball speeds. At full dial the embedded wind carries a
    /// ball 24 units over one crossing, at the serve speed as at the cap,
    /// apart from the integrator's budget of one step's share of the drift.
    #[test]
    fn the_drift_at_the_paddle_is_the_dial_at_the_serve_speed_and_at_the_cap() {
        let tuning = tuning();
        for (rally, sign) in [(UP, 1.0), (DOWN, -1.0)] {
            let flight = flight_of(wind(rally).contribute(&after(12)));
            for speed in [tuning.ball().serve_speed(), tuning.ball().speed_cap()] {
                // From 12 short of the middle, clear of both walls.
                let start_y = -12.0 * sign;
                let drift = sign * (crossing_from(start_y, speed, &flight) - start_y);
                let steps = tuning.crossing_width() / (speed * DT);
                let budget = 24.0 / steps;
                assert!(
                    (24.0 - 0.01..=24.0 + budget + 0.01).contains(&drift),
                    "at {speed} the ball drifted {drift}, not 24 to {}",
                    24.0 + budget
                );
            }
        }
    }

    /// Half the dial, half the drift: 12 units after six returns.
    #[test]
    fn the_drift_follows_the_dial_between_its_ends() {
        let flight = flight_of(wind(UP).contribute(&after(6)));
        let drift = crossing_from(-6.0, 250.0, &flight) + 6.0;
        assert!((11.99..=12.4).contains(&drift), "{drift}");
    }

    /// R4: the wind is lateral. It never changes how fast the ball crosses,
    /// so no wind can stall a rally: a crossing takes the time it would
    /// take in still air, walls and all.
    #[test]
    fn the_wind_never_changes_the_balls_horizontal_speed() {
        let tuning = tuning();
        let mut flight = flight_of(wind(DOWN).contribute(&after(12)));
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

    /// M2 spec 2.3.12, second form: an AI of each star believes its share
    /// of the true drift, in the true direction. Nothing is added to its
    /// error or its delay: the misjudged wind is the whole flaw.
    #[test]
    fn each_star_believes_its_share_of_the_true_drift_and_is_charged_nothing_else() {
        let tuning = tuning();
        for rally in [UP, DOWN] {
            let wind = wind(rally);
            let truth = pull(rally, 12);
            for difficulty in STARS {
                let flaw = wind.flaw(&after(12), difficulty);
                assert_near(flaw.added_error, 0.0, 1e-6);
                assert_eq!(flaw.added_delay, Duration::ZERO);
                assert_near(flaw.believed.speed_gain, 0.0, 1e-6);
                assert_near(flaw.believed.paddle_height_factor, 1.0, 1e-6);
                let believed = flight_of(flaw.believed).force(level_ball(), DT).y;
                let share = tuning.effects().wind().believed_share(difficulty);
                assert_near(believed, share * truth, 1e-3);
            }
        }
    }

    /// More stars, a truer belief: the embedded shares rise with the stars
    /// and none is the whole truth.
    #[test]
    fn the_embedded_shares_rise_with_the_stars_and_stay_short_of_the_truth() {
        let wind = tuning().effects().wind();
        let shares = STARS.map(|difficulty| wind.believed_share(difficulty));
        assert!(shares[0] < shares[1] && shares[1] < shares[2], "{shares:?}");
        assert!(shares[2] < 1.0, "{shares:?}");
    }

    /// With no wind yet, there is nothing to misjudge.
    #[test]
    fn at_the_serve_every_star_believes_in_no_wind() {
        for difficulty in STARS {
            let flaw = wind(UP).flaw(&FlightStart::SERVE, difficulty);
            assert!(flaw.believed.field.is_none());
        }
    }
}
