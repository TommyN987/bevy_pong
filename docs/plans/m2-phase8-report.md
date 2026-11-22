# M2 phase 8 report: Lens

Proposed 2026-11-19. Branch `feat/lens`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-20: steps 1 to 4 of the proposal the user approved that day (plan 8.1; E.1 to E.4, E.7).
Recorded 2026-11-20: step 5 (P.6, E.5, plan 8.2).
Open: step 6, the user's hand check, the session, and the ruling (E.3's `just run`, E.6).

## The proposal's steps

| Step | What | State |
|---|---|---|
| 1 | The count form of the curve, the item rule, Lens's tuning section (spec 2.3.9) | Built. |
| 2 | Lens's core half (E.2) | Built. |
| 3 | Lens's shell half (E.1, E.3) | Built and tested headless. The look was seen by Claude in screenshots of the running game. |
| 4 | Into the bag, and the harness (E.4, E.7) | Built and measured. The measure asked for a base capability; the user ruled it in. |
| 5 | Record (P.6, E.5, plan 8.2) | Done. |
| 6 | Hand check, session, ruling (E.6) | The user's. Not done. |

## Deviations

The first four were proposed before implementation and approved by the user on 2026-11-20. The fifth arose in step 4 and was ruled that day.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.6.5, "AI flaw: its belief lags" | The flaw has a second part: of a lens it sees, a star believes a tuned share of the pull. With the lag alone a computer reads every aged lens perfectly, and R3 cannot hold at a full dial. | Spec 2.6.5 and 2.3.12 amended 2026-11-20. |
| spec 2.6.5, "a fixed size and pull" | The spec named no shape. A lens is a disc; the pull fades to nothing at the rim and scales with the square of the horizontal speed. A lens stays clear of the paddle by a tuned run, and validation refuses numbers that leave it no room. | Spec 2.6.5 amended 2026-11-20. |
| spec 2.2.6.2, spacing | Not taken for lenses: two may overlap, and their pulls add. | Spec 2.6.5 amended 2026-11-20. |
| `.claude/rules/pong_core.md`, "it draws nothing itself" | Lens seeds a generator of its own from the rally's seed to lay its lenses. Read as deriving from the seed: it draws nothing from the shared stream. | ADR 0011 and the rule amended 2026-11-20. |
| spec 2.2.4, 2.9.1; ADR 0011 | The flight start says whether its return count is held, and under a held count no lens is new. See "The held count". | Spec 2.2.4, 2.9.1, 2.3.8, plan 8.1, ADR 0011, both rules files, amended 2026-11-20. |

## Rulings by the user

2026-11-20, on the proposal: "You drive the whole phase. Your other recommendations are approved. Proceed with the implementation." The proposal's rulings:

1. **Roles.** Claude drives every step. The proposal recommended that the user drive the look; the user ruled otherwise.
2. **The flaw.** The lag, and a believed share besides.
3. **Lag under a held count.** Built to the letter and measured in step 4; if the dial test tripped, the cure was a base capability, put to the user then.
4. **No spacing between lenses.**
5. **The look.** Plain meshes, no shader; a grow-in through Bevy's curve API; no sound per lens.
6. **Starting numbers.** Lenses at returns 3, 5, 7; `radius` 8, `max_bend_degrees` 15, `clear_run` 24; `unseen_flights` 3 / 2 / 1; `believed_share` 0.4 / 0.7 / 0.9.

2026-11-20, on the measure, asked with the numbers of "The held count": "Held count is steady (Recommended)". The other choices were to drop the lag and keep the share, or to stop.

2026-11-20, asked before any window was opened: "Yes, open the window".

## Step 1: the count form, the item rule, the tuning section (spec 2.3.9)

### What was built

