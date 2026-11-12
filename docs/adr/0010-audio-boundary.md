# ADR 0010: Audio boundary

Date: 2026-11-09
Status: accepted

## Context

Headless tests run on `MinimalPlugins`, which has no audio, so nothing that makes sound may live where tests run (`docs/design.md`, "built-in audio behind a game-facing boundary").
Bevy's audio is built on rodio, and Bevy discusses a Firewheel-based replacement; C4's beat clock may need a different backend.
There is no sound designer: the sounds are made in code (spec, Decisions).
Spec 1.9.1 asks for this record; the decision was taken in the design round (2026-10-30), amended in the architecture round, and built in phase 9 (2026-11-08, approved by the user).

## Decision

Built-in `bevy_audio` now, reached only through the core's event vocabulary: audio is a shell plugin that observes core messages; the core has no audio type.

- `pong::audio`: `Cue` names what a sound tells the players (paddle hit, wall hit, point, match won); the cue map spawns one player per `BallHitPaddle`, `BallHitWall`, `PointScored`, `MatchEnded`, and imports cue names and their sounds, never how a sound is made.
- `SoundBank` makes the sound for each cue; `Synth` is the bank, sine notes made when the app is built. A bank from embedded files would decode into the same `Sound`.
- `Sound` is our own `Decodable` asset of mono `f32` samples, handed to rodio as they are: no file format, no new dependency.
- `SoundsPlugin` adds Bevy's playback for `Sound` only after Bevy's `AudioPlugin`; without it, as in headless tests, the cues spawn and nothing plays.
- Volume is the settings' percent, set on each cue when it starts.
- The final point plays the point cue, then the win fanfare, which waits out the point cue.

## Alternatives considered

- Synthesized samples written as WAV bytes into Bevy's `AudioSource`: rejected; it needs Bevy's `wav` feature (adding `hound`), a WAV writer, and Bevy's decoder panics if the format feature is off.
- Bevy's `GlobalVolume` for the master volume: rejected; it too applies only when a sound starts, and exists only with `AudioPlugin`, so a headless test could not read it.
- Built-in audio with no boundary, systems emitting sound directly (design round): rejected; they could not run in headless tests.
- Kira from M1, with or without a boundary (design round): rejected; a third-party dependency on the upgrade treadmill four milestones before its clock is needed.
- A facade the game calls (design round, first wording): replaced in the architecture round by the event vocabulary, which needs no audio type in core.

## Consequences

- Swapping the backend at C4 touches `pong::audio` only; `Sound`'s samples survive, its rodio `Source` does not.
- Tests assert one play command per event with audio disabled; how a sound sounds is judged by ear.
- With no audio device, Bevy warns and plays nothing, and its cleanup is skipped too: each cue leaves one entity, a few hundred per match.
- A new cue needs a `Cue` variant, a bank sound, and a message to map; the exhaustive matches point at each.
  Amended 2026-11-12 ([M2 phase 3 report](../plans/m2-phase3-report.md)): an effect's join cue is not a `Cue`. It is a `Sound` in the effect's shell half (`pong::effects`), synthesized through `synth::phrase`, and the cue map plays it on `EffectStarted`. A `Cue` per effect would have made the bank match on `EffectId`, a second match in the crate (ADR 0011). A bank from files would need the shell halves to name files instead.
