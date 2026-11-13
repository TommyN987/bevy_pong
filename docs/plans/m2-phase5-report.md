# M2 phase 5 report: Wind

Started 2026-11-13. Branch `feat/wind`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-13: steps 1 to 6 of the proposal the user approved that day (plan 5.1, 5.2, and 5.3).
Recorded 2026-11-13: step 7, with CI green on the build (`74d9bea`).
Played 2026-11-13: step 8, the user's hand checks and the kids' first Chaos session (plan 5.4).
Closed 2026-11-13 by the user's word. `docs/specs/CURRENT` names phase 6, Spin, on branch `feat/spin`.

## Deviations

Both were proposed before implementation and approved by the user on 2026-11-13.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.2.2 | The vertical bound is derived, not tuned: the playable height per fixed step, 5,280 units/s at the shipped numbers. Validation gains no rule for it; the speed-cap rule stays, so a launch is never cut. | Spec 2.2.2 and plan 5.1 amended 2026-11-13. |
| spec 2.2.4, ADR 0011 | A rally has a seed, and every effect is built for one rally with it. `Random` holds a second stream, of the same seed, for rally seeds. | Spec 2.2.4, plan 5.2, and ADR 0011 amended 2026-11-13. |

## Rulings by the user

- 2026-11-13, after the kids' session: the two as-built points without a ruling are accepted as they are for now, documented, and addressed if either becomes an issue: a believed share above one, and a ball in wind faster than the speed cap. The phase is complete; the next branch is `feat/spin`.
- 2026-11-13, after CI passed on the build: "I started a game, and it looked alright. Didn't have time for more right now." Step 7 now; the kids' sessions after.
- 2026-11-13: the proposal and its recommendations approved. Claude drives. The flaw is the believed share alone. Starting numbers: dial from return 0, full at 12, `max_drift` 24, shares 0.4, 0.7, 0.9. The hand check of phase 4's retuned stars rides with step 8.

## Step 1: the vertical bound (plan 5.1, spec 2.2.2)

- **Contract.** `advance_between_walls` had a precondition: the step's vertical travel is under the playable height. It is now a postcondition: the ball ends inside the walls, to rounding, under any force. The step cuts the vertical speed to the playable height per `dt` before it moves the ball.
- **Why here.** The bound is the wall fold's own limit, so it sits with the fold, in the one function physics and prediction share. No effect and no caller carries it.
- **A hole found by test, in this step's own work.** `crossing_y` recomputed the velocity for its last, partial step, and so ignored the bound there: past the bound it predicted a crossing outside the walls (-46.9 at a limit of 44). It now reads the velocity the wall step used. Below the bound the two are bit-identical; no earlier test moved.
- **Tests.** Red first, by assertion: `a_step_driven_past_the_playable_height_is_cut_to_one_fold_and_lands_inside`, `a_ball_already_faster_than_one_fold_undoes_is_cut_the_same`, `at_the_vertical_bound_the_prediction_is_still_the_simulation`. Passing on its first run, since it pins what the implementation must keep: `a_step_of_no_length_leaves_the_motion_as_it_was`.
- **The second test's own arithmetic was wrong once.** It expected 40 where the course gives -40; the comment and the number were corrected after the fix made the first assertion pass.

Done-condition (spec 2.2.2): "a test at the clamp folds once": the three tests above.
"Tuning validation rejects a configuration that could break the precondition": no configuration can break it now; the first deviation.

## Step 2: the rally's seed

### What was built

- **`RallySeed`** (kernel, `random.rs`): one rally's number.
- **`Random::next_rally_seed`**: drawn from a second `Pcg32` of the same seed, set apart by a constant.
- **`ActiveEffects`** holds the rally's seed and gives it to the registration match; `rally_seed()` reads it.
- **`draw_the_next_rallys_seed`**: on entering a match, and on the transition from `Playing` to `PointScored`. It builds the effects in play again, for the new rally.

### Why it has this shape

