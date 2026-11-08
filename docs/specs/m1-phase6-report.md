# M1 phase 6 report: sanity and heal

Started 2026-11-08. Branch `feat/sanity`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.4.7 | "Heal re-serves and logs at error": the error-level line comes from Bevy's release error handler, which logs the sanity system's returned `SimFault` with its text, not from the heal. `pong_core` has no `bevy_log` feature, and adding it would pull `tracing` and `tracing-subscriber` into the core for one line the handler already writes (`bevy-api-check`, 2026-11-08). | Features README (sanity); spec 1.4.7 to amend at the record step. |
| 1.5.2, plan 3.19 | The soak no longer checks the invariants itself (plan 6.1 amendment); the sanity phase checks them after every step instead of every frame. Its bound past a goal line is one step at the speed cap, a little looser than the soak's one step at the ball's own speed, and is the bound the spec states. The soak asserts `SanityPlugin` is present, so it cannot go quiet. | `pong_core/tests/soak.rs`, testing README, 2026-11-08. |

## Decisions taken in this phase

- `SimFault` is a message enum in domain, one variant per broken invariant, with the values seen: `BallCount`, `BallVelocityNotFinite`, `BallOutsideArena`, `PaddleOutsideArena`. Messages fall outside the vocabulary cap (ADR 0002); the count stays at six types.
- One code path in both builds (architecture, error policy): the sanity system writes the `SimFault`, then returns it as its error, and the handler decides. Dev panics in the faulting step; release logs and the heal runs.
- The heal is `restart_rally_after_fault` in rules, in `Update` after the timers and before the pause port, while the paddles move. It requests `Countdown`, which re-centres the ball and restarts the timer; the countdown's end serves, so `Serve` keeps one emitter (ADR 0002). The score stands. A match already ending is left to end.
- `NextState::set(Countdown)` from `Countdown` re-runs `OnExit` and `OnEnter` in Bevy 0.20.1, whatever the `OnEnter` doc comment says (`bevy-api-check`, quoting Bevy's own test), so one heal serves every substate.
- Faults later in the faulting frame are reported again, and the heal answers them all with one request. The next frame's state transition comes before its fixed steps, so the ball is reset before the next check.
- `SimFault` is registered by rules, its reader and the earlier feature, as ball registers the `Serve` that rules writes. A core group without `SanityPlugin` still builds.
- The harness gains `headless_app_with(seed, adjust)`: the release-policy test sets `ErrorPolicy::Release`; the plan 3.15 two-ball physics test disables `SanityPlugin`, since M1's one-ball rule now faults on its second ball.
- Every test of behaviour built before it was checked against a deliberate break: sanity gated to live play fails the countdown test; no margin past the goal fails the resting-ball test; no match-end guard fails the match-end test; a disabled heal fails the release test.

## Open findings

- A fault the heal cannot cure, such as a second ball, faults on every step in release, and the heal restarts the countdown every frame: the match stalls in its countdown. In M1 only a code bug can spawn a ball, and dev panics on it first. Not handled; the user decides.
