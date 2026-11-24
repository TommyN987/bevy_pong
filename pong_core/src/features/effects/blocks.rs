//! Blocks (M2 spec 2.6.6): solid blocks on the floor that the ball bounces
//! off. They appear at a paddle return, more of them as the rally grows,
//! and stay until the point. Each is drawn on the left half and copied
//! mirrored on the right, so both players face the same floor.
//!
//! Where a rally's blocks stand is chosen once, from the rally's seed, so
//! the shell draws the blocks the ball bounces off.

use std::time::Duration;

use bevy::math::Vec2;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::features::tuning::{BlocksTuning, Tuning};
use crate::kernel::flight::{Contribution, FlightStart};
use crate::kernel::obstacle::Obstacle;
use crate::kernel::random::{Pcg32, RallySeed};

/// Blocks' own half of its generator's seed, above the rally's 32 bits.
/// Lens lays its lenses from a generator seeded with the rally's number
/// alone; seeded alike, Blocks would draw the same numbers and stand each
/// block on a lens.
const OWN_DRAWS: u64 = 0xB10C << 32;

/// Stands blocks on the floor as the rally grows, each as long as the
/// default paddle and as thick as a paddle, turned from upright one way or
/// the other.
///
/// The dial is a count (R9): none at a serve, then one more drawn block,
/// and its copy, at the returns the tuned count names. A bounce off a block
/// is a collision inside a flight and starts none; how the ball bounces,
/// and how often in a step and in a flight, is the base's to bound
/// ([`BounceRule`](crate::kernel::obstacle::BounceRule)). A flight that has
/// spent its bounces flies through the blocks until the next return.
///
/// Invariant: built for one rally; `drawn` is where that rally's blocks
/// stand, in the order they appear, each on the left half inside the
/// tuning's room for a block ([`Tuning::block_room`]). A block never moves,
/// and a later block does not depend on how many the dial holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Blocks {
    tuning: BlocksTuning,
    drawn: Vec<Obstacle>,
}

impl Blocks {
    /// Blocks at `tuning`'s dial, its blocks standing where `rally` chose.
    #[must_use]
    pub fn new(tuning: &Tuning, rally: RallySeed) -> Self {
        let blocks = tuning.effects().blocks();
        let room = tuning.block_room();
        let mut places = Pcg32::seed(OWN_DRAWS | u64::from(rally.value()));
        let drawn = (0..blocks.count().full())
            .map(|_| {
                let from_midfield = places.next_f32_in(room.nearest(), room.farthest());
                let height = places.next_f32_in(-room.highest(), room.highest());
                let tilt = places.next_f32_in(blocks.min_tilt(), blocks.max_tilt());
                let turned = if places.next_u32() & 1 == 0 {
                    tilt
                } else {
                    -tilt
                };
                Obstacle::new(
                    Vec2::new(-from_midfield, height),
                    blocks.half_size(),
                    turned,
                )
            })
            .collect();
        Self {
            tuning: blocks,
            drawn,
        }
    }

    /// The blocks on the floor during the flight that began at `start`:
    /// each drawn block followed by its mirrored copy, in the order they
    /// appeared.
    ///
    /// Postcondition: a pure function of `start`'s return count, and a
    /// prefix of the answer for any later return of the rally, so the shell
    /// draws the blocks the ball bounces off and none moves or leaves
    /// before the point.
    #[must_use]
    pub fn placed(&self, start: &FlightStart) -> Vec<Obstacle> {
        self.pairs(self.tuning.count().after(start.returns), 1.0)
    }

    /// The first `drawn` blocks of the rally, each with its copy, each
    /// turned from upright by `share` of its tilt: the whole of it as they
    /// stand, less as an AI that takes them for more upright believes.
    fn pairs(&self, drawn: u16, share: f32) -> Vec<Obstacle> {
        self.drawn
            .iter()
            .take(usize::from(drawn))
            .flat_map(|block| {
                let block = Obstacle::new(block.centre(), block.half_size(), share * block.tilt());
                [block, block.mirrored_across_midfield()]
            })
            .collect()
    }

    /// The share of a flight of `obstacles` standing on the floor. Blocks
    /// touches no other channel.
    const fn standing(obstacles: Vec<Obstacle>) -> Contribution {
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: None,
            obstacles,
        }
    }
}

