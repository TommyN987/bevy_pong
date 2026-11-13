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

| Candidate                    | Shape                                                                                                         | Outcome                                                                                        |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Arch-1 lean idiomatic        | single crate, `Transform` as state, fixed step without interpolation, effects as components                   | not chosen: no stable effect identity, visible judder                                          |
| Arch-2 core and shell        | workspace, render-free core, logical position with interpolated view, effect descriptors, hand-rolled physics | **chosen, refined below**                                                                      |
| Arch-3 engine-leaning        | avian2d, leafwing input, hanabi, thin game code                                                               | not chosen: AI cannot share avian's math (constraint 4); three crates on the upgrade treadmill |
| Arch-4 plain-Rust simulation | simulation outside ECS, Bevy only renders                                                                     | rejected: abandons the frame's ECS learning goal                                               |

Refinements applied to Arch-2 during review: depend on `bevy` with feature flags rather than sub-crates; the effect descriptor is a spawn table, not an interpreter; the upgrade-isolation argument for the split was withdrawn, the split is justified by test speed and by making render types unreachable from simulation code.

## Precondition

A spike runs before milestone one: a throwaway workspace whose core depends on `bevy` with default features off, `cargo tree` asserted free of `wgpu`, and one `MinimalPlugins` test that deserializes a RON tuning file.
If the core cannot be built render-free, the workspace collapses to a single crate and every other decision here still holds.

## Crate structure

Workspace with two crates.

- **`pong_core`**: simulation, effects, AI, states, persistence, tuning types. Depends on `bevy` with default features off and only `std`, `async_executor`, `bevy_asset`, `bevy_state`, `bevy_color`, `keyboard`, and `serialize` on; ADR 0001 gates the list. Never depends on render, window, audio, or UI.
- **`pong`** (shell, a library plus a binary): window, rendering, view sync, input adapters, audio plugin, UI, overlay. The library holds the plugins; `main.rs` assembles them and tests import them. Depends on `pong_core` and on `bevy` with `2d`, `2d_api`, `ui`, `audio`, `keyboard`, `tonemapping_luts`, `zstd_rust`.

Modules are organized by feature as Bevy plugins, not by layer. The intra-crate guide below refines this.

## Intra-crate structure

Accepted 2026-10-30 as an orientational guide. The ten rules bind; the directory shape does not.

Forced by earlier decisions: a Bevy-free math floor, because the AI calls the ball's physics directly; named fixed-step phases, because five milestones add systems to one step; exhaustive enums at extension points, because an effect without visuals or AI awareness is a silent gap.

Rules:

1. **Feature-first, dependency-down.** Features depend on kernel, vocabulary, and flow. Kernel depends on nothing in the crate. Flow depends only on kernel and vocabulary. The app roof depends on everything; nothing depends on it. A feature may import the public components, resources, and messages of features earlier in the order declared in `features/mod.rs`, never their systems or plugin internals. Amended 2026-11-04 by ADR 0002.
2. **The kernel is ECS-free.** `bevy_math` types are allowed; apps, worlds, systems, and schedules are not. Pure functions and plain data only.
3. **Vocabulary stays tiny.** Only types used by two or more features: seat, side, effect identity, core events. `Side` is defined in `kernel::geometry`, which needs it first (`pong_core/src/domain.md`). A type used by one feature lives in that feature. If it passes a dozen types, it has become a god module. The dozen counts non-message types; each message has exactly one emitting feature, `Notice` excepted (ADR 0002). Identifier types are outside the dozen (ADR 0008).
4. **Phases are declared once,** in `flow/` (ADR 0002). Fixed step runs named sets in order: intent, effects, integrate, collide, rules, sanity. Every `FixedUpdate` system names its phase. The step gate, in `FixedFirst`, decides whether the phases run and is the one fixed-step system outside them (ADR 0003). A system that serves, moves, collides, or judges the ball also joins `BallInPlay` (ADR 0004). No system orders itself relative to another feature's system.
5. **Components own their invariants.** Mutation happens through the component's API from any feature. Cross-feature triggers are events, never direct calls.
6. **Extension points are exhaustive enums.** Effect, boss, and item identities. Adding a variant makes the compiler enumerate every spawn table, cue map, and AI branch to extend. Amended 2026-11-10 by [ADR 0011](adr/0011-effect-identity-and-contract.md): for effects, only identity is a closed enum; behaviour sits behind a per-effect trait with a core half and a shell half, and each crate registers effects in one exhaustive match.
7. **A feature is a directory with a plugin at its root.** Growable sub-concepts (individual effects, bosses) are submodules with a uniform shape: components, systems, spawn function.
8. **The shell mirrors core.** One view module per core body (arena, ball, paddle), beside presentation modules that serve them all (camera, motion, trail, shake); one screen per state; the overlay apart. A port is written by its adapter in `ports/` or by a named screen (the pause screen writes `MatchControl`, as focus loss does). The shell's spawn table matches the same enum as core's. Amended 2026-11-09 at milestone close: the rule said one view per core feature and one adapter per port, which M1's shell outgrew. Amended 2026-11-13 (ADR 0011, [M2 phase 4 report](plans/m2-phase4-report.md)): no spawn table exists; the shell's one match on the effect enum is its registration of each effect's shell half, as the core's is of each core half.
9. **Tests live beside the feature.** A shared testing module behind a cargo feature builds a minimal app with chosen plugins and fake ports.
10. **Size decides depth.** A floor is a file until it needs a directory.

