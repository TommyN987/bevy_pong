# M1 Implementation Plan

Decided 2026-10-30. Sequences [m0-spike.md](m0-spike.md) and [m1-basic-pong.md](m1-basic-pong.md) into phases.
Spec item numbers (1.x.y) refer to the M1 spec. Each phase is one branch and one PR.

## Ordering rationale

1. Risky assumptions first: the render-free core is only probable, so the spike precedes everything; interpolation smoothness is judged by eye, so the first on-screen slice precedes AI, UI, and persistence.
2. Vertical slices over layers: a headless rally proves the architecture before a pixel exists; a two-human match proves the shell before any menu exists.
3. Data before consumers: tuning holds bindings, arena size, and AI entries, so it is built in the core phase; persistence precedes UI because profile select reads it.

Coupling: hooks call `just` recipes, which need a workspace. Phase 0 hooks are guarded on a root `Cargo.toml` existing, so documentation work before phase 2 is unaffected.

## Working agreement

Git is the user's domain. Claude runs only read-only git (status, log, diff, show, blame) and `git stash`. Claude never adds, commits, branches, switches, pushes, pulls, merges, rebases, resets, tags, or opens PRs.

Pair-programming roles vary per phase or per item: Claude drives and the user reviews, or the user drives while Claude navigates and reviews. The project is also the user's Bevy learning vehicle.

## Phase protocol

- **P.1 MUST** The user creates and names the branch. Roles (driver, navigator, reviewer) are agreed for the phase, per item where useful. Done: branch exists; roles stated.
- **P.2 MUST** The driver implements an item. When the user drives, Claude navigates: explains the Bevy idiom and its why, points at API verified against docs.rs for the pinned version, keeps the done-condition in view. When Claude drives, it runs `bevy-api-check` before each Bevy API. Done: done-condition evidence exists, produced by the driver. Amended 2026-11-09 at milestone close (CLAUDE.md non-negotiable 1, `.claude/agents/bevy-api-check.md`): Bevy API is verified against the pinned crate source in the local cargo registry first, docs.rs as fallback.
- **P.3 MUST** `just dev-check` is green before any handoff. Done: green.
- **P.4 MUST** The reviewer checks the item against its done-condition and the architecture rules. When Claude reviews, it reports and changes nothing without the user's word. Done: review recorded in session or PR.
- **P.5 MUST** The user commits, pushes, opens the PR, merges. Claude drafts commit messages on request. Done: PR merged.
- **P.6 MUST** Deviations from the spec are noted in the phase report and land in an ADR or spec amendment at the next record step. Done: no silent deviations at milestone close.

## Phases

### 0. Claude Code setup
Realizes [claude-code-setup.md](../claude-code-setup.md). Depends on nothing.

- **0.1 MUST** CLAUDE.md under forty lines: project in two sentences; pointers to frame, research, design, architecture, specs; Bevy 0.20 with exact patch filled in at 2.7; the `just` recipes; four non-negotiables (verify Bevy API against docs.rs for the pinned version; core never imports shell, render, window, audio, or UI; no new dependency without a treadmill justification; never run mutative git). Done: file exists, under forty lines. Amended 2026-11-09 at milestone close (CLAUDE.md non-negotiable 1, `.claude/agents/bevy-api-check.md`): the first non-negotiable verifies Bevy API through `bevy-api-check`, pinned registry source first, docs.rs as fallback.
- **0.2 MUST** `.claude/rules/pong_core.md` scoped to `pong_core/**`; `tests.md` scoped to `**/tests/**`. Done: both load on a matching edit after phase 2; glob quoting checked.
- **0.3 MUST** Hooks: PostToolUse on Rust edits runs `just fmt` (guarded on workspace existence); PreToolUse on Bash blocks mutative git commands and lists the allowed set; SessionStart prints Bevy version, milestone, git status; PostToolUse on `Cargo.toml` prints the dependency policy. Done: each fires once in a dry trial; a dry `git commit` attempt is blocked.
- **0.4 MUST** Subagent `bevy-api-check`, read-only, fetches docs.rs for the pinned version and reports mismatches. Done: definition present; invoked once on a sample snippet. Amended 2026-11-09 at milestone close (`.claude/agents/bevy-api-check.md`): the agent reads the pinned crate source in the local cargo registry first, docs.rs as fallback.
- **0.5 SHOULD** Skills `/spec`, `/adr`, `/playtest` now; `/bevy-upgrade` and `/milestone-close` when first needed. Done: three skill files invocable. Amended 2026-11-09 at milestone close: `/milestone-close` written from the M1 close; `/bevy-upgrade` waits for the first Bevy upgrade.
- **0.6 MUST** Permissions: cargo subcommands and `just` allowed; mutative git commands and `cargo publish` denied. Done: settings file lists them.
- **0.7 MUST** `docs/playtest.md` template and `docs/adr/0000-template.md`. Done: files exist. (Spec 1.8.1 and part of 1.1.4 live here because they are documents.)
- **0.8 MUST** Local tools: `just`, `cargo-nextest`, `cargo-deny`; Arch packages `pkgconf`, `libxcursor`, `libxrandr`, `libxi`; a Vulkan driver matching the GPU. Done: each tool answers `--version`; `vulkaninfo` lists a device.
- **0.9 SHOULD** Native git `pre-push` hook in `.githooks/` running `just ci-check`, enabled by `just setup` setting `core.hooksPath`. Done: a push with an unformatted file is rejected locally.

