# M2 phase 4 report: AI under effects, and the harness

Started 2026-11-12. Branch `feat/ai-effects`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-12: step 0, and steps 1 to 4 (plan 4.1 to 4.4). Not closed: see "Open at phase close".

**Ruled 2026-11-12:** the finding of step 2, that no rally between three-star computers ends, was answered by a retune of the AI's reaction delays (step 2, "The retune"). Spec 2.5.1 is met for every pairing. The numbers are a first candidate: the user's hand check is owed.

## Deviations

All were proposed before implementation and approved by the user on 2026-11-12, and all are built.

| Item | Deviation | Recorded |
|---|---|---|
| plan 4.1 | The per-star flaw section of an effect's tuning is not built. Shrink's flaw is inherent (spec 2.6.1), so no effect has numbers to read; Wind is the section's first user, in plan phase 5. Phase 4 proves the flaw's two forms with probe effects. | This report. Plan amendment at the next record step. |
| spec 2.3.12 | For Shrink, "a raised flaw misses some" is read as a raised dial: a fixed handicap returns every ball at dial zero and misses some at full dial. | This report. Spec amendment at the next record step. |
| spec 2.5.3, plan 4.3 | The Chaos soak is one test per subset size, not one soak per subset. Each test divides M1's hundred thousand live steps evenly among its subsets (built so in step 3, in place of the budget "chosen from the measured running time" first proposed: an even split needs no number to maintain). The tests for pairs and triples hold no subset until Wind and Spin ship. | This report. Spec amendment at the next record step. |
| spec 2.5.1, 2.5.5 | Both are held for Classic as well as Chaos, by the user's ruling with the retune: every rally ends in every pairing, and more stars win, in both modes. | This report; `tests/ai_matches.rs`. Spec amendment at the next record step. |
| `tuning.ron`, `ai` | The stars' handicaps changed by the user's ruling (step 2, "The retune"): delays 0.35, 0.29, 0.24 s and errors 12, 7, 3, from 0.35, 0.18, 0.05 s and 12, 5, 1. | This report. Playtest-log entry owed. |

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

What was and was not asserted before the ruling (the first two tests have since been replaced, see "The retune"):

| Spec item | Test | State before the retune |
|---|---|---|
| 2.5.1, bounded under Chaos | `chaos_rallies_end_within_the_hit_limit_in_every_pairing_but_the_three_star_mirror` | Met for five of the six pairings. |
| 2.5.1, the three-star mirror | `between_three_star_computers_no_chaos_rally_ends` | Not met. The test pinned the falsification. |
| 2.5.1, the control | `with_no_effect_in_play_no_rally_between_competent_computers_ends` | Met: two-star and three-star mirrors, no effect forced, 300 hits with no point. |
| 2.5.5, star order | `in_chaos_the_computer_with_more_stars_wins_more_matches` | Met: 8 matches to 0 in each of the three pairings, played from both sides. |
| 2.5.2, R3 for the stack | `chaos_rallies_shorten_as_breadth_and_return_count_rise_together` | Met for one-star and two-star mirrors. Three stars cannot shorten: their rallies are unended at every level. |

Options put to the user; the second was taken:

1. **Leave the tuning; let the later effects' flaws carry it.** Wind, Fog, Spin, Lens, and Blocks each bring a per-star flaw (spec 2.3.12), and three stars' numbers there decide whether its rallies end. Spec 2.5.1 is amended to hold for the three-star mirror by milestone close, and the pinned test fails when it starts to. Recommended: it changes nothing the kids have played, and the flaw numbers are tuning the spec already leaves to play.
2. **Retune now:** a larger aim error for three stars, or a lower minimum paddle height. Either changes how Classic or Shrink feels without a session behind it.
3. **Change the AI's model:** a slower return to the centre, or a paddle that cannot cross its range inside one crossing. A design change to M1's controller.

### The retune