- **`ItemCount`** in `kernel/ramp.rs`, beside `Ramp`: none until `starts_at` returns, then the whole part of the linear dial, then `full` from `full_at` on. Reckoned in whole numbers, since thirds are not exact in floating point and a floor would bring an item a return late.
- **The item rule** is its constructor's: `starts_at` zero is refused (`InvalidItemCount::StartsAtTheServe`), as a full return before the start return is. Postcondition of `after`: none at zero returns, never fewer after more returns.
- **Tuning section** `effects.lens`: `starts_at_return`, `full_at_return`, `max_count`, `radius`, `max_bend_degrees`, `clear_run`, `unseen_flights` per star, `believed_share` per star.
- **Refused:** a count full before it starts, or starting at the serve; a radius not positive; a bend not in zero up to a right angle; a negative run or share; twice the radius plus the run above half the crossing width; a radius above half the playable height.
- **`PerStar<T>`:** the tuning's per-star type is generic, with `f32` as before and `u16` for the flights.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.3.9: a whole count | `an_item_arrives_at_the_first_return_the_dial_reaches_its_number`; `the_count_is_exact_where_the_dial_divides_unevenly`; `the_count_stays_full_from_its_full_return_on`; `equal_returns_bring_every_item_at_once`. |
| Spec 2.3.9: validation rejects a full hit before the start hit, and an item effect that starts at hit zero | `a_count_full_before_it_starts_is_refused_as_any_dial_is`; `a_count_that_starts_to_move_at_the_serve_is_refused`; in tuning, `a_lens_count_full_before_it_starts_is_rejected` and `a_lens_count_that_starts_to_move_at_the_serve_is_rejected`. |
| The tuning's refusals | Seven tests of the section in `tuning/mod.rs`. |

### Red first, and where it was not

- **`ItemCount`:** red by compile failure, then green but for one test.
- **One test was wrong, not the code.** The widest-span test expected 65,534 where the whole part is 65,533. It now asks for one item a return over the widest span.
- **Tuning:** red by compile failure, then green on the first run.

## Step 2: Lens's core half (plan 8.1, E.2)

### What was built

- **`EffectId::Lens`**, saved as `lens`, and one registration line per crate.
- **`effects/lens.rs`**: `Lens`, built for one rally from the tuning and the rally's seed.
- **Placement.** A generator seeded by the rally's seed draws each lens's distance from midfield and its height, in order. So a lens's place depends on the seed and on its own number, never on how many the dial holds. Drawn on the left half; the copy is the same disc at `-x`.
- **Where a lens may lie:** its centre from one radius off midfield to the contact line less the clear run less a radius; in height, inside the band the ball's centre travels less a radius. So a lens never touches its copy, no ball bounces off a wall inside one, and the run before the paddle is clear.
- **`Lens::placed(start)`**: the lenses of a flight, each drawn lens followed by its copy. A pure function of the start's return count, and a prefix of the answer for any later return: the shell draws what pulls, and no lens moves or leaves before the point.
- **The field, `Pull`,** is Lens's own, in its module: per lens, a path's curvature `d²y/dx² = -focus · dy · (1 - r²/R²)` inside the disc, times the square of the horizontal speed as an acceleration. The pulls of all lenses on the floor sum.
- **The bound.** A flight's horizontal speed is constant, so a ball passes each x of a disc once. The strongest curving at each x integrates over the disc to `π R² / (4 √3)` per unit of focus; a slope change `s` turns a path by at most `2 atan(s / 2)`. The focus is set so the slope may change by `2 tan(max_bend / 2)`.
- **The flaw** is the second form in two parts: the lenses of `unseen_flights` returns ago, at `believed_share` of their pull.

### Decisions taken in implementation

- **The count of lenses is `u16`,** as returns are; `max_count` has no upper bound of its own.
- **A believed share of nothing is a field of no focus,** not no field: harmless, and one path through the code.
- **The paddle's cramped-arena fixture shrinks the lens too,** as it shrinks Wind's drift and Spin's.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.6.5: one lens bends no crossing more than the tuned angle | `one_lens_bends_no_crossing_more_than_the_tuned_angle`: 1,221 entries (33 offsets, 37 slopes to 1.8), at the serve speed and the cap, at four phases of the step. No bend over the angle plus a fiftieth; some bend at nine tenths of it. |
| Spec 2.6.5: a drawn lens and its copy are mirror images | `a_drawn_lens_and_its_copy_are_mirror_images_across_midfield`, over 200 rallies; `the_copy_pulls_as_the_drawn_lens_does_mirrored`. |
| Spec 2.6.5: pulls toward its centre line | `a_lens_pulls_the_ball_toward_its_centre_line_and_nowhere_outside_itself`. |
| Spec 2.6.5: appears at a paddle hit, stays until the point | `a_serve_flies_over_an_empty_floor`; `a_lens_once_there_stays_where_it_is_as_the_rally_grows`. |
| The dial is in play terms | `a_path_bends_the_same_at_the_serve_speed_and_at_the_cap`. |
| R9: graded, absent at zero | `the_dial_is_the_count_of_lenses_each_with_its_copy`. |
| Placement | `every_lens_lies_on_its_half_inside_the_walls_and_clear_of_the_paddle`, over 200 rallies; `a_rallys_seed_chooses_where_its_lenses_lie`. |
| R4: terminates | `a_lens_never_changes_the_balls_horizontal_speed`; the soaks (step 4). |
| R5: the shared math | `prediction_equals_simulation_through_a_full_floor_of_lenses`: 200 balls over 40 floors. |
| R5 and spec 2.3.12: the zero-flaw AI returns every ball; a raised flaw misses some | `among_lenses_an_ai_that_sees_them_as_they_are_and_has_no_handicap_returns_every_ball` (200 launches); `..believes_too_little_of_their_pull_misses_some_balls`; `among_lenses_too_new_for_it_to_see_..misses_some_balls`; `once_the_lenses_have_lain_long_enough_the_same_ai_returns_every_ball`. |
| R2: skill decides | `with_no_lens_on_the_floor_a_misjudging_ai_returns_every_ball`; R10's test, below. |
| The flaw per star | `each_star_sees_a_lens_once_it_is_old_enough_and_then_believes_its_share`; `the_embedded_lags_fall_and_the_shares_rise_with_the_stars`; `at_the_serve_every_star_believes_in_no_lens`. |

