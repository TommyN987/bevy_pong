# M2 phase 4 report: AI under effects, and the harness

Started 2026-11-12. Branch `feat/ai-effects`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-12: step 0, and steps 1 to 4 (plan 4.1 to 4.4).
Reviewed 2026-11-13, with phase 3, by four independent reviewers; every finding was worked through the same day ("The review").
Not closed: see "Open at phase close". `docs/specs/CURRENT` names phase 4.

This report was rewritten once, on 2026-11-13, after the review found it disagreeing with itself:
it had been written step by step through a finding and two retunes.
Measurements are labelled with the handicaps they were taken at.
Unless a table says otherwise, numbers are at the handicaps `tuning.ron` ships now: delays 0.35, 0.29, 0.24 s and errors 12, 7, 3 for one, two, and three stars.

## Deviations

The first three were proposed before implementation and approved by the user on 2026-11-12. The rest are the user's rulings during the phase, or findings recorded as they stand.

| Item | Deviation | Recorded |
|---|---|---|
| plan 4.1 | The per-star flaw section of an effect's tuning is not built. Shrink's flaw is inherent (spec 2.6.1), so no effect has numbers to read; Wind is the section's first user. Phase 4 proves the flaw's two forms with probe effects. | Plan 4.1 and 5.2, spec 2.3.12 amended 2026-11-13. |
| spec 2.3.12 | For Shrink, "a raised flaw misses some" is read as a raised dial: a fixed handicap returns every ball with whole paddles and misses some at full dial. | Spec amended 2026-11-13. |
| spec 2.5.3, plan 4.3 | The Chaos soak is one test per subset size, not one soak per subset. Each test divides M1's hundred thousand live steps evenly among its subsets. An even split replaced the budget "chosen from the measured running time" first proposed: it leaves no number to maintain. A soak of Chaos as scheduled was added after the review. | Spec 2.5.3, plan 4.3 amended 2026-11-13. |
| spec 2.5.1, 2.5.5 | Both are held for Classic as well as Chaos, by the user's ruling with the retune. | Spec amended 2026-11-13. |
| spec 2.1.2; `tuning.ron`, `ai` | The stars' handicaps were retuned by the user's ruling, and the AI plans a served ball one step after the serve. Classic's computer opponent is not as M1 shipped it. | Spec 2.1.2 amended 2026-11-13. Playtest-log entry owed. |
| spec 2.5.2 | R3 is shown for one-star and two-star mirror matches. It is not shown for three stars. | Spec amended 2026-11-13; owed by milestone close. |
| spec 2.3.2 | The trait is `id`, `contribute`, and `flaw`. The tuning section the item lists is not a part the compiler demands. | Spec amended 2026-11-13. |
| spec 2.9.1 | Hiding the overlay and leaving a match release the sandbox, and its keys do nothing once a match is over: the user's ruling after the review. | Spec amended 2026-11-13. |

## Rulings by the user

- 2026-11-12: the double-counted return is fixed now, before the harness records baselines (step 0).
- 2026-11-12: the serve fix, and a recipe `just ai-report`, with the proposal.
- 2026-11-12: retune the stars now, so that more stars are harder to beat and no star is unbeatable, in Chaos or in Classic (step 2).
- 2026-11-12: after losing 10 to 0 to three stars at the first candidate, errors of 12, 7, and 3; then "leave it like this for now".
- 2026-11-13, after the review: the phase is not closed; the `bevy-api-check` rule gains an exception for an API already used in the repository; hiding the overlay and leaving a match both release the sandbox; Chaos gets a soak as scheduled.

## Step 0

### Record step for phase 3 (P.6)

The seven deviations of the [phase 3 report](m2-phase3-report.md) are recorded:
spec 2.6.1, 2.7.1, 2.7.2, 2.8.4, and 2.9.1 (two deviations, one amendment) carry an amendment dated 2026-11-12;
ADR 0010 was amended when its deviation was taken.

### The double-counted return

