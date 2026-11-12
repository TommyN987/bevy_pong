# M2 phase 4 report: AI under effects, and the harness

Started 2026-11-12. Branch `feat/ai-effects`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built so far: step 0, step 1 (plan 4.1), and step 2 (plan 4.2). Steps 3 and 4 (plan 4.3, 4.4) are not started.

**Open, for the user's ruling:** spec 2.5.1 is not met between two three-star computers (step 2, "The finding").

## Deviations

All were proposed before implementation and approved by the user on 2026-11-12. The first two are built, in step 1; the third waits for step 3.

| Item | Deviation | Recorded |
|---|---|---|
| plan 4.1 | The per-star flaw section of an effect's tuning is not built. Shrink's flaw is inherent (spec 2.6.1), so no effect has numbers to read; Wind is the section's first user, in plan phase 5. Phase 4 proves the flaw's two forms with probe effects. | This report. Plan amendment at the next record step. |
| spec 2.3.12 | For Shrink, "a raised flaw misses some" is read as a raised dial: a fixed handicap returns every ball at dial zero and misses some at full dial. | This report. Spec amendment at the next record step. |
| spec 2.5.3, plan 4.3 | The Chaos soak is one test per subset size, with a step budget per subset chosen from the measured running time, not M1's hundred thousand steps per subset. The tests for pairs and triples hold no subset until Wind and Spin ship. | This report. Spec amendment at the next record step. |

## Decisions taken in this phase

- **The serve and the AI.** The AI counts an approach only once the ball's flight has begun. The serve and the AI's steering both run in the intent phase, unordered, and the serve's flight starts in the effects phase after them: an AI that ran second planned under the cleared flight. Approved 2026-11-12; built in step 1, where its reason turned out narrower than proposed (below).
- **`just ai-report`.** A recipe that prints the harness's tables, since every effect phase reports its R3 measure through it (plan E.4). Approved 2026-11-12; built in step 2: it runs `tests/ai_matches.rs` with the output shown.

## Step 0

### Record step for phase 3 (P.6)

The seven deviations of the [phase 3 report](m2-phase3-report.md) are recorded:
spec 2.6.1, 2.7.1, 2.7.2, 2.8.4, and 2.9.1 (two deviations, one amendment) carry an amendment dated 2026-11-12;
ADR 0010 was amended when its deviation was taken.
The report's "Recorded" column says so per row.

### The double-counted return

Ruled by the user on 2026-11-12: fixed now, before the harness records the baselines every later phase compares against.