### Red first, and where it was not

- **Lens:** against a `Lens` that laid no lens and pulled nothing, red by assertion: the mirror, placement, pull, bend, speed, and prediction tests. Placement was built first, then the pull and the flaw.
- **Passed against the stub, red only once lenses lay:** the flaw's test, since a share of no pull is no pull.
- **Not red first:** `it_names_itself`, the serve's test, the embedded lags and shares, and the AI's six tests, which pin what Lens then gave.
- **Checked against deliberate breaks, each since removed:**
  - the focus a tenth too strong: the bound's test fails, at 15.46 degrees;
  - no fade at the rim: the bound's, the two speeds', and prediction's;
  - the flaw believing the truth: the AI's two tests that some balls are missed, and R10's;
  - the true pull at three tenths: the AI's two tests that every ball comes back, and two more;
  - the lag never acting: the flaw's test, the AI's lag test, and R10's.

## Step 3: Lens's shell half (E.1, E.3)

### What was built

- **Icon:** `◎`, pale green. The font holds the glyph (U+25CE, read from its character map); it is drawn in the HUD in the screenshots.
- **Cue:** three quick notes closing in on the last from above and below.
- **`FloorLensesPlugin`:** makes the floor hold exactly the lenses of the flight the ball is in. It spawns each lens the floor lacks and despawns each the flight no longer has. With Lens out of play, or no flight, the floor is empty.
- **The grow-in:** a new lens grows from a twentieth of its size to the whole in a quarter second, on `EaseFunction::CubicOut`: fast at first, never shrinking.
- **The look:** a disc barely lighter than the floor, a ring at its rim, and its centre line across it, the brightest part. The ring and the line are the lens's children.
- **Where and look are apart,** as for Wind, Spin, and Fog: the plugin touches only the lenses it spawns and is tested headless; meshes and colours are added by the shell half's view.

### Contract

- **What is drawn is what pulls.** The plugin reads `Lens::placed` with the flight's start and the rally's seed, as the core's pull does.
- **A lens changes only where a flight starts or a rally ends** (spec 2.4.3): a flight's start does not change while it lasts, and a rally's lenses only grow in number.
- **The fog does not touch a lens.** `FogBandPlugin` writes the visibility of balls and trail ghosts only, and a lens is drawn above the fog's layer.
- **A lens that leaves takes its ring and line with it,** by the hierarchy.

### Decisions taken

- **A new draw layer, `layer::LENS`,** above the fog and the centre line, under Wind's arrows. The ring and the line sit a little above the disc: all three are opaque, which the depth decides.
- **All three colours are under 1.0 in linear light:** a lens is furniture and does not glow.
- **A newborn lens is small, not of no size:** a scale of zero has no inverse.
- **The lenses grow on the frame clock,** so one that appears as play pauses finishes growing through the pause.

### Done-conditions

| Condition | Evidence |
|---|---|
| E.1: API check before code | See "Bevy API". |
| Spec 2.6.5 and R1: the lenses of the flight, where the core has them | `the_floor_shows_every_lens_of_the_flight_where_the_core_has_it`. |
| R1: the cause on screen before the consequence | `a_lens_comes_with_the_return_that_brings_it_before_the_ball_reaches_it`: the flight before crosses an empty floor; the return puts the lens and its copy there in the frame of the hit, ahead of the ball. |
| Spec 2.2.9, the lens's half (moved from plan 7.1) | `a_lens_inside_the_fog_band_stays_visible_while_the_ball_there_is_hidden`: a lens wholly inside the full band is on the floor and not hidden in each of more than thirty frames the ball is hidden. |
| R9 on screen | `the_floor_is_empty_in_the_countdown_and_under_a_flight_with_no_lens`. |
| R11: nothing flashes | `a_new_lens_grows_to_its_size_without_shrinking_and_holds_it`; `the_lenses_hold_through_the_rally_and_go_at_the_countdown`. A lens changes once, over a quarter second. The review item at milestone close stays. |
| Spec 2.3.3, 2.7.1: icon and cue | `lens_states_an_icon_a_join_cue_and_a_view`; the generic cue and glyph tests now cover Lens. |
| The lenses live with their match | `leaving_the_match_takes_the_lenses_with_it`; `without_lens_in_play_the_floor_stays_empty`. |
| E.3: visible in `just run` | Not seen by the user yet (step 6). Claude saw it in a throwaway binary's screenshots (below). |

