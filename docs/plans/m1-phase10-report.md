# M1 phase 10 report: Polish

Started 2026-11-09. Branch `feat/polish`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](../specs/m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| plan 10.2 | Alt+Enter needed no work: it has toggled the window mode since plan 8.1 (`WindowModePlugin`). | This report. |
| 1.4.9 | Settings file version 2: version 1 plus the last points-to-win picked, the first released migration. | Spec 1.4.9 amended 2026-11-09; features README. |

## Decisions taken in this phase

- Plan 10 (2026-11-09, approved by the user): three parts, each a review stop. A: the ball trail. B: the overlay. C: screen shake and the remembered points-to-win, both of 10.2's MAYs, chosen by the user.
- The overlay ships in release builds, hidden by default, as `docs/architecture.md` decided: its rates only mean something in an optimized build, playtests run release builds, and one code path serves both builds. The user asked whether "visible in a release build" was a mistake (2026-11-09); it means it works there, not that it shows in play.
- Part A (2026-11-09): the trail, split like the ball itself.
  - `views::trail`, where: `Trail` keeps a ball's last ten tick centres, newest first; ghost `back` (1 to 8) is drawn between the samples `back + 1` and `back` ticks old at the ball's own fraction, so the ghosts sit a whole step apart at any display rate. A snap resets every sample to the new spot, so a teleport leaves no streak; a stopped ball's ghosts gather into it within eight ticks. The ghosts are entities of their own, one match long like the ball, naming their ball; the trail joins `motion`'s sets instead of ordering against its systems by name.
  - `views::ball`, how: each ghost is the ball's disc, sized and lit by a fade that falls linearly from the ball (`1 - back / 9`), lit at 0.6 of the ball before it, opaque: on the dark backdrop dimmer reads as fainter, with no blending.
  - TDD: red then green, the positions headless and the look as unit tests. Checked against deliberate breaks: no snap, never sampled, one tick late, the fraction ignored, a ghost short, fading the wrong way, the last ghost gone, the alpha scaled. Each fails a test. A first run of the ghost-count break never applied (its text was not unique), so it reported a false pass; rerun, it fails.
- Part B (2026-11-09, plain F3, the user's choice over Ctrl+F3): `pong::overlay`, apart from the screens (architecture rule 8).
  - The text sits top left over every screen (`GlobalZIndex(1)`), spawned at `Startup` and hidden; F3 flips it. While shown it writes its readings only when they change.
  - Rates are counts over the last closed second or more of game time (`Time` in `Update`, the virtual clock), divided by that exact span; ticks are counted in `FixedPostUpdate`, once per tick. A frame begins a count without being in it, since `n` frames span `n - 1` steps: the first headless run read 61 fps, because the unit tests' clock began at one step, where Bevy's first frame is at zero. The unit tests now begin at zero.
  - Lines: the rates; the ball's speed in arena units per second; each computer player's side and aim, or "centre"; the state, the match state, the menu screen, as present. A line with nothing to read is left out.
  - Core: `ai::Aim { target_y, error }`, held in the plan's `Meet`; `MirrorController::aim` and `side` read it. The draw order is unchanged, so every seeded run is too.
  - TDD: red then green, the rates and the text as unit tests, the plugin headless. Checked against deliberate breaks: the aim never reported, a span closing every frame, dividing by one second, counts never reset, the error unsigned, never hiding, starting shown, no match state, no ball speed, the first frame counted. Each fails a test. The reset break survived at first, since every test closed one span; a test now closes two at different rates. A first try at the first-frame break did not compile, which proves nothing; rewritten, it fails.
- Part C (2026-11-09, both items, the user's choice).
  - Points-to-win: `Settings.points_to_win` is the last pick, `None` before the first, not a default, since tuning owns the default and two copies could drift. The points screen opens on it while tuning still offers it, otherwise on tuning's default (a preselection not in the list would put the cursor on the first row); a pick is kept at once, and the menu's save trigger writes it.
  - Settings file version 2 (`document::v2`), reusing v1's window-mode shape. The pick is the file's own enum, `Picked(n)` or `Unpicked`, not an `Option`: serde reads a missing `Option` field as `None`, and a released shape is exact. `Picked(0)` is `InvalidSaveData::NoPointsToWin`. A v1 file migrates in one step, `From<v1::SettingsFile>`, to a v2 file with no pick, then to the model; v1's own `into_model` is gone, its types unchanged. The `settings_v2.ron` fixture joins `settings_v1.ron`; the fixture-count test failed on the version bump until it did.
  - Screen shake, split like the trail: `views::shake` starts a shake on `PointScored` and moves the x and y of any `Shake` entity; the camera carries one. The offset is two sines (23 and 17 Hz) from zero under a `(1 - t/T)^2` envelope, 1.5 arena units at most, over 0.3 s, then exactly zero: the same wobble every point, no randomness. Tested on a plain entity in the camera's place, since a camera needs the renderer.
  - TDD: red then green each. Checked against deliberate breaks: the migration inventing a pick, a zero accepted, the pick not written, the migration dropping a v1 value, the fallback filter gone, the pick not kept, the saved pick ignored, a shake that never ends, no decay, shaking without a point, z overwritten, stopping a frame early. Each fails a test.
- Not tested, judged by eye: how the trail and the shake look; the overlay's place and size, and its numbers in a release build (`just build-release`).
