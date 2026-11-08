# ADR 0007: Persistence format

Date: 2026-11-08
Status: accepted

## Context

Plan 7.1 needs the save files' on-disk shape before 7.2 can load and write them.
The save is a versioned document owned by the game (architecture constraint 7, `docs/design.md` "persistence lands in Basic Pong").
`docs/architecture.md` "Persistence" fixes two files, an explicit versioned enum with a sequential migration chain, no serde defaults, and fixtures per released version.
`docs/design.md` couples the save payload to Bevy upgrade churn.
7.2's load policy must tell a file from a newer game apart from a corrupt one (spec 1.4.9).

## Decision

RON, one file per kind, each kind on its own version chain; the version is a top-level field read before the rest.

- Files: `settings.ron` in the XDG config directory, `profiles.ron` in the XDG data directory.
- `SavePaths` holds the two directories; the core names the files, since the format is the core's.
- Every file opens with `version: N`. Decoding reads that field alone through a probe that ignores the rest, then:
  a known version parses strictly as that version's frozen shape (`deny_unknown_fields`, no serde defaults),
  wraps in the kind's versioned enum (`ProfilesDocument::V1`, `SettingsDocument::V1`),
  and migrates one version at a time to the current model through its validating constructors.
- A version above the one this game writes is `NewerVersion`; anything else that fails, an unknown older version or a value the model refuses included, is `Corrupt`.
- Each released version's file types live in their own module (`v1`) and never change; version 2 adds `v2`, a variant, and a `v1 → v2` step.
- Colours are stored as the game's own `(red, green, blue)`, not Bevy's colour serialization; profiles are opaque.
- Volume is a whole percent, `0..=100`, default 80; window mode defaults to borderless fullscreen.
- Fixtures: `pong_core/tests/fixtures/<kind>_v<N>.ron`, never edited once released; per kind, a test asserts the fixture versions are exactly `1..=current`.
- Writes go to `<name>.tmp` beside the target, are synced, then renamed over it. The directory is not synced.

## Alternatives considered

- One save version shared by both files: rejected; any change to one file would bump the other, and C1's unlocks touch profiles only.
- The version only as the enum variant in the file (`V1(...)`): rejected; a file from a newer game would be an unknown variant, a parse error like any corruption.
- Bevy's `Srgba` serde form in the file: rejected; a Bevy rename would change the save format.
- Volume as a float `0.0..=1.0`: rejected; a whole percent round-trips exactly and the settings screen steps by ten.
- Syncing the parent directory after the rename: proposed, then dropped; a failure after the rename would break the contract that an error leaves the old file, and atomicity holds without it.
- The `tempfile` crate for test directories: rejected (no new dependency); the harness's `TempSaveDir` uses std only.

## Consequences

- 7.2 branches on `DecodeError::{NewerVersion, Corrupt}` and adds the IO case.
- A power cut just after a rename may bring back the previous file, always complete.
- A format change means a new frozen module, a migration step, a version bump, and a fixture; the count test fails until the fixture exists.
- The fixture naming in `.claude/rules/tests.md` changed from `save_v<N>.ron` to `<kind>_v<N>.ron`.
- Hand-editing a save file is possible but unforgiving: an extra or missing field makes it corrupt.