- **Fault.** A return whose step ended exactly on a paddle's face was found again by the next step's sweep, which starts on the grown rectangle's edge and moves away. `slab` gives that sweep an exit of zero (`-0.0` from the low edge, `0.0` from the high one), and `t_exit < 0.0` refused neither. The second contact was a second return: a second speed step, a second `BallHitPaddle`, a second cue.
- **Contract.** `swept_circle_rect`: a sweep that leaves the grown rectangle at or before its start is no impact. A start that overlaps and stays inside for any part of the sweep is still an impact at time zero, so a paddle that moved onto the ball still deflects it.
- **Change.** `t_exit <= 0.0` in `kernel/collide.rs`. Physics is untouched: the kernel owned the fault.
- **Tests.** Red first: `ball_touching_the_face_and_moving_away_misses` (kernel) and `a_return_whose_step_ends_on_the_face_is_one_return` (physics; it read two `BallHitPaddle`). `ball_touching_the_face_and_moving_in_hits_at_once` passed before and after. Added after the review, passing on its first run: `ball_touching_the_far_face_and_moving_away_misses`, the `0.0` case.
- **Not covered.** A start that rounding leaves a hair inside the grown rectangle, moving away, is still an impact at time zero. By arithmetic, not by test: at the paddles' coordinates the contact point rounds onto the face exactly. No such case was found or built.

## Step 1: the flaw contract and the AI (plan 4.1)

### What was built

- **`Effect::flaw(start, difficulty) -> Flaw`.** The trait's third method, with no default body. `Flaw` has no `Default`: `added_error`, `added_delay`, and `believed`, a `Contribution`.
- **Speed and Shrink** add nothing and believe what they contribute: their flaws are inherent (spec 2.6.0, 2.6.1).
- **`FlightFlaw` and `ActiveEffects::flaw_over(flight, difficulty)`.** The flaws composed: errors and delays by sum, beliefs into a flight.
- **`Flight::began_at` and `Flight::believed_as`** (kernel). A flight keeps the `FlightStart` it began at, so an effect is asked for its flaw with the start it was asked for its contribution with, a held count included. The believed flight keeps the true flight's start and clock.
- **The AI** takes a `Sighting`: the ball's motion, the flight it is in if any, and the effects in play. On the step an approach begins it asks for the flaw, predicts under the believed flight, widens the star's spread by the added error, and lengthens the star's delay by the added delay. One draw per approach, as before.
- **`BallFlight::in_flight` and `clear`.** The component knows whether a serve or return began its flight; a begun flight with no effect in play is as empty as the cleared one.

### Decisions taken

- **The flaw is asked for when an AI plans, not at the flight start.** The effects feature does not know which difficulties play, and a match of two humans should not compose three beliefs per flight. The cost is two contracts on the effect, both stated on the trait since the review: `contribute` depends on nothing that changes while the flight lasts, and the same start and difficulty give the same flaw for as long as it lasts.
- **A known gap, dev builds only.** A tuning file reloaded in mid-flight rebuilds the effects at once, and the ball's flight keeps the old contributions until its next start. An AI that plans in between is told the new numbers about the old flight. Stated on `flaw_over`; not closed, since a reload that reaches the next return is what tuning by hand needs.
- **The belief is a whole `Contribution`,** not a field alone. A channel added later (obstacles, plan 9.2) is then part of every belief by the same compiler walk that makes it part of every contribution.
- **`Sighting`** groups what the controller sees, and the system's `SeenBall` groups the ball query with the effects: both functions were at clippy's seven parameters. No lint was silenced.
- **Classic runs the same path.** Nothing branches on the mode: with Speed alone the composed flaw adds nothing and believes a flight without fields, so the star's handicap is the whole flaw.
- **The star's handicap stays in the `ai` tuning section,** shared between modes. A `Flaw` adds to it.
- **`probe.rs`,** a test-only module of the effects feature, holds `Misjudged`, the probe effect the base's tests and the AI's tests share. It is declared at the end of `effects/mod.rs`, since `arch-check` reads production code up to the first `#[cfg(test)]`.

### The serve fix: built, with a narrower reason than proposed

The proposal said that from Wind on, an AI that ran after the serve would aim a served ball without the wind.
That held for the AI as it was, which predicted under the ball's flight.
It does not hold for the AI as built: the belief is composed from the effects in play, so an AI planning under the cleared flight still believes in every field.

What the fix still buys:

- **`flaw_over`'s precondition holds.** The flight handed to it was started by the effects in play. The cleared flight was not: its start is a serve at count zero whatever the sandbox holds.
- **A held count reaches the serve's plan.** The harness measures at held counts.
- **One behaviour for both system orders.** The AI plans a served ball on the step after the serve, always.

