# M2 phase 6 report: Spin

Started 2026-11-13. Branch `feat/spin`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-13: steps 1 and 2 of the proposal the user approved that day (plan 6.1; E.2).
Open: steps 3 to 6 (the shell half, the bag and the harness, the record step, the session).

## The proposal's steps

| Step | What | State |
|---|---|---|
| 1 | The sandbox holds a hit motion | Built. The harness's `Forcing` takes it in step 4, with the dial ladder, its first user. |
| 2 | Spin's core half (E.2) | Built. |
| 3 | Spin's shell half (E.1, E.3) | Open. The shell half is declared empty until then. |
| 4 | Into the bag, and the harness (E.4, E.7) | Open. Spin is in no bag. |
| 5 | Record (P.6, E.5) | Open. The three deviations below are owed to the spec there. |
| 6 | Hand check, session, ruling (E.6, plan 6.2) | Open; the user's. |

## Deviations

All three were proposed before implementation and approved by the user on 2026-11-13.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.6.4, "AI flaw: reads the spin late" | The flaw is the second form of 2.3.12 alone: each star has read a tuned share of the true spin when it commits its aim. It is not an added delay. | Here. Spec amendment owed at step 5. |
| spec 2.9.1 | The sandbox also holds a hit motion: the paddle motion every return is told. For a harness; no overlay key writes it. | Here. Spec amendment owed at step 5. |
| spec 2.6.4, "a tuned maximum" | Validation refuses a `max_drift` at or above half the playable height, as Wind's. The spec names no bound for Spin. | Here. Spec amendment owed at step 5. |

## Rulings by the user

2026-11-13: "Approved as proposed. You drive." The proposal's six rulings, each as recommended:

1. **Roles.** Claude drives; the user reviews.
2. **The flaw.** The believed share alone.
3. **The harness.** The sandbox holds a hit motion (step 1).
4. **The computer as a spinner.** Not in this phase. Open; decided after the session.
5. **The view's attachment.** The arrowhead is a child entity of the ball, through `bevy-api-check` first (step 3).
6. **Starting numbers.** Dial from return 0, full at 12, `max_drift` 16, shares 0.3, 0.6, 0.85. The bound of the third deviation.

## Why the proposal took this shape

Two facts, each by arithmetic from the tuning numbers. Neither is measured.

- **The computer almost never spins.** It reaches its aim and stands still, so its paddle moves at the hit only when it is late. At the serve speed a crossing takes 1.8 s and the slowest star needs 1.2 s at most, so it is never late. The standing dial test plays Spin alone with no speed ramp, and would see no spun ball.
- **Added reaction delay does nothing at low speed,** for the same reason. A flaw of delay alone would act only near the speed cap.

## Step 1: the sandbox holds a hit motion

### What was built

- **`Sandbox::hold_hit_motion`** and `held_hit_motion`: the paddle motion every return starts its flight with, or `None` for each paddle's own.
- **`ActiveEffects::start_flight`** applies it, where it applies the held count: every flight start still passes through one place.
- **`hold_what_the_sandbox_holds`**, the system that was `hold_the_sandbox_count`: it copies both, each only on a change.

### Contract

- **A held hit motion is forcing.** It marks the match touched, counts in `is_forcing`, and is let go by `release` and with the match.
- **A serve is no hit.** A held motion gives it none: `FlightStart::SERVE` stays as it is.
- **It takes hold at the next flight start,** as the held count does (spec 2.4.3).

### Tests

- Red first, by assertion, with the port's methods present and not yet applied: `a_held_hit_motion_is_the_motion_every_return_starts_its_flight_with` (a still paddle was told as still).
- Passing before and after, since it pins what the implementation must keep: `a_held_hit_motion_leaves_a_serve_without_a_hit`.
- Three earlier tests of the sandbox now cover the held motion too. Red only by compile failure: once the port could hold and release a value, they passed.

## Step 2: Spin's core half (plan 6.1, E.2)

