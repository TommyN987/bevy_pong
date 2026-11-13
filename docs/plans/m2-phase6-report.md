# M2 phase 6 report: Spin

Started 2026-11-13. Branch `feat/spin`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-13: steps 1 to 4 of the proposal the user approved that day (plan 6.1; E.1 to E.4, E.7).
Recorded 2026-11-13: step 5 (P.6, E.5). CI has not run on this build: the push is the user's.
Hand check 2026-11-13, the user's: done, and followed by a tryout retune of Spin (below).
Open: the rest of step 6, the session (E.6, plan 6.2).

## The proposal's steps

| Step | What | State |
|---|---|---|
| 1 | The sandbox holds a hit motion | Built. |
| 2 | Spin's core half (E.2) | Built. |
| 3 | Spin's shell half (E.1, E.3) | Built, tested headless, and seen by the user in `just run`. |
| 4 | Into the bag, and the harness (E.4, E.7) | Built. Measured at the starting numbers, and again at the tryout's. |
| 5 | Record (P.6, E.5) | Done. |
| 6 | Hand check, session, ruling (E.6, plan 6.2) | Hand check done. The session, its ruling, and the hypothesis are open; the user's. |

## Deviations

All three were proposed before implementation and approved by the user on 2026-11-13.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.6.4, "AI flaw: reads the spin late" | The flaw is the second form of 2.3.12 alone: each star has read a tuned share of the true spin when it commits its aim. It is not an added delay. | Spec 2.6.4 and 2.3.12, and plan 6.1, amended 2026-11-13. |
| spec 2.9.1 | The sandbox also holds a hit motion: the paddle motion every return is told. For a harness; no overlay key writes it. | Spec 2.9.1 and 2.5.2, and plan 6.1, amended 2026-11-13. |
| spec 2.6.4, "a tuned maximum" | Validation refuses a `max_drift` at or above half the playable height, as Wind's. The spec named no bound for Spin. | Spec 2.6.4 amended 2026-11-13. |

## Rulings by the user

2026-11-13, after the hand check:

- **More spin, for a tryout.** "I've played a little with the bots, and the spin was barely noticeable in the smaller and mid-length rallies. Then I've played a two-star in the sandbox with just Spin turned on, at level 20, and it feels like there should be more spin throughout." The numbers are in "The hand check and the tryout".
- **The computer as a spinner is a MAY** in Spin's section of the spec, "to be revisited when every effect is implemented and playtested, before the milestone close". Spec 2.6.4 has the sub-item and plan 11.3 carries it. This settles ruling 4 below.

2026-11-13: "Approved as proposed. You drive." The proposal's six rulings, each as recommended:

1. **Roles.** Claude drives; the user reviews.
2. **The flaw.** The believed share alone.
3. **The harness.** The sandbox holds a hit motion (step 1).
4. **The computer as a spinner.** Not in this phase. Open then; since the hand check, a MAY (above).
5. **The view's attachment.** The arrowhead is a child entity of the ball, through `bevy-api-check` first (step 3).
6. **Starting numbers.** Dial from return 0, full at 12, `max_drift` 16, shares 0.3, 0.6, 0.85. The bound of the third deviation. Replaced for a tryout after the hand check (above).

## Why the proposal took this shape

Two facts, each first by arithmetic from the tuning numbers. The first is now measured (step 4).

- **The computer almost never spins.** It reaches its aim and stands still, so its paddle moves at the hit only when it is late. At the serve speed a crossing takes 1.8 s and the slowest star needs 1.2 s at most, so it is never late. The standing dial test plays an effect alone with no speed ramp, and would see no spun ball.
- **Added reaction delay does nothing at low speed,** for the same reason. A flaw of delay alone would act only near the speed cap. Not measured.

## Step 1: the sandbox holds a hit motion

### What was built

- **`Sandbox::hold_hit_motion`** and `held_hit_motion`: the paddle motion every return starts its flight with, or `None` for each paddle's own.
- **`ActiveEffects::start_flight`** applies it, where it applies the held count: every flight start still passes through one place.
- **`hold_what_the_sandbox_holds`**, the system that was `hold_the_sandbox_count`: it copies both, each only on a change.
- **`Forcing::hit_motion`** in the AI-match harness, built in step 4 with the dial ladder, its first user.

