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
- **P.2 MUST** The driver implements an item. When the user drives, Claude navigates: explains the Bevy idiom and its why, points at API verified against docs.rs for the pinned version, keeps the done-condition in view. When Claude drives, it runs `bevy-api-check` before each Bevy API. Done: done-condition evidence exists, produced by the driver.
- **P.3 MUST** `just dev-check` is green before any handoff. Done: green.
- **P.4 MUST** The reviewer checks the item against its done-condition and the architecture rules. When Claude reviews, it reports and changes nothing without the user's word. Done: review recorded in session or PR.
- **P.5 MUST** The user commits, pushes, opens the PR, merges. Claude drafts commit messages on request. Done: PR merged.
- **P.6 MUST** Deviations from the spec are noted in the phase report and land in an ADR or spec amendment at the next record step. Done: no silent deviations at milestone close.

## Phases

### 0. Claude Code setup
Realizes [claude-code-setup.md](../claude-code-setup.md). Depends on nothing.

- **0.1 MUST** CLAUDE.md under forty lines: project in two sentences; pointers to frame, research, design, architecture, specs; Bevy 0.20 with exact patch filled in at 2.7; the `just` recipes; four non-negotiables (verify Bevy API against docs.rs for the pinned version; core never imports shell, render, window, audio, or UI; no new dependency without a treadmill justification; never run mutative git). Done: file exists, under forty lines.
- **0.2 MUST** `.claude/rules/pong_core.md` scoped to `pong_core/**`; `tests.md` scoped to `**/tests/**`. Done: both load on a matching edit after phase 2; glob quoting checked.
- **0.3 MUST** Hooks: PostToolUse on Rust edits runs `just fmt` (guarded on workspace existence); PreToolUse on Bash blocks mutative git commands and lists the allowed set; SessionStart prints Bevy version, milestone, git status; PostToolUse on `Cargo.toml` prints the dependency policy. Done: each fires once in a dry trial; a dry `git commit` attempt is blocked.
- **0.4 MUST** Subagent `bevy-api-check`, read-only, fetches docs.rs for the pinned version and reports mismatches. Done: definition present; invoked once on a sample snippet.
- **0.5 SHOULD** Skills `/spec`, `/adr`, `/playtest` now; `/bevy-upgrade` and `/milestone-close` when first needed. Done: three skill files invocable.
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
- **3.9 SHOULD** Dev override with hot reload and keep-last-good. Done: unit test; manual check once the shell exists.
- **3.10 SHOULD** Schedule randomization 1.5.3. Done: green with it on.

Exit: `just dev-check` green; nothing drawn yet.

### 4. Shell vertical slice: two humans on screen
Realizes spec 1.6.1, 1.6.2 MUSTs, 1.6.6 MUST, 1.7.2. Depends on 3.

- **4.1 MUST** Keyboard adapter from tuning; `SavePaths` from XDG. Done: adapter test.
- **4.2 MUST** Views: meshes, HDR colors, interpolation, snap, letterboxing camera, bloom. Done: no judder by eye; resize keeps aspect.
- **4.3 MUST** Window: borderless fullscreen, title. Done: first start fullscreen.
- **4.4 MUST** Panic hook crash file. Done: forced panic produces the file.
- **4.5 MUST** Temporary dev entry (feature or flag) starting a two-human match directly; removed in 8.3. Done: match starts.
- **4.6 SHOULD** Keyboard ghosting check (spec 1.8.2) run now. Done: result in the playtest log.
- **4.7 MUST** `bevy-api-check` consulted before each rendering API when Claude drives. Done: one report per view module.

Exit: two people play Pong to ten on screen.

### 5. AI
Realizes spec 1.4.6. Depends on 3; parallel with 4.

- **5.1 MUST** Mirror controller, three tuning entries. Done: zero-error returns every ball; large error misses over a seeded run.
- **5.2 MUST** Dev entry gains a vs-computer variant. Done: a human beats one star and loses to three stars in a manual session.

### 6. Sanity and heal
Realizes spec 1.4.7. Depends on 3 and 5.

- **6.1 MUST** Sanity phase, `SimFault`, heal, dev panic versus release heal. Done: NaN injection test both ways; soak rerun green.

### 7. Persistence
Realizes spec 1.4.9. Depends on 3; independent of 4 through 6.

- **7.1 MUST** Models v1, atomic write, versioned enum, fixtures, count test. Done: round-trip and fixture tests green.
- **7.2 MUST** Load and write policies with `Notice`. Done: corrupt, version-99, unwritable-path tests green.
- **7.3 MUST** Guest semantics. Done: test green.

### 8. UI flow
Realizes spec 1.6.3, settings mode in 1.6.6, 1.7.3. Depends on 4, 5, 7.

- **8.1 MUST** All screens per spec 1.6.3 including seat cue, key filter, text entry with filter suspension, name cap, notice line, settings. Done: full keyboard-only flow; filter and name tests green.
- **8.2 SHOULD** Rename and delete. Done: reachable.
- **8.3 MUST** Remove the dev entry from 4.5. Done: gone; ADR note.
- **8.4 SHOULD** Readability review. Done: reviewed against the frame.

### 9. Audio
Realizes spec 1.6.4. Depends on 3 events and 7 settings; independent of 8.

- **9.1 MUST** Verify the in-memory audio source assumption first. Done: one sound plays from generated bytes, or the WAV-bytes-as-asset fallback is adopted and noted.
- **9.2 MUST** `SoundBank` trait, synth builder, cue map, volume from settings. Done: four sounds; buffer test; play-command-per-event test.

### 10. Polish
Realizes spec 1.6.2 SHOULD and MAY, 1.6.5, 1.6.6 MAY. Depends on 8.

- **10.1 SHOULD** Ball trail, overlay. Done: visible.
- **10.2 MAY** Screen shake, Alt+Enter, persisted points-to-win. Done: visible or restored.

### 11. Playtest and close
Realizes spec 1.8.3, 1.9. Depends on all above.

- **11.1 SHOULD** One session per child, logged. Done: entries exist.
- **11.2 MUST** ADRs: error policy, persistence format, audio boundary, plus 0001. Done: four files.
- **11.3 MUST** Close checklist per spec 1.9.3. Done: marked in the M1 spec.

## User actions outside the plan

- Create the GitHub remote before 2.5 can be verified.
- Run `just setup` once to enable the pre-push hook.
- Create a branch per phase.
