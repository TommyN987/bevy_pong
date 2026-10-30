# Architecture

Decided 2026-10-30, after [the design](design.md).
This document records the structural decisions that are expensive to reverse after milestone one.
Everything finer-grained belongs to the milestone specs.

## Constraints the architecture derives from

1. Game logic runs headless; tests use `MinimalPlugins`, which has no window, render, or audio.
2. Effects are a first-class, stable contract: C1 creates them, C3 composes them, C2 fires them, C4 schedules them, the save file names them.
3. Paddle intent is independent of its source: keyboard seat, mirror AI, or boss pattern.
4. The AI predicts with the same math the ball uses.
5. Bevy-version-specific surface is minimized; Bevy breaks every four months and crates lag.
6. Tuning is cheap to iterate; fun is found by playtesting, not by reasoning.
7. Save is a versioned document owned by the game.
8. Audio is reached only through core's event vocabulary.
9. Overhead must pay back within the five milestones.

## Candidates considered

| Candidate | Shape | Outcome |
|---|---|---|
| Arch-1 lean idiomatic | single crate, `Transform` as state, fixed step without interpolation, effects as components | not chosen: no stable effect identity, visible judder |
| Arch-2 core and shell | workspace, render-free core, logical position with interpolated view, effect descriptors, hand-rolled physics | **chosen, refined below** |
| Arch-3 engine-leaning | avian2d, leafwing input, hanabi, thin game code | not chosen: AI cannot share avian's math (constraint 4); three crates on the upgrade treadmill |
| Arch-4 plain-Rust simulation | simulation outside ECS, Bevy only renders | rejected: abandons the frame's ECS learning goal |

Refinements applied to Arch-2 during review: depend on `bevy` with feature flags rather than sub-crates; the effect descriptor is a spawn table, not an interpreter; the upgrade-isolation argument for the split was withdrawn, the split is justified by test speed and by making render types unreachable from simulation code.

## Precondition

A spike runs before milestone one: a throwaway workspace whose core depends on `bevy` with default features off, `cargo tree` asserted free of `wgpu`, and one `MinimalPlugins` test that deserializes a RON tuning file.
If the core cannot be built render-free, the workspace collapses to a single crate and every other decision here still holds.

## Crate structure

Workspace with two crates.

- **`pong_core`**: simulation, effects, AI, states, persistence, tuning types. Depends on `bevy` with default features off and only ECS, state, asset, reflect, and time features on. Never depends on render, window, audio, or UI.
- **`pong`** (binary shell): window, rendering, view sync, input adapters, audio plugin, UI, overlay. Depends on `pong_core` and on `bevy` with `2d`, `ui`, `audio`.

Modules are organized by feature as Bevy plugins, not by layer. The intra-crate guide below refines this.

## Intra-crate structure

Accepted 2026-10-30 as an orientational guide. The ten rules bind; the directory shape does not.

Forced by earlier decisions: a Bevy-free math floor, because the AI calls the ball's physics directly; named fixed-step phases, because five milestones add systems to one step; exhaustive enums at extension points, because an effect without visuals or AI awareness is a silent gap.

Rules:

1. **Feature-first, dependency-down.** Features depend on vocabulary and kernel. Kernel depends on nothing in the crate. The app roof depends on everything. No feature depends on another feature's module; they meet through components and events.
2. **The kernel is ECS-free.** `bevy_math` types are allowed; apps, worlds, systems, and schedules are not. Pure functions and plain data only.
3. **Vocabulary stays tiny.** Only types used by two or more features: seat, side, effect identity, core events. A type used by one feature lives in that feature. If it passes a dozen types, it has become a god module.
4. **Phases are declared once.** Fixed step runs named sets in order: intent, effects, integrate, collide, rules, sanity. Every system names its phase. No system orders itself relative to another feature's system.
5. **Components own their invariants.** Mutation happens through the component's API from any feature. Cross-feature triggers are events, never direct calls.
6. **Extension points are exhaustive enums.** Effect, boss, and item identities. Adding a variant makes the compiler enumerate every spawn table, cue map, and AI branch to extend.
7. **A feature is a directory with a plugin at its root.** Growable sub-concepts (individual effects, bosses) are submodules with a uniform shape: components, systems, spawn function.
8. **The shell mirrors core.** One view module per core feature, one adapter per port, one screen per state, the overlay apart. The shell's spawn table matches the same enum as core's.
9. **Tests live beside the feature.** A shared testing module behind a cargo feature builds a minimal app with chosen plugins and fake ports.
10. **Size decides depth.** A floor is a file until it needs a directory.