### Contract

- **A held hit motion is forcing.** It marks the match touched, counts in `is_forcing`, and is let go by `release` and with the match.
- **A serve is no hit.** A held motion gives it none: `FlightStart::SERVE` stays as it is.
- **It takes hold at the next flight start,** as the held count does (spec 2.4.3).

### Tests

- Red first, by assertion, with the port's methods present and not yet applied: `a_held_hit_motion_is_the_motion_every_return_starts_its_flight_with` (a still paddle was told as still).
- Passing before and after, since it pins what the implementation must keep: `a_held_hit_motion_leaves_a_serve_without_a_hit`.
- Three earlier tests of the sandbox now cover the held motion too. Red only by compile failure: once the port could hold and release a value, they passed.
- The harness's field: red by assertion with the field present and not yet applied, `a_forced_hit_motion_is_held_from_the_first_rally`.

## Step 2: Spin's core half (plan 6.1, E.2)

### What was built

- **Tuning section** `effects.spin`: `starts_at_return`, `full_at_return`, `max_drift`, and `believed_share` per star. Refused: a dial full before it starts; a negative drift or share; `max_drift` at or above half the playable height.
- **`EffectId::Spin`**, saved as `spin`, and one registration line per crate.
- **`effects/spin.rs`**: a return from a paddle that moved in the step of the hit contributes the kernel's `Drift` toward the paddle's motion, by the dial's amount. A still paddle's return and a serve contribute no field.
- **`Spin::drift(start)`**: the signed drift of a flight, a pure function of its start. The field and the shell's view both read it, so what is shown is what curves.
- **The flaw** is the second form alone: an AI believes its star's share of the true drift, in the true direction.
- **It needs no rally seed:** the hitter chooses at every hit.

### Decisions taken in implementation

- **One private type for a believed share.** Wind's tuning and Spin's each held three shares and the same lookup; both now hold a `BelievedShare`. A refactor on green: the public API and Wind's tests are as they were.
- **The sandbox's key 4 toggles Spin:** the overlay's keys follow `EffectId::ALL`.
- **The AI's tests begin the spun flight on the ball directly,** through `ActiveEffects::start_flight` with a return's start. Their launches are placed, not struck, so no paddle hit exists to hold a motion for.

### Done-conditions

| Condition | Evidence |
|---|---|
| Spec 2.6.4: a moving-paddle hit curves the ball that way and a still one does not | `a_moving_paddle_hit_curves_the_ball_that_way_and_a_still_one_does_not`, through the systems, the paddle driven by its seat's intent: 8 to 10.5 units toward the motion over the 110 or so units flown, nothing from a still paddle. `a_moving_paddles_return_curves_the_way_the_paddle_moved_and_a_still_ones_does_not` in the effect. |
| Spec 2.6.4: at dial zero neither does | `at_dial_zero_neither_a_moving_paddle_hit_nor_a_still_one_curves_the_ball`; `at_dial_zero_no_return_is_spun`. |
| Spec 2.6.4: the dial is the drift over one crossing | `the_drift_at_the_far_paddle_is_the_dial_at_the_serve_speed_and_at_the_cap`: 16 units both ways, at 80 and at 250, within one step's share. |
| Spec 2.2.1: prediction equals simulation | Inside the first test above: the crossing flown equals the kernel's prediction from the flight the return began, to 0.01. |
| R9: graded, absent at zero | `the_curve_never_eases_as_the_rally_grows`; `the_drift_of_a_flight_is_the_dials_amount_signed_the_way_the_paddle_moved`. |
| R4: terminates | `spin_never_changes_the_balls_horizontal_speed`: a crossing takes its unspun time, a wall bounce included. |
| R5 and spec 2.3.12: the zero-flaw AI returns every ball; a raised flaw misses some | `an_ai_that_reads_all_of_the_spin_and_has_no_handicap_returns_every_spun_ball` (200 launches); `an_ai_that_reads_too_little_of_the_spin_misses_some_spun_balls`. |
| R2: skill decides | `the_same_misreading_ai_returns_every_ball_that_is_not_spun`: the same 40 balls come back off a still paddle and, at dial zero, off a moving one. |
| The flaw per star | `each_star_believes_its_share_of_the_true_spin_and_is_charged_nothing_else`; `of_a_flight_that_is_not_spun_every_star_believes_in_no_spin`. |

