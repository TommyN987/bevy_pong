# M2 phase 4 report: AI under effects, and the harness

Started 2026-11-12. Branch `feat/ai-effects`. Roles: Claude drives; the user reviews and commits.
Deviations from [m2-chaos-pong.md](../specs/m2-chaos-pong.md) and [m2-plan.md](m2-plan.md) per plan rule P.6, each with where it was recorded.

Built so far: step 0. Steps 1 to 4 (plan 4.1 to 4.4) are not started.

## Deviations

All were proposed before implementation and approved by the user on 2026-11-12. None is built yet.

| Item | Deviation | Recorded |
|---|---|---|
| plan 4.1 | The per-star flaw section of an effect's tuning is not built. Shrink's flaw is inherent (spec 2.6.1), so no effect has numbers to read; Wind is the section's first user, in plan phase 5. Phase 4 proves the flaw's two forms with probe effects. | This report. Plan amendment at the next record step. |
| spec 2.3.12 | For Shrink, "a raised flaw misses some" is read as a raised dial: a fixed handicap returns every ball at dial zero and misses some at full dial. | This report. Spec amendment at the next record step. |
| spec 2.5.3, plan 4.3 | The Chaos soak is one test per subset size, with a step budget per subset chosen from the measured running time, not M1's hundred thousand steps per subset. The tests for pairs and triples hold no subset until Wind and Spin ship. | This report. Spec amendment at the next record step. |

## Decisions taken in this phase

- **The serve and the AI.** The AI counts an approach only once the ball's flight has begun. The serve and the AI's steering both run in the intent phase, unordered, and the serve's flight starts in the effects phase after them: an AI that ran second planned under the cleared flight. Harmless without fields; from Wind on it would aim a served ball without the wind, by the luck of the system order. Approved 2026-11-12; not built yet.
- **`just ai-report`.** A recipe that prints the harness's tables, since every effect phase reports its R3 measure through it (plan E.4). Approved 2026-11-12; not built yet.

## Step 0

### Record step for phase 3 (P.6)

The seven deviations of the [phase 3 report](m2-phase3-report.md) are recorded:
spec 2.6.1, 2.7.1, 2.7.2, 2.8.4, and 2.9.1 (two deviations, one amendment) carry an amendment dated 2026-11-12;
ADR 0010 was amended when its deviation was taken.
The report's "Recorded" column says so per row.

### The double-counted return

Ruled by the user on 2026-11-12: fixed now, before the harness records the baselines every later phase compares against.

- **Fault.** A return whose step ended exactly on a paddle's face was found again by the next step's sweep, which starts on the grown rectangle's edge and moves away: `slab` gives an exit of `-0.0`, and `t_exit < 0.0` did not refuse it. The second contact was a second return: a second speed step, a second `BallHitPaddle`, a second cue.
- **Contract.** `swept_circle_rect`: a sweep that leaves the grown rectangle at or before its start is no impact. A start that overlaps and stays inside for any part of the sweep is still an impact at time zero, so a paddle that moved onto the ball still deflects it.
- **Change.** `t_exit <= 0.0` in `kernel/collide.rs`. Physics is untouched: the kernel owned the fault.
- **Tests, red first.** `ball_touching_the_face_and_moving_away_misses` (kernel) and `a_return_whose_step_ends_on_the_face_is_one_return` (physics, the phase 3 report's case: 60 units/s from one unit off the face) both failed before the change; the physics test read two `BallHitPaddle`. `ball_touching_the_face_and_moving_in_hits_at_once` (kernel) passed before and after: it pins the other side of the boundary.
- **Not covered.** A start that rounding leaves a hair inside the grown rectangle, moving away, is still an impact at time zero. By arithmetic, not by test: at the paddles' coordinates the contact point rounds onto the face exactly, so the step after a return starts on the edge or outside it. No such case was found or built.

## Test changes

356 core tests and 165 shell tests pass, 521 in all; 518 existed before this phase.

No assertion changed and no call site changed.
One comment changed: the AI tests' `launches` said a touching start counts as struck; it now says an overlapping start does.

## Facts learned

- The spec's open fact on sweeps that start inside the rectangle is answered in part and amended in the spec: a start on the edge moving away is no impact. The interior start stays with plan 9.1.