Orientational shape at the final milestone:

```
pong_core/src/
  kernel/        geometry, integrate, collide, predict      (no ECS)
  domain/        Seat, Side, EffectId, BossId, ItemId, events
  features/
    seats/       Seat, intent latch, controller kind
    ball/        Ball, Velocity, Position, snap marker
    paddle/      Paddle with clamped size API
    physics/     systems wrapping kernel; force accumulator
    rules/       scoring, match flow substates, MatchConfig
    ai/          mirror controller; bosses/ one module each
    effects/     EffectPool, spawn table; one module per effect
    meter/       chaos meter and catch-up bias           (C1)
    rooms/       RoomDescriptor, composition loader       (C3)
    items/       pick-ups, inventory, fire                (C2)
    beat/        BeatClock port and beat-driven triggers  (C4)
    persistence/ model, versions, migrations, io
    tuning/      schema, embedded load, dev override
    sanity/      invariant checks, SimFault, heal
  app/           plugin group, states, phase sets
  testing/       headless app builder, fake ports         (cargo feature)

pong/src/
  ports/         keyboard adapter, save paths, beat clock source
  views/         one per core feature; effect view spawn table
  audio/         cue map over core events and states
  ui/            one screen per state; seat cue
  overlay/
  main.rs        assembles core plugin group plus shell plugins
```

Alternatives considered: flat feature files with a shared prelude (rejected: prelude becomes a god module, ordering drifts, physics not callable without ECS); strict ports-and-adapters with trait ports (rejected: duplicates resources and events, contradicts ports-as-resources, demotes ECS); one module per milestone (rejected: milestones are time, not concerns).

## Simulation model

- Core owns `Position` and `Velocity`. The shell writes `Transform` from `Position`, interpolating between fixed steps. One entity, two components.
- Teleports (point reset, portals later) set a snap marker so no interpolation crosses the jump.
- Simulation runs in `FixedUpdate`. Input is latched into intents in `Update` and consumed in `FixedUpdate`, so momentary presses are neither missed nor doubled.
- A seeded random resource is the only source of randomness. The shell seeds from entropy; tests seed deterministically.
- Physics is hand-rolled as pure functions over plain data; systems are thin wrappers. The AI calls the same functions to predict.
- Collision is swept circle-versus-rectangle, and ball speed is capped, because escalation makes the ball fast by design.
- Rotating-arena effects, if built, rotate the ball's frame of reference rather than the geometry, keeping collision axis-aligned. If a later room needs arbitrary polygons, avian2d is adopted then behind the same force and collision-event vocabulary.

## Effects

- `EffectId` is a serializable enum with stable string names. It is the save key, the room vocabulary, and the item payload.
- `EffectId` plus params maps through a spawn table to a component bundle. Behavior lives in systems over those components, one system per behavior where needed.
- Field effects accumulate into a per-step force on the ball.

## Boundaries

- Core emits events (`BallHitPaddle`, `PointScored`, `EffectStarted`, `EffectEnded`, and so on). Shell plugins observe them. Core never calls the shell.
- Three ports flow shell to core as resources: paddle intents from the keyboard adapter, the beat clock at C4 from whichever audio engine owns time, and `SavePaths` from the XDG directories. Tests write fakes into the same resources.
- Audio is a shell plugin that observes core events and app state. Music cues derive from state transitions. Volume is a setting. No audio type exists in core. This amends the design record's facade wording; the substance of that decision stands: built-in `bevy_audio` now, swap at C4 touches one plugin.

## Input

- Raw `ButtonInput<KeyCode>` behind an action enum. No third-party input crate.
- Two seats, left and right, each with a key set defined in tuning data so bindings can change if ghosting is found.
- Menus use a single cursor. Profile selection is sequential per seat with a visible seat cue; during a seat's turn only that seat's keys drive the cursor. One-human modes have the single seat pick both sides.
- Escape pauses or goes back from anywhere.
- Window focus loss clears all intents and pauses, only while in the Playing substate.

## States

Bevy `States` and `SubStates`: `Boot` (waits for settings, profiles, tuning), `Menu`, `InMatch` with substates `Countdown`, `Playing`, `Paused`, `PointScored`, `MatchOver`. Match mode, seat bindings, and score target live in a `MatchConfig` resource.

## Persistence