The amounts in this table are the starting numbers'. The same tests pin the tryout's now: 16 to 19.5 units over the units flown, and 30 over a crossing.

### Red first, and where it was not

- **Tuning:** red by compile failure, then green: five tests.
- **Spin:** red by compile failure. Then, with a `Spin` that spun nothing, red by assertion at `a_moving_paddles_return_curves_the_way_the_paddle_moved_and_a_still_ones_does_not`. The run stops at its first failure, so the other tests were not seen red under that stub.
- **Not red first:** the AI's three Spin tests. They pin what the kernel, the base, and Spin already gave.
- **Checked against two deliberate breaks, since removed.** With every AI believing the truth, `..reads_too_little..misses_some_spun_balls` failed, and the per-star test with it. With every AI believing in no spin, `..reads_all_of_the_spin..returns_every_spun_ball` failed.

## Step 3: Spin's shell half (E.1, E.3)

### What was built

- **Icon:** `↻`, magenta. The font's charset holds the glyph (U+21BB, inside U+2190 to U+2311).
- **Cue:** three quick plain notes, up, back, and up again, then a higher one held. Wind's is two rising notes; Shrink's three falling brass ones.
- **View:** one bright magenta arrowhead riding on the ball for the whole of a spun flight, beside it on the side the ball will curve to and pointing that way. About half a ball across while the dial has barely moved, about a ball across at full dial.
- **The mark is the ball's child entity.** It follows the ball's drawn position, hides when the ball is hidden, and is despawned with it.
- **Where and look are apart,** as for Wind: `SpinMarkPlugin` gives each ball its mark and touches only the mark's parent, `Transform`, and `Visibility`, and is tested headless. The mesh and the material are added by the shell half's view, with the render assets.

### Decisions taken

- **The mark reads `Spin::drift` of the start the ball's flight began at,** the function the core's field is built from. `off_a_moving_paddle_the_mark_shows_and_points_the_way_the_ball_then_curves` ties the two through the ball itself.
- **It shows only while Spin is in play.** A flight's start alone would mark a moving-paddle return in Classic.
- **`Visibility::Inherited`, not `Visible`,** when shown: a hidden ball hides its mark. Fog (plan phase 7) hides the ball; this is what will hide the mark with it.
- **Sizes are in ball radii.** The ball's transform is scaled by its radius, and a child inherits the scale. The mark's draw order is the ball's plus a tenth, so `views::layer` gained no entry.
- **The mark is spawned on `Added<Ball>`,** hidden, and has no scope of its own.
- **Nothing at the paddle, and no sound per hit.** As proposed: the mark on the ball is the whole view.

### Done-conditions

| Condition | Evidence |
|---|---|
| E.1: API check before code | See "Bevy API". |
| R1: the cause on screen before the consequence | `off_a_moving_paddle_the_mark_shows_and_points_the_way_the_ball_then_curves`: the mark is there in the frame of the hit, both ways, and forty frames on the ball has been carried that way. `off_a_still_paddle_no_mark_shows`; `the_ball_carries_one_mark_hidden_until_a_return_is_spun` (a serve); `without_spin_in_play_no_mark_shows_off_a_moving_paddle`. |
| R9 on screen | `the_mark_is_larger_at_full_dial_than_early_in_the_rally`; `at_dial_zero_no_mark_shows_off_a_moving_paddle`. |
| R11: nothing flashes | `the_mark_holds_for_the_whole_flight_and_goes_with_the_rally`: it shows, pointing one way, in every frame of the flight. It changes only where a flight starts or a rally ends: at most once per crossing, and the fastest crossing takes 0.58 s. The review item at milestone close stays. |
| Spec 2.3.3, 2.7.1: icon and cue | `spin_states_an_icon_a_join_cue_and_a_view`; the generic cue and glyph tests now cover Spin. |
| The mark lives with its ball | `leaving_the_match_takes_the_mark_with_the_ball`. |
| E.3: visible in `just run` | Seen by the user on 2026-11-13: "Hand check done. Everything seems to be working." |

