# M1 phase 10 report: Polish

Started 2026-11-09. Branch `feat/polish`. Roles: Claude drives; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| plan 10.2 | Alt+Enter needed no work: it has toggled the window mode since plan 8.1 (`WindowModePlugin`). | This report. |

## Decisions taken in this phase

- Plan 10 (2026-11-09, approved by the user): three parts, each a review stop. A: the ball trail. B: the overlay. C: screen shake and the remembered points-to-win, both of 10.2's MAYs, chosen by the user.
- The overlay ships in release builds, hidden by default, as `docs/architecture.md` decided: its rates only mean something in an optimized build, playtests run release builds, and one code path serves both builds. The user asked whether "visible in a release build" was a mistake (2026-11-09); it means it works there, not that it shows in play.
- Part A (2026-11-09): the trail, split like the ball itself.
  - `views::trail`, where: `Trail` keeps a ball's last ten tick centres, newest first; ghost `back` (1 to 8) is drawn between the samples `back + 1` and `back` ticks old at the ball's own fraction, so the ghosts sit a whole step apart at any display rate. A snap resets every sample to the new spot, so a teleport leaves no streak; a stopped ball's ghosts gather into it within eight ticks. The ghosts are entities of their own, one match long like the ball, naming their ball; the trail joins `motion`'s sets instead of ordering against its systems by name.
  - `views::ball`, how: each ghost is the ball's disc, sized and lit by a fade that falls linearly from the ball (`1 - back / 9`), lit at 0.6 of the ball before it, opaque: on the dark backdrop dimmer reads as fainter, with no blending.
  - TDD: red then green, the positions headless and the look as unit tests. Checked against deliberate breaks: no snap, never sampled, one tick late, the fraction ignored, a ghost short, fading the wrong way, the last ghost gone, the alpha scaled. Each fails a test. A first run of the ghost-count break never applied (its text was not unique), so it reported a false pass; rerun, it fails.
- Not tested, judged by eye: how the trail looks.
