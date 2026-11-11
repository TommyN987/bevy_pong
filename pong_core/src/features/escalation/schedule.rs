//! The schedule's arithmetic (M2 spec 2.4.1, 2.4.4), free of the ECS and of
//! any effect: how many are in play, and which. It names no effect; it turns
//! over whatever it is handed, so it is tested with plain numbers.

use std::collections::VecDeque;
use std::num::NonZeroU8;

/// How many scheduled effects are in play when the leader has `leader` of
/// `target` points: one from the first point, one more for each threshold
/// the leader's progress has reached, never above `cap` nor above the
/// `available` effects.
///
/// Progress is compared in whole numbers, `leader * 100 >= percent * target`,
/// so a boundary such as 3 of 10 against 30 percent is exact.
#[must_use]
pub fn count(
    leader: u8,
    target: NonZeroU8,
    thresholds_percent: &[u8],
    cap: usize,
    available: usize,
) -> usize {
    let reached = thresholds_percent
        .iter()
        .filter(|percent| u32::from(leader) * 100 >= u32::from(**percent) * u32::from(target.get()))
        .count();
    (1 + reached).min(cap).min(available)
}

/// Puts `items` in a random order, drawing each pick from `below(n)`, a whole
/// number in `0..n`. A list of one or none draws nothing.
pub fn shuffle<T>(items: &mut [T], mut below: impl FnMut(u32) -> u32) {
    for last in (1..items.len()).rev() {
        let choices = u32::try_from(last + 1).unwrap_or(u32::MAX);
        let pick = usize::try_from(below(choices)).unwrap_or(0);
        items.swap(last, pick.min(last));
    }
}

/// Who joined and who left in one [`Rotation::advance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes<T> {
    /// Joined the items in play, in order.
    pub started: Vec<T>,
    /// Left the items in play, in order.
    pub ended: Vec<T>,
}

/// A bag turned over a few items at a time: some in play, oldest first, the
/// rest on a bench in the order they will join.
///
/// Invariant: every item of the bag is in exactly one of the two. An item
/// that leaves goes to the back of the bench, so no item returns before
/// every other has had its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotation<T> {
    active: VecDeque<T>,
    bench: VecDeque<T>,
}

/// An empty bag: nothing in play, nothing to join.
impl<T> Default for Rotation<T> {
    fn default() -> Self {
        Self {
            active: VecDeque::new(),
            bench: VecDeque::new(),
        }
    }
}

impl<T: Copy> Rotation<T> {
    /// A rotation with nothing in play yet and `bag` waiting, first to join first.
    pub fn new(bag: impl IntoIterator<Item = T>) -> Self {
        Self {
            active: VecDeque::new(),
            bench: bag.into_iter().collect(),
        }
    }

