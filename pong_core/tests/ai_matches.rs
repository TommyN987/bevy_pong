//! AI-versus-AI matches (M2 spec 2.5.1, 2.5.2, 2.5.5; plan 4.2): do rallies
//! end, do they shorten as the effects escalate, and do more stars win.
//! Played through `testing::ai_match`, each match from its own seed.
//!
//! Every test prints the numbers behind its assertions; `just ai-report`
//! shows them. The phase reports quote that output (plan E.4).
//!
//! Classic is held to the same two questions as Chaos: no star is unbeatable
//! in either mode, and the stars keep their order in both (the user's
//! ruling, M2 phase 4 report).

use std::num::{NonZeroU8, NonZeroU32};

use pong_core::domain::{Difficulty, EffectId, Mode};
use pong_core::features::tuning::{EMBEDDED_RON, Tuning};
use pong_core::kernel::flight::StepMotion;
use pong_core::kernel::geometry::Side;
use pong_core::testing::ai_match::{AiMatch, Forcing, MatchPlayed, RallyLengths};

const ONE: Difficulty = Difficulty::OneStar;
const TWO: Difficulty = Difficulty::TwoStars;
const THREE: Difficulty = Difficulty::ThreeStars;

/// Hits at which a rally of an unforced match is given up as unended. At the
/// shipped handicaps the longest rally `just ai-report` shows has 156 hits;
/// across every handicap swept in M2 plan phase 4, shipped or not, the
/// longest that ended had 195.
const RALLY_HIT_LIMIT: NonZeroU32 = hits(500);

/// Hits at which a rally is given up in a match forced at a held return
/// count, where the question is whether rallies end at all. The ball's speed
/// stands still with the count: at the serve speed 200 hits are over six
/// minutes of one rally, and each unended rally costs the test its whole
/// limit in game time.
const SLOW_RALLY_HIT_LIMIT: NonZeroU32 = hits(200);

const fn hits(limit: u32) -> NonZeroU32 {
    NonZeroU32::new(limit).expect("a positive limit")
}

/// Every pairing of stars, each once.
const PAIRINGS: [(Difficulty, Difficulty); 6] = [
    (ONE, ONE),
    (ONE, TWO),
    (ONE, THREE),
    (TWO, TWO),
    (TWO, THREE),
    (THREE, THREE),
];

/// Chaos at the highest cap, where the most effects stack.
fn chaos() -> Mode {
    Mode::Chaos {
        cap: NonZeroU8::new(3).expect("three is non-zero"),
    }
}

/// A match of `mode` to five points with nothing forced.
fn match_of(mode: Mode, left: Difficulty, right: Difficulty) -> AiMatch {
    AiMatch {
        left,
        right,
        mode,
        points_to_win: NonZeroU8::new(5).expect("five is non-zero"),
        forced: Forcing::default(),
        rally_hit_limit: RALLY_HIT_LIMIT,
    }
}

/// A Chaos match to five points with nothing forced.
fn chaos_match(left: Difficulty, right: Difficulty) -> AiMatch {
    match_of(chaos(), left, right)
}

fn name(mode: Mode) -> &'static str {
    match mode {
        Mode::Classic => "classic",
        Mode::Chaos { .. } => "chaos",
    }
}

fn play_seeds(setup: &AiMatch, seeds: std::ops::Range<u64>) -> Vec<MatchPlayed> {
    seeds.map(|seed| setup.play(seed)).collect()
}

fn lengths(played: &[MatchPlayed]) -> RallyLengths {
    RallyLengths::of(played.iter().flat_map(|one| one.rallies.iter().copied()))
}

fn stars(setup: &AiMatch) -> String {
    format!("{} v {} stars", setup.left.stars(), setup.right.stars())
}

/// Every rally of `mode` between computers ends inside the hit limit, in
/// every pairing of stars, and so every match is finished.
fn every_rally_ends_in_every_pairing(mode: Mode) {
    for (left, right) in PAIRINGS {
        let setup = match_of(mode, left, right);
        let played = play_seeds(&setup, 0..4);
        let lengths = lengths(&played);
        println!("{}, {}: {lengths}", name(mode), stars(&setup));
        assert_eq!(lengths.unended, 0, "{}: {lengths}", stars(&setup));
        assert!(
            played.iter().all(|one| one.winner.is_some()),
            "{}: a match went unfinished",
            stars(&setup)
        );
    }
}