Ruled by the user on 2026-11-12: retune now. Three stars are to be harder to beat than two, and two than one, and no star unbeatable, in Chaos or in Classic. The user's own play behind it: one point scored against three stars, in a long rally, over all attempts.

**The knob is the reaction delay.** A sweep of delay (0.05 to 0.35 s) against aim error (1, 3, 5), by a temporary test since removed, three-star mirror matches and 50 launches per speed:

| Delay | Classic mirror, mean hits (error 1 / 3 / 5) | Chaos mirror, mean hits (error 1 / 3 / 5) | Random launches missed at the cap |
|---|---|---|---|
| 0.05 | not run / none ended / none ended | not run / none ended / 45.0 | 0% |
| 0.12 | not run / none ended / none ended | not run / none ended / 45.0 | 0% |
| 0.18 | not run / none ended / none ended | not run / 2 of 3 unended / 44.7 | 0% |
| 0.22 | 52.2 / 53.2 / 71.8 | 69.9 / 73.6 / 37.9 | 0% |
| 0.25 | 47.4 / 43.2 / 51.4 | 59.9 / 64.9 / 38.5 | 2% |
| 0.28 | 33.6 / 29.5 / 33.3 | 53.3 / 61.6 / 40.0 | 2% |
| 0.31 | 29.6 / 26.0 / 29.7 | 50.7 / 52.3 / 40.5 | 4% |
| 0.35 | 26.5 / 24.9 / 24.8 | 45.6 / 50.8 / 36.5 | 6% |