- **Fault.** A return whose step ended exactly on a paddle's face was found again by the next step's sweep, which starts on the grown rectangle's edge and moves away: `slab` gives an exit of `-0.0`, and `t_exit < 0.0` did not refuse it. The second contact was a second return: a second speed step, a second `BallHitPaddle`, a second cue.
- **Contract.** `swept_circle_rect`: a sweep that leaves the grown rectangle at or before its start is no impact. A start that overlaps and stays inside for any part of the sweep is still an impact at time zero, so a paddle that moved onto the ball still deflects it.
- **Change.** `t_exit <= 0.0` in `kernel/collide.rs`. Physics is untouched: the kernel owned the fault.
- **Tests, red first.** `ball_touching_the_face_and_moving_away_misses` (kernel) and `a_return_whose_step_ends_on_the_face_is_one_return` (physics, the phase 3 report's case: 60 units/s from one unit off the face) both failed before the change; the physics test read two `BallHitPaddle`. `ball_touching_the_face_and_moving_in_hits_at_once` (kernel) passed before and after: it pins the other side of the boundary.
- **Not covered.** A start that rounding leaves a hair inside the grown rectangle, moving away, is still an impact at time zero. By arithmetic, not by test: at the paddles' coordinates the contact point rounds onto the face exactly, so the step after a return starts on the edge or outside it. No such case was found or built.

## Step 1: the flaw contract and the AI (plan 4.1)

Built 2026-11-12, tests first.

### What was built

- **`Effect::flaw(start, difficulty) -> Flaw`.** The trait's third method, with no default body. `Flaw` has no `Default`: `added_error`, `added_delay`, and `believed`, a `Contribution`.
- **Speed and Shrink** add nothing and believe what they contribute: their flaws are inherent (spec 2.6.0, 2.6.1).
- **`FlightFlaw` and `ActiveEffects::flaw_over(flight, difficulty)`.** The flaws composed: errors and delays by sum, beliefs into a flight.
- **`Flight::began_at` and `Flight::believed_as`** (kernel). A flight keeps the `FlightStart` it began at, so an effect is asked for its flaw with the start it was asked for its contribution with, a held count included. The believed flight keeps the true flight's start and clock.
- **The AI** takes a `Sighting`: the ball's motion, the flight it is in if any, and the effects in play. On the step an approach begins it asks for the flaw, predicts under the believed flight, widens the star's spread by the added error, and lengthens the star's delay by the added delay. One draw per approach, as before.
- **`BallFlight::in_flight` and `clear`.** The component knows whether a serve or return began its flight; a begun flight with no effect in play is as empty as the cleared one.

### Decisions taken

- **The flaw is asked for when an AI plans, not at the flight start.** The effects feature does not know which difficulties play, and a match of two humans should not compose three beliefs per flight. The cost is a contract on the effect: the same start and difficulty give the same flaw for as long as the flight lasts.
- **The belief is a whole `Contribution`,** not a field alone. A channel added later (obstacles, plan 9.2) is then part of every belief by the same compiler walk that makes it part of every contribution.
- **`Sighting`** groups what the controller sees, and the system's `SeenBall` groups the ball query with the effects: both functions were at clippy's seven parameters. No lint was silenced.
- **Classic runs the same path.** Nothing branches on the mode: with Speed alone the composed flaw adds nothing and believes a flight without fields, so the star's handicap is the whole flaw, as in M1.
- **The star's handicap stays in the `ai` tuning section,** shared between modes (spec, "Decisions taken"). A `Flaw` adds to it.
- **`probe.rs`,** a test-only module of the effects feature, holds `Misjudged`, the probe effect the base's tests and the AI's tests share. It is declared at the end of `effects/mod.rs`, since `arch-check` reads production code up to the first `#[cfg(test)]`.

### The serve fix: built, with a narrower reason than proposed

The proposal said that from Wind on, an AI that ran after the serve would aim a served ball without the wind.
That held for the AI as it was, which predicted under the ball's flight.
It does not hold for the AI as built: the belief is composed from the effects in play, not read from the ball's flight, so an AI planning under the cleared flight still believes in every field.

What the fix still buys:

- **`flaw_over`'s precondition holds.** The flight handed to it was started by the effects in play. The cleared flight was not: its start is a serve at count zero whatever the sandbox holds.
- **A held count reaches the serve's plan.** Under a sandbox-held count the cleared flight would have asked each effect for its flaw at count zero. The harness (step 2) measures at held counts.
- **One behaviour for both system orders.** The AI plans a served ball on the step after the serve, always. Before, it planned on the serve's step or the next, by the shuffle seed.

The cost: in the order where the AI ran after the serve, Classic's AI reacts to a serve one step (1/60 s) later than it did. No seeded test's assertion moved.

### Done-conditions

| Condition | Evidence |
|---|---|
| The flaw's two forms in the AI (plan 4.1) | `an_effects_added_error_widens_the_spread_of_the_aim`, `an_effects_added_delay_holds_the_paddle_longer`, `the_ai_aims_where_the_flight_it_believes_in_takes_the_ball` (AI); four tests of `flaw_over` (effects base); `a_believed_flight_keeps_the_start_and_the_clock_and_pushes_as_believed` (kernel). |
| Spec 2.3.12 for Shrink: the zero-flaw AI returns every ball | `at_shrinks_full_dial_an_ai_with_no_delay_and_no_error_returns_every_ball`: 200 seeded launches, paddles at the minimum height. |
| Spec 2.3.12 for Shrink: a raised flaw misses some (read as a raised dial, deviation above) | `an_error_whole_paddles_forgive_misses_some_balls_at_shrinks_full_dial`: six units of error return all 40 launches with whole paddles and miss some at full dial. |
| An effect missing its flaw does not compile (spec 2.3.2) | The trait has no default body; adding the method failed the build at Speed, Shrink, and the two test probes until each stated one. |
| "Read from each effect's tuning" (plan 4.1) | Not built: deviation above. Probes carry the numbers. |

### Red first, and where it was not

- Red by compile failure, then green: the kernel's two tests, the base's four, Speed's and Shrink's, `the_ball_is_in_no_flight_until_the_serve_begins_one_and_in_none_after_the_rally`, and the AI's unit tests.
- Red by assertion: `whatever_the_system_order_the_ai_plans_a_served_ball_one_step_on_under_its_flight` failed at seed 1 ("planned on the serve's step") with the system still passing the ball's flight unconditionally, and passed once it passed `in_flight()`.
- **Not red first:** Shrink's two done-condition tests passed on their first run. They pin behaviour Shrink already had, since its flaw needs no code in the AI.
- **A vacuous first draft, caught.** The system-order test first passed under the old wiring. Replacing `Tuning` makes the base rebuild the effects in play from their ids, which dropped the probe added before the first step; and the belief is composed from the effects, so no order showed a wrong aim. The test now adds its probe a few steps into the countdown, asserts the flight pushes, and asserts the step the plan is made on.

### Bevy API

No API new to the repository, so `bevy-api-check` was not run.
`#[derive(SystemParam)]` over a `Query<'w, 's, ..>` and a `Res<'w, ..>` is as in the shell's overlay and in escalation.

## Step 2: the AI-versus-AI harness (plan 4.2)

Built 2026-11-12. The harness and its own tests were written tests first; the spec tests measure behaviour that already existed.

### What was built

- **`testing::ai_match`.** `AiMatch { left, right, mode, points_to_win, forced, rally_hit_limit }` and `play(seed)`, which plays one match on an app of its own and returns a `MatchPlayed`: the winner and every `RallyPlayed` (paddle hits, who won it). `RallyLengths` summarises rallies: count, mean, median, longest, unended.
- **`Forcing`.** What the sandbox forces for a whole match: the set of effects, a held return count, both, or neither.
- **`Sandbox::force_effects(effects)`.** The whole forced set at once. `toggle` is now built on it. The harness needed it: toggling one effect at a time cannot name the empty set without knowing what is in play.
- **`pong_core/tests/ai_matches.rs`.** Five tests on spec 2.5.1, 2.5.2, and 2.5.5, each printing its numbers.
- **`just ai-report`.**

### The finding: three stars never miss, and two stars miss only a shrunk paddle

Spec 2.5.1 asks that seeded AI-versus-AI Chaos matches show bounded rally length. **It does not hold between two three-star computers.** The proposal estimated their rallies at about sixty returns; that estimate was wrong.

Measured 2026-11-12, embedded tuning, matches to five points, hits per rally:

| Pairing | Chaos, cap 3 (6 seeds) | Classic (6 seeds, limit 300) |
|---|---|---|
| 1 v 1 | 40 rallies, mean 2.3, longest 8 | mean 2.4, longest 11 |
| 1 v 2 | 30 rallies, mean 4.7, longest 13 | mean 5.5, longest 22 |
| 1 v 3 | 30 rallies, mean 4.1, longest 16 | mean 4.8, longest 16 |
| 2 v 2 | 40 rallies, mean 44.3, longest 100 | no rally ended |
| 2 v 3 | 30 rallies, mean 58.7, longest 176 | no rally ended |
| 3 v 3 | no rally ended (limit 2,000 on a first run, 500 in the test) | no rally ended |

Mirror matches at a held return count, forced sets, limit 200, 3 seeds (mean hits; "none" means no rally ended):

| Held count | 1 v 1 Speed | 1 v 1 Speed+Shrink | 2 v 2 Speed | 2 v 2 Speed+Shrink | 3 v 3, either |
|---|---|---|---|---|---|
| 0 | 2.8 | 2.8 | none | none | none |
| 6 | 2.6 | 1.8 | none | none | none |
| 12 | 2.3 | 0.8 | none | none | none |
| 15 | 2.6 | 0.8 | none | 24.0 | none |
| 25 | 2.6 | 0.8 | none | 21.7 | none |
| 57 (Chaos's speed cap) | 1.6 | 0.8 | none | 24.5 | none |

One more measurement, by a temporary test since removed: of the AI tests' 200 seeded launches (any height, any angle the paddle can give, any speed from the serve to the cap, the AI resting at the centre), three stars as tuned missed none, and three stars with two stars' handicap missed none.

What the numbers say:

- **Speed alone ends no rally against two or three stars,** in Classic or in Chaos, at any count up to the cap. A likely cause, by arithmetic from the tuning and not isolated by a test: the paddle crosses its whole range of 76 units in 0.84 s, and at the cap the ball needs 1.17 s for the 292 units from one paddle's hit to the other's and back. The fact the spec relied on, that a competent defender returns every ball without escalation, is confirmed; the companion assumption, that the speed ramp is what ends rallies, is false for these AIs.
- **The star's aim error is the only source of misses.** One star's 12 units exceed the whole paddle's reach of 8. Two stars' 5 units are inside it, and outside the shrunk paddle's reach of 5 only at Shrink's full dial, which is why two-star rallies end at a held count of 15 and not at 12. Three stars' 1 unit is inside every reach.
- **In Classic, as M1 shipped it, two and three stars are not beaten by speed or placement** from a centred start. This predates M2. Whether a human beats them some other way is not measured: the harness has no model of a human.
- **R3 per effect, in this instrument:** Shrink shortens rallies for one star (graded) and for two stars (a step at full dial); Speed shortens them for one star only, slightly. Step 4 reports Shrink's measure in full.

What is and is not asserted:

| Spec item | Test | State |
|---|---|---|
| 2.5.1, bounded under Chaos | `chaos_rallies_end_within_the_hit_limit_in_every_pairing_but_the_three_star_mirror` | Met for five of the six pairings. |
| 2.5.1, the three-star mirror | `between_three_star_computers_no_chaos_rally_ends` | **Not met.** The test pins the falsification: it fails the day such a rally ends, and says to move the pairing into the test above. |
| 2.5.1, the control | `with_no_effect_in_play_no_rally_between_competent_computers_ends` | Met: two-star and three-star mirrors, no effect forced, 300 hits with no point. |
| 2.5.5, star order | `in_chaos_the_computer_with_more_stars_wins_more_matches` | Met: 8 matches to 0 in each of the three pairings, played from both sides. |
| 2.5.2, R3 for the stack | `chaos_rallies_shorten_as_breadth_and_return_count_rise_together` | Met for one-star and two-star mirrors. Three stars cannot shorten: their rallies are unended at every level. |

Options for the ruling, none taken:

1. **Leave the tuning; let the later effects' flaws carry it.** Wind, Fog, Spin, Lens, and Blocks each bring a per-star flaw (spec 2.3.12), and three stars' numbers there decide whether its rallies end. Spec 2.5.1 is amended to hold for the three-star mirror by milestone close, and the pinned test fails when it starts to. Recommended: it changes nothing the kids have played, and the flaw numbers are tuning the spec already leaves to play.
2. **Retune now:** a larger aim error for three stars, or a lower minimum paddle height. Either changes how Classic or Shrink feels without a session behind it.
3. **Change the AI's model:** a slower return to the centre, or a paddle that cannot cross its range inside one crossing. A design change to M1's controller.

### Decisions taken

- **One app per match, seeded.** A match is reproduced from its seed alone, with no state from the match before it.
- **Rally length is paddle hits,** as the longest-rally record counts them, not the flight's return count: a held count stands still.
- **A hit limit, not a step limit,** gives up a rally: it does not depend on the frame length, and a ball that is never hit faults in sanity (spec 2.5.6).
- **Four-step frames.** The simulation steps the same at any frame length; longer frames spend fewer on the countdowns. No timing against one-step frames was taken.
- **Levels of the stack come from the mode's data:** the always-on effects plus the first `k` of the bag, at `k` times fifteen returns. A new effect in the bag lengthens the test's ladder with no edit.
- **`RallyPlayed`, not `Rally`:** rules already has a `Rally`.

### Not done

- **No deliberate breaks were run** against the five spec tests. Their discriminating power rests on the tables above: the same pairing ends under Chaos and does not end with no effect forced.
- **The spec tests were not red first.** They measure; the pinned test is the one that states a failure.
- Running times, development machine: the five tests take 1.0 to 5.2 s each, 14.4 s in all, against nextest's 30 s per test. CI's times are not measured yet (plan 4.3 measures the soak's).

## Test changes

386 core tests and 165 shell tests pass, 551 in all; 518 existed before this phase.

Step 0: no assertion changed and no call site changed.
One comment changed: the AI tests' `launches` said a touching start counts as struck; it now says an overlapping start does.

Step 1: no assertion changed. Call sites changed for a changed signature:

- `MirrorController::steer` takes a `Sighting` where it took the ball's motion and a flight: eleven calls in the AI's tests pass `flying(ball)`, a sighting in an empty flight with no effect in play, where they passed the ball and `&Flight::default()`.
- `Effect` gained `flaw`: the effects tests' two probes, `Probe` and `Swinger`, state one.

Step 2: no assertion changed and no call site changed.

## Facts learned

- The spec's open fact on sweeps that start inside the rectangle is answered in part and amended in the spec: a start on the edge moving away is no impact. The interior start stays with plan 9.1.
- Both orders of the serve and the AI's steering occur across shuffle seeds 0 to 11: in eight of the twelve the AI ran after the serve.
- Replacing the `Tuning` resource rebuilds the effects in play from their ids on the next step: a test probe added before that step is replaced by the effect whose name it borrows.
- A belief composed from the effects in play does not depend on the ball's flight for anything but its start and clock.
- The spec's fact "without escalation a competent defender returns every ball" is confirmed by the harness; its label in the spec is still "probable", owed at the next record step.
- With escalation too, a two-star or three-star defender returns every ball that Speed alone throws at it (step 2, "The finding").
