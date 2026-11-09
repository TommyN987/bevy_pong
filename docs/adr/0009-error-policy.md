# ADR 0009: Error policy

Date: 2026-11-09
Status: accepted

## Context

The players are 9 and 11; the developer must still see every bug (`docs/frame.md`).
An earlier one-line policy lacked a taxonomy; review during the architecture round asked for one.
The policy was decided there (2026-10-30, `docs/architecture.md` "Error policy") and built in phases 3, 4, 7, and 9; spec 1.9.1 asks for its record.

## Decision

One code path in both builds; only the handler differs: a dev build panics at the first error, a release build logs it and plays on.

- Principles:
  a child never sees a crash or a stack trace;
  a developer sees every bug the first time it happens;
  the player is told only what they can act on, in one plain sentence, as a `Notice`;
  progress is never half-written;
  every error class has a test, or is made impossible by construction.
- Typed error enums only where a caller branches: persistence load and save (`LoadError`, `DecodeError`, `SaveError`), tuning validation (`TuningError`, `TuningLoadError`).
- Interior systems return `Result<(), BevyError>` with context, to Bevy's app-level handler, set per build by `ErrorPolicy::{Dev, Release}` (`pong_core::app::error_policy`).
- Simulation faults (NaN, a ball outside the arena) are found by the sanity phase each fixed step, written as `SimFault`, and returned as the system's error: the dev handler panics; the release handler logs, and rules answers the message by restarting the rally.
- Tuning and save data enter only through validated constructors; a save value the model refuses makes the file `Corrupt`.
- A panic hook writes a crash file to the XDG state directory in both builds, once per process (`pong::crash`); Bevy hands a panic inside a system to the handler, so release plays on (phase 4).
- Saves are written to a temp file, synced, and renamed over the target (ADR 0007).
- Audio init needs no error of ours: Bevy's `AudioPlugin` warns "No audio device found." and plays nothing (phase 9; ADR 0010).

## Alternatives considered

- Bevy-native `Result` everywhere with a warn handler: rejected; a system erroring every step runs broken forever, and the save policy cannot branch on a boxed error.
- Making every error impossible by type: adopted for data only; disk, device, and `f32` NaN cannot be typed away.
- A supervisor process that restarts on crash: rejected; a restart mid-match is a crash to a child.

## Consequences

- The soak test runs with panics on, so a fault release would heal still fails CI.
- Each class in the architecture's error table names its test; a new class needs a row and a test.
- Release logs are the only trace of a healed fault: a playtest bug report starts from the log and the crash file.
- A future fallible feature (C1's effects, C4's beat clock) picks a row of the table rather than inventing handling.
