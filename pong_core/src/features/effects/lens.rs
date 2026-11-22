//! Lens (M2 spec 2.6.5): lenses on the floor that pull the ball toward
//! their centre lines. They appear at a paddle return, more of them as the
//! rally grows, and stay until the point. Each is drawn on the left half and
//! copied mirrored on the right, so both players face the same floor.
//!
//! Where a rally's lenses lie is chosen once, from the rally's seed, so the
//! shell draws the lenses the ball is pulled by.

use std::f32::consts::PI;
use std::time::Duration;

use bevy::math::Vec2;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::features::tuning::{LensTuning, Tuning};
use crate::kernel::flight::{Contribution, Field, FlightStart};
use crate::kernel::integrate::Motion;
use crate::kernel::random::{Pcg32, RallySeed};

/// One lens on the floor: a disc, pulling toward the horizontal line
/// through its centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedLens {
    /// The middle of the disc; its centre line is `y = centre.y`.
    pub centre: Vec2,
    /// How far the disc reaches from its centre, in arena units.
    pub radius: f32,
}

impl PlacedLens {
    /// This lens mirrored across midfield.
    const fn mirrored(self) -> Self {
        Self {
            centre: Vec2::new(-self.centre.x, self.centre.y),
            radius: self.radius,
        }
    }

    /// How a path at `position` curves under this lens, per unit of
    /// `focus`: its second derivative `d²y/dx²`. Toward the centre line, by
    /// more the farther from it, fading to nothing at the rim, so the pull
    /// has no edge to step over; nothing outside the disc or on the line.
    fn curving(self, position: Vec2) -> f32 {
        let offset = position - self.centre;
        let rim = self.radius * self.radius;
        let inside = 1.0 - offset.length_squared() / rim;
        if inside <= 0.0 {
            return 0.0;
        }
        -offset.y * inside
    }
}

/// The pull of the lenses on the floor during one flight.
///
/// A lens bends the ball's path, not its clock: the acceleration grows with
/// the square of the horizontal speed, so a path curves by `focus` times
/// [`PlacedLens::curving`] per unit of x travelled, at any speed. The dial
/// is what the player sees on the floor, not an acceleration.
#[derive(Debug, Clone, PartialEq)]
struct Pull {
    lenses: Vec<PlacedLens>,
    focus: f32,
}

impl Field for Pull {
    fn lateral(&self, motion: Motion, _seconds_in_flight: f32) -> f32 {
        let curving: f32 = self
            .lenses
            .iter()
            .map(|lens| lens.curving(motion.position))
            .sum();
        self.focus * curving * motion.velocity.x * motion.velocity.x
    }
}

/// The focus at which one lens of `radius` bends no crossing by more than
/// `max_bend` radians.
///
/// A flight's horizontal speed is constant, so a ball passes each x of the
/// disc once, and its slope changes by at most the integral over x of the
/// strongest curving at that x. At `u` from the centre along x the curving
/// peaks at `2 ρ³ / (3 √3 R²)`, with `ρ² = R² - u²`; over the disc that
/// integrates to `π R² / (4 √3)`. A change of slope `s` turns a path by at
/// most `2 atan(s / 2)`, when the path is level halfway. So the slope may
/// change by `2 tan(max_bend / 2)`.
///
/// Precondition (validated tuning's): `radius` is positive and `max_bend`
/// is in `0..π/2`.
fn focus(max_bend: f32, radius: f32) -> f32 {
    let slope_change = 2.0 * (max_bend / 2.0).tan();
    slope_change * 4.0 * 3.0_f32.sqrt() / (PI * radius * radius)
}

/// Puts lenses on the floor as the rally grows, each pulling the ball toward
/// its centre line.
///
/// The dial is a count (R9): none at a serve, then one more drawn lens, and
/// its copy, at the returns the tuned count names. The pull is lateral
/// only, so it never changes how long a crossing takes (R4).
///
/// Invariant: built for one rally; `drawn` is where that rally's lenses lie,
/// in the order they appear, each on the left half. A lens never moves, and
/// a later lens does not depend on how many the dial holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Lens {
    tuning: LensTuning,
    drawn: Vec<Vec2>,
}