### Red first, and where it was not

- **Six of the eight tests of `pong/tests/lens.rs` were red by assertion** against a plugin that did nothing. The two that ask for an empty floor passed against it.
- **`lens_states_an_icon_a_join_cue_and_a_view`** was red by assertion while the view was `None`.
- **The overlay's key line in two tests** was red from the moment the variant existed.
- **The tests were corrected once before the plugin was built:** a serve is launched a step or two into play, so they wait for the lenses to lie before timing the grow-in.
- **Not red first:** the two unit tests of the grow-in's curve, written with it.
- **The meshes and colours have no test.** They are judged by eye.

### What Claude saw

A throwaway example binary booted the shell as `main.rs` does, without sound, saves, the keyboard and focus ports, or the overlay. It forced effects between two three-star computers at thirty returns, saved six screenshots of its window through Bevy, and quit. It is deleted; nothing of it is in the diff.

- **Seen, Lens beside Speed:** three lenses on the left and their copies on the right, mirrored across the centre line; each a dim green disc, ringed, with a brighter line across its middle; `◎` in the HUD; the ball and its trail drawn over the floor.
- **Seen, with Wind and Fog:** the lenses inside the fog's band in plain sight, drawn over the mist; Wind's arrows drawn over the lenses; in one frame no ball in sight while the lenses in the band showed.
- **Not seen:** a lens growing: the first shot came after the quarter second. A ball inside a lens, or its path bending. The join cue was not heard. Motion: screenshots are stills.
- **The window opened on the user's desktop twice,** for about ten seconds each, after the user said yes.
- **A first run opened no window:** the example's first build, with the `dev` feature, outlasted the run's time limit.

## Step 4: into the bag, and the harness (E.4, E.7)

`tuning.ron`: Chaos's bag is `[shrink, wind, spin, fog, lens]`.

### The held count

**The dial test failed when Lens joined the bag, as the proposal's third ruling foresaw.**

- The lag was built to the letter: a star reckons with the lenses of `unseen_flights` returns ago.
- Under a held count every flight starts at the same return. At a held 5, where the second lens arrives, two stars' lag of 2 looks back to return 3: they never saw the second lens.
- Two-star rallies read 4.3 hits at 5 returns and 10.1 from 10 on. The test saw rallies grow from one rung to the next, and failed.
- With the lag switched off in the tuning, as a held count that is a steady state would read: none ended, 14.1, 10.1. The test passed.
- **The user ruled the capability in.** `FlightStart` has a field, `returns_held`; `ActiveEffects::start_flight` sets it while the sandbox holds a count; Lens's flaw looks back no flights when it is set.
- The same holds in the overlay's sandbox: at a held count the computer sees every lens on the floor.
- The lag is tested where it acts, in a rally that counts: the AI's tests begin the ball's flight at the return wanted.
- The test's ladder, allowance, and assertion are as they were.

Built test-first: `a_flight_says_whether_its_return_count_is_held` (the base), `under_a_held_count_no_lens_is_too_new_for_any_star_to_see` (Lens), `the_serve_holds_no_count` (kernel). The first two were red by assertion with the field there and unread. The third passed once the field existed.

### What the bag of five moved

- **Every seeded Chaos number,** since a bigger bag is shuffled differently.
- **Classic's did not:** its six harness lines and its win counts are phase 7's, figure for figure.
- **The stack's ladder did not trip:** two stars read 3.2, 2.0, 1.8 at the seeds they have. No rung was given more.
- **Chaos ●●● has a bench of two:** three of five effects are in play at full breadth.
- **No fixture named the roster.** The overlay's key line in two tests did, as foreseen.

