# M2 phase 9 report: Blocks

Proposed 2026-11-24. Branch `feat/blocks`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-24: steps 1 to 3 of the proposal the user approved that day (plan 9.1, 9.2; E.2 of 9.3).
Not built: steps 4 to 7.

## The proposal's steps

| Step | What | State |
|---|---|---|
| 1 | Pin what `swept_circle_rect` does for a sweep that starts inside; record it in the spec (plan 9.1) | Done. |
| 2 | Obstacle channel in the kernel: tilted sweep, one step with a bounded contact loop, the floor; physics and prediction both take it (plan 9.2) | Done. A third bound, the bounce budget, was added by the user's ruling. |
| 3 | Blocks' core half: variant, tuning section, placement, mirror, flaw (E.2) | Done. Blocks is in no bag. |
| 4 | Shell half: view, icon, join cue, bounce cue (E.1, E.3) | Not started. The shell half is declared empty. |
| 5 | Into the bag; soaks; harness; R3 and R10 (E.4, E.7) | Not started. |
| 6 | Record: spec amendments, ADR, touch list (P.6, E.5) | Not started. |
| 7 | Hand check, session, ruling (E.6) | Not started. |

## Deviations

The first six were proposed before implementation and approved by the user on 2026-11-24. The rest arose in the build.
Only the first is in the spec yet. The others wait for the record step (step 6), by the approved order of steps.

| Item | Deviation | Recorded |
|---|---|---|
| spec, facts: what `swept_circle_rect` returns for a start inside | Answered. See "Step 1". | Spec, "Facts relied on", amended 2026-11-24. |
| spec 2.6.6, "AI flaw: its belief lags, as Lens" | The flaw has a second part: a block it sees a star takes for more upright than it is, by a tuned share of its tilt. With the lag alone a computer reads every aged block perfectly. | This report. |
| spec 2.2.6, the horizontal floor | The floor has no number of its own: it is `ball.max_deflection_degrees`, the steepest a paddle sends the ball. | This report. |
| spec 2.6.6, "the angle range is tuning" | The range excludes upright: a tilt is drawn from `min_tilt_degrees` to `max_tilt_degrees`, either way, and validation refuses a least tilt of zero. | This report. |
| spec 2.6.6, "blocks stay clear of both paddles' contact zones" | Clear of more: of midfield and of both walls by a ball's width, and of the contact line by a tuned run that validation keeps above one step's travel at the speed cap. | This report. |
| spec 1.3.2, the core's messages | One more: `BallHitObstacle`, from physics, once for each bounce. | `domain.rs`, `domain.md`; spec at step 6. |
| spec 2.2.6.1 and "Deliberately absent": "blocks stay solid always" | The bounce budget is taken. After `ball.obstacle_bounces_per_flight` bounces off obstacles in one flight the ball flies through obstacles until its next flight. See "The traps". | This report; the user's ruling below. |
| spec 2.2.6, prediction | `crossing_y` follows a ball an obstacle turns back to the far goal line and gives it no crossing there. It would otherwise step for twenty minutes of flight. | This report. |
| `.claude/rules/pong_core.md`, "through a generator of the effect's own seeded by it" | Blocks seeds its generator with the rally's seed and a constant of its own above it. Seeded as Lens seeds its, every block stood on a lens. | This report. |

## Rulings by the user

2026-11-24, on the proposal: "Proposal approved as per your recommendations. Implement steps 1-3." The proposal's rulings:

1. **Roles.** Claude drives every step.
2. **The flaw.** The lag, and a believed share of a seen block's tilt.
3. **The floor's number.** `ball.max_deflection_degrees`.
4. **Tilt range excludes upright.** 10 to 35 degrees, either way.
5. **No spacing, no bounce budget.** The second half is overtaken by the ruling below.
6. **Look and sound.** For step 4.
7. **Starting numbers.** Two blocks a side, at returns 5 and 8; `clear_run` 24; 4 contacts a step; `unseen_flights` 3 / 2 / 1; `believed_share` 0.4 / 0.7 / 0.9.

2026-11-24, on the traps, asked with the numbers of "The traps": "Budget of 6 (Recommended)". The other choices were a budget of 4, or to stop with no budget and the termination test red.

