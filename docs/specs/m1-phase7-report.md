# M1 phase 7 report: persistence

Started 2026-11-08. Branch `feat/persistence`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.4.9, architecture "Persistence" | Each file kind has its own version chain and fixtures, `<kind>_v<N>.ron`, not one save version with `save_v<N>.ron`. The user chose it: settings and profiles change at different times. | ADR 0007; `.claude/rules/tests.md` and architecture.md amended 2026-11-08. |
| 1.4.9 | The version is a top-level field read by a probe before the strict parse; the versioned enum is built after. A version only as the enum variant in the file would make a newer file indistinguishable from a corrupt one. | ADR 0007. |
| architecture rule 7 | The persistence feature had no plugin in 7.1: nothing in it ran in the app yet. Plan 7.2 added `PersistencePlugin`. | This report; resolved 2026-11-08. |
| plan 7.2, tdd | The green step for the first-start test built the whole load policy (set-aside, stuck, no `SavePaths`), not only what that test needed. Their tests followed, each checked against a deliberate break: no notice on set-aside, no notice without `SavePaths`, a stuck file treated as writable, and a notice on every failed write each fail their test. | This report. |
| plan 7.2 | The dev entry's `log_notices` has no test, like its `log_score`: it only logs, and it leaves with the dev entry in plan 8.3. | This report. |
| 1.3.3, 1.4.5, ADR 0002 | `MatchConfig` carries who each seat plays as; `ProfileId` moved to domain with `Player` and `SeatPlayers`; identifier types are outside the vocabulary cap. Chosen by the user over a separate persistence resource and over `Controller::Human { seat, player }`. | ADR 0008; ADR 0002, spec 1.3.3 and 1.4.5, architecture.md amended 2026-11-08. |
| CLAUDE.md non-negotiable 1 | `Alpha::with_alpha` (bevy_color) was written without a `bevy-api-check` round; the crate compiled against the pinned 0.20.1 source. | This report. |

## Decisions taken in this phase

- Plan 7.1 (2026-11-08, approved by the user): models v1, RON documents, atomic write, fixtures, count tests, the `SavePaths` port, the harness's `TempSaveDir`. ADR 0007 records the format.
- `SavePaths` holds two directories and names the files; the shell maps the XDG config and data directories to it (`pong::ports::save_paths`) and inserts it in `main`. Without a home directory it inserts nothing; plan 7.2 decides what the game does then.
- Profile names are 1 to 12 characters, counted as characters; colours are finite and not negative, as the palette's; profile ids are unique; records are unsigned.
- The parent directory is not synced after the rename (ADR 0007): atomicity holds without it, and an error after the rename would break the contract that an error leaves the old file.
- Every test of behaviour built before it was checked against a deliberate break: byte-counted names, a sorted profile list, every unknown version as corrupt, a version 1 shape without `deny_unknown_fields`, and a write straight into the target file each fail their test.
- Plan 7.2 (2026-11-08, approved by the user): load both files on entering `Boot`; the Boot gate waits for `Profiles` and `Settings` (spec 1.4.10). A file that cannot be used is renamed to `<name>.<unix-seconds>.bak` (`-1`, `-2` on a collision) and the game starts fresh with one `Notice`; a file that cannot be renamed is never written over this session. Without `SavePaths` the game starts fresh, writes nothing, and sends one `Notice`.
- Writes happen on `MatchEnded` only, for each file whose encoding differs from what it last wrote; a failure keeps the model and retries at the next match end, with one `Notice` per session. Comparing against the last write, not Bevy's change detection, is what makes the retry automatic.
- Plan 8.1 needs its own write trigger for the settings and profile screens: today a settings change waits for the next match end.
- The core cannot log, so the `Notice` is the only report of a failed write; the error detail is dropped.
- The dev entry logs every `Notice` at warn level until plan 8.1's notice line exists.
- The read-only-directory tests rely on Unix permissions; run as root they would pass without testing anything. Nothing here runs tests as root.
- Plan 7.3 (2026-11-08): results are recorded on `MatchEnded`, before the match-end write: a win for the winning side's player, a loss for the loser's; Guest and computer sides record nothing; one profile on both seats records nothing (the user's call); an unknown profile id is the system's error, so dev panics and nothing is recorded. Counts saturate at their maximum.
- The approved "reset players to Guest on leaving a match" was dropped with the separate resource it guarded (ADR 0008).