### Measured (`just ai-report`, matches to five points, hits per rally; no rally unended)

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 1 v 1 | 28 rallies, mean 2.4, longest 11 | 33 rallies, mean 2.7, longest 15 |
| 1 v 2 | 20 rallies, mean 5.7, longest 44 | 25 rallies, mean 3.4, longest 13 |
| 1 v 3 | 20 rallies, mean 6.4, longest 26 | 22 rallies, mean 3.3, longest 11 |
| 2 v 2 | 32 rallies, mean 33.5, longest 65 | 29 rallies, mean 16.6, longest 94 |
| 2 v 3 | 22 rallies, mean 30.1, longest 46 | 23 rallies, mean 16.3, longest 60 |
| 3 v 3 | 30 rallies, mean 41.7, longest 143 | 29 rallies, mean 49.1, longest 115 |

More stars won 8 matches to 0 in each of the three pairings, in each mode (spec 2.5.5).
Spec 2.5.1 holds: every rally ended in every pairing.

### Lens's R3 measure (E.4)

Lens alone, no speed ramp beside it, matches to five points, hit limit 200:

| Held count | Lenses drawn | 1 v 1, 8 seeds | 2 v 2, 2 seeds |
|---|---|---|---|
| 0 | 0 | 60 rallies, mean 2.7 | none ended (2 of 2) |
| 5 | 2 | 52 rallies, mean 1.7 | 18 rallies, mean 14.1 |
| 10 and up | 3 | 58 rallies, mean 1.9 | 14 rallies, mean 10.1 |

- **R3 holds for Lens against one and two stars,** by the test's own measure. One star's 1.7 then 1.9 is inside the allowance of half a hit.
- **The dial has three grades,** not a slope: the count is whole.
- **Between computers Lens is the gentlest of the five alone.** At full dial two-star rallies last 10.1 hits among lenses, against 7.3 under Spin held, 3.5 in Wind, 1.6 under Shrink, 0.9 in Fog.
- **These rungs measure the share alone.** Under a held count no lens is new.
- **Against three stars it is not measured.**

### R3 for the stack (spec 2.5.2)

The ladder runs to the cap, so it is the first three of the bag and Spin; Fog and Lens are not on it.

| Level | 1 v 1 | 2 v 2 | 3 v 3 |
|---|---|---|---|
| Speed at 0 returns | 60 rallies, mean 2.7 | none ended | none ended |
| Speed, Shrink at 15 | 57 rallies, mean 0.6 | 87 rallies, mean 3.2 | none ended |
| Speed, Shrink, Wind at 30 | 62 rallies, mean 0.7 | 94 rallies, mean 2.0 | 15 rallies, mean 51.3, longest 164 |
| Speed, Shrink, Wind, Spin at 45 | 55 rallies, mean 0.8 | 81 rallies, mean 1.8 | 15 rallies, mean 22.5, longest 68 |

### R10 (E.7)

`a_ball_two_stars_return_beside_a_new_lens_scores_when_sent_through_it`: in a rally that counts, on the flight its first lens appears, two stars as tuned do not see it yet. A level ball at the serve speed sent beside the lens comes back twenty times of twenty, as the same ball does over an empty floor. Sent through the lens, halfway from its centre line to its rim, it scores.

- **This is R10 in spirit as well as letter.** Which ball scores is the hitter's aim to say, and no draw's.
- Not red first: it pins what Lens gave. Its halves beside the lens and over the empty floor are its controls, and two of the deliberate breaks of step 2 turn it red.

### The soaks

All five pass. With five effects in the bag the subset tests divide their steps among five single effects, ten pairs, and ten triples.
Their printed lines were not read: no recipe prints them.

### Running times

Development machine, inside the full parallel run of `just dev-check`: fifteen heavy tests now, 4.1 to 13.8 s each, against the 30 s limit. Phase 7 had fourteen, at 3.2 to 10.3 s straight after a rebuild.

- The new one: the fifth bag effect's dial test, 12.6 s.
- The stack's test: 13.8 s, the slowest.
- The five soaks: 8.5 to 9.5 s.

## Step 5: the record step (P.6, E.5, plan 8.2)

### Recorded

- **Spec:** 2.2.4 (the held count's flag), 2.2.9 (the lens's half shown), 2.3.8 (the measure below), 2.3.9 (the count form and the item rule built), 2.3.12 (Lens's numbers), 2.5.2 (Lens's measure), 2.6.5 (Lens as built), 2.9.1 (a held count is a steady state). Each amended 2026-11-20.
- **Plan:** 8.1 and 8.2 amended.
- **ADR 0011:** amended 2026-11-20: the flight start says whether its count is held; a field one effect alone needs lives in its module; an item effect lays its items from the rally's seed; its dial is the count form.
- **`.claude/rules/pong_core.md`** and **`.claude/rules/tests.md`:** the same, as rules; a lag is tested in a rally that counts.
- **READMEs:** `features/README.md` (Lens's module, its flaw, the tuning checks, the held count), `kernel/README.md` (`ItemCount`), `testing/README.md` (the soaks' times).