## Step 1: the sweep that starts inside (plan 9.1)

Read in `pong_core/src/kernel/collide.rs` and pinned by four tests there. No behaviour changed.

- **A start inside the grown rectangle is an impact at time zero,** by any depth and whichever way the ball moves.
- **The face reported is the one behind the ball** on the axis entered last. A mirror on it turns a leaving ball back in.
- **A ball sliding along a face it starts on counts as inside.**
- **Under mirror reflection the ball does not leave.** A level ball a ten-thousandth inside a face flipped on every one of 600 steps. A steep one, leaving, has its vertical speed turned.
- **A paddle escapes this:** a face hit launches by side and is no mirror.

Tests: `a_sweep_that_starts_a_hair_inside_and_moves_out_is_an_impact_that_turns_it_back_in`; `a_steep_sweep_that_starts_a_hair_inside_and_moves_out_is_given_a_face_it_is_not_leaving_by`; `a_sweep_along_the_face_it_starts_on_is_an_impact_on_the_face_behind_it`; `a_level_ball_a_hair_inside_shuttles_under_mirror_reflection_and_never_leaves`.
Not red first: each pins what the function already did.

## Step 2: the obstacle channel (plan 9.2, spec 2.2.6)

### What was built

- **`kernel/obstacle.rs`.**
  - `Obstacle`: a rectangle at any tilt, counter-clockwise from upright.
  - `swept_circle_obstacle`: the path is turned into the obstacle's own frame and swept there with `swept_circle_rect`. A start within a skin of a face (a thousandth of a unit), leaving through it or sliding along it, is no contact.
  - `BounceRule`: the base's bounds. `leaving` mirrors the ball on the face and, where that is steeper than the floor, turns it to the floor's angle at its own speed: on the side the mirror sent it, or on the other where that side would turn it back into the face.
  - `advance_among_obstacles`: one fixed step among walls and obstacles, leg by leg. After the rule's contacts in a step the ball flies to the next contact and stops there; the rest of the step is forfeited.
  - `ObstaclesInFlight`: what one step is told of the flight: its obstacles, its rule, its bounces so far.
- **With no obstacle, or a spent budget, the step is `advance_between_walls` to the last bit.** No earlier test moved.
- **The channel.** `Contribution` has a fourth field, `obstacles`; every effect and probe states it. `Flight` holds the union, the rule, and its count of bounces.
- **The base stamps the rule.** `ActiveEffects::start_flight` gives every flight `tuning.ball().bounce_rule()`. No effect states a bound.
- **Tuning, `ball` section:** `contacts_per_step` 4, `obstacle_bounces_per_flight` 6. Each is a whole number above zero by the file's shape.
- **Physics.** `integrate_ball` takes the new step, counts the step's bounces on the flight, and writes `BallHitObstacle` once for each. A bounce starts no flight.
- **Prediction.** `crossing_y` takes the same step and spends the budget ahead. Its signature is unchanged, and so is the AI's production code.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.2.6: a ball aimed level at a 45° face still reaches a paddle line | `a_ball_aimed_level_at_a_45_degree_face_still_reaches_a_paddle_line`; `a_level_ball_on_a_45_degree_face_leaves_at_the_floors_angle_and_its_own_speed`. |
| Spec 2.2.6: a ball in a wedge ends the step inside the contact bound | `a_ball_in_a_wedge_ends_the_step_inside_the_contact_bound`, at bounds of 1, 2, and 4: no step makes more contacts, some step makes as many, the ball is never inside a block, and it comes back out. |
| Spec 2.2.6: prediction equals simulation through two bounces | `prediction_equals_simulation_through_two_bounces` (kernel, with and without a force); `among_obstacles_the_ball_crosses_where_the_kernel_predicts_from_its_flight` (through the systems). |
| Spec 2.2.6: swept in the block's frame | `a_tilted_face_is_struck_where_the_ball_meets_it_and_gives_its_own_normal`; `an_upright_obstacle_is_struck_as_its_rectangle_is`. |
| Plan 9.1's consequence | `the_sweep_after_a_bounce_is_no_second_contact_at_any_tilt`, over 17 tilts and 96 approaches; three tests of the skin's two sides. |
| The floor | Five tests of `BounceRule::leaving`; `off_any_face_the_ball_keeps_its_speed_the_floors_share_and_leaves_the_face` sweeps 72 faces by 35 approaches. |
| Spec 2.2: the channel composes commutatively | `obstacles_given_in_either_order_give_the_same_step`: two crossed blocks struck at the same moment; `a_flight_carries_the_obstacles_of_every_contribution_in_either_order`. |
| The bounds are the base's | `every_flight_bounces_by_the_tuned_rule`; `a_believed_flight_bounces_by_the_true_rule_off_the_believed_obstacles`. |
| A bounce is a collision inside a flight | `a_ball_bounces_off_an_obstacle_of_its_flight_and_the_flight_goes_on`. |
| Spec 2.2.6.1: the budget | `a_flight_that_has_spent_its_bounces_flies_through_obstacles_as_through_air`; `the_last_bounce_of_the_budget_is_made_and_the_rest_of_the_step_flies_through`; `prediction_spends_the_flights_bounce_budget_as_the_simulation_does`; `a_flight_that_has_spent_its_bounces_flies_through_its_obstacles_until_the_next_return` (physics). |
| The prediction gives up at the far goal | `a_ball_a_block_turns_back_for_good_has_no_crossing_and_is_given_up_at_the_far_goal`: under 200 steps, where it took 100,000. |