Orientational shape at the final milestone:

```
pong_core/src/
  kernel/        geometry, integrate, collide, reflect, flight, ramp, predict, random   (no ECS)
  domain/        Seat, EffectId, BossId, ItemId, events
  flow/          AppState, MatchState, phase sets, step gate, FIXED_HZ   (ADR 0002, 0003)
  features/
    seats/       intent port and latch, per-side paddle intents
    ball/        Ball, Velocity, Position, snap marker
    paddle/      Paddle with clamped size API
    physics/     systems wrapping kernel
    rules/       scoring, match-flow transitions, pause port, fault heal
    ai/          mirror controller; bosses/ one module each
    effects/     effect contract and registration, the flight on the ball, sandbox port; one module per effect   (C1)
    escalation/  breadth schedule and rally hit count     (C1; first drawn as meter/, amended 2026-11-11)
    rooms/       RoomDescriptor, composition loader       (C3)
    items/       pick-ups, inventory, fire                (C2)
    beat/        BeatClock port and beat-driven triggers  (C4)
    persistence/ model, document (versions, migrations), load (load policy), io
    tuning/      schema, embedded load, dev override
    sanity/      invariant checks
  app/           plugin group, error policy, Boot gate
  testing/       headless app builder, fake ports         (cargo feature)

pong/src/
  ports/         keyboard adapter, focus, save paths; beat clock source   (C4)
  views/         arena, ball, paddle bodies; camera, motion (interpolation), trail, shake
  effects/       each effect's shell half: icon, join cue, own view; the shell's one match
                 on the effect enum   (C1)
  audio/         cue map over core events; synthesized sound bank; state and music cues   (C4)
  ui/            one screen per state; HUD and its effects, notice line, menu input, seat cue,
                 font, style
  overlay/       developer overlay and sandbox keys, F3
  window.rs      window mode, Alt+Enter toggle
  crash.rs       panic hook, crash file
  xdg.rs         XDG directories
  seed.rs        entropy seed for the core
  lib.rs         the shell's plugins, as a library
  main.rs        assembles core plugin group plus shell plugins
```

Alternatives considered: flat feature files with a shared prelude (rejected: prelude becomes a god module, ordering drifts, physics not callable without ECS); strict ports-and-adapters with trait ports (rejected: duplicates resources and events, contradicts ports-as-resources, demotes ECS); one module per milestone (rejected: milestones are time, not concerns).

## Simulation model

- Core owns `Position` and `Velocity`. The shell writes `Transform` from `Position`, interpolating between fixed steps. One entity, two components.
- Teleports (point reset, portals later) set a snap marker so no interpolation crosses the jump.
- Simulation runs in `FixedUpdate` at one rate, `FIXED_HZ = 60`, in core and tests alike. A step runs while the paddles move, in play and between rallies; the ball's systems run only in play, through the `BallInPlay` set; the step gate closes once a transition is pending (ADR 0003, ADR 0004).
- Input is latched into intents once per frame, before the fixed steps and whenever the paddles move (in play and between rallies), and consumed in `FixedUpdate`, so momentary presses are neither missed nor doubled and every step reads that frame's keys. Amended 2026-11-04 (plan 4.1; phase 4 report) and 2026-11-07 (ADR 0004).
- A seeded random resource is the only source of randomness. The shell seeds from entropy; tests seed deterministically.
- Physics is hand-rolled as pure functions over plain data; systems are thin wrappers. The AI calls the same functions to predict.
- Collision is swept circle-versus-rectangle, and ball speed is capped, because escalation makes the ball fast by design.
- Rotating-arena effects, if built, rotate the ball's frame of reference rather than the geometry, keeping collision axis-aligned. If a later room needs arbitrary polygons, avian2d is adopted then behind the same force and collision-event vocabulary.

## Effects