### Red first, and where it was not

- Red by compile failure, then green on the first run: the eight tests of `pong/tests/spin.rs`, `spin_states_an_icon_a_join_cue_and_a_view`, and the mark's size test.
- **Checked against two deliberate breaks, since removed.** With the mark always pointing up, the direction test and the whole-flight test failed. With Spin's being in play ignored, `without_spin_in_play_no_mark_shows_off_a_moving_paddle` failed.

## Step 4: into the bag, and the harness (E.4, E.7)

`tuning.ron`: Chaos's bag is `[shrink, wind, spin]`. Every condition below held at the starting numbers, and every number in this step is measured at them. The tryout's are in "The hand check and the tryout".

### What the bag of three moved

- **Every seeded Chaos number,** since a bigger bag is shuffled differently. The stack's bottom rung moved too, from 57 rallies of mean 2.4 to 63 of mean 2.7: the schedule draws its bag even under a forced set.
- **Classic's did not:** its six harness lines and its win counts are phase 5's, figure for figure.
- **Chaos ●●● reaches three bag effects for the first time.** With two in the bag, the third threshold was capped at two.

Tests that named the roster or the first bag effect, each kept on its assertion and made to ask instead:

- Escalation: `two_in_the_bag` and `shrink_alone_in_the_bag` are built by `with_chaos_listing`, which replaces Chaos's two lists whatever the embedded ones hold.
- Shell, HUD: three tests of `match_screens.rs` ask the core which bag effect is in play, where they named Shrink.
- Shell, overlay: the key-toggle test finds the scheduled effect's key from the tuning's bag, where it pressed Shrink's.

### Measured (`just ai-report`, matches to five points, hits per rally; no rally unended)

| Pairing | Classic | Chaos, cap 3 |
|---|---|---|
| 1 v 1 | 28 rallies, mean 2.4, longest 11 | 27 rallies, mean 2.3, longest 14 |
| 1 v 2 | 20 rallies, mean 5.7, longest 44 | 20 rallies, mean 3.8, longest 12 |
| 1 v 3 | 20 rallies, mean 6.4, longest 26 | 20 rallies, mean 4.2, longest 15 |
| 2 v 2 | 32 rallies, mean 33.5, longest 65 | 30 rallies, mean 17.0, longest 67 |
| 2 v 3 | 22 rallies, mean 30.1, longest 46 | 21 rallies, mean 16.8, longest 58 |
| 3 v 3 | 30 rallies, mean 41.7, longest 143 | 33 rallies, mean 58.4, longest 128 |

More stars won 8 matches to 0 in each of the three pairings, in each mode (spec 2.5.5).
Spec 2.5.1 holds: every rally ended in every pairing.

### Spin's R3 measure (E.4)

**Unheld, Spin alone does nothing between computers.** Put in the bag before the ladder held a motion, it played the same 63 one-star rallies of mean 2.7 at every rung, and `the_third_bag_effect_alone_shortens_rallies_as_its_dial_rises` failed. That is the first fact above, measured.

Spin alone, every return told as off a paddle moving up, no speed ramp beside it, matches to five points, hit limit 200:

| Held count | Drift | 1 v 1, 8 seeds | 2 v 2, 2 seeds |
|---|---|---|---|
| 0 | 0 | 63 rallies, mean 2.7 | none ended (2 of 2) |
| 5 | 6.7 | 62 rallies, mean 2.6 | 13 rallies, mean 25.4 |
| 10 | 13.3 | 63 rallies, mean 1.9 | 14 rallies, mean 6.1 |
| 15 | 16 | 61 rallies, mean 1.8 | 14 rallies, mean 6.0 |
| 20, 30 | 16 | mean 1.8 | mean 6.0 |