- **An effect value holds no state.** Escalation builds the effects again at every countdown, and a tuning reload builds them again mid-rally. A direction kept inside an effect would be lost at both. So the choice is a pure function of a seed the base holds.
- **A rally is not a countdown.** A resumed pause enters `Countdown` again, and `PointScored` likewise. The draw hangs on the transition from play to a point, which a resume does not make, and on match entry.
- **A second stream, so that nothing else moves.** One more draw per rally from the one stream would shift every seeded number of both modes. The three soak lines are the check, and are as phase 4 printed them: Classic 67,714 frames, 16 matches, 1,025 paddle hits; Chaos as scheduled 67,772 frames, 15 matches, 819 hits, most returns 20; Chaos at full dial 128,802 frames, 85 matches, 1,387 hits.

### Tests

Red first, by assertion (the seed stayed zero): `a_match_begins_with_a_drawn_rally_seed_and_each_point_draws_the_next`, `a_pause_in_a_countdown_or_in_a_point_pause_draws_no_seed`, `a_fault_restart_keeps_the_rallys_seed`.
Red first, by compile failure: `rally_seeds_come_from_their_own_stream_and_leave_every_other_draw_as_it_was`.

## Step 3: Wind's core half (plan 5.2, E.2)

### What was built

- **`Drift`** (kernel, `flight.rs`): a field stated as displacement per crossing, `a = 2 · per_crossing · (vx / crossing_width)²`. In the kernel so that Spin can use it without importing Wind (plan rationale 6, spec 2.3.5).
- **`Tuning::crossing_width`**: the distance between the two contact lines, 146 units. `Paddle::contact_x` now reads it: one formula. Bit-identical to the old one.
- **Wind's tuning section**: `starts_at_return`, `full_at_return`, `max_drift`, and `believed_share` per star. Refused: a dial full before it starts; a negative drift or share; `max_drift` at or above half the playable height (spec 2.6.2).
- **`EffectId::Wind`**, saved as `wind`, and one registration line per crate.
- **`effects/wind.rs`**: `WindDirection::of(seed)`, the seed's lowest bit; `Wind`, whose contribution is a `Drift` of the dial's amount in the rally's direction, and nothing at a dial of zero.
- **The flaw** is the second form alone: an AI believes its star's share of the true drift, in the true direction.
- **The shell half is declared empty.** Wind is in no mode's bag, so no player meets it before step 4 gives it an icon, a cue, and a view.

### Decisions taken in implementation

- **A share above 1 is accepted.** It is a belief too: the AI over-leads. Only a negative share is refused. Not in the proposal; say if it should be capped at 1.
- **The rebuild at a point.** The seed changes at the point and escalation builds the effects at the countdown. Between the two the effects were those of the old seed: `the_wind_in_play_always_blows_the_way_the_rallys_seed_shows` failed in the point pause. `begin_rally` now builds them at once, so the invariant holds at every moment.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.6.2: prediction equals simulation within one unit at every tested dial | `in_wind_the_ball_crosses_where_the_kernel_predicts_at_every_dial`: held counts 0, 3, 6, 12, 30, through the systems, to 0.01. |
| Spec 2.6.2: the drift at the paddle matches the dial at two ball speeds | `the_drift_at_the_paddle_is_the_dial_at_the_serve_speed_and_at_the_cap`: 24 units both ways, at 80 and at 250, within the integrator's budget of one step's share (0.22 and 0.69). `a_drift_carries_the_ball_its_amount_over_one_crossing_at_any_speed` in the kernel. |
| Spec 2.6.2: direction drawn per rally from the seeded source | `a_rallys_seed_chooses_the_direction`, `the_wind_pulls_the_way_the_rallys_seed_chose_at_every_return`, `the_wind_in_play_always_blows_the_way_the_rallys_seed_shows` (both directions over eight rallies). |
| R9: graded, absent at zero | `a_rally_starts_with_no_wind`, `the_pull_never_eases_as_the_rally_grows`, `the_drift_follows_the_dial_between_its_ends`. |
| R4: terminates | `the_wind_never_changes_the_balls_horizontal_speed`: a crossing takes its still-air time, a wall bounce included. |
| R5 and spec 2.3.12: the zero-flaw AI returns every ball; a raised flaw misses some | `in_full_wind_an_ai_that_believes_all_of_it_and_has_no_handicap_returns_every_ball` (200 launches); `in_full_wind_an_ai_that_believes_too_little_of_it_misses_some_balls`. |
| Spec 2.6.2: the AI's error scales with strength | `with_the_dial_at_zero_the_same_misjudging_ai_returns_every_ball`; `each_star_believes_its_share_of_the_true_drift_and_is_charged_nothing_else`. |
| R2: skill decides | The same 40 balls in the same wind: returned by the AI that reads it, missed by the one that misreads it. |