### Red first, and where it was not

- **The sweep and the floor:** 12 of 16 tests red by assertion against stubs. Four that ask for a miss passed against a sweep that never strikes.
- **The skin is needed.** With the sweep built and no skin, three tests stayed red, and `the_sweep_after_a_bounce_is_no_second_contact_at_any_tilt` among them: at a tilt of 80 degrees a bounce was followed by a contact at time zero on another face.
- **The step:** five tests red by assertion against a step that ignored obstacles.
- **The channel, the rule's stamp, the tuning, the budget:** red by compile failure, then green. The physics and effects-base tests were then red by assertion until the systems were wired.
- **Prediction:** red by assertion; it flew through the blocks.
- **Not red first:** `with_no_obstacle_the_step_is_the_wall_step_to_the_last_bit` and two more that pin the fast path.
- **Checked against a deliberate break, since removed:** with the far-goal test switched off, the prediction's test counts 100,000 steps and fails.
- **Two of my tests were wrong, not the code.** A funnel of two blocks was no wedge: the floor threw the ball out in three bounces, and the test became a narrowing channel. And a step I reckoned at two bounces held three.

## Step 3: Blocks' core half (plan 9.3, E.2)

### What was built

- **`EffectId::Blocks`**, saved as `blocks`, and one registration line per crate. The shell's is a half declared empty, in `pong/src/effects/blocks.rs`, until step 4.
- **Tuning section `effects.blocks`:** `starts_at_return`, `full_at_return`, `max_count`, `min_tilt_degrees`, `max_tilt_degrees`, `clear_run`, `unseen_flights` per star, `believed_share` per star. A block's size is not tuned here: it is the paddle's thickness and default length.
- **Refused:** a count full before it starts, or starting at the serve; a least tilt of zero; a tilt of a right angle; the range reversed; a negative run or share; a `clear_run` a ball at the speed cap crosses in one step; no room for a block's centre on its half; a block too long to stand a ball's width clear of both walls.
- **`Tuning::block_room`:** where a block's centre may lie, reckoned with the block's half diagonal so that it holds at any tilt. Validation and placement read the one formula.
- **`effects/blocks.rs`:** `Blocks`, built for one rally. A generator of its own draws each block's distance from midfield, height, tilt, and which way it is turned, in order. `Blocks::placed(start)` is the blocks of a flight, each drawn block followed by its copy, a prefix of the answer for any later return.
- **The flaw** is the second form in two parts: the blocks of `unseen_flights` returns ago, each turned by `believed_share` of its tilt. Under a held count no block is new.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.6.6: a drawn block and its copy are mirror images, by seeded test | `a_drawn_block_and_its_copy_are_mirror_images_across_midfield`: corner for corner, over 200 rallies. |
| Spec 2.6.6: the default paddle's length, the paddle's thickness, an angle from the tuned range | `every_block_is_a_default_paddle_in_size_and_tilted_within_the_tuned_range`; `a_block_is_the_default_paddles_length_and_the_paddles_thickness` (tuning). |
| Spec 2.6.6: clear of both paddles' contact zones | `every_block_stands_clear_of_midfield_the_walls_and_the_paddle`, every corner, over 200 rallies. |
| Spec 2.6.6: appears at a paddle hit, stays until the point | `a_serve_flies_over_an_empty_floor`; `a_block_once_there_stays_where_it_is_as_the_rally_grows`. |
| R9: graded, absent at zero | `the_dial_is_the_count_of_blocks_each_with_its_copy`. |
| R4: terminates | `among_a_full_floor_of_blocks_every_return_reaches_a_contact_line`: 14,000 flights. Red without the budget. |
| R5: the shared math | `prediction_equals_simulation_through_a_full_floor_of_blocks`: 1,400 flights at the cap, the crossing where the ball crosses, or none where it is turned back. |
| R5 and spec 2.3.12: the zero-flaw AI misses nothing; a raised flaw misses some | `among_blocks_an_ai_that_sees_them_as_they_are_and_has_no_handicap_returns_every_ball`; `..believes_too_little_of_their_tilt_misses_some_balls`; `among_blocks_too_new_for_it_to_see_..misses_some_balls`; `once_the_blocks_have_stood_long_enough_the_same_ai_misses_no_ball`. |
| R2: skill decides | `with_no_block_on_the_floor_a_misjudging_ai_returns_every_ball`. |
| The flaw per star | `each_star_sees_a_block_once_it_is_old_enough_and_then_believes_its_share_of_the_tilt`; `under_a_held_count_no_block_is_too_new_for_any_star_to_see`; `the_embedded_lags_fall_and_the_shares_rise_with_the_stars`; `at_the_serve_every_star_believes_in_no_block`. |
| Blocks and lenses do not coincide | `a_rallys_first_block_and_first_lens_lie_independently_of_each_other`. |
| R10 | Step 5's (E.7). |