/// M2 spec 2.5.1, first half: seeded AI-versus-AI Chaos matches show bounded
/// rally length.
#[test]
fn chaos_rallies_end_within_the_hit_limit_in_every_pairing() {
    every_rally_ends_in_every_pairing(chaos());
}

/// No star is unbeatable in Classic either: the speed ramp alone ends every
/// rally, the three-star mirror's too.
#[test]
fn classic_rallies_end_within_the_hit_limit_in_every_pairing() {
    every_rally_ends_in_every_pairing(Mode::Classic);
}

/// M2 spec 2.5.1, second half, confirming the fact relied on: with every
/// dial at zero and no speed ramp a competent defender returns every ball.
/// The sandbox forces no effect at all, Speed included.
#[test]
fn with_no_effect_in_play_no_rally_between_competent_computers_ends() {
    for stars_each in [TWO, THREE] {
        let setup = AiMatch {
            forced: Forcing {
                effects: Some(Vec::new()),
                returns: None,
                hit_motion: None,
            },
            rally_hit_limit: SLOW_RALLY_HIT_LIMIT,
            ..chaos_match(stars_each, stars_each)
        };
        let played = play_seeds(&setup, 0..2);
        let lengths = lengths(&played);
        println!("no effects, {}: {lengths}", stars(&setup));
        assert_eq!(
            (lengths.rallies, lengths.unended),
            (2, 2),
            "{}: a rally ended with nothing escalating: {lengths}",
            stars(&setup)
        );
    }
}

/// M2 spec 2.5.5: seeded win rates rise from one star to two to three, in
/// Chaos and in Classic. Each pairing is played from both sides, so no
/// side's serve or seat decides it.
#[test]
fn in_both_modes_the_computer_with_more_stars_wins_more_matches() {
    for mode in [Mode::Classic, chaos()] {
        for (fewer, more) in [(ONE, TWO), (TWO, THREE), (ONE, THREE)] {
            let seeds = 0..4;
            let more_on_the_right = play_seeds(&match_of(mode, fewer, more), seeds.clone());
            let more_on_the_left = play_seeds(&match_of(mode, more, fewer), seeds);
            let won_by = |played: &[MatchPlayed], side: Side| {
                played.iter().filter(|one| one.winner == Some(side)).count()
            };
            let more_won =
                won_by(&more_on_the_right, Side::Right) + won_by(&more_on_the_left, Side::Left);
            let fewer_won =
                won_by(&more_on_the_right, Side::Left) + won_by(&more_on_the_left, Side::Right);
            println!(
                "{}, {} against {} stars: {more_won} matches to {fewer_won}",
                name(mode),
                more.stars(),
                fewer.stars()
            );
            assert!(
                more_won > fewer_won,
                "{}: {} stars won {more_won}, {} stars won {fewer_won}",
                name(mode),
                more.stars(),
                fewer.stars()
            );
        }
    }
}

/// Sampling noise allowed between two means of ended rallies, in hits. The
/// seeded runs are small, and a mean may wobble by a fraction of a hit from
/// one rung to the next without the effect having eased.
const NOISE: f32 = 0.5;

/// Whether the rallies of `after` are no longer than those of `before`, as
/// far as two seeded runs can tell.
///
/// A run with unended rallies has no true mean: the mean it reports is a
/// floor set by the hit limit. Such runs are compared by how many of their
/// rallies never ended; only two runs whose rallies all ended are compared
/// by their means, within [`NOISE`].
fn no_longer(before: RallyLengths, after: RallyLengths) -> bool {
    if before.unended > 0 || after.unended > 0 {
        // after.unended / after.rallies <= before.unended / before.rallies
        after.unended * before.rallies <= before.unended * after.rallies
    } else {
        after.mean <= before.mean + NOISE
    }
}

/// Whether the rallies of `after` are shorter than those of `before` by
/// more than noise: every rally of `after` ended, and either some of
/// `before` never did, or its mean is higher by more than [`NOISE`].
fn shorter(before: RallyLengths, after: RallyLengths) -> bool {
    after.unended == 0 && (before.unended > 0 || after.mean < before.mean - NOISE)
}

