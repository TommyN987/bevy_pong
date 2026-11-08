# ADR 0003: Match flow ownership: step gate, one fixed rate, serve trigger, ball reset, pause port

Date: 2026-11-04
Status: accepted

## Context

The phase 3 review (2026-11-03) found the match flow broken in ways one-step-per-frame tests could not show.
Bevy facts were verified against the pinned 0.20.1 source through `bevy-api-check`.

- A `NextState` set inside a fixed step applies only at the next frame's `StateTransition` (`bevy_state-0.20.1/src/app.rs:335`).
  Further fixed steps of the same frame still run in `Playing`, and a ball past the goal line scores again on each.
  One 250 ms frame (`Time<Virtual>` max delta, `bevy_time-0.20.1/src/virt.rs:86`) runs up to 15 steps at 60 Hz.
- The test harness runs exactly one fixed step per frame, so no test saw it.
- `request_serve` ran on `OnEnter(Playing)`, which also fires on `Paused → Playing` (`bevy_state-0.20.1/src/state/transitions.rs:250-257`): resuming relaunched the ball.
- The ball returned to the centre only at the serve; it rested past the goal line through `PointScored` and `Countdown`.
- The shell was to set `NextState<MatchState>` for `Paused` itself, with nowhere to record the substate to resume to.
- Core never set `Time<Fixed>`: production would run at Bevy's default 64 Hz (`bevy_time-0.20.1/src/fixed.rs:76`), tests at the harness's 60 Hz.

## Decision

Rules owns every `MatchState` transition, and a fixed step runs only while play is live.
Amended by ADR 0004 (2026-11-07): the phases also run in `Countdown` and `PointScored`, where paddles move; the ball's systems keep this gate through `BallInPlay`.

- Step gate: a system in `FixedFirst` opens the gate when `State<MatchState>` is `Playing` and no `MatchState` or `AppState` transition is pending.
  The phases run only while it is open.
  The step that stops play finishes every phase, sanity included; the remaining steps of that frame skip.
  Whether a pending `NextState` can be read is unverified; `bevy-api-check` answers it before plan 3.13.
  Fallback: a rules-owned `PlayHalted` flag, set beside every `next.set` inside a fixed step, cleared on `OnEnter(Playing)`.
- One fixed rate: `FIXED_HZ = 60` in `flow`; `CorePlugins` applies it to `Time<Fixed>`; the harness reads it.
  The harness gains frames of N steps and frames of half a step.
- Serve on `OnTransition { exited: Countdown, entered: Playing }`.
- Ball reset: on `OnEnter(Countdown)` the ball feature puts the ball at the origin, stationary, with `Snap`.
  Through `PointScored` the ball stays where it went out; whether the view shows it there is plan 4.2's choice.
- Pause port `MatchControl`, a resource. The shell calls `request_pause()` and `request_resume()`.
  Rules accepts a pause from `Countdown`, `Playing`, or `PointScored`, records that substate, and ignores the request in `MatchOver`.
  Resume returns to the recorded substate; a resumed `Countdown` restarts its full timer.
  Focus loss requests a pause only while `Playing`; the shell enforces that (spec 1.6.1). Amended by ADR 0004: wherever the paddles move.

## Alternatives considered

- Park the ball when a point is scored: C1 forces would still move it, and every future in-step stop would need its own guard.
- Run `StateTransition` inside `FixedMain`: works against the engine; shell `OnEnter` systems would run in fixed time.
- Limit frames to one fixed step: the game slows down under lag.
- Bevy's 64 Hz, whose 15,625 µs step is exact: every test and tuning comment is reasoned at 60 Hz.
- The shell sets `NextState` and remembers the pre-pause substate: splits the state machine across crates; nothing stops a resume into `Playing` from `MatchOver`.

## Consequences

- The architecture's shell-to-core ports grow from three to four.
- A pause requested in `Update` applies at the next frame's `StateTransition`, before that frame's fixed steps.
- Tests can run frames of up to 14 steps and of half a step (plan 3.12, 3.13). Corrected 2026-11-04 from 15: at 60 Hz the step rounds up to 16,666,667 ns, so 15 steps exceed the 250 ms clamp; a real 15-step frame needs overstep carried from the frame before.
- Tuning validation can check `speed_cap / FIXED_HZ < arena.height − 2·radius` (plan 3.17).
- Plan 3.12 through 3.14 implement this; plan 4.1 routes focus loss through `MatchControl`.
- Amends `docs/architecture.md` (Simulation model, Boundaries, States) and spec 1.4.5 and 1.6.1.
- Plan 8.3 (2026-11-08): the dev entry is gone, and the shell's pause screen asks through `MatchControl`. Escape pauses wherever the paddles move; Escape, the back key, or Play resumes, after a focus loss too. Its keys are read before the fixed steps, as the focus adapter's are, so rules acts on a request within the frame. Leaving a match, from the pause screen or `MatchOver`, is the shell's `AppState` transition to `Menu`; a match left early sends no `MatchEnded`, so nothing is recorded.