### The diff against the touch list (E.5, plan 8.2, spec 2.3.8)

The phase's diff against `main`: 22 code files, 3 of them new and untracked when counted, beside documents and rules.

| Where | Files | Why |
|---|---|---|
| Inside the list | `effects/lens.rs` in each crate; `domain.rs` (the variant, `ALL`, its guard tests); one line in each crate's registration match; the tuning section (`tuning.ron`, `file.rs`, `tuning/mod.rs`) | Lens itself. |
| Outside, paid once | `kernel/ramp.rs`; `tuning/mod.rs` and `file.rs`; `views/mod.rs` | The count form and the item rule, owed since phase 2; `PerStar<T>`; the draw layer `layer::LENS`. |
| Outside, paid once, for the held count | `kernel/flight.rs`; `effects/mod.rs`; `physics/mod.rs`; the test modules of `speed.rs`, `shrink.rs`, `wind.rs`, `spin.rs`, `fog.rs`, and of the AI | The field, where it is set, and its value stated wherever a flight start is built. |
| Outside, tests and fixtures | The AI's test module; the paddle's cramped-arena fixture; the overlay's key line in two tests; `pong/tests/lens.rs` | Lens's done-conditions in the AI; the lens fitted to a small arena; the view's own tests. |
| Documents and rules | Three READMEs, two rules files, the spec, the plan, ADR 0011, this report | |

- **The list's negative half broke in one place, by one line.** `physics/mod.rs` states `returns_held: false` where a return starts a flight. No behaviour of physics changed.
- **It held for the rest:** the AI's file changed in its test module only; rules, escalation, sanity, and `pong/src/ui` not at all.
- **The kernel changed twice,** both for the base and neither for Lens's field, which is in Lens's module.
- **What the next effect will also touch outside the list:** the two tests that spell out the overlay's key line; its own draw layer if it has a view on the floor; the cramped-arena fixture if it has a size.
- **What it will not pay again:** the count form, the per-star whole number, the held count's flag. Blocks' lag reads the same flag.

## After the hand check (2026-11-22)

The user committed the build (`f68fc58`) and reported two things from play.

### The lenses jumped through the point pause

**Reported:** "when a point is scored, the lenses visually teleport for a few frames (until the countdown starts)."

- **Cause.** The base draws the next rally's seed on the transition from play to a point, and the last flight stays on the ball until the countdown clears it. The view laid the old flight's lenses from `ActiveEffects::rally_seed()`, by then the next rally's.
- **Why no test saw it.** `the_lenses_hold_through_the_rally_and_go_at_the_countdown` ran from the point straight to the countdown and looked at no frame between.
- **Fix, in the base, where the fact lives.** A `Flight` keeps the seed of the rally it began in: `Flight::in_rally`, set by `ActiveEffects::start_flight`, read through `Flight::rally()`. The view reads the flight's own seed. Physics is untouched.
- **Simulation unaffected.** No ball moves in a point pause, and the flight's pull was built when it started.
- **Tests, red first:** `through_the_point_pause_the_lenses_stay_where_the_rally_laid_them` (shell; red by assertion against the old read, the lenses elsewhere in the first frame of the pause); `the_balls_flight_keeps_its_rallys_seed_through_the_point_pause` (base) and `a_flight_remembers_the_rally_it_began_in`, `a_believed_flight_is_of_the_same_rally` (kernel), red by compile failure.
- **Recorded:** spec 2.2.4 and 2.6.5, ADR 0011, the kernel's and the features' READMEs, amended 2026-11-22.
- **Outside the touch list, paid once:** `kernel/flight.rs` and `effects/mod.rs`. Blocks' view will read the same seed.
- **Gate:** `just dev-check` green on 2026-11-22: 740 tests, 4 more.

### A lens does less to a steep ball

**Reported:** "When a ball's trajectory is relatively horizontal, the lens works pretty well, but the more vertical it is, the less effect it produces."

Put to the user as a proposal on 2026-11-22. The ruling, that day: "Implement Option B."