impl Lens {
    /// Lens at `tuning`'s dial, its lenses lying where `rally` chose.
    #[must_use]
    pub fn new(tuning: &Tuning, rally: RallySeed) -> Self {
        let lens = tuning.effects().lens();
        let nearest = lens.radius();
        let farthest = tuning.crossing_width() / 2.0 - lens.clear_run() - lens.radius();
        let highest = tuning.arena().center_limit_y(tuning.ball().radius()) - lens.radius();
        let mut places = Pcg32::seed(u64::from(rally.value()));
        let drawn = (0..lens.count().full())
            .map(|_| {
                let from_midfield = places.next_f32_in(nearest, farthest);
                let height = places.next_f32_in(-highest, highest);
                Vec2::new(-from_midfield, height)
            })
            .collect();
        Self {
            tuning: lens,
            drawn,
        }
    }

    /// The lenses on the floor during the flight that began at `start`:
    /// each drawn lens followed by its mirrored copy, in the order they
    /// appeared.
    ///
    /// Postcondition: a pure function of `start`'s return count, and a
    /// prefix of the answer for any later return of the rally, so the shell
    /// draws the lenses the ball is pulled by and none moves or leaves
    /// before the point.
    #[must_use]
    pub fn placed(&self, start: &FlightStart) -> Vec<PlacedLens> {
        self.pairs(self.tuning.count().after(start.returns))
    }

    /// The first `drawn` lenses of the rally, each with its copy.
    fn pairs(&self, drawn: u16) -> Vec<PlacedLens> {
        self.drawn
            .iter()
            .take(usize::from(drawn))
            .flat_map(|&centre| {
                let lens = PlacedLens {
                    centre,
                    radius: self.tuning.radius(),
                };
                [lens, lens.mirrored()]
            })
            .collect()
    }

    /// The share of a flight of the first `drawn` lenses and their copies,
    /// pulling at `share` of their true pull. No lens, no field.
    fn pulling(&self, drawn: u16, share: f32) -> Contribution {
        let lenses = self.pairs(drawn);
        let field = (!lenses.is_empty()).then(|| {
            let pull = Pull {
                lenses,
                focus: share * focus(self.tuning.max_bend(), self.tuning.radius()),
            };
            Box::new(pull) as Box<dyn Field>
        });
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field,
        }
    }
}

impl Effect for Lens {
    fn id(&self) -> EffectId {
        EffectId::Lens
    }

    fn contribute(&self, start: &FlightStart) -> Contribution {
        self.pulling(self.tuning.count().after(start.returns), 1.0)
    }

