# M2 phase 3 report: Chaos on screen

Started 2026-11-11. Branch `feat/chaos-shell`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built 2026-11-11 and 2026-11-12: all five steps, plan 3.1 to 3.5.
Closed 2026-11-12: the user played a Chaos match with Shrink in `just run` and declared plan 3.6 done; the work is committed on the branch (`0d7a661`, `6948c34`, `af7ea0d`).

## Deviations

The first four were proposed before implementation and approved by the user on 2026-11-11.

| Item | Deviation | Recorded |
|---|---|---|
| spec 2.7.2 | The HUD shows the flight's return count, not rules' touches. Match over's best rally still counts touches, so the two can differ by a rally's end clips. | This report; `effect_hud.rs`. Spec amended 2026-11-12. |
| spec 2.6.1 | Shrink's shell half has no view systems of its own: the paddle view already draws the paddle's rectangle. An eased height is added only if the hand check (plan 3.6) shows the step does not read. | This report; `effects/shrink.rs`. Spec amended 2026-11-12. |
| spec 2.9.1 | The sandbox takes hold at a countdown (effects) or a flight start (return count), not at once. | This report; features README; `docs/architecture.md`. Spec amended 2026-11-12. |
| spec 2.9.1 | A match the sandbox touched records nothing. | This report; features README; `docs/architecture.md`. Spec amended 2026-11-12. |
| spec 2.7.1 | The joining icon is not in the row while it shows large: it takes its place there when play begins. No glide between the two was built. | This report. Spec amended 2026-11-12. |
| ADR 0010 | An effect's join cue is not a `Cue` and does not come from the sound bank: it is a `Sound` in the effect's shell half. | This report; ADR 0010, amended 2026-11-12. |
| spec 2.8.4 | Match over also names the mode played, in one small line. | This report. Spec amended 2026-11-12. |

## Test changes

353 core tests and 165 shell tests pass, 518 in all; 468 existed before this phase.

Call sites changed for a changed signature, assertions untouched:

- `Profile::new` takes a `RecordsByMode`: nine test literals pass `RecordsByMode::default()` where they passed `Records::default()`; `document.rs`'s `two_children` builds one by field.
- `Profile::records` and `Profiles::records_mut` take a `Mode`: every earlier test passes `Mode::Classic`, in `model.rs`, `persistence/mod.rs`, `save_fixtures.rs`, and `pong/tests/match_screens.rs`.
- `Settings` gained `mode`: four test literals take `mode: Mode::Classic`.
- The mode screen sits between Play and the opponent: twelve walks in `pong/tests/menu.rs` go through the new helper `play_classic`, which picks Play and then Classic.

One fixture changed, no assertion: `two_children` gives Zoë Chaos records, so `profiles_come_back_from_their_document_unchanged` round-trips both modes.

One test replaced, because it named the version it pinned: `settings_are_written_as_version_2` is `settings_are_written_as_version_3`.

One assertion added to an earlier test: `the_version_2_settings_fixture_still_decodes_to_its_settings` also expects Classic.

One assertion changed, because spec 2.9.2 adds lines to what it lists: `in_a_match_the_overlay_reads_the_ball_and_the_computers_aim` expects the effects, the return count, and the sandbox keys among the overlay's lines.
The overlay's unit-test `readings()` literal gained three fields, all `None`; its two earlier assertions are untouched.

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
- **Shell registration.** `pong/src/effects/`: one module per effect, and `shell_half(id)`, the shell's one match on `EffectId` (moved here from plan 1.4). `ShellHalf` has no `Default`: an icon, a join cue, and a view, each of which may be none. Speed declares all three none. `EffectViewsPlugin` adds each effect's own view systems; no effect has any yet.
- **Icons are glyphs** of the game's font, one colour per effect. `fc-query` on the embedded DejaVu Sans Bold confirmed candidates for the whole roster. Shrink's is `▼` over `▲`.
- **Effects HUD.** `ui/effect_hud.rs`, beside the score HUD and separate from it, so Classic's nodes are untouched. It is spawned only if the mode's bag is not empty: the mode is asked for its data, not its name.
- **The row follows state, the large icon follows an event.** Which icons are laid out is read from `ActiveEffects` every frame. That an effect just joined is read once from `EffectStarted` and kept in `JoiningIcon` until `OnEnter(Playing)`.
- **Icons are spawned once** and laid out by `Display`, as the menu's rows are: an effect's icon keeps its place, in `EffectId::ALL`'s order.
- **Return count.** The ball's `BallFlight` return count, shown under the row from the first countdown; zero until the first return.
- **Join cue.** `CueSounds` holds a handle per effect that has one, built through `shell_half`; the cue map plays it on `EffectStarted`. `synth::phrase`, `tone`, `rest`, and the two timbres are public for the effects' modules.
- **Mode line.** `ui::mode_label` is the one spelling of a mode's name; the menu's rows and match over's `ModeLine` both use it.
- **Sandbox port.** `Sandbox`, a resource in the effects feature, as `MatchControl` is in rules. The forced set is the whole set in play, the always-on effects included, so "no speed ramp" can be forced. The first toggle starts from the effects in play, so one key changes one effect.
- **Held count.** Copied into `ActiveEffects` each step and applied inside `start_flight`, the one place both the serve and physics start flights; physics is untouched. Released, the rally counts on from the held count.
- **Forced effects are not announced.** Escalation advances the schedule as always, then sets the forced set and writes no message; a release finds the schedule where the score has brought it.
- **Touched matches.** Any forcing marks the match; entering a match clears the mark unless something is still forced; persistence skips a touched match.
- **Sandbox keys**, while the overlay shows in a match: `1` to `9` toggle the effect at that place in `EffectId::ALL`, `[` and `]` hold the count one lower or higher, `0` releases. The overlay lists them, each number with its effect's name.
- **Waiting or in force.** Changed after the user's first hand check (2026-11-12), which read the forced set as what was in play. Each forced value has its own line: `effects from the next point: ..` and `returns from the next hit or serve: ..` while the match has not taken it up, `effects forced: ..` and `returns held: ..` once it has. The overlay decides which by comparing the sandbox with the effects in play and the ball's return count; the core is unchanged.
- **Two systems passed clippy's seven-parameter limit** and group their messages in a `SystemParam`, as the menu's `MenuExits` does: escalation's `Announcements`, the cue map's `CoreEvents`. No lint was silenced.
- **Bevy API.** `bevy-api-check` ran on 2026-11-12 before steps 3 to 5, against the pinned 0.20.1 source. Confirmed: `StateTransition` runs after `PreUpdate` and before `Update`, and messages are swapped only in `First`, so an `Update` reader sees a message written in `OnEnter` in the same frame; `KeyCode::Digit0` to `Digit9`, `BracketLeft`, `BracketRight`; `ButtonInput::just_pressed`; `Display::None` removes a node from layout and drawing. Not read by the agent: whether a sub-state's `OnEnter` runs in the same pass as its parent's; `the_joining_effect_shows_large_in_the_countdown_then_takes_its_place_in_the_row` shows it does. Steps 1 and 2 used no API new to the repository.