Shown in "the countdown" (spec 2.6.2) is step 4's: the core gives the shell `ActiveEffects::rally_seed` and `WindDirection::of`.

### Red first, and where it was not

- Red by compile failure, then green: the kernel's two `Drift` tests, the tuning section's six, Wind's twelve.
- Red by assertion: `each_star_believes_its_share_of_the_true_drift_and_is_charged_nothing_else` (Wind first believed the truth); `the_wind_in_play_always_blows_the_way_the_rallys_seed_shows` (the point pause).
- **Not red first:** `in_wind_the_ball_crosses_where_the_kernel_predicts_at_every_dial`, and the AI's three Wind tests. They pin what the kernel, the base, and Wind already gave.
- **Checked against two deliberate breaks, since removed.** With every AI believing the truth, `..believes_too_little..misses_some_balls` failed. With every AI believing in no wind, `..believes_all_of_it..returns_every_ball` failed.

## Step 4: Wind's shell half (E.1, E.3)

### What was built

- **Icon:** `≋`, pale blue. The font holds the glyph (its charset covers U+2190 to U+2311).
- **Cue:** two plain notes rising a fifth, the second held. Shrink's is three falling brass notes.
- **View:** fourteen small arrowheads on the floor, one per column across the arena, at heights spread by the golden ratio. They point and drift the way the wind blows, at 12 units/s while the dial is at zero and up to 60 at full dial, and wrap from one wall to the other. Dim and under the paddles, the trail, and the ball.
- **Where and look are apart,** as in `views/`: `WindArrowsPlugin` touches only `Transform` and `Visibility` and is tested headless; the mesh and the material are added by the shell half's view, with the render assets.

### Decisions taken

- **The direction is read from the rally's seed,** by the same `WindDirection::of` the core's Wind uses, not from a message. So what the countdown shows cannot differ from what blows; `in_the_countdown_the_arrows_point_and_drift_the_way_the_ball_is_then_carried` ties the two through the ball itself.
- **An arrow turns over by a negative height,** not by a rotation: one fewer new API.
- **The arrows hold still while play is paused and once the match is over,** as the ball's view does.
- **The next rally's wind shows from the point on,** in the point pause already: the seed is drawn at the point.
- **`views::layer` is the crate's now** (`pub(crate)`), with a `WIND` layer between the centre line and the paddles: a file outside the touch list.

### Done-conditions

| Condition | Evidence |
|---|---|
| E.1: API check before code | See "Bevy API". |
| R1: the cause on screen before the consequence | `in_the_countdown_the_arrows_point_and_drift_the_way_the_ball_is_then_carried` (six seeds, both directions); `after_each_point_the_arrows_turn_to_the_next_rallys_wind`. |
| R9 on screen | `the_arrows_drift_faster_at_full_dial_than_in_the_countdown`. |
| R11: nothing flashes | `with_wind_in_play_every_arrow_shows_from_the_countdown_on_and_never_blinks`. An arrow wraps at most once a second and a half, at full dial. The review item at milestone close stays. |
| Spec 2.3.3, 2.7.1: icon and cue | `wind_states_an_icon_a_join_cue_and_a_view`; the generic cue and glyph tests now cover Wind. |
| E.3: visible in `just run` | Seen by the user on 2026-11-13, first in one short game ("it looked alright"), then in the hand check and the session of step 8. |

### Red first, and where it was not

- Red by compile failure, then green on the first run: the seven tests of `pong/tests/wind.rs`. Red by assertion: `wind_states_an_icon_a_join_cue_and_a_view`.
- **Checked against two deliberate breaks, since removed.** With the direction fixed upward, the two direction tests failed. With the pause ignored, `while_play_is_paused_the_arrows_hold_still` failed.

## Step 5: into the bag, and the harness (E.4, E.7)

`tuning.ron`: Chaos's bag is `[shrink, wind]`. The tuning numbers did not move: every condition below held at the starting numbers.

### What the bag of two moved