/// Asserts R3 along `rungs`, lowest dial first: rallies never lengthen from
/// one rung to the next, and are shorter at the top than at the bottom.
fn assert_rallies_shorten(what: &str, rungs: &[RallyLengths]) {
    for pair in rungs.windows(2) {
        assert!(
            no_longer(pair[0], pair[1]),
            "{what}: rallies grew from one rung to the next:\n{}\n{}",
            pair[0],
            pair[1]
        );
    }
    let (bottom, top) = (rungs[0], rungs[rungs.len() - 1]);
    assert!(
        shorter(bottom, top),
        "{what}: rallies at the top are no shorter than at the bottom:\n{bottom}\n{top}"
    );
}

/// Seeds per rung for a mirror match of `stars_each`. One-star rallies are a
/// few hits long, so many are needed for a steady mean and they cost little;
/// two-star rallies at a low dial never end, and each costs its hit limit.
fn seeds_per_rung(stars_each: Difficulty) -> std::ops::Range<u64> {
    if stars_each == ONE { 0..8 } else { 0..2 }
}

/// Return counts held per level of breadth: each step up adds one scheduled
/// effect and this many returns, so breadth and intensity rise together.
const RETURNS_PER_LEVEL: u16 = 15;

/// M2 spec 2.5.2, R3 for the stack: AI-versus-AI rally length shortens as
/// breadth and hit count rise together. Level `k` forces the mode's
/// always-on effects and the first `k` of its bag, at a held return count of
/// `k` times fifteen; the levels run to the cap or the end of the bag.
///
/// Mirror matches of one, two, and three stars. With Speed and Shrink alone
/// three stars miss only near the speed cap, beyond the counts these levels
/// hold: their rallies end at no level until an effect's flaw reaches them.
/// Wind's does, at the third level (M2 phase 5 report).
#[test]
fn chaos_rallies_shorten_as_breadth_and_return_count_rise_together() {
    let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
    let mode = tuning.mode(chaos());
    let levels = mode.bag().len().min(3);
    assert!(levels >= 1, "Chaos schedules no effect");
    for stars_each in [ONE, TWO, THREE] {
        let rungs: Vec<RallyLengths> = (0..=levels)
            .map(|level| {
                let effects: Vec<EffectId> = mode
                    .always()
                    .iter()
                    .chain(&mode.bag()[..level])
                    .copied()
                    .collect();
                let returns =
                    u16::try_from(level).expect("at most three levels") * RETURNS_PER_LEVEL;
                let setup = AiMatch {
                    forced: Forcing {
                        effects: Some(effects.clone()),
                        returns: Some(returns),
                        hit_motion: None,
                    },
                    rally_hit_limit: SLOW_RALLY_HIT_LIMIT,
                    ..chaos_match(stars_each, stars_each)
                };
                let lengths = lengths(&play_seeds(&setup, seeds_per_rung(stars_each)));
                println!(
                    "stack, {}, {effects:?} at {returns} returns: {lengths}",
                    stars(&setup)
                );
                lengths
            })
            .collect();
        assert_rallies_shorten(&format!("the stack, {} stars", stars_each.stars()), &rungs);
    }
}

/// Held return counts an effect's dial is measured at: from zero, where
/// every dial is mild (R9), past where the embedded dials are full.
const DIAL_LADDER: [u16; 6] = [0, 5, 10, 15, 20, 30];

/// The paddle motion every return of a dial test is told. With no speed
/// ramp a computer reaches its aim and stands still at every hit, so an
/// effect that reads the motion, as Spin does, would never act: unheld, Spin
/// alone plays the same rallies at every rung (M2 phase 6 report). An
/// effect that does not read the motion plays the same held or not.
const HELD_HIT_MOTION: StepMotion = StepMotion::Up;

/// R3 per effect (M2 spec 2.5.2, plan E.4): `effect` alone, with no speed
/// ramp beside it, at each rung of the ladder, every return told as off a
/// paddle moving [`HELD_HIT_MOTION`]. The rallies of the mirror match of
/// `stars_each`, rung by rung.
fn rallies_up_the_dial(effect: EffectId, stars_each: Difficulty) -> Vec<RallyLengths> {
    DIAL_LADDER
        .iter()
        .map(|&returns| {
            let setup = AiMatch {
                forced: Forcing {
                    effects: Some(vec![effect]),
                    returns: Some(returns),
                    hit_motion: Some(HELD_HIT_MOTION),
                },
                rally_hit_limit: SLOW_RALLY_HIT_LIMIT,
                ..chaos_match(stars_each, stars_each)
            };
            let lengths = lengths(&play_seeds(&setup, seeds_per_rung(stars_each)));
            println!(
                "dial, {effect:?} alone at {returns} returns, {}: {lengths}",
                stars(&setup)
            );
            lengths
        })
        .collect()
}