### Red first, and where it was not

- **Tuning:** red by compile failure, then green but for two tests. One was my fixture: a block too long for the walls was refused for its half first. The other was an earlier test; see "Test changes".
- **Blocks:** against a `Blocks` that stood no block, red by assertion: the dial, the mirror, the size and tilt, the seed, the contribution, the held count, and prediction. The rest ask for an empty floor or pin the embedded numbers.
- **The independence test** was red by assertion against the generator seeded as Lens's: the two lay 0.00000002 of their rooms apart.
- **Not red first:** the AI's five tests, which pin what Blocks then gave.
- **Checked against deliberate breaks, each since removed:**
  - the flaw believing the truth and seeing every block: the flaw's test and the AI's two tests that some balls are missed;
  - the lag never acting: the flaw's test and the AI's lag test.

### The AI among blocks

- **The zero-flaw test launches at the serve speed.** The proposal said so: a block can turn a ball at a paddle from close by, and at speed no paddle reaches it. At the serve speed nothing got past.
- **Of 200 launches at a full floor, 90 reached the AI and 110 were turned back** to the side they came from. 15 of the 90 came off a block.
- **A launch turned back is no miss and no return.** The AI's tests among blocks have a launcher of their own with that third outcome.

## The traps

**The termination test found a ball that never arrives.** In one seeded rally a return at the serve speed bounced between a block, its mirrored copy, and the bottom wall, the same three bounces over and over, and was in the air after the flight limit of 20 s.

Measured in throwaway tests, since deleted: 3,000 rallies at a full floor, 35 returns each, 105,000 flights at each speed, no budget.

| | Serve speed | Speed cap |
|---|---|---|
| Still in the air after 5 s | 3,177 | 118 |
| After 10 s | 175 | 90 |
| After 20 s, the flight limit | 100 | 75 |
| After a minute | 75 | 62 |

- **Bounces a flight:** none in 43 of 100, one in 45, two in 7, three or four in 4, more in 1.
- **Cause.** A block and its mirrored copy, with a wall, give a ball's direction back after a few bounces, whatever it was. The floor is not what holds the ball: with the floor folding steep angles back in place of setting them all to 60 degrees, 70 flights of 105,000 were still in the air after 20 s.
- **The spec foresaw it** as 2.2.6.1, to be taken only if a trap was observed.

