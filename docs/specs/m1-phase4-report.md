# M1 phase 4 report: shell vertical slice

Started 2026-11-04. Branch `feat/shell-vertical`. Roles: Claude drives in Explain, Code, Commit cycles; the user reviews and commits.
Deviations from [m1-basic-pong.md](m1-basic-pong.md) and [m1-plan.md](m1-plan.md) per plan rule P.6, each with where it was recorded.

## Deviations

| Item | Deviation | Recorded |
|---|---|---|
| 1.6.1, plan 4.1 | The keyboard adapter reports once per frame in `RunFixedMainLoop` (`RunFixedMainLoopSystems::BeforeFixedMainLoop`), only while `MatchState` is `Playing`, not every `Update` during a match. In `Update` every report reached the fixed steps a frame late, and a tap during a countdown or pause latched and fired at the serve or resume. | Spec 1.6.1, plan 4.1, `docs/architecture.md` (Simulation model), and the `SeatsPlugin` contract amended 2026-11-04. |
| 1.6.1 | Focus loss clears the intents after that frame's keyboard report, in every state, and asks `MatchControl` to pause only while `Playing`. Bevy releases `ButtonInput` keys one frame after focus loss, so a clear before the report would be overwritten by the stale keys. The latest focus change of a frame decides; focus lost and regained within one frame does nothing. | `pong::ports::focus`, 2026-11-04. |
| plan 4.1 | `pong` is a library plus a thin binary. A binary crate warns on every item `main` does not yet use, and integration tests cannot import a binary. | Plan 4.1 amended 2026-11-04; `pong/src/lib.rs`. |
| plan 4.1, 7.1 | The `SavePaths` resource moves to 7.1; 4.1 builds only the shell's XDG directory resolver, which 4.4's crash file also needs. A port shaped before its reader exists is a guess. | Plan 4.1, 7.1, and spec 1.6.1 amended 2026-11-04. |
| ADR 0001 | `pong` names Bevy's `keyboard` feature, which `bevy_window` already enables, so the adapter does not depend on another crate's feature list. `pong` gains a dev-dependency on `pong_core` with `testing`: our own crate, no new dependency. | Comments in `pong/Cargo.toml`, 2026-11-04. |

## Decisions taken in this phase

- Up and down held together cancel to `Still`, like two equal opposing forces. "Last pressed wins" needs press order, more state, and a rule a child cannot see; on a shared keyboard both keys at once is often ghosting.
- Shell plugins assume `DefaultPlugins` and do not register Bevy's own messages or resources; tests add `InputPlugin` and register `WindowFocused` themselves.

## Open

- After a focus loss the match stays `Paused` until a resume exists; the pause screen arrives in phase 8. The dev entry (4.5) needs a temporary resume key.
- The embedded watcher may not see `pong_core/src/features/tuning/tuning.ron` under `cargo run -p pong`: it watches `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`. Inferred from the Bevy source, not observed; plan 3.9's manual check settles it.