- `EffectId` is a serializable enum with stable string names. It is the save key, the room vocabulary, and the item payload.
- `EffectId` plus params maps through a spawn table to a component bundle. Behavior lives in systems over those components, one system per behavior where needed.
- Field effects accumulate into a per-step force on the ball.

Amended 2026-11-11 by [ADR 0011](adr/0011-effect-identity-and-contract.md) and the [M2 spec](specs/m2-chaos-pong.md): the spawn table and the per-step force above are superseded. An effect implements a trait and contributes to composition channels. Its parameters change only at a flight start (a serve or a paddle hit). A field is a pure function of the ball's motion and time in flight, evaluated by physics and the AI's prediction alike; M1's `Force` accumulator is removed. Classic's speed ramp is the first effect.

## Boundaries

- Core emits events (`BallHitPaddle`, `PointScored`, `EffectStarted`, `EffectEnded`, and so on). Shell plugins observe them. Core never calls the shell.
- Four ports flow shell to core as resources: paddle intents from the keyboard adapter, pause and resume requests through `MatchControl` (ADR 0003), the beat clock at C4 from whichever audio engine owns time, and `SavePaths` from the XDG directories. Tests write fakes into the same resources. Besides the ports, the menu writes the match's `MatchConfig` and the player's `Settings` and `Profiles`, which persistence saves, and the shell moves `AppState` out of `Menu` and `InMatch`; rules alone moves `MatchState` (ADR 0003). Amended 2026-11-09 at milestone close. Amended 2026-11-12 ([M2 phase 3 report](plans/m2-phase3-report.md)): a fifth port, the developer `Sandbox`, is written by the overlay's keys; it forces the effects in play from the next countdown and holds the return count from the next flight start, and a match it touched records nothing.
- Audio is a shell plugin that observes core events and app state ([ADR 0010](adr/0010-audio-boundary.md)). Music cues derive from state transitions. Volume is a setting. No audio type exists in core. This amends the design record's facade wording; the substance of that decision stands: built-in `bevy_audio` now, swap at C4 touches one plugin.

## Input

- Raw `ButtonInput<KeyCode>` behind an action enum. No third-party input crate.
- Two seats, left and right, each with a key set defined in tuning data so bindings can change if ghosting is found.
- Menus use a single cursor. Profile selection is sequential per seat with a visible seat cue; during a seat's turn only that seat's keys drive the cursor. One-human modes have the single seat pick both sides.
- Escape pauses or goes back from anywhere.
- Window focus loss clears all intents and pauses wherever the paddles move: `Countdown`, `Playing`, `PointScored` (ADR 0004).

## States

Bevy `States` and `SubStates`: `Boot` (waits for settings, profiles, tuning), `Menu`, `InMatch` with substates `Countdown`, `Playing`, `Paused`, `PointScored`, `MatchOver`. Match mode, seat bindings, who each seat plays as (ADR 0008), and score target live in a `MatchConfig` resource.
Rules owns every `MatchState` transition; the shell requests pause and resume through `MatchControl`, and resume returns to the substate paused from (ADR 0003).

## Persistence

- Core performs file IO through an injected `SavePaths` resource using `std::fs`. The shell computes paths; tests inject a temp directory.
- Two files: settings per device in the XDG config directory; profiles per child in the XDG data directory.
- The save model is an explicit versioned enum with a sequential migration chain. No serde field defaults for migration.
- One fixture file per released version lives in the core test tree. A test loads each against a golden model; another asserts fixture count equals the current version. Amended by ADR 0007: each file kind has its own version chain and fixtures, and the file format is specified there.
- Corrupt or newer-version file: rename with a timestamp suffix, start with an empty model, show one plain sentence in the menu. Never crash, never overwrite.
- Profiles are open-ended. A built-in, non-persisted Guest always exists; a Guest side records nothing, the opposing child's records still count.

## Tuning and assets

- Tuning is one RON file embedded in the binary, the single source of truth. In dev builds it is overridden from the source tree with hot reload via `embedded_watcher`. Release reads it once at boot.
- Fonts and tuning are embedded, later music; sounds are synthesized in code. The binary is self-contained and no `assets/` folder ships.

## Presentation

- All simulation, tuning, and AI values are in arena units, never pixels. The camera fits the arena with letterboxing. UI scales with viewport-relative units.
- A debug overlay on a function key shows fixed-step rate, ball speed, AI target and error, active effects, and the rally's hit count (first written as "meter value", amended 2026-11-11). It ships in release builds, hidden by default. Amended 2026-11-12 ([M2 phase 3 report](plans/m2-phase3-report.md)): while shown in a match it is also the sandbox of M2 spec 2.9.1, through the `Sandbox` port. Amended 2026-11-13 ([M2 phase 4 report](plans/m2-phase4-report.md)): hiding the overlay releases the sandbox, as leaving the match does, and the keys do nothing once a match is over.