### What was built

- **Tuning section** `effects.spin`: `starts_at_return`, `full_at_return`, `max_drift`, and `believed_share` per star. Refused: a dial full before it starts; a negative drift or share; `max_drift` at or above half the playable height.
- **`EffectId::Spin`**, saved as `spin`, and one registration line per crate.
- **`effects/spin.rs`**: a return from a paddle that moved in the step of the hit contributes the kernel's `Drift` toward the paddle's motion, by the dial's amount. A still paddle's return and a serve contribute no field.
- **`Spin::drift(start)`**: the signed drift of a flight, a pure function of its start. The field reads it, and the shell will in step 3, so what is shown is what curves.
- **The flaw** is the second form alone: an AI believes its star's share of the true drift, in the true direction.
- **The shell half is declared empty.** Spin is in no mode's bag, so no player meets it before step 3.
- **It needs no rally seed:** the hitter chooses at every hit.

### Decisions taken in implementation

- **One private type for a believed share.** Wind's tuning and Spin's each held three shares and the same lookup; both now hold a `BelievedShare`. A refactor on green: the public API and Wind's tests are as they were.
- **The sandbox's key 4 toggles Spin from now on,** before Spin has an icon: the overlay's keys follow `EffectId::ALL`. Wind stood the same way between its steps 3 and 4.
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

R3 and R10 are step 4's (E.4, E.7); R1 and R11 are step 3's.

### Red first, and where it was not

- **Tuning:** red by compile failure, then green: five tests.
- **Spin:** red by compile failure. Then, with a `Spin` that spun nothing, red by assertion at `a_moving_paddles_return_curves_the_way_the_paddle_moved_and_a_still_ones_does_not`. The run stops at its first failure, so the other tests were not seen red under that stub.
- **Not red first:** the AI's three Spin tests. They pin what the kernel, the base, and Spin already gave.
- **Checked against two deliberate breaks, since removed.** With every AI believing the truth, `..reads_too_little..misses_some_spun_balls` failed, and the per-star test with it. With every AI believing in no spin, `..reads_all_of_the_spin..returns_every_spun_ball` failed.

## Test changes

464 core tests and 182 shell tests pass, 646 in all; 622 existed before this phase. 24 were added, all in the core: 2 for the held hit motion, 5 for the tuning section, 14 for Spin, 3 for the AI.

No assertion of an earlier test changed for behaviour. Changed for the roster's growth or a new rule:

- `all_lists_every_effect_once` and the name round-trip (domain): an arm and a name for `Spin`.
- The overlay's two tests of the sandbox key line: it now reads `1 Speed, 2 Shrink, 3 Wind, 4 Spin`.
- `a_reload_that_shrinks_the_arena_refits_paddles_before_they_next_move` (paddle): its cramped arena also lowers Spin's `max_drift` to 4, since 16 is refused there.
- `VALID` in the tuning tests gained Spin's section.
- Three tests of the sandbox (effects) also hold, release, and force by a hit motion.
- `launch_at_ai` (AI) now calls `launch_at_ai_in_the_flight`, which the Spin tests share; its steps and every assertion that uses it are the same.

Nothing seeded moved by construction: no draw was added, and Chaos's bag is as it was. Every seeded assertion passes; the soak's printed lines were not compared by eye.

## Bevy API

Steps 1 and 2 wrote no API new to the repository, so `bevy-api-check` was not run.

## Gate

`just dev-check` green on 2026-11-13 after step 2: 646 tests.
On that first run after the rebuild two tests ran past 30 s and passed. No name was captured; the run after it had none slow. The crash tests are the known case (`.config/nextest.toml`).

## Open after step 2

- **Steps 3 to 6,** as in the table above.
- **The spec does not yet say what was built:** the three deviations wait for step 5.
- **The computer as a deliberate spinner:** ruling 4; decided after the session.
- **Not shown yet:** that Spin shortens rallies (R3), and a hit that scores because of it (R10). Step 4.
