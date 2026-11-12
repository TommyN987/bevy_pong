# Command surface for bevy_pong. Hooks, CI, humans, and agents call these recipes
# and nothing else, so every gate runs the same command everywhere.
# See docs/specs/m1-basic-pong.md (1.1.7–1.1.10) and docs/claude-code-setup.md.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Where the dev build's file watcher looks for the embedded tuning file (spec 1.4.8). Bevy
# watches `BEVY_ASSET_ROOT`, else the running crate's directory, which for `pong` is `pong/`
# and misses `pong_core/src/features/tuning/tuning.ron`. It must be absolute.
dev_asset_root := justfile_directory() / "pong_core"

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
test:
    cargo nextest run --workspace
    cargo test --workspace --doc

# Core crate tests only: the fast loop (ADR 0001).
test-core:
    cargo nextest run -p pong_core

# The AI-versus-AI harness's numbers (M2 plan 4.2, E.4): rally lengths per pairing,
# win rates per star, and the stack's R3 measure, printed by the tests that assert on them.
ai-report:
    cargo nextest run -p pong_core --test ai_matches --no-capture

# License, advisory, and source checks.
deny:
    cargo deny --log-level error check

# Layering (ADR 0002): flow imports no feature and not the roof; a feature imports
# neither the roof nor a feature later in the order of pong_core/src/features/mod.rs.
# ADR 0011: an effect's module imports the effects base and never another effect.
# Production code only: each file is read up to its first `#[cfg(test)]` line, so
# test modules must sit at the end of their file.
arch-check:
    #!/usr/bin/env bash
    set -euo pipefail
    order=(tuning seats ball paddle effects physics rules escalation ai sanity persistence)
    production() { sed '/^#\[cfg(test)\]/,$d' "$1"; }
    position() { local i; for i in "${!order[@]}"; do [[ ${order[$i]} == "$1" ]] && { echo "$i"; return; }; done; echo -1; }
    failed=0
    for file in $(find pong_core/src/flow -name '*.rs'); do
        if production "$file" | grep -nE 'crate::(app|features)\b'; then
            echo "^ $file: flow imports only kernel, domain, and Bevy"; failed=1
        fi
    done
    for dir in pong_core/src/features/*/; do
        feature=$(basename "$dir")
        own=$(position "$feature")
        if (( own < 0 )); then
            echo "$dir: feature '$feature' is not in the feature order"; failed=1; continue
        fi
        for file in $(find "$dir" -name '*.rs'); do
            if production "$file" | grep -nE 'crate::app\b|crate::features::\{'; then
                echo "^ $file: no roof imports; name one feature per path"; failed=1
            fi
            for imported in $(production "$file" | grep -oE 'crate::features::[a-z_]+' | sed 's/.*:://' | sort -u); do
                [[ $imported == "$feature" ]] && continue
                if (( $(position "$imported") < 0 || $(position "$imported") >= own )); then
                    echo "$file: '$feature' imports '$imported', which is not earlier in the feature order"; failed=1
                fi
            done
        done
    done
    # ADR 0011: an effect reaches the effects base and nothing beside it.
    for file in $(find pong_core/src/features/effects -name '*.rs' ! -path 'pong_core/src/features/effects/mod.rs'); do
        if production "$file" | grep -nE '(super|features::effects)::(\{|[a-z_]+)'; then
            echo "^ $file: an effect imports the effects base by name, never another effect"; failed=1
        fi
    done
    exit "$failed"

# Interactive gate: formats in place, then the layering check, lint, and tests.
dev-check: fmt arch-check lint test

# Non-mutating gate for CI and the pre-push hook: format check, layering, lint, tests, deny.
ci-check: fmt-check arch-check lint test deny

# Run the game with the dev feature (dynamic linking).
run:
    BEVY_ASSET_ROOT={{dev_asset_root}} cargo run -p pong --features dev

# Keyboard ghosting check (spec 1.8.2): the game with each change of held seat keys logged.
# Hold W, S, D and Up, Down, Right together; the log should list six keys.
key-check:
    BEVY_ASSET_ROOT={{dev_asset_root}} RUST_LOG=pong::ports::keyboard=debug cargo run -p pong --features dev

# Run an optimized build without dev features.
run-release:
    cargo run -p pong --release

# Release binary, as CI builds it.
build-release:
    cargo build -p pong --release --locked