### 1. Spike
Realizes M0. Depends on 0.8.

- **1.1 MUST** M0 packages 0.1 through 0.5. Done: ADR 0001 with measured numbers and the kept-or-collapsed decision.
- **1.2 MUST** Delete the throwaway workspace. Done: gone.

### 2. Repository foundation
Realizes spec 1.1. Depends on ADR 0001.

- **2.1 MUST** Workspace per ADR 0001, exact Bevy patch pinned, toolchain file, `.gitignore`, license. Done: `cargo build` on a clean clone.
- **2.2 MUST** `justfile` with all recipes working; `dev-check` and `ci-check` per spec 1.1.8 and 1.1.9. Done: planted-failure tests pass.
- **2.3 MUST** Lint configuration and `cargo-deny` config. Done: `just ci-check` green.
- **2.4 MUST** Dev-only `dynamic_linking` feature. Done: `ldd` check per spec 1.1.2.
- **2.5 MUST** GitHub Actions workflow: Ubuntu deps, `just`, nextest, cache; `just ci-check` then release build. Done: green once the user has created the remote.
- **2.6 MUST** Hooks now active: a trial edit fires `just fmt`; the user's trial push fires the pre-push hook. Done: both observed.
- **2.7 MUST** CLAUDE.md updated with the exact pinned patch. Done: matches `Cargo.toml`.

### 3. Core vertical slice: headless rally
Realizes spec 1.2, 1.3, 1.4.1 through 1.4.5, 1.4.8, 1.4.10, 1.5, 1.7.1. Depends on 2.

- **3.1 MUST** Kernel 1.2.1 through 1.2.7 with unit tests. Done: green.
- **3.2 MUST** Vocabulary 1.3.1, 1.3.2. Done: compiles; count under twelve.
- **3.3 MUST** Tuning 1.4.8: schema, embedded load, validation, palette list; dev override deferred to 3.9. Done: CI parse test green; crafted invalid value rejected.
- **3.4 MUST** App assembly 1.4.10 plus error handler per profile (1.7.1). Done: schedule-walking test green; failing-system test differs by profile.
- **3.5 MUST** Seats and intents 1.4.1 with fake intents. Done: two-step latch test green.
- **3.6 MUST** Ball, paddle, physics 1.4.2 through 1.4.4 with seeded random. Done: headless rally event sequence test green.
- **3.7 MUST** Rules 1.4.5. Done: substate cycle test to `MatchOver` green.
- **3.8 MUST** Testing module 1.5.1 and soak 1.5.2 with nextest timeout. Done: soak under thirty seconds locally.
- **3.9 SHOULD** Dev override with hot reload and keep-last-good. Done: unit test; manual check once the shell exists. Manual check passed 2026-11-07 once `just run` set `BEVY_ASSET_ROOT` ([phase 4 report](m1-phase4-report.md)).
- **3.10 SHOULD** Schedule randomization 1.5.3. Done: green with it on.

Added 2026-11-04 from the phase 3 review (findings and dispositions in [m1-phase3-report.md](m1-phase3-report.md); decisions in ADR 0002 and ADR 0003).
All on `feat/headless-rally`; the phase 3 PR waits for them.
Order: 3.11, then 3.12, then 3.13 through 3.18 in any order, then 3.19, 3.20, 3.21.

