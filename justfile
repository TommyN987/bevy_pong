# Command surface for bevy_pong. Hooks, CI, humans, and agents call these recipes
# and nothing else, so every gate runs the same command everywhere.
# Phase 2 of docs/specs/m1-plan.md adds fmt, lint, test, dev-check, ci-check, run.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes.
default:
    @just --list

# One-time local setup: point git at the repo's hooks (pre-push runs `just ci-check`).
setup:
    git config core.hooksPath .githooks
    @echo "git hooks enabled from .githooks/"
