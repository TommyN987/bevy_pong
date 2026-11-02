# M1 phase 3 report: core vertical slice

Started 2026-10-31. Branch `feat/headless-rally`. Roles: Claude drives, the user reviews.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.3.1 | `Side` is defined in `kernel::geometry`, not in the vocabulary module. Geometry is the lowest layer that needs it, and rule 1 of the intra-crate structure makes kernel types reachable from every feature. It still counts toward the vocabulary limit. | Spec 1.3.1 amended by the user, 2026-11-01. `docs/architecture.md` still lists `Side` under `domain/`; amend at milestone close. |
| 1.3.2 | `BallHitPaddle { side }` instead of `{ seat }`. The AI's paddle has no seat (`Controller::Ai`), so a seat field could not be filled in a vs-computer match. | Spec 1.3.2 amended, 2026-11-02. |
| 1.3.2 | `PointScored { scorer }` instead of `{ side }`. "Side" did not say whether it meant the scorer or the conceder. | Spec 1.3.2 amended, 2026-11-02. |
| 1.2.6 | The done-condition says "equals simulation". No simulation exists before 3.6, so the 3.1 test compares `crossing_y` against an independent analytic zigzag within one unit. | Deferred: add the simulation-equality test in 3.6. |
| 1.1 | `thiserror` 2 added to `pong_core`. Not listed in spec 1.1. Treadmill justification: proc-macro only, no Bevy types, the same major Bevy already compiles, so it cannot lag a Bevy upgrade. | Comment on the dependency in the workspace `Cargo.toml`, 2026-11-01. |

## Decisions taken in this phase

- Kernel constructors are the single source of range truth. `kernel::Range` owns each predicate and its wording; tuning validation (3.3) calls the constructors and reports their errors rather than keeping its own range table.
- Validation errors embed the inner `OutOfRange` in their message rather than exposing it as a `source`, so a report prints one sentence.
- Swept collision treats rectangle corners as square. A corner clip is reported a hair early; paddles are struck on faces, and every contact maps to one of four `Face`s.
- Core events are Bevy 0.20.1 `Message` types (buffered), not observer `Event`s. Shell audio and views read them per frame; buffers written in `FixedUpdate` survive to `Update` readers in the same frame.

## Items

| Item | Status |
|---|---|
| 3.1 Kernel | done, reviewed, committed |
| 3.2 Vocabulary | done; nine of twelve vocabulary types |
| 3.3 Tuning | next |
