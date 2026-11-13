# M2 phase 5 report: Wind

Started 2026-11-13. Branch `feat/wind`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-13: steps 1 to 3 of the proposal the user approved that day (plan 5.1, and the core half of 5.2).
Not built: steps 4 to 8 (the shell half, Wind in the bag and the harness, the wall cue limit, the record step, the hand check and the kids' session).
Not closed. `docs/specs/CURRENT` names phase 5.

## Deviations

Both were proposed before implementation and approved by the user on 2026-11-13.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.2.2 | The vertical bound is derived, not tuned: the playable height per fixed step, 5,280 units/s at the shipped numbers. Validation gains no rule for it; the speed-cap rule stays, so a launch is never cut. | Owed at step 7. |
| spec 2.2.4, ADR 0011 | A rally has a seed, and every effect is built for one rally with it. `Random` holds a second stream, of the same seed, for rally seeds. | Owed at step 7. |

## Rulings by the user

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

## Test changes

439 core tests and 168 shell tests pass, 607 in all; 574 existed before this phase. 33 were added, all in the core.

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or a new rule:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Wind`.
- The overlay's two tests of the sandbox key line: it now reads `1 Speed, 2 Shrink, 3 Wind`.
- `a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move` (paddle): its cramped arena, 12 high, now also lowers `max_drift` to 4, since 24 is refused there.
- `VALID` in the tuning tests gained Wind's section.
- `under_a_field_the_ball_crosses_where_the_kernel_predicts_from_its_flight` (effects) now calls three helpers the new Wind test shares; its assertions are the same.

## Bevy API

Steps 1 to 3 wrote no API new to the repository, so `bevy-api-check` was not run.
`OnTransition` is as rules uses it for the serve.

## Open

- **Wind can be forced from the sandbox before it can be seen.** Key 3 in the overlay puts it in play today, with no icon, cue, or view. Developer-only, until step 4.
- **Docs owed at step 7:** the two spec amendments above, ADR 0011's note on the rally seed, and the touch-list comparison (E.5). Files outside the touch list so far: the kernel (`integrate`, `predict`, `flight`, `random`), `domain.rs`, the effects base, `paddle`, and the overlay's key line.
- **The numbers are starting numbers.** The harness tunes them in step 5, the kids' session after.
- **Carried from phase 4:** the hand check of the retuned stars and its playtest entry, at step 8; R3 against three stars, at step 5.