    /// The items in play, oldest first.
    pub fn active(&self) -> impl Iterator<Item = T> + '_ {
        self.active.iter().copied()
    }

    /// Brings the items in play to `count`, then, if `rotate` and the count
    /// did not rise, swaps the oldest for the next on the bench.
    ///
    /// A rising count only adds: one joins and none leaves (M2 spec 2.4.4).
    /// With an empty bench nothing rotates. A count above the bag fills as
    /// far as the bag goes.
    pub fn advance(&mut self, count: usize, rotate: bool) -> Changes<T> {
        let mut changes = Changes {
            started: Vec::new(),
            ended: Vec::new(),
        };
        if self.active.len() < count {
            while self.active.len() < count {
                let Some(next) = self.bench.pop_front() else {
                    break;
                };
                self.active.push_back(next);
                changes.started.push(next);
            }
            return changes;
        }
        while self.active.len() > count {
            let Some(oldest) = self.active.pop_front() else {
                break;
            };
            self.bench.push_back(oldest);
            changes.ended.push(oldest);
        }
        if rotate && let Some(oldest) = self.active.pop_front() {
            if let Some(next) = self.bench.pop_front() {
                self.bench.push_back(oldest);
                self.active.push_back(next);
                changes.ended.push(oldest);
                changes.started.push(next);
            } else {
                self.active.push_front(oldest);
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use super::{Changes, Rotation, count, shuffle};

    fn target(points: u8) -> NonZeroU8 {
        NonZeroU8::new(points).expect("a positive target")
    }

    const THRESHOLDS: [u8; 2] = [30, 70];

    /// Counts at every leader score from zero to match point.
    fn counts(points: u8, cap: usize, available: usize) -> Vec<usize> {
        (0..points)
            .map(|leader| count(leader, target(points), &THRESHOLDS, cap, available))
            .collect()
    }

    /// M2 spec 2.4.1's done-condition, target 5: one at 0–1, two at 2–3, three at 4.
    #[test]
    fn to_five_points_the_count_is_one_then_two_from_two_then_three_at_four() {
        assert_eq!(counts(5, 3, 6), [1, 1, 2, 2, 3]);
    }

    /// Target 10: one at 0–2, two at 3–6, three at 7–9. Three of ten is
    /// exactly thirty percent, and counts.
    #[test]
    fn to_ten_points_the_count_is_one_then_two_from_three_then_three_from_seven() {
        assert_eq!(counts(10, 3, 6), [1, 1, 1, 2, 2, 2, 2, 3, 3, 3]);
    }

    /// Target 21: one at 0–6, two at 7–14, three at 15–20.
    #[test]
    fn to_twenty_one_points_the_count_is_one_then_two_from_seven_then_three_from_fifteen() {
        let counts = counts(21, 3, 6);
        assert_eq!(counts[..7], [1; 7]);
        assert_eq!(counts[7..15], [2; 8]);
        assert_eq!(counts[15..], [3; 6]);
    }

    #[test]
    fn a_cap_of_one_or_two_holds_the_count_there() {
        assert_eq!(counts(5, 1, 6), [1, 1, 1, 1, 1]);
        assert_eq!(counts(5, 2, 6), [1, 1, 2, 2, 2]);
    }

    /// M2 spec 2.4.1 as amended: the shipped effects cap the count too, and
    /// a mode with an empty bag has none in play.
    #[test]
    fn the_count_never_exceeds_the_effects_available() {
        assert_eq!(counts(5, 3, 2), [1, 1, 2, 2, 2]);
        assert_eq!(counts(5, 3, 0), [0, 0, 0, 0, 0]);
        assert_eq!(
            count(4, target(5), &[], 3, 6),
            1,
            "no thresholds, one in play"
        );
    }

    #[test]
    fn a_shuffle_keeps_every_item_and_follows_its_draws() {
        let mut items = [1, 2, 3, 4];
        // Every draw picks the first: each item in turn swaps with the front.
        shuffle(&mut items, |_| 0);
        assert_eq!(items, [2, 3, 4, 1]);
        let mut sorted = items;
        sorted.sort_unstable();
        assert_eq!(sorted, [1, 2, 3, 4]);
    }

    #[test]
    fn a_shuffle_of_one_or_none_draws_nothing() {
        let mut draws = 0;
        shuffle(&mut [7], |_| {
            draws += 1;
            0
        });
        shuffle::<u8>(&mut [], |_| {
            draws += 1;
            0
        });
        assert_eq!(draws, 0);
    }

    fn in_play(rotation: &Rotation<u8>) -> Vec<u8> {
        rotation.active().collect()
    }

    #[test]
    fn a_rising_count_adds_one_and_none_leaves_even_when_a_rotation_is_due() {
        let mut rotation = Rotation::new([1, 2, 3, 4]);
        assert_eq!(
            rotation.advance(1, false),
            Changes {
                started: vec![1],
                ended: vec![]
            }
        );
        assert_eq!(
            rotation.advance(2, true),
            Changes {
                started: vec![2],
                ended: vec![]
            }
        );
        assert_eq!(in_play(&rotation), [1, 2]);
    }

    /// M2 spec 2.4.4: exactly one swaps per rotation, the oldest out.
    #[test]
    fn a_rotation_swaps_the_oldest_for_the_next_and_nothing_else() {
        let mut rotation = Rotation::new([1, 2, 3, 4]);
        rotation.advance(2, false);
        assert_eq!(
            rotation.advance(2, true),
            Changes {
                started: vec![3],
                ended: vec![1]
            }
        );
        assert_eq!(in_play(&rotation), [2, 3]);
    }

    #[test]
    fn without_a_rotation_due_nothing_changes() {
        let mut rotation = Rotation::new([1, 2, 3]);
        rotation.advance(2, false);
        assert_eq!(
            rotation.advance(2, false),
            Changes {
                started: vec![],
                ended: vec![]
            }
        );
        assert_eq!(in_play(&rotation), [1, 2]);
    }

    /// M2 spec 2.4.4: no item repeats before the bag empties.
    #[test]
    fn no_item_returns_before_every_other_has_had_its_turn() {
        let mut rotation = Rotation::new([1, 2, 3, 4, 5]);
        let mut joined: Vec<u8> = rotation.advance(1, false).started;
        for _ in 0..9 {
            joined.extend(rotation.advance(1, true).started);
        }
        assert_eq!(joined, [1, 2, 3, 4, 5, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn with_an_empty_bench_nothing_rotates() {
        let mut rotation = Rotation::new([1, 2]);
        rotation.advance(2, false);
        assert_eq!(
            rotation.advance(2, true),
            Changes {
                started: vec![],
                ended: vec![]
            }
        );
        assert_eq!(in_play(&rotation), [1, 2]);
    }

    #[test]
    fn a_count_above_the_bag_fills_as_far_as_the_bag_goes() {
        let mut rotation = Rotation::new([1, 2]);
        assert_eq!(rotation.advance(3, false).started, [1, 2]);
        assert_eq!(in_play(&rotation), [1, 2]);
    }

    #[test]
    fn a_falling_count_retires_the_oldest_to_the_back_of_the_bench() {
        let mut rotation = Rotation::new([1, 2, 3]);
        rotation.advance(2, false);
        assert_eq!(rotation.advance(1, false).ended, [1]);
        assert_eq!(in_play(&rotation), [2]);
        assert_eq!(rotation.advance(3, false).started, [3, 1]);
    }

    #[test]
    fn an_empty_bag_has_nothing_in_play_whatever_is_asked() {
        let mut rotation = Rotation::<u8>::default();
        let changes = rotation.advance(3, true);
        assert!(changes.started.is_empty() && changes.ended.is_empty());
        assert_eq!(in_play(&rotation), Vec::<u8>::new());
    }
}
