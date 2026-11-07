# flow

When core systems run: the app and match states, the fixed-step phases, the step gate, and the one fixed rate.
Imports only kernel, domain, and Bevy;
features import flow, and flow imports no feature.

## `states.rs`

`AppState` is the top-level flow: `Boot → Menu → InMatch`.
`MatchState` is a sub-state of `InMatch`, declared with `#[source(AppState = AppState::InMatch)]`:
`Countdown`, `Playing`, `PointScored`, `Paused`, `MatchOver`.
Rules makes every `MatchState` transition (ADR 0003).

A sub-state exists only while its parent matches:

| Moment | `State<MatchState>` | `NextState<MatchState>` |
|---|---|---|
| Entering `InMatch` | Inserted at its default, `Countdown` | Present |
| In a match | Present | Present |
| Leaving `InMatch` | Removed | Present (reset to `Unchanged`) |

So code that may run outside a match takes `Option<Res<State<MatchState>>>`,
and `in_state(MatchState::X)` is false, not a panic, when the state is absent.
A `NextState<MatchState>::set` made outside a match is dropped.

In one transition, `OnExit` runs child before parent and `OnEnter` parent before child.
Each `OnEnter` is a full schedule run that flushes its commands,
so `OnEnter(Countdown)` sees entities spawned in `OnEnter(InMatch)`.
`NextState::set` always transitions, even to the current state, re-running its `OnExit` and `OnEnter`;
`set_if_different` does not.

## `phases.rs`

`FixedStepPhase` is a `SystemSet` per stage of one fixed step, declared once in order:

| Phase | Work |
|---|---|
| `Intent` | Ports become paddle intents; serves launch. |
| `Effects` | Active effects adjust state and add forces. Empty in M1. |
| `Integrate` | Paddles and ball move. |
| `Collide` | Swept collisions, reflections, hit messages. |
| `Rules` | Scoring and match end. |
| `Sanity` | Invariant checks. Empty until plan 6.1. |

`configure` attaches the order and the gate to the sets, not to systems:

```rust
for phase in FixedStepPhase::IN_ORDER {
    app.configure_sets(FixedUpdate, phase.run_if(step_gate_is_open));
}
for pair in FixedStepPhase::IN_ORDER.windows(2) {
    app.configure_sets(FixedUpdate, pair[1].after(pair[0]));
}
```

A feature system names its phase with `.in_set(FixedStepPhase::X)` and nothing else.
Every `FixedUpdate` core system has a phase;
the schedule walker in the test harness lists any that does not.

## The step gate

`StepGate` is open when the paddles move in the current `MatchState` (`Countdown`, `Playing`, `PointScored`) and neither state has a transition pending.
Its narrower half, `ball_in_play`, also needs `Playing`; it gates the `BallInPlay` set,
which every system that serves, moves, collides, or judges the ball joins beside its phase (ADR 0004).
`open_or_close_step_gate` decides both once per fixed step, in `FixedFirst`.
It is the one fixed-step core system without a phase.

Checking `NextState` closes the remaining steps of a frame once a step requests a transition,
because the transition applies only at the next frame's `StateTransition`.

Deciding once, in `FixedFirst`, makes all phases of a step agree.
Bevy evaluates a set's run condition lazily, once per schedule run, when the set's first system is due.
A condition reading `NextState` directly would pass for `Intent`,
then fail for `Sanity` after `Rules` requested `PointScored`,
skipping Sanity in exactly the step that scored.
The test `the_step_that_stops_play_finishes_every_phase_and_the_rest_of_its_frame_runs_none` guards this.

## `mod.rs`

`FIXED_HZ = 60.0` is the one fixed rate, in the game and in every test;
Bevy's default is 64 Hz.
Tuning reasons about speeds per step at this rate.

`FlowPlugin` registers both states, configures the phases, and inserts `Time::<Fixed>::from_hz(FIXED_HZ)`.
It adds Bevy's `StatesPlugin` if absent:
`MinimalPlugins` lacks it, and without it registering a state panics.
Plugin order against `TimePlugin` does not matter:
`TimePlugin` creates the fixed clock only if absent, and `insert_resource` overwrites.