The cost: in the order where the AI ran after the serve, the AI reacts to a serve one step (1/60 s) later than it did. No earlier test's assertion moved.

### Done-conditions

| Condition | Evidence |
|---|---|
| The flaw's two forms in the AI (plan 4.1) | `an_effects_added_error_widens_the_spread_of_the_aim`, `an_effects_added_delay_holds_the_paddle_longer`, `an_effects_flaw_adds_to_the_stars_own_error_and_delay`, `the_ai_aims_where_the_flight_it_believes_in_takes_the_ball` (AI); four tests of `flaw_over` (effects base); `a_believed_flight_keeps_the_start_and_the_clock_and_pushes_as_believed` (kernel). |
| Spec 2.3.12 for Shrink: the zero-flaw AI returns every ball | `at_shrinks_full_dial_an_ai_with_no_delay_and_no_error_returns_every_ball`: 200 seeded launches, paddles at the minimum height. |
| Spec 2.3.12 for Shrink: a raised flaw misses some (read as a raised dial) | `an_error_whole_paddles_forgive_misses_some_balls_at_shrinks_full_dial`: six units of error return all 40 launches with whole paddles and miss some at full dial. |
| An effect missing its flaw does not compile (spec 2.3.2) | The trait has no default body; adding the method failed the build at Speed, Shrink, and the two test probes until each stated one. |
| "Read from each effect's tuning" (plan 4.1) | Not built: first deviation above. Probes carry the numbers. |

In the launch tests a ball counts as returned only if the paddle turned it back: a clip by the paddle's end, which lets the ball fly on, is a miss (since the review).

### Red first, and where it was not

- Red by compile failure, then green: the kernel's two tests, the base's four, Speed's and Shrink's, `the_ball_is_in_no_flight_until_the_serve_begins_one_and_in_none_after_the_rally`, and the AI's unit tests of step 1.
- Red by assertion: `whatever_the_system_order_the_ai_plans_a_served_ball_one_step_on_under_its_flight` failed at seed 1 with the system still passing the ball's flight unconditionally.
- **Not red first:** Shrink's two done-condition tests, which pin behaviour Shrink already had; and `an_effects_flaw_adds_to_the_stars_own_error_and_delay`, added after the review to behaviour already built.
- **A vacuous first draft, caught.** The system-order test first passed under the old wiring: replacing `Tuning` had made the base rebuild the effects and drop the probe. The test now adds its probe a few steps into the countdown, asserts the flight pushes, and asserts the step the plan is made on.
- **What that test cannot see:** which of the serve and the AI ran first. A temporary debug test, since removed, showed the AI planning on the serve's step in eight of seeds 0 to 11 before the fix. A change to the schedule's shuffle could leave one order untested and the test green; its doc comment says so.

## Step 2: the AI-versus-AI harness (plan 4.2)

### What was built

- **`testing::ai_match`.** `AiMatch { left, right, mode, points_to_win, forced, rally_hit_limit }` and `play(seed)`, which plays one match on an app of its own and returns a `MatchPlayed`: the winner and every `RallyPlayed` (paddle hits, who won it). `RallyLengths` summarises rallies: count, mean, median, longest, unended.
- **`Forcing`.** What the sandbox forces for a whole match: the set of effects, a held return count, both, or neither.
- **`Sandbox::force_effects(effects)`.** The whole forced set at once; `toggle` is built on it.
- **`pong_core/tests/ai_matches.rs`** and **`just ai-report`**, which runs it with the output shown.
- **Since the review:** `rally_hit_limit` is a `NonZeroU32`; and a match with neither a hit nor a point in thirty seconds of game time panics with its setup and seed, so a broken fixture says why. No test reaches that guard: a stalled match could not be built without breaking the rules feature.

### The finding, at the handicaps M1 shipped

Everything in this section was measured on 2026-11-12 at M1's handicaps: delays 0.35, 0.18, 0.05 s and errors 12, 5, 1. None of it describes the game as it ships now.

Spec 2.5.1 asks that seeded AI-versus-AI Chaos matches show bounded rally length. **At those handicaps it did not hold between two three-star computers.** The proposal had estimated their rallies at about sixty returns; that estimate was wrong.

Matches to five points, hits per rally:

| Pairing | Chaos, cap 3 (6 seeds) | Classic (6 seeds, limit 300) |
|---|---|---|
| 1 v 1 | 40 rallies, mean 2.3, longest 8 | mean 2.4, longest 11 |
| 1 v 2 | 30 rallies, mean 4.7, longest 13 | mean 5.5, longest 22 |
| 1 v 3 | 30 rallies, mean 4.1, longest 16 | mean 4.8, longest 16 |
| 2 v 2 | 40 rallies, mean 44.3, longest 100 | no rally ended |
| 2 v 3 | 30 rallies, mean 58.7, longest 176 | no rally ended |
| 3 v 3 | no rally ended (limit 2,000) | no rally ended |

Mirror matches at a held return count, forced sets, limit 200, 3 seeds (mean hits; "none" means no rally ended):

| Held count | 1 v 1 Speed | 1 v 1 Speed+Shrink | 2 v 2 Speed | 2 v 2 Speed+Shrink | 3 v 3, either |
|---|---|---|---|---|---|
| 0 | 2.8 | 2.8 | none | none | none |
| 6 | 2.6 | 1.8 | none | none | none |
| 12 | 2.3 | 0.8 | none | none | none |
| 15 | 2.6 | 0.8 | none | 24.0 | none |
| 25 | 2.6 | 0.8 | none | 21.7 | none |
| 57 (Chaos's speed cap) | 1.6 | 0.8 | none | 24.5 | none |

By a temporary test since removed: of the AI tests' 200 seeded launches (any height, any angle the paddle can give, any speed from the serve to the cap, the AI resting at the centre), three stars missed none, and three stars given two stars' handicap missed none.

What those numbers said, of M1's handicaps:

- **Speed alone ended no rally against two or three stars,** in Classic or in Chaos, at any count up to the cap.
- **The star's aim error was the only source of misses.** One star's 12 units exceed the whole paddle's reach. Two stars' 5 were inside it, and outside the shrunk paddle's only at Shrink's full dial. Three stars' 1 was inside every reach.
- **In Classic, as M1 shipped it, two and three stars were not beaten by speed or placement** from a centred start. Whether a human beat them some other way was not measured: the harness has no model of a human.

Three options were put to the user: leave the tuning and let the later effects' flaws carry it (recommended); retune now; or change the AI's model. The user ruled a retune.

### The retune

Ruled 2026-11-12: three stars are to be harder to beat than two, and two than one, and no star unbeatable, in Chaos or in Classic. The user's own play behind it: one point scored against three stars, in a long rally, over all attempts.

**A sweep,** by a temporary test since removed: the three-star AI's delay (0.05 to 0.35 s) against its error (1, 3, 5), in mirror matches and 50 launches per speed.

| Delay | Classic mirror, mean hits (error 1 / 3 / 5) | Chaos mirror, mean hits (error 1 / 3 / 5) | Random launches missed at the cap |
|---|---|---|---|
| 0.05 | none ended (the finding) / none ended / none ended | none ended (the finding) / none ended / 45.0 | 0% |
| 0.12 | not run / none ended / none ended | not run / none ended / 45.0 | 0% |
| 0.18 | not run / none ended / none ended | not run / 2 of 3 unended / 44.7 | 0% |
| 0.22 | 52.2 / 53.2 / 71.8 | 69.9 / 73.6 / 37.9 | 0% |
| 0.25 | 47.4 / 43.2 / 51.4 | 59.9 / 64.9 / 38.5 | 2% |
| 0.28 | 33.6 / 29.5 / 33.3 | 53.3 / 61.6 / 40.0 | 2% |
| 0.31 | 29.6 / 26.0 / 29.7 | 50.7 / 52.3 / 40.5 | 4% |
| 0.35 | 26.5 / 24.9 / 24.8 | 45.6 / 50.8 / 36.5 | 6% |

The error-1 rows for delays of 0.12 and 0.18 did not run: the sweep's first case failed on its own fixture.
At speeds of 100, 150, and 200 no launch was missed at any delay in this sweep.

- **Under about 0.2 s nothing ended in Classic; above it everything did.** That matches arithmetic from the tuning: at the cap a level ball takes 0.584 s to cross, and a paddle at the centre needs up to 0.4 s to reach the far end of its range, so a delay under 0.184 s always arrives.
- **The error did almost nothing in Classic** while inside the paddle's reach. In Chaos an error of 5 ended rallies through Shrink, whatever the delay.
- **A delay-made miss is earned.** It needs speed and placement: a ball far from the centre the AI waits at.

**First try:** delays only, to 0.29 s for two stars and 0.24 s for three, errors left at 5 and 1. Every rally then ended in every pairing, in both modes. The user played three stars and lost 10 to 0.

**Second try, the same evening,** by the user's ruling: errors to 12, 7, and 3. This is what ships:

| Stars | M1 | Now |
|---|---|---|
| One | 0.35 s, error 12 | unchanged |
| Two | 0.18 s, error 5 | 0.29 s, error 7 |
| Three | 0.05 s, error 1 | 0.24 s, error 3 |

What the delays mean for a player, by arithmetic from the tuning, for a level ball with the AI at the centre and its error left aside:
three stars are beaten at the cap by a ball crossing more than about 39 units from the centre (the ball's centre turns at 44), and from a speed of about 228, which Classic reaches at the 22nd return;
two stars by a ball more than about 34.5 out at the cap, and from about 212, the 19th return.
In Chaos the ramp is 3 a return, so those speeds come at the 50th and 44th return: there Shrink, and the effects to come, do the work.

**Measured at the shipped handicaps** (`just ai-report`, 4 seeds a pairing, matches to five points, hits per rally; no rally unended):

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 1 v 1 | 28 rallies, mean 2.4, longest 11 | 25 rallies, mean 2.3, longest 6 |
| 1 v 2 | 20 rallies, mean 5.7, longest 44 | 22 rallies, mean 4.1, longest 17 |
| 1 v 3 | 20 rallies, mean 6.4, longest 26 | 20 rallies, mean 5.3, longest 16 |
| 2 v 2 | 32 rallies, mean 33.5, longest 65 | 27 rallies, mean 14.7, longest 26 |
| 2 v 3 | 22 rallies, mean 30.1, longest 46 | 20 rallies, mean 19.2, longest 30 |
| 3 v 3 | 30 rallies, mean 41.7, longest 143 | 33 rallies, mean 66.0, longest 105 |

More stars won 8 matches to 0 in each of the three pairings, in each mode.
With no effect forced into play, two-star and three-star mirrors score no point in 200 hits.
Against the first try, Classic and the three-star mirror moved little; two stars' Chaos rallies shortened from a mean of 38.0 to 14.7, since an error of 7 is outside the shrinking paddle's reach before full dial.

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
| 0.60, 3 (20 launches per speed) | 0 | 0 | 15 | 30 |

- **An error of 7 or less caused no miss below the cap.** Misses appear between 7 and 8: one launch in a hundred at 8. The rows at 9 and 10 fit a miss share of about `1 - 8.6 / error`; that is a fit to this table, not a derivation.
- **So the shipped errors leave two and three stars missing nothing below the speed cap** against a whole paddle. A player who cannot yet carry a rally to the cap is given no points by the second try.
- **A longer delay lowers the speed at which a well-placed ball wins,** to 146 units/s at 0.6 s by the arithmetic above. Random launches barely show it: at 0.6 s none of 20 was missed at 100 or 150, and the misses came at 200 and above. A placed ball is not a random one, and no test here places balls at lower speeds.
- **In this AI model an error past the reach is what makes a star miss an ordinary ball,** and it does so by chance, at any speed, not by the shot's merit. It would also end the control of spec 2.5.1: a rally with no effect in play would then end.

**Tests of the retune,** written against M1's handicaps, where four failed, then green:

| Test | Asks |
|---|---|
| `chaos_rallies_end_within_the_hit_limit_in_every_pairing` | Spec 2.5.1 for all six pairings. |
| `classic_rallies_end_within_the_hit_limit_in_every_pairing` | The same of Classic. |
| `in_both_modes_the_computer_with_more_stars_wins_more_matches` | Spec 2.5.5, and the same of Classic. |
| `at_the_speed_cap_a_level_ball_along_the_wall_beats_three_stars` | The earned miss: one unit inside either wall at 250. |
| `three_stars_return_the_same_ball_sent_slower_or_nearer_the_centre` | Passed before and after: at 80 and at 150 along the wall, and at 250 from 20 out. |

A test that had pinned the three-star gap, and a Chaos-only version of the first and third, were written at step 2 and replaced by these.

### Decisions taken

- **One app per match, seeded.** A match is reproduced from its seed alone.
- **Rally length is paddle hits,** as the longest-rally record counts them, not the flight's return count: a held count stands still.
- **A hit limit, not a step limit,** gives up a rally: it does not depend on the frame length.
- **Four-step frames.** The simulation steps the same at any frame length; longer frames spend fewer on the countdowns. No timing against one-step frames was taken.
- **Levels of the stack come from the mode's data:** the always-on effects plus the first `k` of the bag, at `k` times fifteen returns.
- **`RallyPlayed`, not `Rally`:** rules already has a `Rally`.

## Step 3: the soak (plan 4.3)

- **One runner.** `tests/soak.rs` has a `Soak` (app seed, player seed, mode, a forced set at full dial or none, live steps) whose `run` is M1's soak loop.
- **The refactor changed nothing in Classic.** Before and after, and again after the review's changes, the seeded run prints the same line: 67,714 frames, 16 matches, 1,025 paddle hits, top speed 250.00002. Its three assertions are untouched.
- **The players predict under the ball's own flight,** so a field effect is tracked from the phase it ships in.
- **Chaos, one test per subset size 1, 2, 3.** Every choice of that many effects from Chaos's bag is forced beside the mode's always-on effects, with the return count held at `u16::MAX`: full for every dial whatever return its ramp is full at. Since the review the run forces anew at every match, as leaving a match releases the sandbox, and asserts per subset that no other set of effects was ever in play, that flights began at the held count, that a return reached the speed cap, and that a match ended.
- **Chaos as scheduled,** added after the review by the user's ruling: nothing forced, a hundred thousand live steps. It asserts a match ended, at least 550 paddle hits, every effect of the bag in play at some time, and some flight beginning at fifteen returns or more.
- **The subsets come from the mode's tuning,** so an effect added to the bag is soaked with no edit to the test.

Measured, no fault in any:

| Soak | Frames | Matches | Paddle hits | Top speed | Most returns |
|---|---|---|---|---|---|
| Classic | 67,714 | 16 | 1,025 | 250 | not read |
| Chaos as scheduled | 67,772 | 15 | 819 | 140 | 20 |
| Chaos, Speed and Shrink at full dial | 128,802 | 85 | 1,387 | 250 | held |

The pair and triple tests found no subset: the bag ships one effect.

**The spec's open fact,** whether a soak per subset of six effects fits CI, is answered by construction: a test's time does not grow with the roster, since its steps are divided. With six effects in the bag a subset gets 16,666 steps as one of 6 singles, 6,666 as one of 15 pairs, and 5,000 as one of 20 triples; with seven, 14,285, 4,761, and 2,857. At full dial the measured run played 85 matches of at least five points each in its 100,000 steps, so a rally took at most about 235 live steps, and the smallest share still plays a dozen rallies or more.

**Noticed, not changed:** the Classic soak's floor of 1,000 paddle hits has little room left. The run makes 1,025; the floor's comment says the first green run made 1,118. The number was first read in this phase at step 3, so when it fell is not known.

Not done: no deliberate break was run against the Chaos soaks.

## Step 4: Shrink's R3 measure (plan 4.4, E.4)

One test per place in Chaos's bag forces the effect at that place alone, with no speed ramp beside it, up a ladder of held return counts, for one-star and two-star mirror matches. It asserts rallies never lengthen from one rung to the next and are shorter at the top than at the bottom. A new effect in the bag is measured with no edit. Seven places are covered; a place the bag does not fill passes with nothing to measure.

Shrink alone, matches to five points, hit limit 200 (Shrink's dial starts to move after return 3 and is full at 15):

| Held count | Paddle height | 1 v 1, 8 seeds | 2 v 2, 2 seeds | 3 v 3 |
|---|---|---|---|---|
| 0 | 14 | 60 rallies, mean 2.4 | none ended (2 of 2) | none ended |
| 5 | 13 | 55 rallies, mean 1.9 | 2 of 3 unended | none ended |
| 10 | 10.5 | 56 rallies, mean 1.1 | 12 rallies, mean 7.1 | none ended |
| 15 | 8 | 56 rallies, mean 0.7 | 13 rallies, mean 3.2 | none ended |
| 20 | 8 | mean 0.7 | mean 3.2 | none ended |
| 30 | 8 | mean 0.7 | mean 3.2 | none ended |

The three-star column was measured once, by a temporary variant of the test at 3 seeds; the test itself plays one and two stars.

- **R3 holds for Shrink against one and two stars.**
- **R3 does not show against three stars.** Their aim error of 3 is inside the reach of the shortest paddle, and the ball here stays at the serve speed, so no rally ends at any rung.
- **The soak's subsets include Shrink** (step 3), which completes E.4.

The stack's measure (spec 2.5.2): one-star mirror, Speed at 0 returns 60 rallies of mean 2.4, Speed and Shrink at 15 returns 56 of mean 0.7; two-star mirror, none ended at the first level, 13 rallies of mean 3.2 at the second.

**How "shorter" is judged,** since the review. A run with unended rallies has no true mean: the mean it prints is a floor set by the hit limit. Such runs are compared by how many of their rallies never ended. Two runs whose rallies all ended are compared by their means, with half a hit allowed for sampling noise between rungs and more than half a hit required between bottom and top. `censored_runs_are_compared_by_their_unended_rallies_not_their_means` pins that the comparisons can fail.

## The review (2026-11-13)

Four reviewers, each independent of the author and of each other, read phase 3 and phase 4: phase 4's code, phase 3's code, the tests of both, and the docs against each other and the code. None found a blocking fault in the code. One docs finding blocked: this report and `CURRENT` disagreed on whether the phase was closed.

What changed in answer, beyond what the steps above already say:

- **The sandbox does not outlive what shows it** (user's ruling). `release_the_sandbox_with_the_match` runs on leaving a match; the overlay releases on hide; the sandbox keys do nothing once a match is over, and the overlay does not offer them there. Tests: `leaving_a_match_releases_what_the_sandbox_forces`, `hiding_the_overlay_releases_the_sandbox`, `once_the_match_is_over_the_sandbox_keys_do_nothing_and_are_not_offered`.
- **The overlay's released line** names the latest moment the game has its own effects and count back, and is true after a count-only release.
- **An effect's join cue is built on demand:** `ShellHalf::join_cue` is a constructor, so asking for an icon synthesizes nothing.
- **Tests that could not fail, or not for the right reason:** the two R3 tests (above); the touched-match rule, now tried through a forced set as well as a held count; forced effects under a bag of two, where the schedule's turning and its silence can be seen (`while_effects_are_forced_the_schedule_turns_unseen_and_unannounced`); the bracket key from a rally's own count above zero.
- **The `bevy-api-check` rule** gained its exception, in `CLAUDE.md` and `.claude/rules/pong_core.md` (user's ruling).
- **Docs:** 51 relative links that still pointed at the plans' old place; the feature order in `pong_core/README.md`; `Rally`'s description; the architecture tree; smaller stale lines; and the record step, which amended the spec and plan for this phase.

Left as they are, each for a stated reason:

- **The glyph-uniqueness test** compares nothing while one effect has an icon. It starts to bite with Wind's icon.
- **Which order the serve and the AI ran in** is not observable by a test (step 1).
- **The launch `(72, 2)` at 200,** repeated across test files with its reason stated in one: not gathered into a shared helper, since the files are in two crates' separate test trees.
- **The stall guard** of the harness has no test (step 2).

## Spec items of this phase

| Item | State |
|---|---|
| 2.3.12, the flaw's two forms | Done for the contract and the AI. For Shrink read as a raised dial. The per-star tuning section waits for Wind. |
| 2.5.1, rallies end under Chaos; the control | Done for all six pairings; held for Classic too. |
| 2.5.2, R3 for the stack and per effect | Done for one and two stars. Not shown for three stars: owed by milestone close. |
| 2.5.3, soak per mode | Done for the shipped roster, in the amended form. |
| 2.5.4, sanity under Chaos | Done: the soaks and every harness match run under the panicking policy with no fault. |
| 2.5.5, stars order | Done, in both modes: 8 matches to 0 in every pairing. |

## Open at phase close

- **The user's hand check of the shipped handicaps.** The first candidate lost the user a match 10 to 0 against three stars; the second is unplayed. By the launch table it gives a player no points below the speed cap. The user's word on 2026-11-12: "leave it like this for now".
- **A playtest-log entry** for the two observations behind the retune and for the hand check.
- **R3 against three stars** (spec 2.5.2): to be shown once an effect's flaw reaches them, from Wind on.
- **CI's running times:** not measured. See "Running times".
- **The Classic soak's paddle-hit floor:** 1,025 against 1,000.
- **Seeded numbers across machines.** The simulation uses `sin_cos`, which Rust does not promise to be bit-identical across platforms. Whether CI's seeded numbers equal the development machine's is not known until CI runs these tests; the margins that matter are stated under "Running times" and in the tables.

## Running times

Development machine, each test alone or in `just ai-report`'s serial run:
the AI-match tests take 1.6 to 5.2 s each (the dial test 5.2 s for one effect, each further effect in a test of its own);
the Classic soak 3.6 to 4.1 s, Chaos as scheduled 4.2 s, Chaos at full dial 7.2 s.

Inside the full parallel run of `just dev-check` the same tests take longer, since nine heavy tests share the cores: 8.5 to 13.8 s each on 2026-11-13.
Nextest ends a test at 30 s. CI has not run them.

## Test changes

406 core tests and 168 shell tests pass, 574 in all; 518 existed before this phase, and 558 before the review.

No assertion of a test that existed before this phase was changed. Call sites changed for a changed signature:

- `MirrorController::steer` takes a `Sighting` where it took the ball's motion and a flight: eleven calls in the AI's tests pass `flying(ball)`.
- `Effect` gained `flaw`: the effects tests' two probes, `Probe` and `Swinger`, state one.
- The AI tests' `three_stars_with` finds the embedded three-star entry by its text, which now reads 0.24 and 3.0.
- The soak's `Player::intent` takes the ball's velocity along x and its crossing where it took the tuning and the ball's motion.
- The AI tests' `launch_at_ai` counts a return only when the ball turned back. Every launch test passes unchanged under it.

Tests of this phase changed by the review: the two R3 tests assert through `no_longer` and `shorter`; the per-effect one became one test per bag place; the Chaos subset soak asserts the forcing held through every match; `a_new_match_is_touched_only_if_something_was_forced_for_it_beforehand` replaces a test that tried a held count only; the overlay's unit tests expect the new released line and the sandbox keys only while the sandbox is open.

Comments corrected: the AI tests' `launches` (step 0); `match_screens.rs`, which still described the double-counted return as current (review).

## Bevy API

Steps 0 to 4 wrote no API new to the repository, so `bevy-api-check` was not run for them, under the exception the user ruled on 2026-11-13.
`#[derive(SystemParam)]` over a `Query<'w, 's, ..>` and a `Res<'w, ..>` is as in the shell's overlay and in escalation.

The review's changes wrote one: the `OnExit` schedule label, for releasing the sandbox with its match.
**Deviation: it was written before it was checked.** `bevy-api-check` ran on 2026-11-13, after the code and its test were green, against the pinned 0.20.1 source, and confirmed the code as written:

- `bevy::state::state::OnExit` exists and needs only the `bevy_state` feature.
- In a transition from `InMatch` to `Menu` it runs in `StateTransition`, after the sub-state's own exit and before `OnTransition` and `OnEnter(Menu)`; entities marked `DespawnOnExit(InMatch)` are still alive while it runs.
- It does not run when the app starts, nor when the app is dropped in a match.
- It does run for a transition from `InMatch` to `InMatch` requested with `NextState::set`, whatever the type's own doc comment says. Nothing requests one today; the system's doc comment records it.

## Facts learned

- A sweep that starts on the grown rectangle's edge and moves away is no impact, from either edge. The interior start stays with plan 9.1.
- Replacing the `Tuning` resource rebuilds the effects in play from their ids on the next step: a test probe added before that step is replaced by the effect whose name it borrows.
- A belief composed from the effects in play does not depend on the ball's flight for anything but its start and clock.
- Without escalation a two-star or three-star defender returns every ball: confirmed, and the spec's fact relabelled.
- At M1's handicaps the speed ramp ended no rally against two or three stars. At the shipped handicaps it ends every one: the reaction delay decides which.
- A mean over rallies cut off at a hit limit is a floor, not a mean; two such floors compare as equal whatever the rallies would have done.
- Forcing the sandbox before a match is entered holds for that match; anything forced during a match is released when it is left.
- A match forced into `MatchOver` by setting the state has no winner, and the match-over screen errors on it: a test reaches match over by scoring.