- **R3 holds for Spin against one and two stars, held.**
- **What the measure is:** the dial acting through the flaw when every return is spun one way. It is not how often a computer spins: unheld, never, at these speeds.
- **Every dial test holds the motion now.** Shrink and Wind do not read it: Wind's ladder printed the same lines held and unheld.
- **Against three stars it is not measured.** By arithmetic, as for Wind: their share of 0.85 leaves the aim 2.4 off at most, and with their error of 3 that is inside the whole paddle's reach of 8.

### R3 for the stack (spec 2.5.2)

Played unheld, as the game plays. The ladder has a fourth rung now:

| Level | 1 v 1 | 2 v 2 | 3 v 3 |
|---|---|---|---|
| Speed at 0 returns | 63 rallies, mean 2.7 | none ended | none ended |
| Speed, Shrink at 15 | 58 rallies, mean 0.6 | 18 rallies, mean 2.1 | none ended |
| Speed, Shrink, Wind at 30 | 55 rallies, mean 0.7 | 13 rallies, mean 1.7 | 14 rallies, mean 50.4, longest 146 |
| Speed, Shrink, Wind, Spin at 45 | 53 rallies, mean 0.7 | 15 rallies, mean 1.1 | 18 rallies, mean 18.4, longest 63 |

The fourth rung adds Spin and fifteen returns of speed together, 215 units/s against 170. How much of its shortening is Spin's was not separated.

### R10 (E.7)

`a_ball_three_stars_return_off_a_still_paddle_scores_off_one_moving_toward_the_wall`: three stars as tuned return a level ball at the cap from 27 off the centre when the paddle that sent it stood still. At full dial, off a paddle moving toward the near wall, the same ball curves out along the wall and scores; off a paddle moving the other way it comes back. With the dial at zero it comes back whatever the paddle did.

Not red first: it pins what Spin gave. Its dial-at-zero half is its control.

### The soaks

All five pass, and the test of every three effects has a subset for the first time: Shrink, Wind, and Spin at full dial beside Speed.
Their printed lines were not read: no recipe prints them.

### Running times

Development machine, inside the full parallel run of `just test`, warm: thirteen heavy tests now, 3.2 to 11.0 s each, against the 30 s limit. Phase 5 had eleven, at 3.2 to 8.8 s.
The two new ones: the third bag effect's dial test 9.7 s, the soak of every three effects 9.6 s.
On one cold run under load, straight after a rebuild, the same tests took 8.3 to 17.0 s.

## Step 5: the record step (P.6, E.5)

### Recorded

- **Spec:** 2.3.8 (the comparison below), 2.3.12 (Spin's per-star numbers), 2.5.2 (the held hit motion in the dial tests; the stack's fourth rung), 2.6.4 (Spin as built; the flaw; the bound; the computer does not spin on purpose), 2.9.1 (the held hit motion). Each amended 2026-11-13.
- **Plan:** 6.1 amended.
- **ADR 0011:** not amended. Nothing in the contract changed: Spin is built on the hook, the field, and the flaw the ADR already describes.
- **`.claude/rules/tests.md`:** the dial tests hold a hit motion; the stack's test holds none.
- **READMEs:** `features/README.md` (Spin's module, the sandbox, the tuning checks), `testing/README.md` (`Forcing`, the soak's times).

### The diff against the touch list (E.5, spec 2.3.8)

The phase's diff against `main`: 20 files outside `docs/`, one of them new and untracked when counted (`pong/tests/spin.rs`).

| Where | Files | Why |
|---|---|---|
| Inside the list | `effects/spin.rs` in each crate; `domain.rs` (the variant, `ALL`, its guard tests); one line in each crate's registration match; the tuning section (`tuning.ron`, `file.rs`, `tuning/mod.rs`) | Spin itself. |
| Outside, paid once | The effects base; `testing/ai_match.rs`; `tuning/mod.rs` | The sandbox's held hit motion and the harness's field for it; the shared `BelievedShare`. |
| Outside, tests and fixtures | The AI's, the escalation's, and the paddle's test modules; `ai_matches.rs`; the overlay's key line in two tests; `match_screens.rs`; `pong/tests/spin.rs` | Spin's done-conditions in the AI; fixtures that named the roster; the harness; the view's own tests. |
| Docs and rules | Two READMEs, `.claude/rules/tests.md`, the spec, the plan, this report | |

