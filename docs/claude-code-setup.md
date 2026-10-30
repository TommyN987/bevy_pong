# Claude Code Setup

Decided 2026-10-30. Describes the intended Claude Code configuration for this repository.
Implemented in plan phase 0 on branch `chore/claude-setup` (2026-10-31): `CLAUDE.md`, `.claude/settings.json`, `.claude/hooks/`, `.claude/rules/`, `.claude/agents/bevy-api-check.md`, `.claude/skills/{spec,adr,playtest}`, `.githooks/pre-push`, `justfile` (`setup` recipe), `docs/specs/CURRENT`. The git guard has a test script at `.claude/hooks/test-git-guard.sh`; permission deny rules are a second layer under it.

## Working agreement

Git is the user's domain. Claude runs only read-only git and `git stash`. Pair-programming roles vary per phase: Claude may drive, navigate, or review. See the phase protocol in [specs/m1-plan.md](specs/m1-plan.md).

## Requirements the setup answers

1. Keep Claude honest about Bevy's API. Its training predates Bevy 0.20 and Bevy breaks every four months. This is the largest quality risk in the project.
2. Enforce the architecture's crate boundaries mechanically, not only by review.
3. Make the quality gates cheap and impossible to forget: fmt, clippy, tests.
4. Make the rituals repeatable: spec, ADR, Bevy upgrade, playtest log, milestone close.
5. Point to the docs instead of duplicating them. The frame, design, and architecture documents are the truth.
6. Cost little to maintain.

The user's operating-mode framework (#Frame, #Research, #Design, #Record, #Spec, #Code) already lives in a prompt hook. The setup does not re-implement modes; skills are the rituals the modes produce.

## Chosen shape

**CLAUDE.md**, under forty lines:
- what the project is, in two sentences
- pointers to `docs/frame.md`, `docs/research.md`, `docs/design.md`, `docs/architecture.md`
- the pinned Bevy version
- the `just` recipes: `dev-check`, `ci-check`, `test`, `run`
- four non-negotiables: verify Bevy API against docs.rs for the pinned version before writing it; core never imports shell, render, window, audio, or UI; no new dependency without a treadmill justification; never run mutative git (the user owns commits, branches, pushes, PRs)
- where specs and ADRs live

**Rules** in `.claude/rules/`, path-scoped:
- `pong_core.md` scoped to `pong_core/**`: forbidden imports; physics as pure functions; vocabulary floor stays tiny
- `tests.md` scoped to `**/tests/**`: fixture conventions, soak test conventions, headless builder usage

**Hooks** (amended 2026-10-30, twice: all hooks call `just` recipes; and Claude performs no mutative git, so the commit-time Claude Code hook was replaced):
- PostToolUse on Rust file edits: `just fmt`, guarded on a root `Cargo.toml` existing
- PreToolUse on Bash: blocks mutative git commands (add, commit, push, pull, fetch, branch, checkout, switch, merge, rebase, reset, tag, cherry-pick, revert, clean, `gh pr create`) and lists the allowed set: status, log, diff, show, blame, stash
- SessionStart: print pinned Bevy version, current milestone, git status
- PostToolUse on `Cargo.toml` edits: print the dependency policy as a non-blocking reminder

The quality gate for the user's own git flow is a native `pre-push` hook in `.githooks/` running `just ci-check`, enabled once by `just setup` (sets `core.hooksPath`). Pre-push rather than pre-commit, because clippy on a Bevy workspace costs tens of seconds and small commits should stay cheap. `just dev-check` is the mutating variant for interactive use.

**Skills**:
- `/spec`: scaffold a milestone spec from a template mirroring the design invariants
- `/adr`: write an architecture decision record
- `/bevy-upgrade`: walk the migration guide, bump, build, list lagging crates
- `/playtest`: append a session to the playtest log in observe-then-interpret format
- `/milestone-close`: check CI, docs, ADRs, and playtest before declaring a milestone done

**Subagents**:
- one custom, read-only `bevy-api-check`: fetches docs.rs for the pinned version and reports API mismatches before Bevy code is written
- reused from installed plugins: `silent-failure-hunter` (maps to the error policy), `pr-test-analyzer` (maps to the every-class-has-a-test rule), `type-design-analyzer` (maps to validated constructors and component-owned invariants)

**Permissions**: cargo subcommands and `just` allowed; mutative git commands and `cargo publish` denied.

**Not now**: MCP servers, scheduled routines, multi-agent workflows, custom status line. The installed automation-recommender skill is to be run after milestone one, when a codebase exists to analyze.

## Alternatives considered

- Minimal (CLAUDE.md, fmt hook, CI): rejected because it leaves the Bevy API drift risk unaddressed.
- Full automation (workflows per milestone, release-check routine, status line): rejected as cost and moving parts beyond a hobby Pong; a release check is one search per session.

## Known caveat

Path-scoped rules were reported by one tester to require unquoted `paths` globs. Verify when creating the first rule.
