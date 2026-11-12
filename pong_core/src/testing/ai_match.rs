//! AI-versus-AI matches (M2 spec 2.5, plan 4.2): a seeded headless match
//! between two computers, reported as the length of every rally and who won.
//! The instrument for termination (2.5.1), R3 (2.5.2), and the order of the
//! stars (2.5.5); every effect phase reports through it (plan E.4).

use std::fmt;
use std::num::NonZeroU8;

use bevy::app::App;

use super::{
    FrameLength, boot, enter_match, frame, headless_app, record, set_frame_length, take_recorded,
};
use crate::domain::{
    BallHitPaddle, Controller, Difficulty, EffectId, MatchConfig, MatchEnded, Mode, PointScored,
    SeatPlayers,
};
use crate::features::effects::Sandbox;
use crate::kernel::geometry::Side;

/// Fixed steps per frame while a match is played. The simulation steps the
/// same at any frame length; longer frames spend fewer on the countdowns.
const STEPS_PER_FRAME: u32 = 4;

/// One match between two computers, to be played from any number of seeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiMatch {
    /// The stars of the computer on the left.
    pub left: Difficulty,
    /// The stars of the computer on the right.
    pub right: Difficulty,
    /// The game played.
    pub mode: Mode,
    /// Points that win the match.
    pub points_to_win: NonZeroU8,
    /// What the developer sandbox forces, from the first rally on.
    pub forced: Forcing,
    /// Paddle hits at which a rally is given up as unended, and the match
    /// with it. Precondition: not zero.
    pub rally_hit_limit: u32,
}

/// What the developer [`Sandbox`] forces for a whole match (M2 spec 2.9.1).
/// The default forces nothing: the mode's schedule and the rally decide.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Forcing {
    /// The whole set of effects in play, the always-on ones included;
    /// `None` leaves the set to the mode's schedule.
    pub effects: Option<Vec<EffectId>>,
    /// The return count every flight starts at, which holds every dial
    /// still; `None` lets the rally count.
    pub returns: Option<u16>,
}

/// One rally of a played match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RallyPlayed {
    /// Paddle hits in the rally, end clips included: what the game's
    /// longest-rally record counts. Under a held return count the flight's
    /// own count stands still, and this one does not.
    pub hits: u32,
    /// The side that won the point; `None` for a rally given up at the
    /// match's [`rally_hit_limit`](AiMatch::rally_hit_limit).
    pub won_by: Option<Side>,
}

/// A match as it was played.
///
/// Invariant: every rally but the last was won; the last was won exactly
/// when `winner` is some side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPlayed {
    /// The side that reached the points to win; `None` for a match given up
    /// at an unended rally.
    pub winner: Option<Side>,
    /// Every rally, in the order played.
    pub rallies: Vec<RallyPlayed>,
}

impl AiMatch {
    /// Plays the match from `seed`, which seeds the random source and the
    /// order of unordered systems. The same seed plays the same match.
    ///
    /// Postcondition: returns once a side has won, or once a rally has
    /// reached the hit limit. A ball that neither scores nor is hit faults
    /// in the sanity phase (M2 spec 2.5.6), which panics under the harness.
    ///
    /// # Panics
    /// If the app cannot boot or enter the match, or on any simulation fault.
    #[must_use]
    pub fn play(&self, seed: u64) -> MatchPlayed {
        let mut app = self.begin(seed);
        let mut rallies = Vec::new();
        let mut hits = 0_u32;
        loop {
            frame(&mut app);
            let hit = take_recorded::<BallHitPaddle>(&mut app).len();
            hits += u32::try_from(hit).expect("a frame's paddle hits fit u32");
            // A countdown separates two points, so a frame holds at most one.
            if let Some(point) = take_recorded::<PointScored>(&mut app).first() {
                rallies.push(RallyPlayed {
                    hits,
                    won_by: Some(point.scorer),
                });
                hits = 0;
            }
            if let Some(ended) = take_recorded::<MatchEnded>(&mut app).first() {
                return MatchPlayed {
                    winner: Some(ended.winner),
                    rallies,
                };
            }
            if hits >= self.rally_hit_limit {
                rallies.push(RallyPlayed { hits, won_by: None });
                return MatchPlayed {
                    winner: None,
                    rallies,
                };
            }
        }
    }