- **The list's negative half held:** no production code of physics, AI, rules, escalation, sanity, or the HUD changed. The AI's and the escalation's files changed in their test modules only; physics, rules, sanity, `pong/src/ui`, and `pong/src/views` not at all.
- **What the next effect will also touch outside the list:** the two tests that spell out the overlay's key line. Nothing else that this phase's fixtures touched: they ask the tuning or the core now.
- **What it will not pay again:** the held hit motion, the harness's field, the shared share type.

## The hand check and the tryout (step 6, begun)

The user's hand check on 2026-11-13: "Everything seems to be working." What was looked at beyond that was not reported.

### The tryout's numbers

The user asked for more spin, with no numbers. These are Claude's, to be judged by play:

| Number | Starting | Tryout |
|---|---|---|
| Dial starts, full at return | 0, 12 | 0, 6 |
| `max_drift` | 16 | 30 |
| Drift after 1, 3, 6, 12 returns | 1.3, 4, 8, 16 | 5, 15, 30, 30 |
| Believed share, one / two / three stars | 0.3 / 0.6 / 0.85 | 0.63 / 0.79 / 0.92 |
| Misread at full dial, in units | 11.2 / 6.4 / 2.4 | 11.1 / 6.3 / 2.4 |

- **Both halves of the report are answered:** the dial is full twice as early, for the short and middle rallies, and the full drift is nearly twice as far, past Wind's 24.
- **The shares were raised with the drift, which the user did not ask for.** Ruling 6 sized the flaw by its misreading in units, and that is kept. At the old shares the tryout's drift would leave two stars 12 units off, past the paddle's reach of 8 with no error at all, so a spun ball would beat them nearly every time. Putting the old shares back is one line of `tuning.ron`.
- **This is not the retune of spec 2.10.2,** as Claude reads it: that one follows the session. This one precedes it, from the developer's play.
- **No playtest log entry was written** for the developer's play.

### Tests that pinned the embedded numbers

Changed to the tryout's numbers before the tuning was, and red against the old tuning:

- Spin's drift tests: 15 after three returns, 30 at full dial and over a crossing.
- The systems test of spec 2.6.4: 16 to 19.5 units over the units flown.
- The AI's crafted flaw reads three stars' share as 0.92.
- **R10's ball moved,** from 27 off the centre to 13. At the tryout's drift a ball from 27 strikes the wall and comes back inside the paddle's reach; from 13 it curves out to the wall and scores, as the test's name says.
- The paddle's cramped arena lowers the new `max_drift`.
- Shell: the mark's size is compared after one return, since the dial is at a third after two.

### Measured again (`just ai-report`)

Classic's lines are unchanged. More stars won 8 to 0 in every pairing and mode, and every rally ended.

| Pairing | Chaos, cap 3, at the tryout's numbers |
|---|---|
| 1 v 1 | 27 rallies, mean 2.3, longest 14 |
| 1 v 2 | 20 rallies, mean 3.8, longest 12 |
| 1 v 3 | 20 rallies, mean 4.2, longest 15 |
| 2 v 2 | 27 rallies, mean 17.6, longest 89 |
| 2 v 3 | 20 rallies, mean 17.5, longest 75 |
| 3 v 3 | 29 rallies, mean 58.5, longest 115 |

Spin alone, a hit motion held:

| Held count | Drift | 1 v 1, 8 seeds | 2 v 2, 2 seeds |
|---|---|---|---|
| 0 | 0 | 63 rallies, mean 2.7 | none ended (2 of 2) |
| 5 | 25 | 60 rallies, mean 2.4 | 16 rallies, mean 8.4 |
| 10 and up | 30 | 55 rallies, mean 1.4 | 15 rallies, mean 7.0 |

