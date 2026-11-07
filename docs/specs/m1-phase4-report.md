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
| plan 4.2, 4.8 | Views sample each body's centre every fixed tick in `FixedPostUpdate` and draw between the last two samples once a frame (`RunFixedMainLoopSystems::AfterFixedMainLoop`). 4.8 is declined: `StepStart` updates only on live steps, so between it and `Position` a stopped ball would shimmer through every countdown, point pause, and pause; sampling every tick also covers paddles. | Plan 4.2 and 4.8 amended 2026-11-04; `pong::views::motion`. |
| 1.6.2 | Letterboxing is `ScalingMode::AutoMin` over a black backdrop with nothing drawn beyond the arena, not bars cut by `Camera.viewport`. `AutoMin` shows the whole arena at its aspect and extra world on a differently shaped window; the only gain of true bars, clipping what lies outside the arena, clips nothing. | Spec 1.6.2 amended 2026-11-04; `pong::views::camera`. |
| 1.6.2, ADR 0001 | `pong` enables Bevy's `tonemapping_luts` and `zstd_rust`, adding `ktx2` 0.5 (Apache-2.0) and `ruzstd` 0.9 (MIT, with `twox-hash`). Without them TonyMcMapface reads a magenta placeholder table. Treadmill justification: they are Bevy's own optional dependencies at versions Bevy pins, so they move with each Bevy release rather than lagging it; the alternative was a tonemapper without a table, against the spec. | Spec 1.6.2 amended 2026-11-04; comment in `pong/Cargo.toml`. |
| plan 4.2 | The 4.2 eye check needs a running game; order is 4.2 code, 4.5 dev entry, 4.2 eye check, 4.3, 4.4. | Plan 4.2 amended 2026-11-04. |
| ADR 0001 | `pong` names Bevy's `keyboard` feature, which `bevy_window` already enables, so the adapter does not depend on another crate's feature list. `pong` gains a dev-dependency on `pong_core` with `testing`: our own crate, no new dependency. | Comments in `pong/Cargo.toml`, 2026-11-04. |

## Decisions taken in this phase

- Up and down held together cancel to `Still`, like two equal opposing forces. "Last pressed wins" needs press order, more state, and a rule a child cannot see; on a shared keyboard both keys at once is often ghosting.
- Shell plugins assume `DefaultPlugins` and do not register Bevy's own messages or resources; tests add `InputPlugin` and register `WindowFocused` themselves.
- The XDG resolver (`pong::xdg::AppDirs`) is plain Rust, not a Bevy resource: the panic hook (4.4) needs the state directory before the app runs. It takes the environment as a lookup function, because `std::env::set_var` is `unsafe` in edition 2024 and the workspace forbids `unsafe_code`; tests pass a fake environment.
- Each base is its `XDG_*_HOME` if absolute, else its default under `HOME`, plus the directory `bevy_pong`. A relative or empty variable is ignored, as the XDG specification requires. `HOME` must be absolute, and is needed only by a base that falls back to it; with no usable base the resolver returns `NoHomeDirectory`, and each caller decides what that means for its file.
- The resolver computes paths and creates no directory; the writer of a file creates its directory and handles its own failure (persistence write policy, spec 1.4.9).
- Errors in `pong` derive with `thiserror`, as in `pong_core`; the workspace dependency is already justified (phase 3 report), so `pong` names it without a new treadmill entry.
- The dev entry (4.5, `pong::dev_entry`) is added only with the `dev` feature, by `cfg!` in `main.rs`, so it is compiled, linted, and tested in every build. `just run` starts a match; a build without `dev` stops at the empty menu until 8.3.
- The dev entry's one key is Escape, the spec's key for pause and back-out: it pauses from `Countdown`, `Playing`, or `PointScored`, resumes from `Paused` (focus loss included), and from `MatchOver` returns to the menu, which starts the next match. This covers the temporary resume key and leaves no unfinished match.
- With no score display before phase 8, the dev entry logs each score change at info level.
- `main` logs the core seed at info level on every launch (`pong::seed`), so a launch can be replayed from its seed.
- The window (4.3, `pong::window`) opens borderless fullscreen on `MonitorSelection::Primary`. At creation Bevy passes no current monitor, so `Current` warns on every start and falls back to none; on Wayland, which has no primary monitor, both leave the choice to the compositor. Settings-driven mode stays with 7.x.
- Title and app ID (Wayland `app_id`, X11 `WM_CLASS`) are `bevy_pong`, a placeholder until the game has a name. On Hyprland the game runs as a native Wayland client: Bevy's `2d` feature enables both `wayland` and `x11`, and winit prefers Wayland.
- Plan 4.5 checked 2026-11-06 by the user: two launches of `just run` served at different first angles.
- Plan 4.2 eye check 2026-11-06 by the user: no judder on the 60 Hz laptop panel, and resizing keeps the arena's aspect with bars. The moving ball shows a dim trailing copy that vanishes when paused; with one mesh and no temporal pass in the pipeline, this is read as panel ghosting of a near-white ball on black. If it bothers players, a dark grey backdrop or a dimmer ball softens it (phase 10 polish).

## Bevy API checks (plan 4.7)

One `bevy-api-check` report per view module, before the code, against the pinned 0.20.1 source; corrections it made are listed.

| Module | Checked | Corrections |
|---|---|---|
| `views::camera` | `Camera2d`, `Hdr`, `Bloom`, `Tonemapping`, `DebandDither`, `ClearColor`, `Projection`, `OrthographicProjection::default_2d`, `ScalingMode::AutoMin`, resize handling; `resource_exists_and_changed` | HDR is the `Hdr` component, not `Camera { hdr }`; `Camera2d` defaults to `Linear` tonemapping and no dither; TonyMcMapface needs `tonemapping_luts` and `zstd_rust`; `AutoMin` shows extra world, not bars. `resource_exists_and_changed` was written before its check and confirmed after (false while absent, true on insert and reinsert). |
| `views::motion` | `Time<Fixed>::overstep_fraction`, `RunFixedMainLoopSystems::AfterFixedMainLoop`, `FixedPostUpdate`, `Added`, `#[require(Transform)]`, `Transform` without `TransformPlugin` | none |
| `views::{arena, ball, paddle}`, `views` | `Mesh2d`, `MeshMaterial2d`, `ColorMaterial` from `Color`, `Color::linear_rgb` above 1.0 reaching the HDR target, `Assets<Mesh>::add` of `Rectangle`/`Circle`, `Assets` present by `Startup` | `Rectangle` and `Circle` are in `bevy::shape`, not `bevy::math` |

## Open

- After a focus loss the match stays `Paused` until a resume exists; the pause screen arrives in phase 8. Until then the dev entry's Escape resumes (4.5).
- `testing::boot` gives up after 1,000 frames, but the tuning load it waits on runs on a background thread in wall-clock time. On 2026-11-06, with the machine busy after a long build, three tests failed with "the app never left Boot" and passed on rerun; not reproduced in 25 later runs. Fix in the harness, not in 4.5: wait on the load itself, or a wall-clock budget, rather than a frame count.
- The embedded watcher may not see `pong_core/src/features/tuning/tuning.ron` under `cargo run -p pong`: it watches `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`. Inferred from the Bevy source, not observed; plan 3.9's manual check settles it.