    /// A booted app in the match's first countdown, with the forcing set
    /// and the recorders [`play`](Self::play) reads.
    fn begin(&self, seed: u64) -> App {
        let mut app = headless_app(seed);
        record::<BallHitPaddle>(&mut app);
        record::<PointScored>(&mut app);
        record::<MatchEnded>(&mut app);
        boot(&mut app);
        let mut sandbox = app.world_mut().resource_mut::<Sandbox>();
        if let Some(effects) = &self.forced.effects {
            sandbox.force_effects(effects);
        }
        if self.forced.returns.is_some() {
            sandbox.hold_returns(self.forced.returns);
        }
        set_frame_length(&mut app, FrameLength::Steps(STEPS_PER_FRAME));
        enter_match(
            &mut app,
            MatchConfig {
                left: Controller::Ai(self.left),
                right: Controller::Ai(self.right),
                players: SeatPlayers::GUESTS,
                points_to_win: self.points_to_win,
                mode: self.mode,
            },
        );
        app
    }
}

/// How long a set of rallies ran, in paddle hits.
///
/// An unended rally counts with the hits it had when it was given up, so
/// with any unended the mean, the median, and the longest are lower bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RallyLengths {
    /// How many rallies.
    pub rallies: usize,
    /// How many of them were given up at the hit limit.
    pub unended: usize,
    /// Mean hits per rally; zero of no rallies.
    pub mean: f32,
    /// The middle rally's hits, the lower of two; zero of no rallies.
    pub median: u32,
    /// The most hits in one rally; zero of no rallies.
    pub longest: u32,
}

impl RallyLengths {
    /// The lengths of `rallies`, in any order.
    #[must_use]
    pub fn of(rallies: impl IntoIterator<Item = RallyPlayed>) -> Self {
        let rallies: Vec<RallyPlayed> = rallies.into_iter().collect();
        let mut hits: Vec<u32> = rallies.iter().map(|rally| rally.hits).collect();
        hits.sort_unstable();
        let total: f64 = hits.iter().copied().map(f64::from).sum();
        // More rallies than `u32` holds would only flatten the mean.
        let count = u32::try_from(hits.len()).unwrap_or(u32::MAX);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a mean of hit counts is reported to one decimal"
        )]
        let mean = if count == 0 {
            0.0
        } else {
            (total / f64::from(count)) as f32
        };
        Self {
            rallies: rallies.len(),
            unended: rallies
                .iter()
                .filter(|rally| rally.won_by.is_none())
                .count(),
            mean,
            median: hits
                .get(hits.len().saturating_sub(1) / 2)
                .copied()
                .unwrap_or(0),
            longest: hits.last().copied().unwrap_or(0),
        }
    }
}