- **Cause.** Two things, both from the spec's lateral-only forces. A steep ball crossed the centre line inside the disc, so it was pulled one way and then back. And a lateral pull changes a path's slope: a steep path needs far more of it for the same turn, and spends less width in the disc.
- **Option B, built.** The pull is toward the centre, still lateral: to the side on which the ball's course, carried straight on, would pass the centre, by the distance at which it would pass. A level ball is pulled as before.
- **Not built:** option C, a true well with a pull along the arena too. It would change the force channel, prediction, and termination, and wants the horizontal floor of plan phase 9.
- **The strength is sized by measure now.** `LARGEST_SLOPE_CHANGE` is 0.511 of `focus · R²`: a straight level path's `√3 / 4`, and 18 parts in 100 over it for the curved paths of a lens at 30 degrees. The derivation of step 2 no longer holds: a path turned toward the centre gains over a straight one, by 5 in 100 at 5 degrees and 18 in 100 at 30.
- **So the bend has a cap.** Validation refuses `max_bend_degrees` above 30, where the measure ends (`TuningError::LensBendAboveTheLargest`). It took a bend up to a right angle before.
- **The look:** a dot at the centre where the centre line was. Seen in one more screenshot run, the third of the two or three the user allowed.

Measured in a throwaway simulation, at 150 units a second and the embedded numbers: how far the ball's crossing moves 60 units past the lens, for the best course and the average course at each approach angle. The paddle reaches 8.

| Approach angle | Toward the line: best, average | Toward the centre: best, average |
|---|---|---|
| 0° | 15.1, 9.4 | 14.2, 8.9 |
| 30° | 12.4, 7.2 | 12.7, 7.6 |
| 45° | 8.1, 4.8 | 10.2, 6.1 |
| 60° | 3.9, 2.4 | 7.0, 4.3 |

- **The gain is smaller than the proposal's table said.** That table was of the pull before it was sized: a quarter more at 45 degrees and four fifths more at 60, where the proposal showed a half more and twice. The proposal named a tenth less strength; sizing for the cap of 30 degrees cost 15 parts in 100.
- **A level ball is bent a little less than before:** 13.7 degrees at most where the tuned angle is 15, against 14.1.
- **It still falls with steepness.** That is the lateral pull's own limit, and option C's to lift.

Tests, red first against the pull toward the line:

- `a_steep_ball_is_turned_one_way_from_rim_to_rim`;
- `a_steep_ball_is_bent_two_fifths_as_far_as_a_level_one`;
- `a_lens_pulls_the_ball_toward_its_centre_and_nowhere_outside_itself`, which replaces the test of the centre line and adds that a ball headed at the centre is not pulled.

Tests, red by compile failure: `a_lens_bend_above_the_largest_its_pull_is_sized_for_is_rejected` (tuning).

Changed with them:

- `one_lens_bends_no_crossing_more_than_the_tuned_angle` runs at 5 degrees, the embedded angle, and 30; its entries cover every course that meets the disc at each slope; its lower mark is four fifths of the angle, where it was nine tenths.
- `the_copy_pulls_as_the_drawn_lens_does_mirrored` also flies a steep ball the mirrored way.
- The tuning's range test lost its row for a bend of 90 degrees, now the cap's to refuse.
- **Checked against a deliberate break, since removed:** with the share at 0.47 the bound's test fails at 30 degrees, at 30.9.

Measured again (`just ai-report`); Classic's lines and the stack's ladder are unchanged:

| Pairing | Chaos, cap 3 |
|---|---|
| 1 v 1 | 34 rallies, mean 2.1, longest 11 |
| 1 v 2 | 25 rallies, mean 4.2, longest 13 |
| 1 v 3 | 23 rallies, mean 3.4, longest 11 |
| 2 v 2 | 31 rallies, mean 14.1, longest 94 |
| 2 v 3 | 23 rallies, mean 14.9, longest 60 |
| 3 v 3 | 29 rallies, mean 59.6, longest 159 |

| Held count | Lenses drawn | 1 v 1, 8 seeds | 2 v 2, 2 seeds |
|---|---|---|---|
| 0 | 0 | 60 rallies, mean 2.7 | none ended (2 of 2) |
| 5 | 2 | 59 rallies, mean 1.9 | 17 rallies, mean 7.5 |
| 10 and up | 3 | 58 rallies, mean 1.3 | 16 rallies, mean 7.4 |

More stars won 8 matches to 0 in every pairing, and every rally ended. Between computers Lens bites harder than it did: two-star rallies at full dial last 7.4 hits, where they lasted 10.1.

- **Inside the touch list:** Lens's two modules, its tuning section, the variant's comment. One line of `views/mod.rs`, a comment.
- **Bevy API:** none new to the repository; the dot is a `Circle`, as the disc is.
- **Recorded:** spec 2.6.5 and 2.5.2 amended 2026-11-22; the features' README.
- **Gate:** `just dev-check` green on 2026-11-22: 743 tests, 3 more.