    /// The second form alone (M2 spec 2.3.12), in two parts. The belief
    /// lags (M2 spec 2.6.5): an AI does not see a lens until it has lain
    /// for its star's `unseen_flights`, so it reckons with the lenses of
    /// that many returns ago. Under a held count those returns had the
    /// count this one has, so every lens has lain long enough (M2 spec
    /// 2.9.1). And of the lenses it sees it expects its
    /// star's share of the pull, as of Wind's drift. A miss made either way
    /// is the hitter's to earn, by sending the ball through a lens the
    /// other side misjudges; nothing is left to a draw.
    fn flaw(&self, start: &FlightStart, difficulty: Difficulty) -> Flaw {
        let lag = if start.returns_held {
            0
        } else {
            self.tuning.unseen_flights(difficulty)
        };
        let seen = self.tuning.count().after(start.returns.saturating_sub(lag));
        Flaw {
            added_error: 0.0,
            added_delay: Duration::ZERO,
            believed: self.pulling(seen, self.tuning.believed_share(difficulty)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::math::Vec2;

    use super::{Lens, PlacedLens};
    use crate::domain::{Difficulty, EffectId};
    use crate::features::effects::Effect;
    use crate::features::tuning::{EMBEDDED_RON, LensTuning, Tuning};
    use crate::kernel::flight::{Contribution, Flight, FlightStart};
    use crate::kernel::integrate::{Motion, advance_between_walls, integrate};
    use crate::kernel::predict::crossing_y;
    use crate::kernel::random::RallySeed;
    use crate::kernel::test_support::assert_near;

    const DT: f32 = 1.0 / 60.0;
    /// The dial at its full count, whatever return the tuning puts that at.
    const FULL_DIAL: u16 = u16::MAX;
    const STARS: [Difficulty; 3] = [
        Difficulty::OneStar,
        Difficulty::TwoStars,
        Difficulty::ThreeStars,
    ];

    fn tuning() -> Tuning {
        Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid")
    }

    fn numbers() -> LensTuning {
        tuning().effects().lens()
    }

    fn lens(rally: u32) -> Lens {
        Lens::new(&tuning(), RallySeed::new(rally))
    }

    fn after(returns: u16) -> FlightStart {
        FlightStart {
            returns,
            hit: None,
            returns_held: false,
        }
    }

    /// The first return at which the embedded dial holds `count` drawn lenses.
    fn the_return_that_brings(count: u16) -> u16 {
        (0..=FULL_DIAL)
            .find(|&returns| numbers().count().after(returns) >= count)
            .expect("the embedded dial reaches that count")
    }

    /// The flight one contribution makes, begun at the serve.
    fn flight_of(contribution: Contribution) -> Flight {
        Flight::start(FlightStart::SERVE, [contribution])
    }

    /// The drawn lenses of `placed`, without their copies.
    fn drawn(placed: &[PlacedLens]) -> Vec<PlacedLens> {
        placed.iter().copied().step_by(2).collect()
    }

    /// The first rally whose drawn lenses at full dial lie clear of each
    /// other, so that a ball inside one is pulled by that one alone.
    fn a_rally_of_lenses_apart() -> Lens {
        (0..1_000)
            .map(lens)
            .find(|lens| {
                let drawn = drawn(&lens.placed(&after(FULL_DIAL)));
                drawn.iter().enumerate().all(|(index, one)| {
                    drawn[index + 1..]
                        .iter()
                        .all(|other| one.centre.distance(other.centre) > 2.0 * one.radius)
                })
            })
            .expect("some rally of a thousand draws its lenses apart")
    }

    /// A ball at `position` crossing level at the cap.
    fn level_at(position: Vec2) -> Motion {
        Motion {
            position,
            velocity: Vec2::new(250.0, 0.0),
        }
    }

    /// The pull `contribution` gives a level ball at `position`.
    fn pull_at(contribution: Contribution, position: Vec2) -> f32 {
        flight_of(contribution).force(level_at(position), DT).y
    }

    /// Halfway from a lens's centre line to its rim, above the centre.
    fn above(lens: PlacedLens) -> Vec2 {
        lens.centre + Vec2::new(0.0, lens.radius / 2.0)
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(lens(1).id(), EffectId::Lens);
    }

    /// R9: absent at zero. A lens appears at a paddle return, so a serve
    /// flies over an empty floor, pulled by nothing.
    #[test]
    fn a_serve_flies_over_an_empty_floor() {
        for rally in 0..20 {
            let floor = lens(rally).placed(&FlightStart::SERVE);
            assert!(floor.is_empty(), "rally {rally}: {floor:?}");
            assert!(lens(rally).contribute(&FlightStart::SERVE).field.is_none());
        }
    }

    /// M2 spec 2.6.5's dial, R9's grades, and R3's shape: the dial is the
    /// count of lenses. Each return holds the drawn lenses the tuned count
    /// names and a copy of each, the full count is reached, and it is more
    /// than none.
    #[test]
    fn the_dial_is_the_count_of_lenses_each_with_its_copy() {
        let lens = lens(1);
        let count = numbers().count();
        for returns in 0..=the_return_that_brings(count.full()) + 5 {
            assert_eq!(
                lens.placed(&after(returns)).len(),
                2 * usize::from(count.after(returns)),
                "after {returns} returns"
            );
        }
        assert!(count.full() > 0, "the embedded dial brings no lens");
        assert_eq!(
            lens.placed(&after(FULL_DIAL)).len(),
            2 * usize::from(count.full())
        );
    }

    /// M2 spec 2.6.5: a lens stays until the point. The lenses of one
    /// return are the first lenses of every later return, each where it
    /// was.
    #[test]
    fn a_lens_once_there_stays_where_it_is_as_the_rally_grows() {
        let lens = lens(7);
        let full = lens.placed(&after(FULL_DIAL));
        for returns in 0..=the_return_that_brings(numbers().count().full()) {
            let now = lens.placed(&after(returns));
            assert_eq!(now[..], full[..now.len()], "after {returns} returns");
        }
    }

    /// M2 spec 2.6.5's done-condition: a drawn lens and its copy are mirror
    /// images, across midfield. The drawn one lies on the left.
    #[test]
    fn a_drawn_lens_and_its_copy_are_mirror_images_across_midfield() {
        for rally in 0..200 {
            let placed = lens(rally).placed(&after(FULL_DIAL));
            assert!(!placed.is_empty(), "the embedded dial brings no lens");
            for pair in placed.chunks(2) {
                let (drawn, copy) = (pair[0], pair[1]);
                assert!(drawn.centre.x < 0.0, "rally {rally}: {drawn:?}");
                assert_eq!(
                    copy.centre,
                    Vec2::new(-drawn.centre.x, drawn.centre.y),
                    "rally {rally}"
                );
                assert_eq!(copy.radius.to_bits(), drawn.radius.to_bits());
            }
        }
    }

    /// Where a lens may lie: on its own half, so it never touches its copy;
    /// inside the band the ball's centre travels, so no ball bounces off a
    /// wall inside one; and short of the paddle by the tuned clear run.
    #[test]
    fn every_lens_lies_on_its_half_inside_the_walls_and_clear_of_the_paddle() {
        let tuning = tuning();
        let numbers = numbers();
        let contact_line = tuning.crossing_width() / 2.0;
        let ceiling = tuning.arena().center_limit_y(tuning.ball().radius());
        for rally in 0..200 {
            for lens in lens(rally).placed(&after(FULL_DIAL)) {
                assert_near(lens.radius, numbers.radius(), 1e-6);
                let (x, y) = (lens.centre.x.abs(), lens.centre.y.abs());
                assert!(x >= lens.radius, "rally {rally}: {lens:?} crosses midfield");
                assert!(
                    x + lens.radius <= contact_line - numbers.clear_run() + 1e-4,
                    "rally {rally}: {lens:?} reaches into the clear run"
                );
                assert!(
                    y + lens.radius <= ceiling + 1e-4,
                    "rally {rally}: {lens:?} reaches past the ball's ceiling"
                );
            }
        }
    }

    /// The lenses are the rally's: the same seed lays the same floor, for
    /// every question asked of it, and the rallies between them use the
    /// whole of the room a lens may lie in.
    #[test]
    fn a_rallys_seed_chooses_where_its_lenses_lie() {
        assert_eq!(
            lens(5).placed(&after(FULL_DIAL)),
            lens(5).placed(&after(FULL_DIAL))
        );
        assert_ne!(
            lens(5).placed(&after(FULL_DIAL)),
            lens(6).placed(&after(FULL_DIAL))
        );
        let tuning = tuning();
        let numbers = numbers();
        let nearest = numbers.radius();
        let farthest = tuning.crossing_width() / 2.0 - numbers.clear_run() - numbers.radius();
        let highest = tuning.arena().center_limit_y(tuning.ball().radius()) - numbers.radius();
        let firsts: Vec<Vec2> = (0..200)
            .map(|rally| lens(rally).placed(&after(FULL_DIAL))[0].centre)
            .collect();
        let quarter_x = (farthest - nearest) / 4.0;
        assert!(firsts.iter().any(|at| -at.x < nearest + quarter_x));
        assert!(firsts.iter().any(|at| -at.x > farthest - quarter_x));
        assert!(firsts.iter().any(|at| at.y > highest / 2.0));
        assert!(firsts.iter().any(|at| at.y < -highest / 2.0));
    }

    /// M2 spec 2.6.5: a lens pulls toward its centre line. Down above the
    /// line and up below it, by the same amount; not at all on the line, at
    /// the rim, or outside the disc.
    #[test]
    fn a_lens_pulls_the_ball_toward_its_centre_line_and_nowhere_outside_itself() {
        let lens = a_rally_of_lenses_apart();
        let one = lens.placed(&after(FULL_DIAL))[0];
        let pull = |offset: Vec2| pull_at(lens.contribute(&after(FULL_DIAL)), one.centre + offset);
        let half = one.radius / 2.0;
        let above = pull(Vec2::new(0.0, half));
        assert!(above < 0.0, "above the line the pull was {above}");
        assert_near(pull(Vec2::new(0.0, -half)), -above, 1e-3);
        assert_near(
            pull(Vec2::new(half, half)),
            pull(Vec2::new(-half, half)),
            1e-3,
        );
        assert_near(pull(Vec2::ZERO), 0.0, 1e-6);
        assert_near(pull(Vec2::new(half, 0.0)), 0.0, 1e-6);
        assert_near(pull(Vec2::new(0.0, one.radius)), 0.0, 1e-6);
        assert_near(pull(Vec2::new(0.0, one.radius + 0.5)), 0.0, 1e-6);
        assert_near(pull(Vec2::new(one.radius + 0.5, half)), 0.0, 1e-6);
    }

    /// The copy pulls as the drawn lens does, mirrored: both players face
    /// the same floor.
    #[test]
    fn the_copy_pulls_as_the_drawn_lens_does_mirrored() {
        let lens = a_rally_of_lenses_apart();
        for one in drawn(&lens.placed(&after(FULL_DIAL))) {
            let spot = above(one) + Vec2::new(one.radius / 3.0, 0.0);
            let mirrored = Vec2::new(-spot.x, spot.y);
            let here = pull_at(lens.contribute(&after(FULL_DIAL)), spot);
            let there = pull_at(lens.contribute(&after(FULL_DIAL)), mirrored);
            assert!(here < 0.0);
            assert_near(there, here, 1e-3);
        }
    }

    /// The angle a ball's path turns by in crossing `one` lens of `lens`'s
    /// full floor: sent at `speed` along `slope` so as to pass `offset`
    /// above the lens's centre if nothing bent it, the first step starting
    /// `phase` of a step short of the disc. Stepped as the game steps a
    /// ball, away from the walls.
    fn bend_through(
        lens: &Lens,
        one: PlacedLens,
        (offset, slope): (f32, f32),
        (speed, phase): (f32, f32),
    ) -> f32 {
        let flight = flight_of(lens.contribute(&after(FULL_DIAL)));
        let lead = one.radius + phase * speed * DT;
        let mut motion = Motion {
            position: one.centre + Vec2::new(-lead, offset - slope * lead),
            velocity: Vec2::new(speed, slope * speed),
        };
        let before = motion.velocity.to_angle();
        while motion.position.x < one.centre.x + one.radius {
            motion = integrate(motion, flight.force(motion, DT), DT);
        }
        motion.velocity.to_angle() - before
    }

    /// Entries into a lens of `radius`: every offset from the centre line
    /// to either rim, at every slope a paddle can send a ball along and
    /// beyond.
    fn entries(radius: f32) -> impl Iterator<Item = (f32, f32)> {
        (-16..=16_i16).flat_map(move |offset| {
            (-18..=18_i16)
                .map(move |slope| (radius * f32::from(offset) / 16.0, f32::from(slope) / 10.0))
        })
    }

    /// M2 spec 2.6.5's done-condition: one lens bends no crossing more than
    /// the tuned angle. Over every entry, at the serve speed and at the
    /// cap, wherever in a step the ball meets the disc.
    ///
    /// The bound is the path's; the fixed step adds its own budget, since a
    /// ball at the cap crosses a lens in a handful of steps. A fiftieth of
    /// the angle covers it at the embedded radius. The dial also means what
    /// it says: some crossing bends by nine tenths of the angle.
    #[test]
    fn one_lens_bends_no_crossing_more_than_the_tuned_angle() {
        let tuning = tuning();
        let max_bend = numbers().max_bend();
        let lens = a_rally_of_lenses_apart();
        let one = lens.placed(&after(FULL_DIAL))[0];
        let mut largest = 0.0_f32;
        for entry in entries(one.radius) {
            for speed in [tuning.ball().serve_speed(), tuning.ball().speed_cap()] {
                for phase in [0.0, 0.25, 0.5, 0.75] {
                    let bend = bend_through(&lens, one, entry, (speed, phase)).abs();
                    assert!(
                        bend <= max_bend * 1.02,
                        "entering at {entry:?} at {speed} the path bent {} degrees, over {}",
                        bend.to_degrees(),
                        max_bend.to_degrees()
                    );
                    largest = largest.max(bend);
                }
            }
        }
        assert!(
            largest >= max_bend * 0.9,
            "no crossing bent more than {} of {} degrees",
            largest.to_degrees(),
            max_bend.to_degrees()
        );
    }

    /// The dial is in play terms: a lens bends a path, the same path at any
    /// speed, apart from the fixed step's budget.
    #[test]
    fn a_path_bends_the_same_at_the_serve_speed_and_at_the_cap() {
        let tuning = tuning();
        let lens = a_rally_of_lenses_apart();
        let one = lens.placed(&after(FULL_DIAL))[0];
        let entry = (one.radius / 2.0, 0.0);
        let slow = bend_through(&lens, one, entry, (tuning.ball().serve_speed(), 0.5));
        let fast = bend_through(&lens, one, entry, (tuning.ball().speed_cap(), 0.5));
        assert!(slow < -0.5 * numbers().max_bend(), "{slow}");
        assert_near(fast, slow, 0.03 * numbers().max_bend());
    }

    /// A ball sent level from the left contact line at `start_y`, at the cap.
    fn sent_level_from(start_y: f32) -> Motion {
        Motion {
            position: Vec2::new(-tuning().crossing_width() / 2.0, start_y),
            velocity: Vec2::new(250.0, 0.0),
        }
    }

    /// R5, the shared math (M2 spec 2.2.1): through a full floor of lenses
    /// the crossing the AI's prediction gives is where the ball, stepped as
    /// physics steps it, crosses the far contact line.
    #[test]
    fn prediction_equals_simulation_through_a_full_floor_of_lenses() {
        let tuning = tuning();
        let (arena, radius) = (tuning.arena(), tuning.ball().radius());
        let far_line = tuning.crossing_width() / 2.0;
        let mut bent = 0;
        for rally in 0..40 {
            let lens = lens(rally);
            for start_y in [-30.0, -10.0, 0.0, 15.0, 35.0] {
                let start = sent_level_from(start_y);
                let predicting = flight_of(lens.contribute(&after(FULL_DIAL)));
                let predicted = crossing_y(arena, radius, start, &predicting, far_line, DT)
                    .expect("the ball flies at the far contact line");
                let mut flight = flight_of(lens.contribute(&after(FULL_DIAL)));
                let mut motion = start;
                let mut before = start;
                while motion.position.x < far_line {
                    before = motion;
                    let force = flight.force(motion, DT);
                    motion = advance_between_walls(arena, radius, motion, force, DT).motion;
                    flight.tick();
                }
                let (low, high) = (
                    before.position.y.min(motion.position.y),
                    before.position.y.max(motion.position.y),
                );
                assert!(
                    (low - 1e-3..=high + 1e-3).contains(&predicted),
                    "rally {rally} from {start_y}: predicted {predicted}, flew {low} to {high}"
                );
                bent += u32::from((predicted - start_y).abs() > 1.0);
            }
        }
        assert!(bent >= 20, "only {bent} of 200 balls met a lens");
    }

    /// R4: the pull is lateral. It never changes how fast the ball crosses,
    /// so no lens can stall a rally.
    #[test]
    fn a_lens_never_changes_the_balls_horizontal_speed() {
        let tuning = tuning();
        let lens = a_rally_of_lenses_apart();
        let one = lens.placed(&after(FULL_DIAL))[0];
        let mut flight = flight_of(lens.contribute(&after(FULL_DIAL)));
        let mut motion = Motion {
            position: Vec2::new(
                -tuning.crossing_width() / 2.0,
                one.centre.y + one.radius / 2.0,
            ),
            velocity: Vec2::new(80.0, 0.0),
        };
        let mut pulled = false;
        for _ in 0..105 {
            let force = flight.force(motion, DT);
            pulled |= force.y != 0.0;
            let step =
                advance_between_walls(tuning.arena(), tuning.ball().radius(), motion, force, DT);
            flight.tick();
            motion = step.motion;
            assert_near(motion.velocity.x, 80.0, 1e-6);
        }
        assert!(pulled, "the course was to take the ball through a lens");
    }

    #[test]
    fn it_touches_neither_the_speed_nor_the_paddles() {
        let contribution = lens(1).contribute(&after(FULL_DIAL));
        assert_near(contribution.speed_gain, 0.0, 1e-6);
        assert_near(contribution.paddle_height_factor, 1.0, 1e-6);
    }

    /// M2 spec 2.6.5 and 2.3.12, second form: the belief lags. An AI does
    /// not see a lens until it has lain for its star's unseen flights; from
    /// then on it expects its star's share of the lens's pull. Nothing is
    /// added to its error or its delay.
    #[test]
    fn each_star_sees_a_lens_once_it_is_old_enough_and_then_believes_its_share() {
        let lens = a_rally_of_lenses_apart();
        let numbers = numbers();
        let drawn = drawn(&lens.placed(&after(FULL_DIAL)));
        let last_arrival = the_return_that_brings(numbers.count().full());
        for difficulty in STARS {
            let lag = numbers.unseen_flights(difficulty);
            let share = numbers.believed_share(difficulty);
            for returns in 0..=last_arrival + lag + 2 {
                let flaw = lens.flaw(&after(returns), difficulty);
                assert_near(flaw.added_error, 0.0, 1e-6);
                assert_eq!(flaw.added_delay, Duration::ZERO);
                assert_near(flaw.believed.speed_gain, 0.0, 1e-6);
                assert_near(flaw.believed.paddle_height_factor, 1.0, 1e-6);
                let believed = flight_of(flaw.believed);
                let flown = flight_of(lens.contribute(&after(returns)));
                for (index, one) in (1..).zip(&drawn) {
                    let truth = flown.force(level_at(above(*one)), DT).y;
                    let old_enough = returns >= the_return_that_brings(index) + lag;
                    let expected = if old_enough { share * truth } else { 0.0 };
                    assert_near(believed.force(level_at(above(*one)), DT).y, expected, 1e-3);
                    if old_enough {
                        assert!(truth < 0.0, "lens {index} after {returns} returns");
                    }
                }
            }
        }
    }

    /// M2 spec 2.9.1: a held count is a steady state. Under one every
    /// earlier flight started at this count, so no lens is too new to see:
    /// each star believes its share of every lens on the floor, the flight
    /// the dial first holds it too.
    #[test]
    fn under_a_held_count_no_lens_is_too_new_for_any_star_to_see() {
        let lens = a_rally_of_lenses_apart();
        let numbers = numbers();
        let drawn = drawn(&lens.placed(&after(FULL_DIAL)));
        for difficulty in STARS {
            let share = numbers.believed_share(difficulty);
            for (count, one) in (1..).zip(&drawn) {
                let held = FlightStart {
                    returns_held: true,
                    ..after(the_return_that_brings(count))
                };
                let truth = pull_at(lens.contribute(&held), above(*one));
                assert!(truth < 0.0, "lens {count} is on the floor");
                let believed = pull_at(lens.flaw(&held, difficulty).believed, above(*one));
                assert_near(believed, share * truth, 1e-3);
            }
        }
    }

    /// More stars, a quicker eye and a truer belief: the embedded lags fall
    /// as the stars rise and the shares rise, and no star sees a lens the
    /// flight it appears or believes the whole of its pull.
    #[test]
    fn the_embedded_lags_fall_and_the_shares_rise_with_the_stars() {
        let numbers = numbers();
        let lags = STARS.map(|difficulty| numbers.unseen_flights(difficulty));
        assert!(lags[0] > lags[1] && lags[1] > lags[2], "{lags:?}");
        assert!(lags[2] >= 1, "{lags:?}");
        let shares = STARS.map(|difficulty| numbers.believed_share(difficulty));
        assert!(shares[0] < shares[1] && shares[1] < shares[2], "{shares:?}");
        assert!(shares[2] < 1.0, "{shares:?}");
    }

    /// With no lens yet, there is nothing to misjudge.
    #[test]
    fn at_the_serve_every_star_believes_in_no_lens() {
        for difficulty in STARS {
            let flaw = lens(3).flaw(&FlightStart::SERVE, difficulty);
            assert!(flaw.believed.field.is_none());
        }
    }
}
