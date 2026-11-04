# pong_core

The render-free simulation core of bevy_pong.
It decides what happens in a match; the shell (`pong/`) draws it, plays its sounds, and reads the keyboard.
It runs headless, so every rule is testable without a window.

Boundaries and the error policy: [docs/architecture.md](../docs/architecture.md).
Layering: [ADR 0002](../docs/adr/0002-intra-crate-layering.md).
Match flow and the step gate: [ADR 0003](../docs/adr/0003-match-flow-ownership.md).

## Bevy in brief

The core uses six Bevy building blocks and nothing more exotic.
No observers, hooks, or reflection-driven machinery:
the schedule is the one place that says when things run.

| Block | What it is | Why Bevy has it |
|---|---|---|
| World | Entities (bare IDs) carrying components (data), plus resources (singletons). | One store for all state. In 0.20 resources are components on hidden entities, so insertion and change detection behave alike for both. |
| System | A plain function; its parameters (`Res`, `ResMut`, `Query`, `Commands`, `MessageReader`) declare its data access. | Declared access lets the scheduler see conflicts and track changes per system. |
| Schedule | A graph of systems with ordering edges and run conditions, walked by an executor. | Order is data, not call sites. |
| Commands | Deferred spawn, insert, remove. | Structural changes move entities between tables and would invalidate running iterations. They apply at sync points: auto-inserted between ordered systems when the earlier one has commands, and at every schedule's end. |
| Message | A double-buffered queue per type; each `MessageReader` keeps its own cursor. | Many independent readers of one stream, without coupling to the writer. |
| State | `State<S>` (current) and `NextState<S>` (requested); the `StateTransition` schedule applies requests and runs `OnExit`, `OnTransition`, `OnEnter`. | Mode changes happen at one known point in the frame. |

A plugin is a `build(&mut App)` function that registers systems, resources, and messages.
The `App` is a world, its schedules, and a runner that calls `update()`.

## One frame

One `app.update()` runs these schedules in order.
Our systems are in brackets.

```
(first update only) StateTransition → OnEnter(Boot) → PreStartup → Startup [begin_loading] → PostStartup
───────────────────────────────────────────────────────────────────────────────────────────────────────
First            time: Real → Virtual (delta clamped at 250 ms); message buffers swap (see below)
PreUpdate        asset server inserts finished loads into Assets<Tuning>
StateTransition  apply NextState (AppState before MatchState)
                 → OnExit (child before parent) + DespawnOnExit
                 → OnTransition
                 → OnEnter (parent before child)
RunFixedMainLoop overstep += virtual delta
                 while overstep ≥ one step (16 666 667 ns at 60 Hz):
                   FixedFirst      [open_or_close_step_gate]
                   FixedUpdate     Intent → Effects → Integrate → Collide → Rules → Sanity
                   FixedPostUpdate [Bevy: signal "a fixed step ran"] [harness: recorders]
Update           [leave_boot, promote/reload tuning, countdown/point timers, pause port]
PostUpdate       asset events written as messages
Last
```

State changes apply in exactly one place per frame, before the fixed loop.
A `NextState::set` made in fixed step *k* takes effect at the next frame's `StateTransition`.
The remaining steps of the frame still see the old state.
Without a guard, a 14-step frame after a goal scored 14 points;
the step gate ([flow](src/flow/README.md)) stops it.

The fixed clock is swapped into the generic `Time` for each fixed iteration.
`Res<Time>` in `FixedUpdate` has a delta of one step;
in `Update` it has the frame's virtual delta.
Fixed systems still name `Res<Time<Fixed>>`, so the clock they mean is stated.

Message buffers swap only after a frame in which a fixed step ran.
Bevy's `TimePlugin` holds the swap in `First` until `FixedPostUpdate` signals.
A frame with zero fixed steps never swaps,
so a message written in a fixed step survives for the remaining steps and for `Update`.

The first `update()` has zero delta and runs no fixed step.
Bevy's virtual clock caps a frame at 250 ms: at most 14 steps at 60 Hz.

## Layers

A layer imports only layers below it.
Rust enforces direction only between crates, so `just arch-check` enforces it here by grep,
inside `just dev-check` and `just ci-check`.

```
kernel  ←  domain  ←  flow  ←  features (tuning → seats → ball → paddle → physics → rules → …)  ←  app
                                                                                                     ↑
                                                                                         testing, tests/soak
```

| Layer | Path | Holds | Imports |
|---|---|---|---|
| [kernel](src/kernel/README.md) | `src/kernel/` | Pure math: geometry, integration, collision, reflection, prediction, PCG32. | `bevy::math` only |
| [domain](src/domain.md) | `src/domain.rs` | Vocabulary shared by two or more features; the core's messages. | kernel |
| [flow](src/flow/README.md) | `src/flow/` | App and match states, fixed-step phases, step gate, `FIXED_HZ`. | kernel, domain, Bevy |
| [features](src/features/README.md) | `src/features/` | One plugin per feature, in a declared order. | lower layers; earlier features' public data types |
| [app](src/app/README.md) | `src/app/` | `CorePlugins`, error policy, the Boot gate. | everything |
| [testing](src/testing/README.md) | `src/testing/`, `tests/` | Headless harness, soak. | everything; behind the `testing` feature |

## One point, traced

A frame of four fixed steps; the ball crosses the right goal line in step 2.

| When | What happens |
|---|---|
| Step 2, Collide | The ball misses the paddle. |
| Step 2, Rules | `Score` rises by one; `PointScored` is written; `NextState = PointScored`. |
| Step 2, Sanity | Runs: the gate was decided in `FixedFirst`. |
| Steps 3–4, `FixedFirst` | A transition is pending: the gate closes, nothing moves. |
| Same frame, `Update` | The shell can read `PointScored`. The pause port defers any request. |
| Next frame, `StateTransition` | `MatchState` becomes `PointScored`; `OnEnter` starts the point-pause timer. |
| Point pause elapsed | The timer requests `Countdown`. Next frame, `OnEnter(Countdown)` clears seat intents, centres the ball with `Snap`, and starts the countdown timer. |
| Countdown elapsed | The timer requests `Playing`. At the transition, `Serve` is written toward the conceding side. |
| That frame's first fixed step | Intent reads `Serve`; the ball moves. |

During the point pause the ball rests past the goal line:
nothing moves outside `Playing`.