A budget of N bounces a flight, over 35,000 flights at a full floor:

| Budget | Flights that spend it | Longest flight, serve speed | Longest, cap |
|---|---|---|---|
| 3 | 5.1% | 8.6 s | 2.8 s |
| 4 | 2.6% | 9.8 s | 3.1 s |
| 6 | 0.84% | 12.9 s | 4.1 s |
| 8 | 0.40% | 14.6 s | 4.7 s |
| 12 | 0.18% | 20.5 s | 6.6 s |

- **The user ruled 6.**
- **Why it ends every flight:** after its last bounce a ball keeps its horizontal direction and at least half its speed across, so it reaches a goal line.
- **What a player sees when it is spent:** the ball pass through a block. Step 4's view has to show spent blocks as spent.

## Test changes

At the end of step 3: 605 core tests and 215 shell tests, 820 in all; 743 existed before this phase. 77 were added, all in the core.

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or the base:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Blocks`.
- `every_core_event_survives_a_write_and_read_in_a_headless_app` (domain): one more message.
- The overlay's two tests of the sandbox key line: it now ends `6 Lens, 7 Blocks`.
- `VALID` in the tuning tests gained Blocks' section and the ball's two counts.
- `everything_must_fit_the_arena_and_each_refusal_names_its_values` (tuning): its last accepted case, a speed cap of 5,279, now needs an arena 400 wide and a clear run of 90. A ball that fast crosses Blocks' clear run of 24 in one step, which the new rule refuses.
- `a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move` (paddle): its cramped tuning has a default paddle 5 long and a least of 4, where they were 9 and 8. A block is as long as the default paddle and has to stand clear of walls 12 apart.
- Every earlier `Contribution` literal states `obstacles: Vec::new()`: 15 places.
- `launch_at_ai_as_the_return` (AI) now calls `begin_the_flight_of_a_return`, split out of it; every test that uses it is the same.
- The kernel's `simulated_crossing` (prediction tests) steps among the flight's obstacles; its tests have none.

## Bevy API

No API new to the repository was written. `bevy-api-check` did not run.
The kernel uses `bevy::math` only, in calls the repository already made: `Vec2::new`, `splat`, `dot`, `length`, `abs`, `from_angle`, `Rect::from_center_half_size`. Physics writes one more message through `MessageWriter`, as it writes two already.

## Process

Each is a departure from `CLAUDE.md` or the test rules, stated here as they ask.

- **Filtered test runs went through `cargo nextest run` directly,** with a package and a filter, during the red and green cycles, and `cargo clippy` once for the core alone. The recipes take no filter. The gate below is the recipe.
- **Before the proposal, the sweep was tried in a throwaway Python port outside the repository.** It showed the shuttle that step 1 then pinned by test.
- **The traps and the budgets were measured in throwaway tests inside `blocks.rs` and `obstacle.rs`,** and one throwaway change to the floor. All are deleted; nothing of them is in the diff.

## Gate

`just dev-check` green on 2026-11-24 with steps 1 to 3 and the budget: format, layering, lint, 820 tests, doctests. This report, after it, is a document only.
CI: not run by Claude. The user's, with the commits.

## Open after step 3

- **Blocks is in no bag.** Nothing of it plays until steps 4 and 5.
- **Not recorded yet:** every deviation above but the first. Spec 2.2.6, 2.2.6.1, 2.6.6, 2.3.8, 2.3.12, "Deliberately absent", ADR 0011, and both rules files wait for step 6.
- **Half of all returns at a full floor come back to the hitter's side.** Whether that shortens rallies (R3) or shields goals is step 5's measure and the session's to judge. The spec calls it uncertain.
- **A spent block is passed through.** Step 4 has to show it, and the session to say whether it reads.
- **Not shown by test:** the zero-flaw AI at speeds above the serve speed among blocks.
- **Blocks' numbers are Claude's starting numbers.**
- **Carried:** the true well for Lens, to be put to the user after this phase; Ice (plan 10.1).
