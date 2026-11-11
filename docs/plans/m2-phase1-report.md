# M2 phase 1 report: Refactor: flight, contract, Speed

Started and built 2026-11-11. Branch `refactor/basic`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

All five were proposed before implementation and approved by the user on 2026-11-11.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.2.4, plan 1.2 | The flight counts returns, not touches. A return is a face hit, which turns the ball back and starts a flight. An end clip writes `BallHitPaddle` and starts none. Rules is unchanged and still counts touches for the rally record. M1 raised the speed on a face hit only, so only the return count reproduces Classic. | This report; features README. Spec amendment at the next record step. |
| plan 1.4, spec 2.3.9 | The shared curve type is not built. Speed is linear and capped by the channel; Shrink is the curve's first user, in plan phase 2. | This report. Plan amendment at the next record step. |
| plan 1.4, spec 2.3.3, 2.3.10 | The shell's registration match is not built. Nothing in the shell consumes an effect until the icons of plan phase 3. The core's match exists. | This report. Plan amendment at the next record step. |
| spec 2.1.1 | The tuning file keeps its shape. `ball.per_hit_increase` feeds Speed; the per-mode layout arrives with modes in plan phase 2. | This report. |
| spec 2.0.1 | Two kinds of test change beyond changed call sites, listed below. | This report. Spec amendment at the next record step. |

## Test changes (plan 1.7)

M1's suite passes: 415 tests, of which 391 were M1's. No test differs in the last float bits; none needed a tolerance changed.

Call sites changed for a changed signature, assertions untouched:

- `crossing_y` gained a `&Flight` parameter: five calls in `kernel/predict.rs`, one in `features/physics/mod.rs`, one in `pong_core/tests/soak.rs`. Each passes `&Flight::default()`.
- `MirrorController::steer` gained a `&Flight` parameter: eleven calls in the tests of `features/ai/mod.rs`.

Tests of the removed `SpeedRule::after_hit`, re-pointed at its replacement `SpeedRule::launch`, asserting the same property:

- `kernel/reflect.rs`: `a_thousand_reflections_never_exceed_the_cap`, `zero_increase_leaves_speed_unchanged`, `positive_increase_is_monotone_until_the_cap_then_flat`.
- `features/tuning/mod.rs`: one line of `valid_file_yields_kernel_types_with_degrees_turned_into_radians` (80 becomes 87).
- `features/physics/mod.rs`: `speed_rises_by_the_tuned_increase_on_every_paddle_hit_until_the_cap` computes the expected speed from the return count.

One assertion changed, because it encoded the rule spec 2.0.4 replaces:

- `features/physics/mod.rs`: `a_ball_faster_than_the_paddle_is_thick_still_hits_the_face_speeds_up_and_deflects` placed a ball at speed 200 with no returns and expected 207, the old rule applied to the ball's current speed. Under spec 2.0.4 the ball leaves its first return at 87. The test keeps its purpose, spec 1.2.3's tunnelling check, and is renamed `..._still_hits_the_face_and_deflects`.

One assertion added to an M1 test: `the_group_adds_every_core_plugin_once` also expects `EffectsPlugin`.

## Decisions taken in this phase

- **Four steps, one risky.** The paddle's step motion, the flight as plumbing, the swap of the speed rule, then order and docs. Only the third could change behaviour.
- **Kernel, `flight.rs`.** `Field` is a trait with one pure method. `Flight` holds the return count, a step clock, the summed speed gain, and the fields. `Contribution` has no `Default`, so an effect writes every field. Both channels compose by sum.
- **The clock counts steps.** A `u16` step count is exact in `f32`, so physics and prediction read the same time; returns are `u16` for the same reason. Both stop at their limit.
- **Prediction reads ahead.** `crossing_y` asks `Flight::force_ahead(motion, step, dt)` and never mutates the flight. Its final interpolation uses the velocity the force has already changed, as the integrator does; with no force that is bit-identical to M1.
- **The integrator is unchanged.** `advance_between_walls` already took a force. Only the source changed: the flight, in place of the `Force` component, which is removed.
- **The launch speed is a channel.** Effects add a gain; the base applies `min(serve + gain, cap)` in `SpeedRule::launch`. `SpeedRule` keeps the per-hit increase and the cap, validated together as before; Speed reads the increase.
- **Effects feature.** `Effect` has `id` and `contribute`, no defaults. `build` is the core's one match on `EffectId`. `ActiveEffects` is the resource; `BallFlight` the component, required by `Ball`. Entering a match puts Speed alone in play, until modes exist.
- **Flight starts.** A countdown clears the flight (`OnEnter(Countdown)`, where M1 reset the rally). A serve starts the first flight in the effects phase, after the ball feature launched the ball in intent. A return starts the next inside the collide system, by a direct call: the system needs the new speed in the middle of its own run.
- **The rest of a hit's step flies under the new flight**, not under zero force as in M1. With no fields the two are the same.
- **A tuning reload reaches the effects in play.** They are rebuilt when `Tuning` changes, so the dev override keeps working for the increase.
- **The paddle records what it did.** `move_by` stores the displacement after the wall clamp; `set_height` no longer goes through `move_by`, so a resize leaves the record alone.
- **`EffectId`** sits in the domain with a stable serialized name, outside the vocabulary cap as an identifier (ADR 0008).
- **`arch-check`** gained the effects rule: an effect's module imports the effects base by name and never another effect. A planted cross-import failed it.
- **ADR 0002's order** is amended in place: `effects` after `paddle`, before `physics`.
- **Bevy API.** No API new to the repository was used: `register_required_components`, `OnEnter`, `MessageReader`, `Resource`, `Component`, and change detection all appear in M1 code. `bevy-api-check` was therefore not run.

## Checked against deliberate breaks

Each of these twelve fails at least one test:
no clock tick in the physics step; the flight's force ignored by physics; a return counted twice; Speed one return ahead; a countdown that does not clear the flight; a serve that starts no flight; the paddle's motion taken from what was asked; the hit hiding the paddle's motion; prediction freezing the flight's clock; a reload not reaching the effects; the cap not applied at launch; a resize rewriting the step motion.

The first two survived at first: Classic has no field, so nothing exercised the physics system's use of the flight.
`under_a_field_the_ball_crosses_where_the_kernel_predicts_from_its_flight` was added for them.

## Facts learned

- The soak runs in 3.5 s on the development machine. The spec's question, whether a soak per subset of six effects fits CI, now has one number; plan 4.3 measures the rest.
- Spec fact resolved: the paddle moves in the integrate phase, so its step motion is known in collide.

## Open at handoff

- Plan 1.8, the hand check that Classic feels the same in `just run`, is the user's.
- Test-only seam: `ActiveEffects::add` puts a probe in play under Speed's borrowed name, since the identity enum is closed. Plan phase 2 may replace it when the mode sets the effects.
