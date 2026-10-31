# Command surface for bevy_pong. Hooks, CI, humans, and agents call these recipes
# and nothing else, so every gate runs the same command everywhere.
# See docs/specs/m1-basic-pong.md (1.1.7–1.1.10) and docs/claude-code-setup.md.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes.
default:
    @just --list

# One-time local setup: point git at the repo's hooks (pre-push runs `just ci-check`).
setup:
    git config core.hooksPath .githooks
    @echo "git hooks enabled from .githooks/"

# Format the workspace in place.
fmt:
    cargo fmt --all

# Verify formatting without changing files.
fmt-check:
    cargo fmt --all -- --check

# Clippy over every target; warnings are errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Full test suite: nextest for unit and integration tests, cargo for doctests.
# `--no-tests=warn` is temporary until plan phase 3 adds the first tests; remove it then.
test:
    cargo nextest run --workspace --no-tests=warn
    cargo test --workspace --doc

# Core crate tests only: the fast loop (ADR 0001).
test-core:
    cargo nextest run -p pong_core

# License, advisory, and source checks.
deny:
    cargo deny check

# Interactive gate: formats in place, then lint, then tests.
dev-check: fmt lint test

# Non-mutating gate for CI and the pre-push hook: format check, lint, tests, deny.
ci-check: fmt-check lint test deny

# Run the game with the dev feature (dynamic linking).
run:
    cargo run -p pong --features dev

# Run an optimized build without dev features.
run-release:
    cargo run -p pong --release

# Release binary, as CI builds it.
build-release:
    cargo build -p pong --release --locked