/// How many effects Chaos's bag can hold and still have each one measured:
/// one test below per place in the bag.
const BAG_PLACES: usize = 7;

/// M2 spec 2.5.2 and R3, per effect (plan E.4): an effect raises the miss
/// probability, monotone in its dial; falsified if rally length does not
/// shorten as the dial rises. The effect at `place` in Chaos's bag is
/// measured, whichever it is, so a new effect is held to this with no edit
/// here. One test per place keeps each inside the per-test time limit
/// however long the bag grows; a place the bag does not fill yet has
/// nothing to measure, and its test says so and passes.
///
/// Mirror matches of one and of two stars. Three stars' aim error is inside
/// the reach of the shortest paddle, so Shrink alone ends none of their
/// rallies at any rung (M2 phase 4 report).
fn the_bag_effect_alone_shortens_rallies_as_its_dial_rises(place: usize) {
    let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
    let Some(&effect) = tuning.mode(chaos()).bag().get(place) else {
        println!("dial: Chaos's bag has no effect at place {place}");
        return;
    };
    for stars_each in [ONE, TWO] {
        assert_rallies_shorten(
            &format!("{effect:?} alone, {} stars", stars_each.stars()),
            &rallies_up_the_dial(effect, stars_each),
        );
    }
}

macro_rules! one_dial_test_per_bag_place {
    ($($test:ident: $place:expr,)*) => {
        $(
            #[test]
            fn $test() {
                the_bag_effect_alone_shortens_rallies_as_its_dial_rises($place);
            }
        )*

        /// Every effect of the bag has a place a test measures, and the bag
        /// is not empty: no effect goes unmeasured for want of a test. The
        /// tests' places are each place once, so none is measured twice
        /// while another is skipped.
        #[test]
        fn every_effect_of_chaos_bag_has_a_dial_test() {
            let places: &[usize] = &[$($place),*];
            assert_eq!(places, (0..BAG_PLACES).collect::<Vec<_>>());
            let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
            let bag = tuning.mode(chaos()).bag().len();
            assert!(
                (1..=BAG_PLACES).contains(&bag),
                "the bag holds {bag} effects; the dial tests cover 1 to {BAG_PLACES}"
            );
        }
    };
}

one_dial_test_per_bag_place! {
    the_first_bag_effect_alone_shortens_rallies_as_its_dial_rises: 0,
    the_second_bag_effect_alone_shortens_rallies_as_its_dial_rises: 1,
    the_third_bag_effect_alone_shortens_rallies_as_its_dial_rises: 2,
    the_fourth_bag_effect_alone_shortens_rallies_as_its_dial_rises: 3,
    the_fifth_bag_effect_alone_shortens_rallies_as_its_dial_rises: 4,
    the_sixth_bag_effect_alone_shortens_rallies_as_its_dial_rises: 5,
    the_seventh_bag_effect_alone_shortens_rallies_as_its_dial_rises: 6,
}

/// The comparisons above must be able to fail: a run pinned at the hit
/// limit is not "longer" than another pinned there, and a top rung with any
/// unended rally has not been shown shorter than anything.
#[test]
fn censored_runs_are_compared_by_their_unended_rallies_not_their_means() {
    let run = |rallies: usize, unended: usize, mean: f32| RallyLengths {
        rallies,
        unended,
        mean,
        median: 0,
        longest: 0,
    };
    let never_ends = run(3, 3, 200.0);
    let mostly_unended = run(4, 3, 195.2);
    let all_end_long = run(20, 0, 199.0);
    let all_end_short = run(20, 0, 4.0);

    assert!(no_longer(never_ends, never_ends));
    assert!(no_longer(never_ends, mostly_unended));
    assert!(!no_longer(mostly_unended, never_ends));
    assert!(no_longer(mostly_unended, all_end_long));
    assert!(!no_longer(all_end_short, mostly_unended));
    // Between runs whose rallies all ended, the means decide, within noise.
    assert!(no_longer(all_end_short, run(20, 0, 4.4)));
    assert!(!no_longer(all_end_short, run(20, 0, 4.6)));

    assert!(shorter(never_ends, all_end_long));
    assert!(!shorter(never_ends, mostly_unended));
    assert!(!shorter(never_ends, never_ends));
    assert!(shorter(all_end_long, all_end_short));
    assert!(!shorter(all_end_short, run(20, 0, 3.6)));
}