The error-1 rows for delays up to 0.18 did not run (the sweep's first case failed on its own fixture); the embedded 0.05 and error 1 is the "no rally ended" of the finding above.
At speeds of 100, 150, and 200 no launch was missed at any delay.

- **Under about 0.2 s nothing ends in Classic; above it everything does.** That matches the arithmetic: at the cap a level ball takes 0.584 s to cross, and a paddle at the centre needs up to 0.4 s to reach the far end of its range, so a delay under 0.184 s always arrives.
- **The error does almost nothing in Classic** below the paddle's reach of 8. In Chaos an error of 5 ends rallies through Shrink, whatever the delay.
- **A delay-made miss is earned.** It needs speed and placement: a ball far from the centre the AI waits at. An error past the paddle's reach would instead roll dice at any speed, and would break the fact the spec relies on, that without escalation a competent defender returns every ball.

**The change,** in `tuning.ron`, delays only:

| Stars | Before | After |
|---|---|---|
| One | 0.35 s, error 12 | unchanged |
| Two | 0.18 s, error 5 | **0.29 s**, error 5 |
| Three | 0.05 s, error 1 | **0.24 s**, error 1 |

What that means for a player, by arithmetic from the tuning (a level ball, the AI at the centre):
three stars are beaten at the cap by a ball crossing more than about 39 units from the centre (the ball's centre turns at 44), and from a speed of about 228, which Classic reaches at the 22nd return;
two stars by a ball more than about 34.5 out at the cap, and from about 212, the 19th return.
In Chaos the ramp is 3 a return, so those speeds come at the 50th and 44th return: there Shrink, and the effects to come, do the work.

**Measured after the change** (`just ai-report`, 4 seeds a pairing, matches to five points, hits per rally):

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 1 v 1 | mean 2.4, longest 11 | mean 2.3, longest 6 |
| 1 v 2 | mean 5.3, longest 14 | mean 4.2, longest 12 |
| 1 v 3 | mean 5.2, longest 16 | mean 4.4, longest 16 |
| 2 v 2 | mean 29.0, longest 52 | mean 38.0, longest 68 |
| 2 v 3 | mean 31.9, longest 57 | mean 45.1, longest 72 |
| 3 v 3 | mean 49.1, longest 137 | mean 60.6, longest 73 |

No rally was unended. More stars won 8 matches to 0 in each of the three pairings, in each mode. With no effect forced, two-star and three-star mirrors still score no point in 300 hits.

**Tests, red first.** Written against the old numbers, where four failed, then green with the new:

| Test | Asks |
|---|---|
| `chaos_rallies_end_within_the_hit_limit_in_every_pairing` | Spec 2.5.1 for all six pairings. Replaces the test that left the three-star mirror out. |
| `classic_rallies_end_within_the_hit_limit_in_every_pairing` | The same of Classic: no star unbeatable there. New. |
| `in_both_modes_the_computer_with_more_stars_wins_more_matches` | Spec 2.5.5, and the same of Classic. Replaces the Chaos-only test. |
| `at_the_speed_cap_a_level_ball_along_the_wall_beats_three_stars` | The earned miss: one unit inside either wall at 250. New. |
| `three_stars_return_the_same_ball_sent_slower_or_nearer_the_centre` | Passed before and after: at 80 and at 150 along the wall, and at 250 from 20 out. New. |

`between_three_star_computers_no_chaos_rally_ends` is deleted: the gap it pinned is closed.
The stack's R3 test still plays one and two stars only: three stars miss only near the speed cap, beyond the counts its levels hold.

**Second try, the same evening.** The user played three stars at the delays above and lost 10 to 0. Ruled: nerf the errors to 12, 7, and 3. In `tuning.ron` now:

| Stars | Delay | Error |
|---|---|---|
| One | 0.35 s | 12 |
| Two | 0.29 s | **7** (was 5) |
| Three | 0.24 s | **3** (was 1) |

Every test of the retune passes unchanged at these numbers. Measured (`just ai-report`, hits per rally):

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 2 v 2 | mean 33.5, longest 65 | mean 14.7, longest 26 |
| 2 v 3 | mean 30.1, longest 46 | mean 19.2, longest 30 |
| 3 v 3 | mean 41.7, longest 143 | mean 66.0, longest 105 |

More stars still won 8 matches to 0 in every pairing and mode; the control still holds.
Against the first try, Classic and the three-star mirror moved little; two stars' Chaos rallies shortened to under half, since an error of 7 is outside the shrinking paddle's reach long before full dial.

**What the errors do and do not do,** by a temporary test since removed: 100 launches per speed at the three-star AI resting at the centre, any height, any angle, percent missed.

| Delay, error | at 100 | at 150 | at 200 | at 250 (cap) |
|---|---|---|---|---|
| 0.24, 3 (three stars now) | 0 | 0 | 0 | 2 |
| 0.29, 7 (two stars now) | 0 | 0 | 0 | 5 |
| 0.24, 8 | 1 | 1 | 1 | 3 |
| 0.24, 9 | 4 | 3 | 2 | 4 |
| 0.29, 9 | 4 | 3 | 2 | 7 |
| 0.29, 9.5 | 6 | 5 | 13 | 12 |
| 0.29, 10 | 14 | 13 | 12 | 15 |
| 0.35, 12 (one star; 20 launches per speed) | 30 | 30 | 30 | 30 |

- **An error inside the paddle's reach causes no miss.** The misses begin near 8.6, a little past the 8 of half the paddle plus the ball's radius, and grow as about `1 - 8.6 / error`.
- **So 7 and 3 leave two and three stars missing nothing below the speed cap** against a whole paddle. A player who cannot yet carry a rally to the cap is given no points by this change.
- **A longer delay does not reach lower speeds either:** at 0.6 s and error 3, nothing was missed at 100 or 150 (20 launches per speed).
- **In this AI model, only an error past the reach makes a star miss at ordinary speeds,** and it does so by chance, at any speed, not by the shot's merit. It would also end the control of spec 2.5.1: a rally with no effect in play would then end.

**Owed.**

- The user's hand check of both delays in `just run`. They are a first candidate from the harness, which has no model of a human; `tuning.ron` reloads while the game runs.
- A playtest-log entry for the observation behind the ruling and for the hand check, by the spec's rule that numbers are found by play.

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

## Step 3: the soak over subsets (plan 4.3)

Built 2026-11-12.

- **One runner.** `tests/soak.rs` has a `Soak` (app seed, player seed, mode, a forced set at full dial or none, live steps) whose `run` is M1's soak loop. The Classic test calls it with M1's seeds and steps.
- **The refactor changed nothing in Classic.** Before and after, the seeded run printed the same line: 67,714 frames, 16 matches, 1,025 paddle hits, top speed 250.00002. Its three assertions are untouched.
- **The players predict under the ball's own flight,** not an empty one, so a field effect is tracked from the phase it ships in. Without fields the two are the same.
- **Chaos, one test per subset size 1, 2, 3.** Every choice of that many effects from Chaos's bag is forced beside the mode's always-on effects, with the return count held at `u16::MAX`: full for every dial whatever return its ramp is full at. Per subset the test asserts the forced effects were in play, a return reached the speed cap, and a match ended.
- **The subsets come from the mode's tuning,** so an effect added to the bag is soaked with no edit to the test (spec 2.3.8's touch list).
- **`subsets_of`** was written test first (red by compile failure).

Spec 2.5.3 and 2.5.4, measured: Speed and Shrink at full dial, 100,000 live steps in 128,802 frames, 85 matches, 1,387 paddle hits, no fault. The pair and triple tests found no subset: the bag ships one effect.

**Running time, development machine:** Classic 3.8 s, the Chaos test 6.8 s alone (5.7 s and 8.8 s inside the full parallel run), against nextest's 30 s per test.

**The spec's open fact,** whether a soak per subset of six effects fits CI, is answered by construction: a test's time does not grow with the roster, since its steps are divided. With six effects in the bag a subset gets 16,666 steps as one of 6 singles, 6,666 as one of 15 pairs, and 5,000 as one of 20 triples; with seven, 14,285, 4,761, and 2,857. At full dial the measured run played 85 matches of at least five points each in its 100,000 steps, so a rally took at most about 235 live steps, and the smallest share still plays a dozen rallies or more. No spec amendment is needed for fit. **CI's own times are not measured:** they are read from the first push's run.

Not done: no deliberate break was run against the Chaos soak. What it would catch is what sanity catches; that the forcing took hold is asserted per subset.

**Noticed, not changed:** the Classic soak's floor of 1,000 paddle hits has little room left. The run makes 1,025; it made 1,118 when the floor was set. The drop came before this step (the double-return fix of step 0 is the only change to a soak of human seats); the floor is the user's to move.

## Step 4: Shrink's R3 measure (plan 4.4, E.4)

Built 2026-11-12, at the retuned handicaps (delays 0.35, 0.29, 0.24 s; errors 12, 7, 3).

`each_bag_effect_alone_shortens_rallies_as_its_dial_rises` forces each effect of Chaos's bag alone, with no speed ramp beside it, up a ladder of held return counts, and asserts for one-star and two-star mirrors that mean rally length never rises from one rung to the next and is lower at the top than at the bottom. A new effect in the bag is held to it with no edit.

Shrink alone, mean hits per rally, mirror matches to five points, 3 seeds, hit limit 200 (Shrink's dial starts to move after return 3 and is full at 15):

| Held count | Paddle height | 1 v 1 | 2 v 2 | 3 v 3 |
|---|---|---|---|---|
| 0 | 14 | 2.8 | none ended | none ended |
| 5 | 13 | 2.5 | 195.2 (3 of 4 unended) | none ended |
| 10 | 10.5 | 1.3 | 7.4 | none ended |
| 15 | 8 | 0.8 | 4.0 | none ended |
| 20 | 8 | 0.8 | 4.0 | none ended |
| 30 | 8 | 0.8 | 4.0 | none ended |

- **R3 holds for Shrink against one and two stars:** rally length falls as the dial rises and never rises.
- **R3 does not show against three stars.** Their aim error of 3 is inside the reach of the shortest paddle, and the ball here stays at the serve speed, so no rally ends at any rung. The three-star row was measured by a temporary variant of the test; the test itself plays one and two stars.
- **The soak's subsets include Shrink** (step 3), which completes E.4.

The stack's measure (spec 2.5.2), from `just ai-report` at the same handicaps: one-star mirror, Speed at 0 returns 2.8 hits, Speed and Shrink at 15 returns 0.8; two-star mirror, none ended at the first level, 3.5 hits at the second.

## Spec items of this phase

| Item | State |
|---|---|
| 2.3.12, the flaw's two forms | Done for the contract and the AI (step 1). For Shrink read as a raised dial (deviation). The per-star tuning section waits for Wind (deviation). |
| 2.5.1, rallies end under Chaos; the control | Done for all six pairings, after the retune; held for Classic too. |
| 2.5.2, R3 for the stack and per effect | Done for one and two stars. Not shown for three stars: with Speed and Shrink alone they miss only near the speed cap. |
| 2.5.3, soak per mode | Done for the shipped roster; CI's times unmeasured. |
| 2.5.4, sanity under Chaos | Done: the soak and every harness match run under the panicking policy with no fault. |
| 2.5.5, stars order | Done, in both modes: 8 matches to 0 in every pairing. |

## Open at phase close

- **The user's hand check of the retuned stars.** The first candidate lost the user a match 10 to 0 against three stars; the second (errors 12, 7, 3) is unplayed. By the launch table of step 2 it gives a player no points below the speed cap.
- **A playtest-log entry** for the two observations behind the retune and for the hand check.
- **R3 against three stars** (spec 2.5.2): to be shown once an effect's flaw reaches them, from Wind on.
- **CI's running times** for the soak and the AI matches: read from the first push.
- **The Classic soak's paddle-hit floor:** 1,025 against 1,000.
- **The next record step** owes the spec and plan the five deviations above, the confirmed fact ("without escalation a competent defender returns every ball"), and the answered fact (the soak's fit).
- **`docs/specs/CURRENT`** still names phase 4: the bump is the user's word.

## Test changes

393 core tests and 165 shell tests pass, 558 in all; 518 existed before this phase.

Step 0: no assertion changed and no call site changed.
One comment changed: the AI tests' `launches` said a touching start counts as struck; it now says an overlapping start does.

Step 1: no assertion changed. Call sites changed for a changed signature:

- `MirrorController::steer` takes a `Sighting` where it took the ball's motion and a flight: eleven calls in the AI's tests pass `flying(ball)`, a sighting in an empty flight with no effect in play, where they passed the ball and `&Flight::default()`.
- `Effect` gained `flaw`: the effects tests' two probes, `Probe` and `Swinger`, state one.

Step 2: no assertion changed and no call site changed.

The retune: the AI tests' `three_stars_with` finds the embedded three-star entry by its text, which now reads 0.24 and 3.0; no assertion of an earlier test changed. Two tests of step 2 were replaced and one deleted, as listed under "The retune".

Step 3: the soak's `Player::intent` takes the ball's velocity along x and its crossing where it took the tuning and the ball's motion and predicted for itself; the Classic test's three assertions are unchanged, and its seeded run is the same to the last number.

Step 4: no assertion changed and no call site changed.

## Facts learned

- The spec's open fact on sweeps that start inside the rectangle is answered in part and amended in the spec: a start on the edge moving away is no impact. The interior start stays with plan 9.1.
- Both orders of the serve and the AI's steering occur across shuffle seeds 0 to 11: in eight of the twelve the AI ran after the serve.
- Replacing the `Tuning` resource rebuilds the effects in play from their ids on the next step: a test probe added before that step is replaced by the effect whose name it borrows.
- A belief composed from the effects in play does not depend on the ball's flight for anything but its start and clock.
- The spec's fact "without escalation a competent defender returns every ball" is confirmed by the harness; its label in the spec is still "probable", owed at the next record step.
- With escalation too, a two-star or three-star defender returns every ball that Speed alone throws at it (step 2, "The finding").