## Error policy

Decided 2026-10-30 after review found the earlier one-line policy lacked a taxonomy. Recorded as [ADR 0009](adr/0009-error-policy.md).

Principles:

1. A child never sees a crash or a stack trace.
2. A developer sees every bug the first time it happens.
3. One code path in both builds; only the handler differs. Dev builds panic on invariants and faults; release builds heal and log.
4. The player is told only what they can act on, in one plain sentence, via a `Notice` core event rendered by the shell.
5. Progress is never half-written: atomic write via temp file and rename.
6. Every error class has a test, or the class is made impossible by construction.

Mechanism: typed error enums only where a caller branches (persistence load and save, tuning validation). Audio init needs none: Bevy's audio plugin handles a missing device itself (amended 2026-11-08, [phase 9 report](plans/m1-phase9-report.md)). Interior systems return `Result<(), BevyError>` with context and go to Bevy's app-level error handler, set per build profile. The sanity system writes a `SimFault` and returns it as its error in both builds; the handler panics in dev and logs it at error level in release, and rules answers the message by restarting the rally (amended 2026-11-09 at milestone close, [ADR 0009](adr/0009-error-policy.md)). Tuning and save data use validated constructors, so invalid values cannot enter. A panic hook writes a crash file to the XDG state directory.

| Class                 | Example                                  | Detect                                    | Dev                                                                               | Release                                            | Notice           | Test                                     |
| --------------------- | ---------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------- | ---------------- | ---------------------------------------- |
| Programmer invariant  | missing component, impossible transition | `?` in a `Result` system; clamping setters; sanity system (amended 2026-11-09 at milestone close) | panic                                                                             | log error, skip system this step                   | none             | headless App tests                       |
| Simulation fault      | NaN velocity, ball outside arena         | sanity system each fixed step             | panic                                                                             | reset rally, log                                   | none             | seeded soak test with random intents     |
| Persistence read      | corrupt, newer version, unreadable       | `LoadError::{Corrupt, NewerVersion, Io}`  | as release                                                                        | back up, fresh model                               | one line         | fixture per kind                         |
| Persistence write     | disk full, permission                    | `SaveError::Io`                           | as release                                                                        | keep in memory, retry at next match end or menu change (amended 2026-11-09 at milestone close) | once per session | unwritable injected path                 |
| Data validation       | tuning out of range, negative wins       | validated constructor                     | CI test fails on embedded file; dev override keeps last good values, says why in one `Notice` (amended 2026-11-09 at milestone close) | cannot occur for embedded; save treated as Corrupt | one line, dev override only (amended 2026-11-09 at milestone close) | CI range-checks embedded tuning          |
| Audio init            | no device                                | Bevy's `AudioPlugin`: warns, plays nothing | log, continue silent                                                              | silent mode                                        | none             | shell test with audio disabled           |
| Render or window loss | GPU lost, no window                      | Bevy render error handler                 | exit with log                                                                     | exit with log and crash file                       | impossible       | none, accepted                           |
| Uncaught panic        | anything missed above                    | panic hook                                | crash file, then default                                                          | in a system: crash file, log, skip the system; elsewhere: crash file, exit | none | forced panic writes the file, by test |

Amended 2026-11-07 (phase 4 report): Bevy catches a panic inside a system and hands it to the error handler, so in release play goes on; the hook writes the crash file in both builds, once per process.

The soak test runs with panic enabled, so a fault that release would heal still fails CI.

Alternatives considered: Bevy-native `Result` everywhere with a warn handler (rejected: a system erroring every step runs broken forever, and the save policy cannot branch on a boxed error); making every error impossible by type (adopted for data only; disk, device, and `f32` NaN cannot be typed away); a supervisor process that restarts on crash (rejected: a restart mid-match is a crash to a child).

## Testing and CI

- Unit tests on pure functions: physics, AI prediction, migrations.
- Headless `App` tests in core per plugin.
- Shell adapter tests headless where possible (input mapping, state transitions).
- CI: `just ci-check` (fmt check, layering check, clippy with warnings denied, nextest and doctests, cargo-deny), then a release build of the shell, checked with `ldd` to link no `bevy_dylib`. `dynamic_linking` is dev-only and gated off in release.

## Playtesting

A playtest document accompanies the specs: per milestone, ten minutes per child; observe rather than ask; write three observations and verbatim quotes before interpreting; change at most one thing per session. The two-cursor menu idea is recorded there as a hypothesis to test if the kids fight over the menu.

## Deferred to specs

Arena unit size, exact key maps, file names, the event list, overlay contents, the shape of the spawn table, and the tuning schema.
