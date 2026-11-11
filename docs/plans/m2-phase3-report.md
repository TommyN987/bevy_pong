# M2 phase 3 report: Chaos on screen

Started 2026-11-11. Branch `feat/chaos-shell`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built so far: steps 1 and 2 of five (plan 3.1, 3.2, and the delta of 3.4). Steps 3 to 5 (plan 3.3, the rest of 3.4, 3.5) are open.

## Deviations

Proposed before implementation and approved by the user on 2026-11-11. Each takes effect in the step named.

| Item | Deviation | Step | Recorded |
|---|---|---|---|
| spec 2.7.2 | The HUD shows the flight's return count, not rules' touches. Match over's best rally still counts touches, so the two can differ by a rally's end clips. | 3, open | This report. |
| spec 2.6.1 | Shrink's shell half has no view systems of its own: the paddle view already draws the paddle's rectangle. An eased height is added only if the hand check (plan 3.6) shows the step does not read. | 4, open | This report. |
| spec 2.9.1 | The sandbox takes hold at a countdown (effects) or a flight start (return count), not at once. | 5, open | This report. |
| spec 2.9.1 | A match the sandbox touched records nothing. | 5, open | This report. |

Steps 1 and 2 deviate from nothing.

## Test changes

Steps 1 and 2: 344 core tests and 145 shell tests pass, 489 in all; 468 existed before this phase.

Call sites changed for a changed signature, assertions untouched:

- `Profile::new` takes a `RecordsByMode`: nine test literals pass `RecordsByMode::default()` where they passed `Records::default()`; `document.rs`'s `two_children` builds one by field.
- `Profile::records` and `Profiles::records_mut` take a `Mode`: every earlier test passes `Mode::Classic`, in `model.rs`, `persistence/mod.rs`, `save_fixtures.rs`, and `pong/tests/match_screens.rs`.
- `Settings` gained `mode`: four test literals take `mode: Mode::Classic`.
- The mode screen sits between Play and the opponent: twelve walks in `pong/tests/menu.rs` go through the new helper `play_classic`, which picks Play and then Classic.

One fixture changed, no assertion: `two_children` gives Zoë Chaos records, so `profiles_come_back_from_their_document_unchanged` round-trips both modes.

One test replaced, because it named the version it pinned: `settings_are_written_as_version_2` is `settings_are_written_as_version_3`.

One assertion added to an earlier test: `the_version_2_settings_fixture_still_decodes_to_its_settings` also expects Classic.

## Decisions taken in this phase

- **Records per mode.** `RecordsByMode` holds one `Records` per mode. `of(mode)` and `of_mut(mode)` match on `Mode` and ignore the cap, so no key type joins the vocabulary and a third mode does not compile until it has records.
- **Profiles v2.** `records: (classic: (..), chaos: (..))`, reusing v1's records shape once per mode. A v1 file migrates with its records under Classic.
- **Settings v3.** v2 plus `mode: Classic` or `Chaos(cap: n)`, the file's own enum, so renaming a `Mode` variant cannot change the file. A v2 file migrates to Classic. A cap of zero is corrupt (`InvalidSaveData::NoChaosCap`); an extra field inside `Chaos(..)` is corrupt.
- **`Settings::mode` is not optional.** Classic before the first pick; no tuning default competes with it, unlike points-to-win.
- **Migration chain.** Each version converts to the next by `From`; only the newest shape builds the model. `v1::ProfilesFile::into_model` and `v2::SettingsFile::into_model` are gone, as v1's settings one went at plan 10.2.
- **One-way step.** A build before this phase sets a v2 profiles file or a v3 settings file aside as "from a newer version" and starts fresh.
- **Mode screen.** `MenuScreen::Mode` between `Main` and `Opponent`, driven by either seat; rows `Classic`, `Chaos ●`, `Chaos ●●`, `Chaos ●●●`; heading `Mode`. The pick goes to the draft and to `Settings` at once, as points-to-win does.
- **A saved cap no row offers** leaves the cursor on the first row, Classic.
- **Match over** reads the played mode's records (spec 2.8.4), pulled forward from step 4: the records API change forced a choice there, and a Chaos match would otherwise have shown no gains between the steps. The line naming the mode stays in step 4.
- **Bevy API.** No API new to the repository was used in steps 1 and 2: serde derives, `SubStates`, and the menu's existing systems. `bevy-api-check` was therefore not run.

## Checked against deliberate breaks

Each of these four fails at least one test:
a v1 profile's records migrated under Chaos; a v2 settings file migrated to Chaos; a mode pick not written to the settings; backing out of the opponent screen skipping the mode screen.

## Facts learned

- `#[serde(deny_unknown_fields)]` is a container attribute: on an enum it covers every struct variant; on a variant it does not compile.
- A shell test build takes over two minutes after a shell source change; the core's takes about ten seconds.

## Open at handoff

- Steps 3 to 5.
- Only Shrink is in Chaos's bag, so the three Chaos rows play alike until Wind lands.
- Chaos is reachable by hand; Shrink shows through the paddle view, with no icon, cue, or hit count yet.