- **3.11 MUST** `flow` layer and feature order per ADR 0002. Done: (a) no production code in `features/` or `flow/` imports `crate::app`; (b) no feature imports one later in the order declared in `features/mod.rs`; (c) `flow` imports only kernel, domain, and Bevy; (d) `entering_countdown_clears_seat_intents` lives in seats and passes; (e) rules does not import seats; (f) the `resource_exists::<MatchConfig>` guard on seats is gone. Amended 2026-11-09 at milestone close (`pong_core/src/features/seats/mod.rs`): (d)'s test is now two in seats, `a_key_press_from_the_menu_is_forgotten_on_entering_the_first_countdown` and `a_key_press_from_the_last_rally_is_forgotten_on_entering_every_later_countdown`.
  - **3.11.1 SHOULD** `just arch-check` inside `dev-check` and `ci-check`, checking (a) through (c) by grep. Done: a planted import of `features::rules` inside `paddle` fails `just dev-check`.
- **3.12 MUST** One fixed rate and harness frame-length control per ADR 0003. Done: a `CorePlugins` app has a `Time<Fixed>` timestep of `1 / FIXED_HZ`; the harness has no rate constant of its own; an app-level test with half-step frames shows a tap moving the paddle exactly once.
- **3.13 MUST** Step gate per ADR 0003; `bevy-api-check` first confirms whether a pending `NextState` can be read. Done, with 14-step frames (the longest that fit Bevy's 250 ms clamp at 60 Hz; corrected 2026-11-04 from 15): a goal raises the score by exactly 1 and writes one `PointScored`; the final point writes exactly one `MatchEnded`; a recording system in the sanity phase runs in the step that scores; no later step in that frame runs any phase.
- **3.14 MUST** Serve trigger, ball reset, and `MatchControl` pause port per ADR 0003. Done: `Paused → Playing` writes no `Serve` and leaves the ball's position and velocity unchanged; entering `Countdown` after a point puts the ball at the origin, stationary, with `Snap`; a pause from each of `Countdown`, `Playing`, `PointScored` resumes to that substate; a pause request in `MatchOver` is ignored.
- **3.15 SHOULD** Per-entity ball step state: `BallStepStart` becomes a `StepStart(Motion)` component, `BallForce` a `Force(Vec2)` component. Built as `StepStart(Vec2)`, position only (phase report, 2026-11-04). Done: both resources gone; a test with two hand-spawned balls shows each swept from its own start.
- **3.16 SHOULD** Paddle owns its geometry: the component keeps `side`, `center_y`, `height`; `Paddle::new(side, &PaddleTuning)`; `set_height(h, bounds, arena)` keeps the paddle inside the walls; `rect` reads thickness and inset from tuning. Done: growing against a wall stays inside without `move_by(0.0)`; a reloaded `thickness` changes an existing paddle's `rect` within one frame.
- **3.17 MUST** Cross-field tuning validation: `height_max ≤ arena.height`; `2·radius < arena.height`; `inset + thickness < arena.width / 2`; `speed_cap / FIXED_HZ < arena.height − 2·radius`; no seat key bound to `Escape`. Done: each rule rejects a crafted RON naming its path; the embedded file passes.
- **3.18 MUST** Seeded core group: `CorePlugins { seed: u64 }`; the shell seeds from std entropy, no new dependency. Shell seeding deferred to 4.5 (2026-11-04), where the shell first builds an app. Done: a `CorePlugins` app has `Random` without the harness inserting it; the group's doc lists every precondition.
- **3.19 MUST** Soak coverage. Done: at least 100,000 steps counted in `Playing`; one side tracks the ball with seeded noise, the other plays random intents; each frame's length drawn from {½, 1, 2, 4} steps; paddles inside the walls on every step; the speed cap reached at least once; a minimum paddle-hit count set from the first green run; under 30 s.
- **3.20 SHOULD** Small corrections. Done: the `Seat` doc says one seat picks both sides in menus; `domain.rs` imports at the top; the harness doc says only schedules present at build are configured; the `SeatsPlugin` contract says to report a press released within one frame as a move for that frame.
- **3.21 MUST** Record step. ADRs 0002 and 0003, the architecture, design, and spec amendments, and the corrected report rows were recorded 2026-11-04. Done: the phase report lists every deviation 3.11 through 3.20 introduce, and the amended documents match the code at phase close.

Exit: `just dev-check` green; nothing drawn yet.

### 4. Shell vertical slice: two humans on screen
Realizes spec 1.6.1, 1.6.2 MUSTs, 1.6.6 MUST, 1.7.2. Depends on 3.

- **4.1 MUST** Keyboard adapter from tuning; `SavePaths` from XDG. Done: adapter test. Amended 2026-11-04: a press released within one frame reports a move for that frame, by test; focus loss pauses through `MatchControl` (ADR 0003). Amended 2026-11-04 ([phase 4 report](m1-phase4-report.md)): the adapter reports once per frame before the fixed steps, only while `Playing`; `pong` becomes a library plus a thin binary; 4.1 builds the shell's XDG directory resolver, which 4.4 reuses, and the `SavePaths` resource moves to 7.1. Amended 2026-11-07 (ADR 0004): the adapter reports whenever the paddles move.
- **4.2 MUST** Views: meshes, HDR colors, interpolation, snap, letterboxing camera, bloom. Done: no judder by eye; resize keeps aspect. Amended 2026-11-04 ([phase 4 report](m1-phase4-report.md)): the eye check waits for the dev entry, so the order is 4.2 code and headless tests, then 4.5, then the 4.2 eye check, then 4.3 and 4.4; the view samples each body every fixed tick (4.8 declined); the camera letterboxes by `ScalingMode::AutoMin` over a dark backdrop.
- **4.3 MUST** Window: borderless fullscreen, title. Done: first start fullscreen.
- **4.4 MUST** Panic hook crash file. Done: forced panic produces the file.
- **4.5 MUST** Temporary dev entry (feature or flag) starting a two-human match directly; removed in 8.3. Done: match starts. Amended 2026-11-04: the shell builds `CorePlugins { seed }` with a seed from std entropy (`RandomState`, no new dependency); done when two launches serve at different first angles.
- **4.6 SHOULD** Keyboard ghosting check (spec 1.8.2) run now. Done: result in the playtest log.
- **4.7 MUST** `bevy-api-check` consulted before each rendering API when Claude drives. Done: one report per view module.
- **4.9 MUST** Paddles move between rallies (ADR 0004). Added 2026-11-07 after the first two-human runs. Done: tests show a paddle moving in `Countdown` and `PointScored` while the ball waits, and still while paused or after the match; a held key moves the paddle during the countdown through the shell.
- **4.8 MAY** Ball view interpolation reads `StepStart` (3.15). Added 2026-11-04. Done: no shell-side copy of the previous ball position. Declined 2026-11-04 (phase 4 report): `StepStart` updates only on live steps, so a view reading it replays the last step's motion through every pause; the view samples every fixed tick, paddles included.

Exit: two people play Pong to ten on screen.

### 5. AI
Realizes spec 1.4.6. Depends on 3; parallel with 4.

- **5.1 MUST** Mirror controller, three tuning entries. Done: zero-error returns every ball; large error misses over a seeded run.
- **5.2 MUST** Dev entry gains a vs-computer variant. Done: a human beats one star and loses to three stars in a manual session. Manual session passed 2026-11-08: `just run --vs 1` won, `just run --vs 3` lost ([phase 5 report](m1-phase5-report.md)).

### 6. Sanity and heal
Realizes spec 1.4.7. Depends on 3 and 5.

- **6.1 MUST** Sanity phase, `SimFault`, heal, dev panic versus release heal. Done: NaN injection test both ways; soak rerun green. Amended 2026-11-04: sanity checks exactly one ball exists in M1; heal re-serves through rules so `Serve` keeps one emitter (ADR 0002); the soak's own invariant checks go once sanity covers them.

### 7. Persistence
Realizes spec 1.4.9. Depends on 3; independent of 4 through 6.

- **7.1 MUST** Models v1, atomic write, versioned enum, fixtures, count test. Done: round-trip and fixture tests green. Amended 2026-11-04: also the `SavePaths` port resource, inserted by the shell from the XDG resolver of 4.1, and the harness's temp-dir fake (spec 1.5.1); its reader decides its shape.
- **7.2 MUST** Load and write policies with `Notice`. Done: corrupt, version-99, unwritable-path tests green.
- **7.3 MUST** Guest semantics. Done: test green.
- **7.4 MUST** Rally counter owned by rules, feeding the "longest rally" record (spec 1.4.9) and, later, C1's meter. Added 2026-11-04. Done: the match's longest rally is readable when `MatchEnded` fires, by test.

### 8. UI flow
Realizes spec 1.6.3, settings mode in 1.6.6, 1.7.3. Depends on 4, 5, 7.

- **8.1 MUST** All screens per spec 1.6.3 including seat cue, key filter, text entry with filter suspension, name cap, notice line, settings. Done: full keyboard-only flow; filter and name tests green. Amended 2026-11-08 ([phase 7 report](m1-phase7-report.md)): persistence writes only at match end (7.2), so the settings and profile screens add their own save trigger, or a change is lost if the game closes before a match; the notice line replaces the dev entry's notice log. Amended 2026-11-08 (ADR 0008): the menu builds `MatchConfig.players` from each seat's pick and does not offer one profile to both seats. Amended 2026-11-08 ([phase 8 report](m1-phase8-report.md)): paddles take their players' profile colours (spec 1.6.2), which no item had named.
- **8.2 SHOULD** Rename and delete. Done: reachable. Done 2026-11-08 ([phase 8 report](m1-phase8-report.md)): an Edit row on profile select, by test.
- **8.3 MUST** Remove the dev entry from 4.5. Done: gone; ADR note. Amended 2026-11-07 (phase 4 report): after a focus loss the pause screen resumes play, since the dev entry's Escape was the only resume until then. Done 2026-11-08: removed with `just run`'s arguments; note in ADR 0003; the pause screen resumes after a focus loss, by test.
- **8.4 SHOULD** Readability review. Done: reviewed against the frame. Done 2026-11-08 ([phase 8 report](m1-phase8-report.md)): the review table and its changes; with it, a profile's colour changes from Edit (the user's addition) and the HUD shows the countdown.