A bag of two is shuffled, which draws, so every seeded Chaos number moved. Classic's did not: its soak still prints 67,714 frames, 16 matches, 1,025 paddle hits, and its harness lines are phase 4's.

Tests that named Shrink as the one bag effect, each kept on its assertion:

- Escalation: `two_in_the_bag` finds the new text; four tests that follow one named effect now play a fixture with Shrink alone in the bag (`shrink_alone_in_the_bag`); `speed_after` reads the launch speed under the same fixture, since a wind in play bends the ball within the step of the return.
- Shell, audio: `entering_a_chaos_match_plays_the_joining_effects_cue` asks the core which bag effect joined, where it named Shrink.
- Shell, HUD: `the_joining_effect_shows_large_in_the_countdown_then_takes_its_place_in_the_row` reads the glyphs of the icons laid out, where it read every icon spawned: two effects have icons now.

### Measured (`just ai-report`, matches to five points, hits per rally; no rally unended)

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 1 v 1 | 28 rallies, mean 2.4, longest 11 | 29 rallies, mean 2.1, longest 12 |
| 1 v 2 | 20 rallies, mean 5.7, longest 44 | 20 rallies, mean 3.5, longest 10 |
| 1 v 3 | 20 rallies, mean 6.4, longest 26 | 20 rallies, mean 5.9, longest 16 |
| 2 v 2 | 32 rallies, mean 33.5, longest 65 | 30 rallies, mean 10.6, longest 25 |
| 2 v 3 | 22 rallies, mean 30.1, longest 46 | 20 rallies, mean 14.1, longest 23 |
| 3 v 3 | 30 rallies, mean 41.7, longest 143 | 27 rallies, mean 55.0, longest 156 |

More stars won 8 matches to 0 in each of the three pairings, in each mode (spec 2.5.5).
Spec 2.5.1 holds: every rally ended in every pairing.

### Wind's R3 measure (E.4)

Wind alone, no speed ramp beside it, matches to five points, hit limit 200 (the dial is full at 12 returns):

| Held count | Drift | 1 v 1, 8 seeds | 2 v 2, 2 seeds |
|---|---|---|---|
| 0 | 0 | 57 rallies, mean 2.4 | none ended (2 of 2) |
| 5 | 10 | 56 rallies, mean 2.4 | 16 rallies, mean 14.5 |
| 10 | 20 | 56 rallies, mean 1.6 | 12 rallies, mean 6.5 |
| 15 | 24 | 66 rallies, mean 1.6 | 14 rallies, mean 5.5 |
| 20, 30 | 24 | mean 1.6 | mean 5.5 |

- **R3 holds for Wind against one and two stars.**
- **Against three stars, Wind alone ends nothing at the serve speed:** their share of 0.9 leaves the aim 2.4 off at most, and with their error of 3 that is inside the whole paddle's reach of 8. Not measured; by arithmetic.

### R3 against three stars: shown (owed from phase 4, spec 2.5.2)

The stack's test now plays three-star mirrors too:

| Level | 1 v 1 | 2 v 2 | 3 v 3 |
|---|---|---|---|
| Speed at 0 returns | 57 rallies, mean 2.4 | none ended | none ended |
| Speed, Shrink at 15 | 61 rallies, mean 0.6 | 15 rallies, mean 2.0 | none ended |
| Speed, Shrink, Wind at 30 | 56 rallies, mean 0.7 | 15 rallies, mean 1.9 | 15 rallies, mean 43.8, longest 103 |

Wind's flaw is what reaches three stars: 2.4 of misjudged drift beside an error of 3 is past the shrunk paddle's reach of 5.
The one-star rise from 0.6 to 0.7 is inside the half hit the comparison allows for noise.

### The soaks

| Soak | Frames | Matches | Paddle hits | Top speed | Most returns |
|---|---|---|---|---|---|
| Classic | 67,714 | 16 | 1,025 | 250 | not read |
| Chaos as scheduled | 66,543 | 15 | 865 | 167.5 | 28 |
| Speed, Shrink at full dial (half the budget) | 66,974 | 46 | 684 | 250 | held |
| Speed, Wind at full dial (half the budget) | 41,120 | 15 | 951 | 287.1 | held |
| Speed, Shrink, Wind at full dial | 126,642 | 82 | 1,419 | 287.8 | held |