impl Effect for Blocks {
    fn id(&self) -> EffectId {
        EffectId::Blocks
    }

    fn contribute(&self, start: &FlightStart) -> Contribution {
        Self::standing(self.placed(start))
    }

    /// The second form alone (M2 spec 2.3.12), in two parts, as Lens's.
    /// The belief lags (M2 spec 2.6.6): an AI does not see a block until it
    /// has stood for its star's `unseen_flights`, so it reckons with the
    /// blocks of that many returns ago. Under a held count those returns
    /// had the count this one has, so every block has stood long enough (M2
    /// spec 2.9.1). And a block it sees it takes for more upright than it
    /// is: turned by its star's share of the tilt, where it stands and at
    /// its size. A straight ball it reads as well as ever; a miss is the
    /// hitter's to earn, by sending the ball off a block the other side
    /// misjudges, and nothing is left to a draw.
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
            believed: Self::standing(self.pairs(seen, self.tuning.believed_share(difficulty))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::math::Vec2;

    use super::Blocks;
    use crate::domain::{Difficulty, EffectId};
    use crate::features::effects::Effect;
    use crate::features::tuning::{BlocksTuning, EMBEDDED_RON, Tuning};
    use crate::flow::FIXED_HZ;
    use crate::kernel::flight::{Flight, FlightStart};
    use crate::kernel::integrate::Motion;
    use crate::kernel::obstacle::{Obstacle, advance_among_obstacles};
    use crate::kernel::predict::crossing_y;
    use crate::kernel::random::RallySeed;
    use crate::kernel::test_support::{assert_near, assert_vec_near};

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

    fn numbers() -> BlocksTuning {
        tuning().effects().blocks()
    }

    fn blocks(rally: u32) -> Blocks {
        Blocks::new(&tuning(), RallySeed::new(rally))
    }

    fn after(returns: u16) -> FlightStart {
        FlightStart {
            returns,
            hit: None,
            returns_held: false,
        }
    }

    /// The first return at which the embedded dial holds `count` drawn blocks.
    fn the_return_that_brings(count: u16) -> u16 {
        (0..=FULL_DIAL)
            .find(|&returns| numbers().count().after(returns) >= count)
            .expect("the embedded dial reaches that count")
    }

    /// The drawn blocks of `placed`, without their copies.
    fn drawn(placed: &[Obstacle]) -> Vec<Obstacle> {
        placed.iter().copied().step_by(2).collect()
    }

    /// The four corners of `block`.
    fn corners(block: Obstacle) -> [Vec2; 4] {
        let across = Vec2::from_angle(block.tilt()) * block.half_size().x;
        let along =
            Vec2::from_angle(block.tilt() + std::f32::consts::FRAC_PI_2) * block.half_size().y;
        [
            block.centre() + across + along,
            block.centre() + across - along,
            block.centre() - across - along,
            block.centre() - across + along,
        ]
    }

    /// How far outside `block`, grown by a ball's `radius`, the ball's
    /// centre at `position` is: negative when it is inside.
    fn clear_of(block: Obstacle, radius: f32, position: Vec2) -> f32 {
        let from_centre = position - block.centre();
        let across = from_centre.dot(Vec2::from_angle(block.tilt())).abs();
        let along = from_centre
            .dot(Vec2::from_angle(block.tilt() + std::f32::consts::FRAC_PI_2))
            .abs();
        let grown = block.half_size() + Vec2::splat(radius);
        (across - grown.x).max(along - grown.y)
    }

    #[test]
    fn it_names_itself() {
        assert_eq!(blocks(1).id(), EffectId::Blocks);
    }

    /// M2 spec 2.3.9, the item rule: no item lies on the field at a serve.
    #[test]
    fn a_serve_flies_over_an_empty_floor() {
        for rally in 0..20 {
            assert_eq!(blocks(rally).placed(&FlightStart::SERVE), []);
            assert_eq!(blocks(rally).contribute(&FlightStart::SERVE).obstacles, []);
        }
    }

    /// M2 spec 2.6.6 and R9: the dial is the count of blocks, from none to
    /// the tuned most, each drawn block with its copy.
    #[test]
    fn the_dial_is_the_count_of_blocks_each_with_its_copy() {
        let blocks = blocks(7);
        let full = numbers().count().full();
        assert!(full >= 2, "the dial has grades between none and all");
        for count in 1..=full {
            let arrives = the_return_that_brings(count);
            assert_eq!(
                blocks.placed(&after(arrives - 1)).len(),
                usize::from(2 * (count - 1))
            );
            assert_eq!(blocks.placed(&after(arrives)).len(), usize::from(2 * count));
        }
        assert_eq!(
            blocks.placed(&after(FULL_DIAL)).len(),
            usize::from(2 * full)
        );
    }

    /// M2 spec 2.6.6: a block appears at a paddle hit and stays until the
    /// point. What stood there at one return stands there at every later one.
    #[test]
    fn a_block_once_there_stays_where_it_is_as_the_rally_grows() {
        for rally in 0..50 {
            let blocks = blocks(rally);
            let all = blocks.placed(&after(FULL_DIAL));
            for returns in 0..=the_return_that_brings(numbers().count().full()) + 2 {
                let placed = blocks.placed(&after(returns));
                assert_eq!(
                    placed,
                    all[..placed.len()],
                    "rally {rally}, return {returns}"
                );
            }
        }
    }

    /// M2 spec 2.6.6's done-condition: a drawn block and its copy are
    /// mirror images, by seeded test. Corner for corner, across midfield.
    #[test]
    fn a_drawn_block_and_its_copy_are_mirror_images_across_midfield() {
        let mut pairs = 0;
        for rally in 0..200 {
            for pair in blocks(rally).placed(&after(FULL_DIAL)).chunks(2) {
                let [one, copy] = pair else {
                    panic!("a block without its copy in rally {rally}");
                };
                assert!(one.centre().x < 0.0 && copy.centre().x > 0.0, "{pair:?}");
                let mut mirrored: Vec<Vec2> = corners(*one)
                    .into_iter()
                    .map(|corner| Vec2::new(-corner.x, corner.y))
                    .collect();
                let mut copied = corners(*copy).to_vec();
                let by_place = |a: &Vec2, b: &Vec2| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x));
                mirrored.sort_by(by_place);
                copied.sort_by(by_place);
                for (corner, mirror) in copied.iter().zip(&mirrored) {
                    assert_vec_near(*corner, *mirror, 1e-4);
                }
                pairs += 1;
            }
        }
        assert_eq!(pairs, 200 * usize::from(numbers().count().full()));
    }

    /// M2 spec 2.6.6: each block has the default paddle's length and the
    /// paddle's thickness, at an angle drawn from the tuned range, turned
    /// either way and never upright.
    #[test]
    fn every_block_is_a_default_paddle_in_size_and_tilted_within_the_tuned_range() {
        let tuning = tuning();
        let numbers = numbers();
        let (mut left, mut right) = (0, 0);
        let (mut least, mut most) = (f32::MAX, f32::MIN);
        for rally in 0..200 {
            for block in drawn(&blocks(rally).placed(&after(FULL_DIAL))) {
                assert_near(block.half_size().x, tuning.paddle().thickness() / 2.0, 1e-6);
                assert_near(
                    block.half_size().y,
                    tuning.paddle().height_default() / 2.0,
                    1e-6,
                );
                let tilt = block.tilt().abs();
                assert!(
                    (numbers.min_tilt()..=numbers.max_tilt()).contains(&tilt),
                    "rally {rally}: tilted {tilt}"
                );
                left += u32::from(block.tilt() > 0.0);
                right += u32::from(block.tilt() < 0.0);
                least = least.min(tilt);
                most = most.max(tilt);
            }
        }
        assert!(
            left > 100 && right > 100,
            "{left} one way, {right} the other"
        );
        let span = numbers.max_tilt() - numbers.min_tilt();
        assert!(least < numbers.min_tilt() + 0.1 * span, "least {least}");
        assert!(most > numbers.max_tilt() - 0.1 * span, "most {most}");
    }

    /// M2 spec 2.6.6: blocks stay clear of both paddles' contact zones, and
    /// of more. No corner of a block comes within a ball's width of
    /// midfield or of a wall, and a ball touching a block is at least the
    /// clear run from the contact line on that half.
    #[test]
    fn every_block_stands_clear_of_midfield_the_walls_and_the_paddle() {
        let tuning = tuning();
        let ball = tuning.ball().radius().value();
        let contact_line = tuning.crossing_width() / 2.0;
        let wall = tuning.arena().half_extents().y;
        let clear_run = numbers().clear_run();
        for rally in 0..200 {
            for block in blocks(rally).placed(&after(FULL_DIAL)) {
                for corner in corners(block) {
                    assert!(corner.x.abs() >= 2.0 * ball, "rally {rally}: {corner}");
                    assert!(
                        corner.y.abs() <= wall - 2.0 * ball,
                        "rally {rally}: {corner}"
                    );
                    assert!(
                        corner.x.abs() + ball <= contact_line - clear_run + 1e-4,
                        "rally {rally}: {corner}"
                    );
                }
            }
        }
        // Validation's side of it: a ball cannot cross the clear run in a step.
        assert!(f64::from(clear_run) > f64::from(tuning.ball().speed_cap()) / FIXED_HZ);
    }

    /// M2 spec 2.2.4: a rally's blocks stand where its seed says, the same
    /// for everyone who asks, and elsewhere in another rally.
    #[test]
    fn a_rallys_seed_chooses_where_its_blocks_stand() {
        let first = blocks(11).placed(&after(FULL_DIAL));
        assert_eq!(blocks(11).placed(&after(FULL_DIAL)), first);
        let mut places: Vec<Vec2> = (0..100)
            .map(|rally| blocks(rally).placed(&after(FULL_DIAL))[0].centre())
            .collect();
        places.dedup_by(|a, b| (*a - *b).length() < 1e-3);
        assert_eq!(
            places.len(),
            100,
            "two rallies stood their first block alike"
        );
    }

    /// What stands on the floor is what the ball bounces off, and Blocks
    /// touches no other channel.
    #[test]
    fn the_blocks_on_the_floor_are_its_whole_contribution() {
        let blocks = blocks(3);
        for returns in [0, the_return_that_brings(1), FULL_DIAL] {
            let contribution = blocks.contribute(&after(returns));
            assert_eq!(contribution.obstacles, blocks.placed(&after(returns)));
            assert!(contribution.field.is_none());
            assert_near(contribution.speed_gain, 0.0, 1e-6);
            assert_near(contribution.paddle_height_factor, 1.0, 1e-6);
        }
        assert_eq!(
            blocks.contribute(&after(FULL_DIAL)).obstacles.len(),
            usize::from(2 * numbers().count().full())
        );
    }

    /// Courses a return can take off the left paddle under `tuning`: five
    /// heights, seven angles up to the steepest a paddle sends the ball, at
    /// `speed`.
    fn returns_at(tuning: &Tuning, speed: f32) -> Vec<Motion> {
        let steepest = tuning.ball().max_deflection().radians();
        let mut courses = Vec::new();
        for start_y in [-36.0, -18.0, 0.0, 9.0, 30.0] {
            for thirds in -3_i8..=3 {
                let angle = steepest * f32::from(thirds) / 3.0;
                courses.push(Motion {
                    position: Vec2::new(-tuning.crossing_width() / 2.0, start_y),
                    velocity: Vec2::from_angle(angle) * speed,
                });
            }
        }
        courses
    }

    /// How a ball's flight to a contact line ended.
    struct Flown {
        /// The ball as the last step began.
        before: Motion,
        /// The ball once past a contact line.
        end: Motion,
        /// Whether that last step bounced off a wall.
        last_step_met_a_wall: bool,
        /// Bounces off blocks on the way.
        bounces: u16,
    }

    /// Steps a ball as physics steps it, under no force, until it passes a
    /// contact line: never inside a block while the flight has a bounce
    /// left, and bouncing no more often than its budget. `None` if the
    /// flight limit ran out first.
    fn flown(tuning: &Tuning, start: Motion, flight: &Flight) -> Option<Flown> {
        let contact_line = tuning.crossing_width() / 2.0;
        let radius = tuning.ball().radius();
        let mut motion = start;
        let mut obstacles = flight.obstacles_in_flight();
        let mut seconds = 0.0;
        while seconds < tuning.ball().flight_limit_seconds() {
            let before = motion;
            let step =
                advance_among_obstacles(tuning.arena(), radius, motion, Vec2::ZERO, DT, obstacles);
            motion = step.motion;
            obstacles = obstacles.after(step.obstacle_bounces);
            assert!(obstacles.bounced <= obstacles.rule.bounces_per_flight().get());
            for block in flight.obstacles() {
                assert!(
                    obstacles.spent() || clear_of(*block, radius.value(), motion.position) > -2e-3,
                    "inside a block at {}",
                    motion.position
                );
            }
            if motion.position.x.abs() > contact_line {
                return Some(Flown {
                    before,
                    end: motion,
                    last_step_met_a_wall: step.wall_bounces > 0,
                    bounces: obstacles.bounced,
                });
            }
            seconds += DT;
        }
        None
    }

    /// R4: it terminates. Among a full floor of blocks every return, at the
    /// serve speed and at the cap, reaches a contact line inside the flight
    /// limit, at the speed it left with, and is never inside a block while
    /// its flight has a bounce left. Without the bounce budget some never
    /// arrive: a ball can bounce between a block, its copy, and a wall for
    /// good (M2 spec 2.2.6.1, M2 phase 9 report). Some flights here spend
    /// the budget.
    #[test]
    fn among_a_full_floor_of_blocks_every_return_reaches_a_contact_line() {
        let tuning = tuning();
        let rule = tuning.ball().bounce_rule();
        let mut bounced = 0;
        let mut turned_back = 0;
        let mut spent = 0;
        for rally in 0..200 {
            let blocks = Blocks::new(&tuning, RallySeed::new(rally));
            let flight = Flight::start(FlightStart::SERVE, [blocks.contribute(&after(FULL_DIAL))])
                .bouncing_by(rule);
            for speed in [tuning.ball().serve_speed(), tuning.ball().speed_cap()] {
                for start in returns_at(&tuning, speed) {
                    let Some(flown) = flown(&tuning, start, &flight) else {
                        panic!("rally {rally}: a ball from {start:?} is still in the air");
                    };
                    assert_near(flown.end.velocity.length(), speed, 1e-2 * speed);
                    bounced += u32::from(flown.bounces > 0);
                    turned_back += u32::from(flown.end.position.x < 0.0);
                    spent += u32::from(flown.bounces == rule.bounces_per_flight().get());
                }
            }
        }
        assert!(
            bounced > 2_000,
            "only {bounced} of 14,000 balls met a block"
        );
        assert!(
            turned_back > 500,
            "only {turned_back} balls were turned back"
        );
        assert!(spent > 20, "only {spent} flights spent their bounces");
    }

    /// R5, the shared math (M2 spec 2.2.6): through a full floor of blocks
    /// the AI's prediction is the simulation. Where it gives a crossing of
    /// the far contact line the ball crosses there; where it gives none the
    /// ball was turned back and leaves by its own side.
    #[test]
    fn prediction_equals_simulation_through_a_full_floor_of_blocks() {
        let tuning = tuning();
        let (arena, radius) = (tuning.arena(), tuning.ball().radius());
        let far_line = tuning.crossing_width() / 2.0;
        let wall = arena.center_limit_y(radius);
        let mut through_bounces = 0;
        let mut turned_back = 0;
        for rally in 0..40 {
            let blocks = Blocks::new(&tuning, RallySeed::new(rally));
            let flight = Flight::start(FlightStart::SERVE, [blocks.contribute(&after(FULL_DIAL))])
                .bouncing_by(tuning.ball().bounce_rule());
            for start in returns_at(&tuning, tuning.ball().speed_cap()) {
                let predicted = crossing_y(arena, radius, start, &flight, far_line, DT);
                let flown =
                    flown(&tuning, start, &flight).expect("every return reaches a contact line");
                let Some(predicted) = predicted else {
                    assert!(
                        flown.end.position.x < 0.0,
                        "rally {rally} from {start:?}: {:?}",
                        flown.end
                    );
                    turned_back += 1;
                    continue;
                };
                assert!(
                    flown.end.position.x > 0.0,
                    "rally {rally} from {start:?}: {:?}",
                    flown.end
                );
                // The crossing lies on the last step's path: between its
                // ends, or between an end and the wall it bounced off.
                let (mut low, mut high) = (
                    flown.before.position.y.min(flown.end.position.y),
                    flown.before.position.y.max(flown.end.position.y),
                );
                if flown.last_step_met_a_wall {
                    if flown.end.velocity.y > 0.0 {
                        low = -wall;
                    } else {
                        high = wall;
                    }
                }
                assert!(
                    (low - 1e-3..=high + 1e-3).contains(&predicted),
                    "rally {rally} from {start:?}: predicted {predicted}, flew {low} to {high}"
                );
                through_bounces += u32::from(flown.bounces > 0);
            }
        }
        assert!(
            through_bounces >= 100,
            "only {through_bounces} crossed after a bounce"
        );
        assert!(turned_back >= 50, "only {turned_back} were turned back");
    }

    /// The block of `believed` that stands where `block` does, if any.
    fn standing_at(believed: &[Obstacle], block: Obstacle) -> Option<Obstacle> {
        believed
            .iter()
            .copied()
            .find(|seen| (seen.centre() - block.centre()).length() < 1e-4)
    }

    /// M2 spec 2.6.6 and 2.3.12, second form: the belief lags. An AI does
    /// not see a block until it has stood for its star's unseen flights;
    /// from then on it sees it where it stands, at its own size, and takes
    /// it for more upright than it is: turned by its star's share of the
    /// tilt. Nothing is added to its error or its delay.
    #[test]
    fn each_star_sees_a_block_once_it_is_old_enough_and_then_believes_its_share_of_the_tilt() {
        let blocks = blocks(5);
        let numbers = numbers();
        let all = blocks.placed(&after(FULL_DIAL));
        let last_arrival = the_return_that_brings(numbers.count().full());
        for difficulty in STARS {
            let lag = numbers.unseen_flights(difficulty);
            let share = numbers.believed_share(difficulty);
            for returns in 0..=last_arrival + lag + 2 {
                let flaw = blocks.flaw(&after(returns), difficulty);
                assert_near(flaw.added_error, 0.0, 1e-6);
                assert_eq!(flaw.added_delay, Duration::ZERO);
                assert_near(flaw.believed.speed_gain, 0.0, 1e-6);
                assert_near(flaw.believed.paddle_height_factor, 1.0, 1e-6);
                assert!(flaw.believed.field.is_none());
                let mut seen = 0;
                for (index, pair) in (1..).zip(all.chunks(2)) {
                    let old_enough = returns >= the_return_that_brings(index) + lag;
                    for block in pair {
                        let believed = standing_at(&flaw.believed.obstacles, *block);
                        assert_eq!(
                            believed.is_some(),
                            old_enough,
                            "block {index} after {returns} returns, {difficulty:?}"
                        );
                        if let Some(believed) = believed {
                            assert_vec_near(believed.half_size(), block.half_size(), 1e-6);
                            assert_near(believed.tilt(), share * block.tilt(), 1e-6);
                            seen += 1;
                        }
                    }
                }
                assert_eq!(flaw.believed.obstacles.len(), seen);
            }
        }
    }

    /// M2 spec 2.9.1: a held count is a steady state. Under one every
    /// earlier flight started at this count, so no block is too new to see.
    #[test]
    fn under_a_held_count_no_block_is_too_new_for_any_star_to_see() {
        let blocks = blocks(5);
        for difficulty in STARS {
            for count in 1..=numbers().count().full() {
                let held = FlightStart {
                    returns_held: true,
                    ..after(the_return_that_brings(count))
                };
                let standing = blocks.placed(&held);
                assert_eq!(standing.len(), usize::from(2 * count));
                let believed = blocks.flaw(&held, difficulty).believed.obstacles;
                assert_eq!(believed.len(), standing.len());
                for block in standing {
                    assert!(standing_at(&believed, block).is_some());
                }
            }
        }
    }

    /// More stars, a quicker eye and a truer belief: the embedded lags fall
    /// as the stars rise and the shares rise, and no star sees a block the
    /// flight it appears or believes the whole of its tilt.
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

    /// With no block yet, there is nothing to misjudge.
    #[test]
    fn at_the_serve_every_star_believes_in_no_block() {
        for difficulty in STARS {
            let flaw = blocks(3).flaw(&FlightStart::SERVE, difficulty);
            assert_eq!(flaw.believed.obstacles, []);
        }
    }
}