## Checked against deliberate breaks

Each of these twelve fails at least one test:
a v1 profile's records migrated under Chaos; a v2 settings file migrated to Chaos; a mode pick not written to the settings; backing out of the opponent screen skipping the mode screen; a held count ignored at a flight start; a new match never touched; forced effects overwritten by the schedule; a touched match recorded; the effects HUD shown in Classic; the large icon never settling; the sandbox keys working with the overlay hidden; an effect's join cue not played.

## Facts learned

- `#[serde(deny_unknown_fields)]` is a container attribute: on an enum it covers every struct variant; on a variant it does not compile.
- A shell test build takes over two minutes after a shell source change; the core's takes about ten seconds.
- **A return can count twice.** Found by a HUD test, not fixed. A ball at 60 units/s placed one unit from a paddle's face ends its step exactly on the face (`toi` 1, no remainder to travel); the next step's sweep starts on the grown rectangle's edge moving away, `slab` gives an exit of `-0.0`, which `t_exit < 0.0` does not refuse, and `swept_circle_rect` reports an impact at `toi` 0 on a left or right face. Physics counts a second return: a second speed step, a second `BallHitPaddle`, a second cue. The mechanism is read from `kernel/collide.rs`; the double count is observed (the HUD read 2). It needs a step to end on the face within float rounding, so it is rare in play, and it predates M2. It also answers part of the spec's open fact on sweeps that start inside the rectangle.
  Fixed 2026-11-12 in M2 plan phase 4, by the user's ruling ([phase 4 report](m2-phase4-report.md), step 0): a sweep that leaves at or before its start is no impact.
- The sandbox's two delays read as a fault when the overlay showed only the forced values: a scripted run of the user's key sequence showed the core at the designed state at every stage.
- The join cue is spawned on the real path into a Chaos match, by `entering_a_chaos_match_plays_the_joining_effects_cue`; the user confirmed it by ear.
- A state change requested in a test applies in the next frame: `run_until(.. == Playing)` right after `NextState::set(Countdown)` returns at once, still in `Playing`.

## Hand check (plan 3.6)

Done by the user on 2026-11-12, in two rounds.

- Round one: the match behaved correctly on its own. The sandbox read as faulty: the overlay showed only the forced values, not when they land. The join cue was first reported missing; the user had expected it at each shrink, and confirmed it plays at the join.
- Between the rounds: the overlay's waiting-or-in-force lines and named keys (`af7ea0d`).
- Round two: the user followed the five-step sandbox script and confirmed it works.
- No change to Shrink's view was asked for: the eased height of the spec 2.6.1 deviation was not built.

## Open at phase close

- The double-counted return above: the user's ruling on when to fix it. Blocks (plan 9.1) reads the same function. Ruled and fixed 2026-11-12, in M2 plan phase 4.
- Only Shrink is in Chaos's bag, so the three Chaos rows play alike until Wind lands.
- Spec 2.7.1's done-condition, a viewer naming the effects without reading, waits for the kids' session (plan 5.4).
- No glide of the joining icon into the row; decided when seen.