No fault in any. The pair test is live for the first time; the triple still finds no subset.

- **A ball in wind flies faster than the speed cap,** 287 against 250. The cap is on the launch speed; the wind adds vertical speed in flight, and never horizontal speed. Sanity's slack past a goal line is by horizontal travel, so it holds.
- **The same set shows in two orders** in the scheduled soak, `[Speed, Wind, Shrink]` and `[Speed, Shrink, Wind]`: the order is the bag's rotation, and every channel composes commutatively.

### R10 (E.7)

`a_ball_three_stars_return_in_still_air_scores_when_sent_with_the_wind`: three stars as tuned return a level ball at the cap from 20 off the centre. At full dial, sent on the side the wind blows toward, it is carried to the wall and scores; sent on the other side it comes back; with the dial at zero it comes back from both.

### Running times

Development machine, inside the full parallel run of `just dev-check`: eleven heavy tests now, 3.2 to 8.8 s each, against the 30 s limit.
The two new ones: the pair soak 8.6 s, the second bag effect's dial test 8.6 s. The stack's test, with three stars, 6.6 s.

## Step 6: the wall cue rate limit (plan 5.3)

- **The limit is the shell's.** The core reports every bounce, since each happened; the cue map plays at most one wall cue per 150 ms, and one for a frame's wall hits.
- **Why 150 ms.** A pinned ball bounces every other step, thirty times a second. Wall to wall takes the ball over a quarter of a second at its fastest, so no bounce of ordinary play is dropped.
- **Tests.** Red first, by assertion: `a_ball_pinned_to_a_wall_plays_a_bounded_number_of_wall_cues_a_second` (30 cues, now 6), `wall_hits_in_the_same_frame_play_one_cue`. Passing before and after: `wall_hits_a_quarter_of_a_second_apart_each_play`.
- **One earlier test changed its vehicle.** `a_cue_plays_at_the_volume_set_then_and_goes_when_done` sent two wall hits a frame apart to compare volumes; it sends paddle hits now. Its assertions are the same.
- **Not shown by test:** that the core does pin a ball. The limit is tested against the message rate, not against a pinned rally.

## Step 7: the record step (P.6, E.5)

### Recorded

