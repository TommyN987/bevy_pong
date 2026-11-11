# M2 phase 2 report: Chaos, headless

Started and built 2026-11-11. Branch `feat/chaos-headless`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

The first five were proposed before implementation and approved by the user on 2026-11-11.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.2.3 | One height factor for both paddles, not one per side. Catch-up bias is a WONT and Shrink shortens both; a per-side factor would be a channel change, which the compiler walks through every effect. | This report; features README. Spec amended 2026-11-11. |
| spec 2.3.9 | The curve type yields a number only. Its whole-count form, and the rule that an item effect starts at return one or later, wait for Lens (plan phase 8): no item effect exists to validate against. | This report. Spec amended 2026-11-11. |
| spec 2.6.1 | Paddles restore at the countdown, not at the instant of the point. The flight is cleared on entering `Countdown`, so the paddles stay short through the point pause. | This report; features README. Spec amended 2026-11-11. |
| spec 2.3.7 | Started and ended messages are written for bag effects only. An always-on effect never starts, and leaving a match ends nothing. | This report; `domain.md`. Spec amended 2026-11-11. |
| spec 2.1.1 | Both modes are tuning data naming their effects (`always`, `bag`), with thresholds in whole percent. Classic is the mode with an empty bag; nothing branches on the mode but the tuning lookup. | This report; features README. Spec amended 2026-11-11. |
| spec 2.3.7 | The order of an ended message before a started one is not tested. They are two message types with a reader each, so no test observes their order across types. | This report. Spec amended 2026-11-11. |
| plan 2.1 | `Mode::Chaos { cap: NonZeroU8 }`, no new cap type. A cap above the schedule's reach never binds, so it needs no validation. Vocabulary: 9 capped types of 12. | This report; `domain.rs`. Plan and spec 2.1.1 amended 2026-11-11. |

## Test changes

M1's and phase 1's suites pass: 468 tests, of which 415 existed before this phase. No assertion changed.

Call sites changed for a changed signature or shape, assertions untouched:

- `MatchConfig` gained `mode`: 24 test literals take `mode: Mode::Classic`, and the menu's one production literal does too.
- `BallTuning::speed_rule()` is gone. `speed_cap()` replaces it in the soak and one AI test helper. `tuning.mode(Mode::Classic).speed_rule()` replaces it in two physics tests, one tuning test, and the effects tests' helper.
- The tuning tests' `VALID` fixture moved `per_hit_increase` into `modes` and gained `modes`, `effects`, and `ball.flight_limit_seconds`.
- `Speed::new` takes a `SpeedRule`; its tests build it from Classic's.

One assertion added to an earlier test: `the_group_adds_every_core_plugin_once` also expects `EscalationPlugin`.

## Decisions taken in this phase

- **Kernel.** `ramp.rs` holds `Ramp`, the curve a dial rides: a start value until one return, linear to a full value at another. `reflect::launch_speed` is the one statement of the speed channel's bound; `SpeedRule::launch` calls it. `Flight` gained the paddle height factor, composed by product, and exposes its step count. `Flight`'s `Default` is written by hand, since a derived one would start the factor at zero.
- **The contract's first walk.** `Contribution` gained `paddle_height_factor`; `speed.rs` stopped compiling until Speed wrote `1.0`. Adding `EffectId::Shrink` stopped the core's registration match compiling until Shrink was built there.
- **Tuning.** `modes.classic` and `modes.chaos` each hold a per-hit increase, `always`, `bag`, `thresholds_percent`, and `rotate_every_points`. `ball.per_hit_increase` moved there; `ball.speed_cap` stays, the same in every mode. A `SpeedRule` is built per mode. `effects.shrink` holds the two returns of Shrink's dial. `ball.flight_limit_seconds` is new. Validation refuses: an effect named twice in a mode, thresholds that do not rise or pass a hundred, a Chaos increase not below Classic's, a Shrink dial full before it starts.
- **Shrink.** Its factor rides a ramp from one to `height_min / height_default`, so the dial is in play terms and the two returns are the only numbers tuned.
- **The paddle channel.** One base system in the effects phase sets both paddles to the default height times the flight's factor, through `Paddle::set_height`. It writes only when the factor differs, by bit pattern, from the one it last wrote. Writing every step would have overridden the paddle feature's own refit after a tuning reload, which an M1 test asserts.
- **Escalation.** A new feature after rules, the one writer of `ActiveEffects`. `schedule.rs` is pure and generic: `count`, `shuffle`, `Rotation<T>`. Entering a match shuffles the mode's bag; entering a countdown brings the effects in play up to date.
- **Points, not countdowns.** A countdown is also entered after a fault and on resuming a paused countdown. The schedule remembers how many rotation intervals of points it has acted on, so those re-entries change nothing.
- **Whole-number thresholds.** `leader * 100 >= percent * target`: 3 of 10 against 30 percent is exact, with no float at a decision boundary.
- **Classic's random stream is unchanged.** A bag of one or none draws nothing from the seeded source.
- **Flight-time invariant.** `SimFault::FlightTooLong`, checked in sanity from the flight's step count against `ball.flight_limit_seconds`, 20 s as shipped. The clock runs only in play. The existing heal restarts the rally.
- **The menu starts Classic.** `pong/src/ui/menu/mod.rs` builds `mode: Mode::Classic` until the mode screen of plan phase 3.
- **Feature order.** `escalation` joined after `rules`; ADR 0002's list and `arch-check` follow.
- **Bevy API.** No API new to the repository was used. `bevy-api-check` was therefore not run. One behaviour is relied on by precedent, not by reading: the match's `OnEnter` runs before its first countdown's, as the ball's spawn and first placing already rely on; `chaos_starts_its_first_bag_effect_before_the_first_serve` would fail otherwise.

## Checked against deliberate breaks

Each of these twenty fails at least one test:
a threshold that must be passed, not reached; the count ignoring the cap; the count ignoring the bag's size; a rise that also rotates; the leaving item jumping the queue; every countdown rotating; always-on effects left out; Chaos ignoring its cap; a new match keeping the old schedule; no start announced; no end announced; Shrink doing nothing; height factors adding up; the paddles rewritten every step; the paddles never following the flight; Chaos given Classic's speed rule; no flight ever too long; an equal Chaos ramp accepted; a ramp ignoring where it starts; the flight's clock reporting zero.

## Facts learned

- Three tests of the first draft failed for a script reason: they scored while the countdown ran, when the ball is not in play. The scoring helper now waits for play.
- A headless Chaos match between two one-star computers to five points takes about a tenth of a second.

## Open at handoff

- Shrink already shows on screen: M1's paddle view sizes each paddle to its rectangle every frame (`pong/src/views/paddle.rs`, `fit_paddles`). It has no cue and no icon; plan phase 3 gives Shrink its shell half. Chaos is not reachable from the menu yet, so this is unseen by hand.
- Only Shrink is in Chaos's bag, so every cap plays alike until Wind lands.
- Test-only seam, kept from phase 1: `ActiveEffects::add`.
