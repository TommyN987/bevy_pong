# bevy_pong

Pong in Rust + Bevy with escalating chaos twists, built for two kids (9 and 11) and as a Bevy/ECS learning project. Hobby project held to a commercial quality bar.

## Read first
- `docs/frame.md`: problem, constraints, non-goals
- `docs/design.md`: chosen twists, milestone spine, binding decisions
- `docs/architecture.md`: crate layout, boundaries, error policy, intra-crate rules
- `docs/specs/`: milestone specs and plan; `docs/specs/CURRENT` names the active phase
- `docs/adr/`: decision records; `docs/claude-code-setup.md`: this tooling

## Toolchain
- Bevy `=0.20.1`, pinned in the workspace `Cargo.toml`. Verify every API on docs.rs for that exact version.
- Commands are `just` recipes only: `just dev-check` (fmt, clippy -D warnings, nextest, doctests), `just ci-check` (same, fmt in check mode, plus cargo-deny), `just test`, `just test-core`, `just run`, `just build-release`.
- Workspace: `pong_core` (render-free simulation, headless-testable) and `pong` (shell: window, render, audio, UI).
- Clippy pedantic with no upfront allows. Silence inline with `#[expect(clippy::x, reason = "...")]`; promote to a workspace allow only at ~5 expects across 2+ modules.

## Non-negotiables
1. Verify every Bevy API against docs.rs for the pinned version before writing it. Use the `bevy-api-check` agent.
2. `pong_core` never imports shell, render, window, audio, or UI crates.
3. No new dependency without a treadmill justification: Bevy breaks every ~4 months and crates lag.
4. Never run mutative git: no add, commit, branch, switch, push, pull, merge, rebase, reset, tag, or PR. Read-only git and `git stash` only. The user owns history.

## Working agreement
Roles vary per phase: Claude drives, or the user drives with Claude navigating and reviewing. Every work item has a done-condition in its spec. `just dev-check` is green before any handoff. Deviations from spec go in the phase report, then into an ADR or spec amendment.