- **Spec:** 2.2.2 (the derived bound; the cap is the launch's), 2.2.4 (the rally's seed), 2.3.8 (the comparison below), 2.3.12 (Wind's per-star numbers; a share above one), 2.5.2 (R3 for three stars), 2.6.2 and its sub-item (Wind as built; the cue limit). Each amended 2026-11-13.
- **Plan:** 5.1 and 5.2 amended.
- **ADR 0011:** amended. An effect value holds no state; a choice per rally comes from the rally's seed; a shared field lives in the kernel; the bound is derived.
- **`.claude/rules/pong_core.md`:** the effects rule says the same in one line, for the effect phases to come.

Two decisions taken in implementation were not in the proposal and are in the spec as built: a believed share above one is accepted (2.3.12), and a ball in a field may fly faster than the speed cap (2.2.2). The user accepted both on 2026-11-13, for now; each amendment says so.

### The diff against the touch list (E.5, spec 2.3.8)

The phase's diff against `main` before this step: 29 files.

| Where | Files | Why |
|---|---|---|
| Inside the list | `effects/wind.rs` in each crate; `domain.rs` (the variant, `ALL`, its guard test); one line in each crate's registration match; the tuning section (`tuning.ron`, `file.rs`, `tuning/mod.rs`) | Wind itself. |
| Outside, paid once (ADR 0011) | Kernel `integrate.rs`, `predict.rs`; `random.rs`, `domain.rs`, the effects base; kernel `flight.rs`; `tuning/mod.rs`, `paddle/mod.rs`; `views/mod.rs` | The vertical bound; the rally's seed; `Drift`; the crossing width; a draw layer. |
| Outside, its own plan item | `audio/cues.rs`, `tests/audio.rs` | Plan 5.3. |
| Outside, tests and fixtures | The AI's and the escalation's test modules; `ai_matches.rs`, `soak.rs`; the overlay's key line in two tests; `match_screens.rs`; a paddle fixture | Wind's done-conditions in the AI; fixtures that named the roster; the harness. |
| Docs | Three READMEs, `domain.md`, this report | |

- **The list's negative half held:** no production code of physics, AI, rules, escalation, sanity, or the HUD changed. The AI's and the escalation's files changed in their test modules only; physics, rules, sanity, and `pong/src/ui` not at all.
- **What the next effect will also touch outside the list:** the two tests that spell out the overlay's key line. The escalation and shell tests no longer name the first bag effect.
- **What it will not pay again:** the bound, the seed, the kernel field (Spin's too), the crossing width, the layer.

### CI

Run 43432652619 on `74d9bea`, 2026-11-13: green. The eleven heavy tests took 2.3 to 4.7 s each inside the full run, nextest 16.0 s for all 622, the `just ci-check` job 2 min 39 s.
Every seeded assertion of this phase passed on CI's machine; the run prints no numbers.

## Step 8: the hand checks and the kids' session (plan 5.4, E.6)

The user's, on 2026-11-13, reported after the session and logged in the [playtest log](../playtest.md) that day.

- **Hand checks:** done by the user: Wind in `just run`, and the stars as retuned in phase 4. No detail was reported.
- **The session:** the 9-year-old, the 11-year-old, and the parent. The children played each other, each played the parent, and they played the computer.
- **Reported:** both children like both Chaos effects, and like Chaos more than Classic. The 9-year-old beat the two-star computer in Chaos.
- **Ruling per effect (E.6, spec 2.10.2):** neither a retune nor a drop, for Shrink or for Wind.
- **Phase 4's owed entry** is in the log too: the developer's play behind the retune of the stars, dated 2026-11-12.

No tuning number changed after the session: Wind ships at the starting numbers.

## Test changes

440 core tests and 182 shell tests pass, 622 in all; 574 existed before this phase. 48 were added: 34 in the core, 14 in the shell.

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or a new rule:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Wind`.
- The overlay's two tests of the sandbox key line: it now reads `1 Speed, 2 Shrink, 3 Wind`.
- `a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move` (paddle): its cramped arena, 12 high, now also lowers `max_drift` to 4, since 24 is refused there.
- `VALID` in the tuning tests gained Wind's section.
- `under_a_field_the_ball_crosses_where_the_kernel_predicts_from_its_flight` (effects) now calls three helpers the new Wind test shares; its assertions are the same.

The changes of steps 4 to 6 to earlier tests are listed in those steps: the escalation fixtures, two shell tests, and the volume test.

## Bevy API

Steps 1 to 3 wrote no API new to the repository, so `bevy-api-check` was not run.
`OnTransition` is as rules uses it for the serve.

Step 4 wrote one: `bevy::shape::Triangle2d`. `bevy-api-check` ran before the code, against the pinned 0.20.1 source:

- `Triangle2d::new(a, b, c)` is at `bevy::shape`, beside `Circle` and `Rectangle`; `bevy::math::primitives` does not exist at this version.
- It converts into a `Mesh`, so `meshes.add(Triangle2d::new(..))` works as it does for a rectangle. The builder normalises the winding.
- The 2D mesh pipeline does not cull (`cull_mode: None`), so a negative scale draws; frustum culling ignores the sign too.
- `Mesh2d` brings `Visibility` only where the camera plugin is installed. Headless, an entity that needs one is given one: the arrows are spawned with `Visibility::Hidden`.
- An entity spawned with `Transform` and `Visibility` takes `Mesh2d` and its material later, and keeps the visibility it had.

Steps 5 and 6 wrote none: `Local` and `Time` are as the effects base and the overlay use them.

## Open at phase close

- **Accepted as built, for now:** a believed share above one; a ball in wind faster than the speed cap, 287 units/s against 250 in the soak. To be addressed if either becomes an issue.
- **R11 for Wind's arrows:** the review item at milestone close (spec 2.7.4). The arrows were seen in play; no flashing was reported, and none was asked about.
- **Not shown by test:** that the core pins a ball to a wall; that Wind alone shortens three-star rallies.
- **Not recorded:** quotes, the session's length, and what the hand checks looked at.
- **Open hypotheses in the playtest log:** catch-up bias, with no outcome reported; Wind against Spin, which waits for Spin.