impl fmt::Display for RallyLengths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} rallies: mean {:.1}, median {}, longest {}, {} unended",
            self.rallies, self.mean, self.median, self.longest, self.unended
        )
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;

    use super::{AiMatch, Forcing, MatchPlayed, RallyLengths, RallyPlayed};
    use crate::domain::{Difficulty, EffectId, Mode};
    use crate::features::effects::ActiveEffects;
    use crate::features::paddle::Paddle;
    use crate::flow::states::MatchState;
    use crate::kernel::geometry::Side;
    use crate::kernel::test_support::assert_near;
    use crate::testing::{frame, match_state, run_until};

    fn mirror(stars: Difficulty, points: u8) -> AiMatch {
        AiMatch {
            left: stars,
            right: stars,
            mode: Mode::Classic,
            points_to_win: NonZeroU8::new(points).expect("a positive target"),
            forced: Forcing::default(),
            rally_hit_limit: 1_000,
        }
    }

    fn won_by(played: &MatchPlayed, side: Side) -> usize {
        played
            .rallies
            .iter()
            .filter(|rally| rally.won_by == Some(side))
            .count()
    }

    #[test]
    fn a_played_match_reports_its_winner_and_one_rally_per_point() {
        let played = mirror(Difficulty::OneStar, 3).play(3);
        let winner = played.winner.expect("one-star rallies end");
        let loser = match winner {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        };
        assert_eq!(won_by(&played, winner), 3);
        assert!(won_by(&played, loser) < 3);
        assert_eq!(
            played.rallies.len(),
            won_by(&played, winner) + won_by(&played, loser),
            "every rally of a finished match was won"
        );
    }

    #[test]
    fn the_same_seed_plays_the_same_match_and_another_seed_another() {
        let setup = mirror(Difficulty::OneStar, 3);
        assert_eq!(setup.play(3), setup.play(3));
        assert_ne!(setup.play(3), setup.play(4));
    }

    /// The limit is what lets a harness ask whether rallies end at all: a
    /// rally that reaches it is reported unended and the match unfinished.
    #[test]
    fn a_rally_that_reaches_the_hit_limit_ends_the_match_unfinished() {
        let setup = AiMatch {
            rally_hit_limit: 2,
            ..mirror(Difficulty::ThreeStars, 3)
        };
        let played = setup.play(3);
        assert_eq!(played.winner, None);
        assert_eq!(
            played.rallies.last(),
            Some(&RallyPlayed {
                hits: 2,
                won_by: None
            })
        );
    }

    /// What is forced is in play from the first rally: the set at the first
    /// countdown, the count at the first serve.
    #[test]
    fn a_forced_set_and_a_held_count_are_in_play_from_the_first_serve() {
        let setup = AiMatch {
            mode: Mode::Chaos {
                cap: NonZeroU8::new(1).expect("one is non-zero"),
            },
            forced: Forcing {
                effects: Some(vec![EffectId::Shrink]),
                // The embedded Shrink is at full dial from return 15.
                returns: Some(15),
            },
            ..mirror(Difficulty::OneStar, 3)
        };
        let mut app = setup.begin(3);
        assert_eq!(
            app.world().resource::<ActiveEffects>().ids(),
            [EffectId::Shrink],
            "no Speed: the forced set is the whole set"
        );
        run_until(
            &mut app,
            |app| match_state(app) == MatchState::Playing,
            1_000,
            "play never began",
        );
        frame(&mut app);
        frame(&mut app);
        let heights: Vec<f32> = app
            .world_mut()
            .query::<&Paddle>()
            .iter(app.world())
            .map(|paddle| paddle.height())
            .collect();
        assert_eq!(heights, [8.0, 8.0]);
    }

    #[test]
    fn a_match_with_nothing_forced_plays_its_mode() {
        let app = mirror(Difficulty::OneStar, 3).begin(3);
        assert_eq!(
            app.world().resource::<ActiveEffects>().ids(),
            [EffectId::Speed]
        );
    }

    fn rally(hits: u32, won_by: Option<Side>) -> RallyPlayed {
        RallyPlayed { hits, won_by }
    }

    #[test]
    fn rally_lengths_count_the_rallies_and_give_their_mean_median_and_longest() {
        let lengths = RallyLengths::of([
            rally(9, None),
            rally(2, Some(Side::Left)),
            rally(4, Some(Side::Right)),
        ]);
        assert_eq!(lengths.rallies, 3);
        assert_eq!(lengths.unended, 1);
        assert_near(lengths.mean, 5.0, 1e-6);
        assert_eq!(lengths.median, 4);
        assert_eq!(lengths.longest, 9);
        assert_eq!(
            lengths.to_string(),
            "3 rallies: mean 5.0, median 4, longest 9, 1 unended"
        );
    }

    #[test]
    fn of_an_even_number_of_rallies_the_median_is_the_lower_middle_one() {
        let lengths = RallyLengths::of([1, 3, 5, 100].map(|hits| rally(hits, Some(Side::Left))));
        assert_eq!(lengths.median, 3);
    }

    #[test]
    fn no_rallies_have_no_lengths() {
        let lengths = RallyLengths::of([]);
        assert_eq!((lengths.rallies, lengths.unended), (0, 0));
        assert_near(lengths.mean, 0.0, 1e-6);
        assert_eq!((lengths.median, lengths.longest), (0, 0));
    }
}