- Core performs file IO through an injected `SavePaths` resource using `std::fs`. The shell computes paths; tests inject a temp directory.
- Two files: settings per device in the XDG config directory; profiles per child in the XDG data directory.
- The save model is an explicit versioned enum with a sequential migration chain. No serde field defaults for migration.
- One fixture file per released version lives in the core test tree. A test loads each against a golden model; another asserts fixture count equals the current version.
- Corrupt or newer-version file: rename with a timestamp suffix, start with an empty model, show one plain sentence in the menu. Never crash, never overwrite.
- Profiles are open-ended. A built-in, non-persisted Guest always exists; a Guest side records nothing, the opposing child's records still count.

## Tuning and assets

- Tuning is one RON file embedded in the binary, the single source of truth. In dev builds it is overridden from the source tree with hot reload via `embedded_watcher`. Release reads it once at boot.
- All assets are embedded: fonts, sounds, tuning, later music. The binary is self-contained and no `assets/` folder ships.

## Presentation

- All simulation, tuning, and AI values are in arena units, never pixels. The camera fits the arena with letterboxing. UI scales with viewport-relative units.
- A debug overlay on a function key shows fixed-step rate, ball speed, AI target and error, active effects, and meter value. It ships in release builds, hidden by default.

## Error policy

Decided 2026-10-30 after review found the earlier one-line policy lacked a taxonomy.

Principles:

1. A child never sees a crash or a stack trace.
2. A developer sees every bug the first time it happens.
3. One code path in both builds; only the handler differs. Dev builds panic on invariants and faults; release builds heal and log.
4. The player is told only what they can act on, in one plain sentence, via a `Notice` core event rendered by the shell.
5. Progress is never half-written: atomic write via temp file and rename.
6. Every error class has a test, or the class is made impossible by construction.

Mechanism: typed error enums only where a caller branches (persistence load and save, tuning validation, audio init). Interior systems return `Result<(), BevyError>` with context and go to Bevy's app-level error handler, set per build profile. Invariant checks panic in dev and emit a `SimFault` event in release, answered by a heal system that logs the state at error level. Tuning and save data use validated constructors, so invalid values cannot enter. A panic hook writes a crash file to the XDG state directory.

| Class | Example | Detect | Dev | Release | Notice | Test |
|---|---|---|---|---|---|---|
| Programmer invariant | missing component, impossible transition | `?` in a `Result` system, invariant macro | panic | log error, skip system this step | none | headless App tests |
| Simulation fault | NaN velocity, ball outside arena | sanity system each fixed step | panic | reset rally, log | none | seeded soak test with random intents |
| Persistence read | corrupt, newer version, unreadable | `LoadError::{Corrupt, NewerVersion, Io}` | as release | back up, fresh model | one line | fixture per kind |
| Persistence write | disk full, permission | `SaveError::Io` | as release | keep in memory, retry at next match end | once per session | unwritable injected path |
| Data validation | tuning out of range, negative wins | validated constructor | CI test fails on embedded file; dev override keeps last good values, shows banner | cannot occur for embedded; save treated as Corrupt | none | CI range-checks embedded tuning |
| Audio init | no device | `Result` at plugin startup | log, continue silent | silent mode | none | shell test with audio disabled |
| Render or window loss | GPU lost, no window | Bevy render error handler | exit with log | exit with log and crash file | impossible | none, accepted |
| Uncaught panic | anything missed above | panic hook | default | crash file, exit | none | none, accepted; data safe by principle 5 |

The soak test runs with panic enabled, so a fault that release would heal still fails CI.

Alternatives considered: Bevy-native `Result` everywhere with a warn handler (rejected: a system erroring every step runs broken forever, and the save policy cannot branch on a boxed error); making every error impossible by type (adopted for data only; disk, device, and `f32` NaN cannot be typed away); a supervisor process that restarts on crash (rejected: a restart mid-match is a crash to a child).

## Testing and CI

- Unit tests on pure functions: physics, AI prediction, migrations.
- Headless `App` tests in core per plugin.
- Shell adapter tests headless where possible (input mapping, state transitions).
- CI: fmt check, clippy with warnings denied, tests, release build of the shell. `dynamic_linking` is dev-only and gated off in release.

## Playtesting

A playtest document accompanies the specs: per milestone, ten minutes per child; observe rather than ask; write three observations and verbatim quotes before interpreting; change at most one thing per session. The two-cursor menu idea is recorded there as a hypothesis to test if the kids fight over the menu.

## Deferred to specs

Arena unit size, exact key maps, file names, the event list, overlay contents, the shape of the spawn table, and the tuning schema.