### 9. Audio
Realizes spec 1.6.4. Depends on 3 events and 7 settings; independent of 8.

- **9.1 MUST** Verify the in-memory audio source assumption first. Done: one sound plays from generated bytes, or the WAV-bytes-as-asset fallback is adopted and noted. Done 2026-11-08 ([phase 9 report](m1-phase9-report.md)): no fallback; a custom `Decodable` asset plays synthesized samples, audible by the user's hand check.
- **9.2 MUST** `SoundBank` trait, synth builder, cue map, volume from settings. Done: four sounds; buffer test; play-command-per-event test. Done 2026-11-08 ([phase 9 report](m1-phase9-report.md)): all three, by test.

### 10. Polish
Realizes spec 1.6.2 SHOULD and MAY, 1.6.5, 1.6.6 MAY. Depends on 8.

- **10.1 SHOULD** Ball trail, overlay. Done: visible. Done 2026-11-09 ([phase 10 report](m1-phase10-report.md)): the ball trail, and the overlay on F3, in release builds too; both pending the user's look.
- **10.2 MAY** Screen shake, Alt+Enter, persisted points-to-win. Done: visible or restored. Done 2026-11-09 ([phase 10 report](m1-phase10-report.md)): all three; Alt+Enter since 8.1; the shake pending the user's look.

### 11. Playtest and close
Realizes spec 1.8.3, 1.9. Depends on all above.

- **11.1 SHOULD** One session per child, logged. Done: entries exist. Done 2026-11-09 ([playtest log](../playtest.md)): one shared session of both children, reported by the parent; the menu-fight hypothesis answered no.
- **11.2 MUST** ADRs: error policy, persistence format, audio boundary, plus 0001, 0002, 0003. Done: six files. Amended 2026-11-04. Amended 2026-11-09 at milestone close: the six are ADR 0001, 0002, 0003, 0007 (persistence format), 0009 (error policy), and 0010 (audio boundary); all exist, 0009 and 0010 written 2026-11-09. Ten ADRs in all, 0001 through 0010. Done 2026-11-09 ([phase 11 report](m1-phase11-report.md)): all six exist.
- **11.3 MUST** Close checklist per spec 1.9.3. Done: marked in the M1 spec. Done 2026-11-09: every row marked, the AI and readability reviews signed by the user.

## User actions outside the plan

- Create the GitHub remote before 2.5 can be verified.
- Run `just setup` once to enable the pre-push hook.
- Create a branch per phase.