## Test changes

522 core tests and 214 shell tests pass, 736 in all; 686 existed before this phase. 50 were added: 39 in the core (6 of the count form, 7 of the tuning, 18 of Lens, 6 of the AI, 1 of the effects base, 1 of the kernel's flight) and 11 in the shell (8 in `pong/tests/lens.rs`, 1 of the shell half, 2 of the grow-in).

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or the base:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Lens`.
- The overlay's two tests of the sandbox key line: it now reads `1 Speed, 2 Shrink, 3 Wind, 4 Spin, 5 Fog, 6 Lens`.
- `VALID` in the tuning tests gained Lens's section.
- `a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move` (paddle): its cramped tuning shrinks the lens's radius.
- Every earlier test that builds a `FlightStart` states `returns_held: false`: 20 places.
- `launch_at_ai_off_a_paddle_moving` (AI) now calls `launch_at_ai_as_the_return` with the rally's first return; every test that uses it is the same.

## Bevy API

Steps 1, 2, and 4 wrote no API new to the repository.

Step 3's placing (`FloorLensesPlugin`) and look wrote three. `bevy-api-check` ran once against the pinned 0.20.1 source, before the code. Claude read no Bevy source directly.

- **Easing** is `bevy::curve::{Curve, EaseFunction}`, behind the feature `bevy_curve`, which the shell's `2d` feature turns on; the core's features do not. `bevy::math::curve` does not exist at 0.20.1. `EaseFunction` is itself a `Curve<f32>` over zero to one: `EaseFunction::CubicOut.sample_clamped(t)`. `EaseFunction::eval` is private. It is pure math and needs no plugin, so it runs headless.
- **`bevy::shape::Annulus`:** `Annulus::new(inner_radius, outer_radius)`, inner first; `meshes.add` takes it.
- **`EntityCommands::despawn`** despawns `ChildOf` children too: `Children` is a linked relationship target. There is no `despawn_recursive` at 0.20.1.
- **`materials.add(Color)`** with alpha one is opaque. Opaque 2D meshes are binned, not sorted, and ordered by depth: by the global z, a child's being its parent's plus its own. Two at the same z have no order, hence the heights of the ring and the line.
- **Already in use here:** `Mesh2d`, `MeshMaterial2d`, `ColorMaterial`, `Circle`, `Rectangle`, `ChildOf`, `DespawnOnExit`, `Time`, the screenshot API of the throwaway binary.
- **Not verified by the agent:** the 2D camera's near and far range. The running game answered it: all three parts of a lens are drawn.

## Process

Each is a departure from `CLAUDE.md` or the test rules, stated here as they ask.

- **Filtered test runs went through `cargo nextest run` directly,** with a package and a filter, during the red and green cycles. The recipes take no filter. The gate below is the recipe.
- **The throwaway binary was built and run with `cargo build` and `cargo run --example`,** with the `dev` feature and the recipe's asset root. No recipe runs an example.
- **Before the proposal, the field was tried in a throwaway Python script outside the repository,** against the game's step and speeds: the bound held to a hundredth at radius 8.

## Gate

`just dev-check` green on 2026-11-20 with steps 1 to 4 and the held count: format, layering, lint, 736 tests, doctests. The record step, after it, changed documents and rules only.
CI: not run by Claude. The user's, with the commits.

## Open at phase close

- **Step 6:** the hand check in `just run` is begun (2026-11-22, two findings above); the session with the children and the ruling are open: keep, retune once, or drop (E.6, spec 2.10.1, 2.10.2).
- **A lens still does less to a steep ball,** though more than it did. Whether the centre's pull is enough is the session's to say; a true well is the cure not taken.
- **`max_bend_degrees` is still 15,** and a level ball is bent a little less at it than before the change.
- **Every number of Lens's section is Claude's starting number,** untried by a hand: the arrivals at 3, 5, 7, the radius, the bend of 15 degrees, the run, the lags, the shares.
- **Lens is gentle between computers.** Whether a person finds it so is the session's to say; `max_bend_degrees` and the shares are one line each.
- **Not shown by test:** that Lens alone shortens three-star rallies.
- **R11 for the grow-in:** the review item at milestone close (spec 2.7.4).
- **Rotation at full breadth** now has a bench of two. Plan 11.1 chooses the interval from play.
- **Carried, still open:** the computer as a deliberate spinner (plan 11.3); Wind against Spin told apart; the catch-up hypothesis in the playtest log.
