//! AI-versus-AI matches under Chaos (M2 spec 2.5.1, 2.5.2, 2.5.5; plan 4.2):
//! do rallies end, do they shorten as the effects escalate, and do more stars
//! win. Played through `testing::ai_match`, each match from its own seed.
//!
//! Every test prints the numbers behind its assertions; `just ai-report`
//! shows them. The phase reports quote that output (plan E.4).
//!
//! One finding is pinned here as a test of its own, not hidden: between two
//! three-star computers no Chaos rally ends, so spec 2.5.1 is not met for
//! that pairing (M2 phase 4 report).

use std::num::NonZeroU8;

use pong_core::domain::{Difficulty, EffectId, Mode};
use pong_core::features::tuning::{EMBEDDED_RON, Tuning};
use pong_core::kernel::geometry::Side;
use pong_core::testing::ai_match::{AiMatch, Forcing, MatchPlayed, RallyLengths};

const ONE: Difficulty = Difficulty::OneStar;
const TWO: Difficulty = Difficulty::TwoStars;
const THREE: Difficulty = Difficulty::ThreeStars;

/// Hits at which a rally is given up as unended. The longest rally that
/// ended in the first measured run had 176 hits (two stars against three).
const RALLY_HIT_LIMIT: u32 = 500;

/// Chaos at the highest cap, where the most effects stack.
fn chaos() -> Mode {
    Mode::Chaos {
        cap: NonZeroU8::new(3).expect("three is non-zero"),
    }
}

/// A Chaos match to five points with nothing forced.
fn chaos_match(left: Difficulty, right: Difficulty) -> AiMatch {
    AiMatch {
        left,
        right,
        mode: chaos(),
        points_to_win: NonZeroU8::new(5).expect("five is non-zero"),
        forced: Forcing::default(),
        rally_hit_limit: RALLY_HIT_LIMIT,
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

/// M2 spec 2.5.1, first half: seeded AI-versus-AI Chaos matches show bounded
/// rally length. Every pairing but the three-star mirror, which
/// `between_three_star_computers_no_chaos_rally_ends` pins.
#[test]
fn chaos_rallies_end_within_the_hit_limit_in_every_pairing_but_the_three_star_mirror() {
    for (left, right) in [
        (ONE, ONE),
        (ONE, TWO),
        (ONE, THREE),
        (TWO, TWO),
        (TWO, THREE),
    ] {
        let setup = chaos_match(left, right);
        let played = play_seeds(&setup, 0..6);
        let lengths = lengths(&played);
        println!("chaos, {}: {lengths}", stars(&setup));
        assert_eq!(lengths.unended, 0, "{}: {lengths}", stars(&setup));
        assert!(
            played.iter().all(|one| one.winner.is_some()),
            "{}: a match went unfinished",
            stars(&setup)
        );
    }
}

/// M2 spec 2.5.1 is NOT met for this pairing, and this test says so until it
/// is: no rally between two three-star computers ends under Chaos as shipped
/// (Speed and Shrink). Their aim error of one unit is inside the reach of
/// even the shortest paddle, and the paddle crosses its whole range faster
/// than the ball returns at the speed cap.
///
/// When an effect's flaw makes three stars miss, this test fails: delete it
/// and add the pairing to the test above.
#[test]
fn between_three_star_computers_no_chaos_rally_ends() {
    let setup = chaos_match(THREE, THREE);
    let played = play_seeds(&setup, 0..3);
    let lengths = lengths(&played);
    println!("chaos, {}: {lengths}", stars(&setup));
    assert_eq!(
        (lengths.rallies, lengths.unended),
        (3, 3),
        "a three-star mirror rally ended: spec 2.5.1 may now hold for it: {lengths}"
    );
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
            },
            rally_hit_limit: 300,
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

/// M2 spec 2.5.5: seeded win rates rise from one star to two to three. Each
/// pairing is played from both sides, so no side's serve or seat decides it.
#[test]
fn in_chaos_the_computer_with_more_stars_wins_more_matches() {
    for (fewer, more) in [(ONE, TWO), (TWO, THREE), (ONE, THREE)] {
        let seeds = 0..4;
        let more_on_the_right = play_seeds(&chaos_match(fewer, more), seeds.clone());
        let more_on_the_left = play_seeds(&chaos_match(more, fewer), seeds);
        let won_by = |played: &[MatchPlayed], side: Side| {
            played.iter().filter(|one| one.winner == Some(side)).count()
        };
        let more_won =
            won_by(&more_on_the_right, Side::Right) + won_by(&more_on_the_left, Side::Left);
        let fewer_won =
            won_by(&more_on_the_right, Side::Left) + won_by(&more_on_the_left, Side::Right);
        println!(
            "chaos, {} against {} stars: {more_won} matches to {fewer_won}",
            more.stars(),
            fewer.stars()
        );
        assert!(
            more_won > fewer_won,
            "{} stars won {more_won}, {} stars won {fewer_won}",
            more.stars(),
            fewer.stars()
        );
    }
}

/// Return counts held per level of breadth: each step up adds one scheduled
/// effect and this many returns, so breadth and intensity rise together.
const RETURNS_PER_LEVEL: u16 = 15;

/// M2 spec 2.5.2, R3 for the stack: AI-versus-AI rally length shortens as
/// breadth and hit count rise together. Level `k` forces the mode's
/// always-on effects and the first `k` of its bag, at a held return count of
/// `k` times fifteen; the levels run to the cap or the end of the bag.
///
/// Mirror matches of one and of two stars. Three stars never miss
/// (`between_three_star_computers_no_chaos_rally_ends`), so their rallies
/// are unended at every level and cannot shorten.
#[test]
fn chaos_rallies_shorten_as_breadth_and_return_count_rise_together() {
    let tuning = Tuning::from_ron(EMBEDDED_RON).expect("the embedded tuning is valid");
    let mode = tuning.mode(chaos());
    let levels = mode.bag().len().min(3);
    assert!(levels >= 1, "Chaos schedules no effect");
    for stars_each in [ONE, TWO] {
        let means: Vec<f32> = (0..=levels)
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
                    },
                    rally_hit_limit: 200,
                    ..chaos_match(stars_each, stars_each)
                };
                let lengths = lengths(&play_seeds(&setup, 0..3));
                println!(
                    "stack, {}, {effects:?} at {returns} returns: {lengths}",
                    stars(&setup)
                );
                lengths.mean
            })
            .collect();
        for pair in means.windows(2) {
            assert!(
                pair[1] <= pair[0],
                "{} stars: rallies grew from one level to the next: {means:?}",
                stars_each.stars()
            );
        }
        assert!(
            means[levels] < means[0],
            "{} stars: the full stack is no shorter than none: {means:?}",
            stars_each.stars()
        );
    }
}
