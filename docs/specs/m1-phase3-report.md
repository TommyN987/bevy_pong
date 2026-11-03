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
| 1.4.1 | The binding schema names the fourth key `back`, where the spec says "reserved". The key backs out of menus now and is reserved in play; the name follows its live role. | This report, 2026-11-02; spec 1.4.1 wording to amend at milestone close. |
| 1.4.8 | Angles in the tuning file are degrees, converted to radians at validation. The spec names "maximum reflection angle" without a unit; degrees are what a person editing the file expects. | File comment and `TuningFile` doc, 2026-11-02. |
| ADR 0001 | `pong_core` enables three more Bevy features: `bevy_color` (palette type), `keyboard` and `serialize` (`KeyCode` bindings as data). `cargo tree` confirmed the tree still holds no render, window, audio, or UI crate. | Comment on the dependency in `pong_core/Cargo.toml`, 2026-11-02; ADR 0001 consequence to amend at milestone close. |
| 1.4.1 | Intents are not "consumed and cleared in `FixedUpdate`". `SeatIntents` keeps a held level per seat, re-reported every `Update` and read by every fixed step, plus a tap latch spent by the first fixed step. Clearing the level each fixed step would move a held paddle once per frame, half speed whenever a frame runs two fixed steps. | Done-condition to amend: "a hold moves on every fixed step; a press shorter than a frame moves exactly once and is never missed". Tests in `features/seats`. |
| 1.4.1 | `PaddleIntent::Still` instead of `None`, to keep it apart from `Option::None` at every use site. | This report, 2026-11-03. |
| 1.4.5 | `MatchConfig` is defined in vocabulary, not in rules: seats, rules, the AI, and the menu all read it. Vocabulary count is now ten of twelve. `points_to_win` is `NonZeroU8`, and tuning's options use the same type, so zero is unrepresentable rather than checked (the `PointsToWinZero` error variant is gone). | This report, 2026-11-03. |
| 1.5.1 | The schedule-walking check lives in `app::phases` under `#[cfg(test)]` for now, not in the `testing` module, which does not exist until 3.8. | Move it in 3.8. |
| ADR 0001 | `pong_core` enables Bevy's `debug` feature as a dev-dependency only, so the schedule walker can name a system that forgot its phase. Resolver 3 keeps it out of normal builds. | Comment in `pong_core/Cargo.toml`, 2026-11-03. |
| 1.1 | `thiserror` 2 added to `pong_core`. Not listed in spec 1.1. Treadmill justification: proc-macro only, no Bevy types, the same major Bevy already compiles, so it cannot lag a Bevy upgrade. | Comment on the dependency in the workspace `Cargo.toml`, 2026-11-01. |

## Decisions taken in this phase

- Kernel constructors are the single source of range truth. `kernel::Range` owns each predicate and its wording; tuning validation (3.3) calls the constructors and reports their errors rather than keeping its own range table.
- Validation errors embed the inner `OutOfRange` in their message rather than exposing it as a `source`, so a report prints one sentence.
- Swept collision treats rectangle corners as square. A corner clip is reported a hair early; paddles are struck on faces, and every contact maps to one of four `Face`s.
- Tuning is a `Resource` and an `Asset` at once. The loader validates, so a file that fails validation never becomes an asset; on hot reload (3.9) the asset system keeps the previous good asset by itself.
- Tuning errors name the file path of the refused value as a string (`ball.radius`) rather than one enum variant per field. Nothing branches on which field failed; the name exists to be printed.
- `expect` is allowed in test code only (`clippy.toml`, `allow-expect-in-tests`); production core still denies `unwrap` and `expect`. A test whose fixture is invalid should stop with the reason, and `unwrap_or_else(|e| panic!(..))` was the same thing spelled longer.
- `clippy::needless_pass_by_value` fires on every Bevy system parameter. It is silenced inline per system with a reason; promote to a workspace allow when it reaches about five across two modules.
- The intent phase has two writers of `PaddleIntents` by design: seats write the sides a human holds, the AI (plan 5.1) writes the sides it holds. They touch disjoint sides, so no ordering between them is declared, per rule 4.
- Contract for the shell's keyboard adapter (4.1): report intents only while a match is in progress and call `SeatIntents::clear` on focus loss. Rules (3.7) should clear intents on entering `Countdown` so a menu key press does not become the first paddle move.
- Bevy 0.20.1's default error handler is `match_severity`, not `panic`; an error built from a string carries `Severity::Panic`, so the default still panics. `ErrorPolicyPlugin` sets the handler explicitly anyway, because release must log rather than panic and the app accepts one handler only. The shell must not set its own.
- `AppStatePlugin` adds Bevy's `StatesPlugin` itself when absent, so `MinimalPlugins` test apps and the `DefaultPlugins` shell share one `CorePlugins` group without an ordering contract for states. `AssetPlugin` stays a precondition of the group.
- Phase order is declared once as `FixedStepPhase::IN_ORDER` and chained pairwise, because arrays of sets do not implement the schedule-config trait and a tuple would be a second listing.
- After `Schedule::initialize`, systems move out of the schedule graph into the executable; the walker lists them through `Schedule::systems()` and asks the graph only for set membership.
- Core events are Bevy 0.20.1 `Message` types (buffered), not observer `Event`s. Shell audio and views read them per frame; buffers written in `FixedUpdate` survive to `Update` readers in the same frame.

## Items

| Item | Status |
|---|---|
| 3.1 Kernel | done, reviewed, committed |
| 3.2 Vocabulary | done, reviewed, committed; nine of twelve vocabulary types |
| 3.3 Tuning | done; dev override deferred to 3.9 per plan |
| 3.4 App assembly | done |
| 3.5 Seats and intents | done |
| 3.6 Ball, paddle, physics | next |