The stack's fourth rung, with Spin at 45 returns: 55 one-star rallies of mean 0.7, 15 two-star of mean 1.1, 14 three-star of mean 32.5 with the longest 108. The third rung's three-star mean is 50.4, so R3 holds for the stack.
At the starting numbers that fourth three-star mean was 18.4. The misreading in units is the same in both, so the difference is not explained by the flaw's size; it was not looked into.

These pairings are between computers, which seldom spin. They say little about how the tryout plays for a person who spins at every hit: that is for the hand and the session.

## Test changes

466 core tests and 192 shell tests pass, 658 in all; 622 existed before this phase. 36 were added: 26 in the core, 10 in the shell.

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or a new rule:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Spin`.
- The overlay's two tests of the sandbox key line: it now reads `1 Speed, 2 Shrink, 3 Wind, 4 Spin`.
- `a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move` (paddle): its cramped arena also lowers Spin's `max_drift` to 4, since 16 is refused there.
- `VALID` in the tuning tests gained Spin's section.
- Three tests of the sandbox (effects) also hold, release, and force by a hit motion.
- `launch_at_ai` (AI) now calls `launch_at_ai_in_the_flight`, which the Spin tests share; its steps and every assertion that uses it are the same.
- The harness's forcing literals, four of them, name the new field; the dial ladder's holds a motion.
- The escalation, HUD, and overlay fixtures of step 4.

## Bevy API

Steps 1, 2, and 4 wrote no API new to the repository, so `bevy-api-check` was not run for them.

Step 3 wrote one area new to the repository: a world-space parent and child. `bevy-api-check` ran before the code, against the pinned 0.20.1 source:

- `bevy::ecs::hierarchy::ChildOf(pub Entity)`, with `.parent()`. Spawning with `ChildOf(parent)` in the bundle is the documented direct way; no feature gates it.
- A child's global transform is its parent's times its own, the parent's scale included. The ball's is scaled by its radius in x and y and by one in z, so the mark's place and size are in ball radii and its draw order is the ball's plus its own.
- A child with `Visibility::Inherited` is hidden by a hidden parent; one with `Visibility::Visible` is not. A parent with no visibility at all leaves the child visible.
- `Children` is declared `linked_spawn`: despawning the parent despawns its children, so the mark needs no `DespawnOnExit`. The agent did not find the derive code that wires it; `leaving_the_match_takes_the_mark_with_the_ball` shows the behaviour at the pinned version.
- `MinimalPlugins` has neither `TransformPlugin` nor the visibility plugin: headless, nothing propagates. The tests read the mark's parent, its local `Transform`, and its own `Visibility`.
- The validity check at this version is `ValidateParentHasComponentPlugin<C>`: it warns `B0004` in `Last` when a child has a global transform or an inherited visibility and its parent still has none. The mark is spawned in `Update`, after the ball was dressed in the fixed loop's attach set. **Not reported:** whether `just run` logged such a warning; the hand check found everything working and said nothing of the log.

## Gate

`just dev-check` green on 2026-11-13 with all of steps 1 to 5: 658 tests. Green again after the tryout retune, at the same 658.
After step 2 alone it was green at 646. On that run, the first after a rebuild, two tests ran past 30 s and passed; no name was captured. The crash tests are the known case (`.config/nextest.toml`).

## Open at phase close

- **Step 6, the user's:** the session.
  - The tryout's numbers judged by play: kept, changed, or put back.
  - The session, with the ruling: keep, retune once, or drop (E.6).
  - The Wind-versus-Spin hypothesis observed and logged (plan 6.2).
- **The computer as a deliberate spinner:** a MAY (spec 2.6.4, plan 11.3), decided before milestone close with every effect built and playtested. Against the computer, Spin is mostly the player's to use.
- **R11 for the mark:** the review item at milestone close (spec 2.7.4).
- **Not shown by test:** that Spin alone shortens three-star rallies; how much of the stack's fourth rung is Spin's.
- **Not measured:** that added delay is inert at low speed.
- **The heavy tests' times:** thirteen now, up to 11 s warm and 17 s cold under load, against 30 s. Each bag effect to come adds a dial test.
- **CI:** not run on this build.
