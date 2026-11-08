# M1 phase 7 report: persistence

Started 2026-11-08. Branch `feat/persistence`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.4.9, architecture "Persistence" | Each file kind has its own version chain and fixtures, `<kind>_v<N>.ron`, not one save version with `save_v<N>.ron`. The user chose it: settings and profiles change at different times. | ADR 0007; `.claude/rules/tests.md` and architecture.md amended 2026-11-08. |
| 1.4.9 | The version is a top-level field read by a probe before the strict parse; the versioned enum is built after. A version only as the enum variant in the file would make a newer file indistinguishable from a corrupt one. | ADR 0007. |
| architecture rule 7 | The persistence feature has no plugin in 7.1: nothing in it runs in the app yet. Plan 7.2 adds it with the load systems. | This report. |
| CLAUDE.md non-negotiable 1 | `Alpha::with_alpha` (bevy_color) was written without a `bevy-api-check` round; the crate compiled against the pinned 0.20.1 source. | This report. |

## Decisions taken in this phase

- Plan 7.1 (2026-11-08, approved by the user): models v1, RON documents, atomic write, fixtures, count tests, the `SavePaths` port, the harness's `TempSaveDir`. ADR 0007 records the format.
- `SavePaths` holds two directories and names the files; the shell maps the XDG config and data directories to it (`pong::ports::save_paths`) and inserts it in `main`. Without a home directory it inserts nothing; plan 7.2 decides what the game does then.
- Profile names are 1 to 12 characters, counted as characters; colours are finite and not negative, as the palette's; profile ids are unique; records are unsigned.
- The parent directory is not synced after the rename (ADR 0007): atomicity holds without it, and an error after the rename would break the contract that an error leaves the old file.
- Every test of behaviour built before it was checked against a deliberate break: byte-counted names, a sorted profile list, every unknown version as corrupt, a version 1 shape without `deny_unknown_fields`, and a write straight into the target file each fail their test.
