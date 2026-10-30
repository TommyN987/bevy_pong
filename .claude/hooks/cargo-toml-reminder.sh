#!/usr/bin/env bash
# PostToolUse(Edit|Write): when a Cargo.toml changes, inject the dependency policy
# back into Claude's context. Non-blocking.
set -uo pipefail
f=$(jq -r '.tool_input.file_path // .tool_response.filePath // empty' 2>/dev/null) || exit 0
[ "$(basename -- "$f")" = "Cargo.toml" ] || exit 0
jq -n '{
  hookSpecificOutput:{hookEventName:"PostToolUse",
    additionalContext:"Dependency policy (docs/architecture.md): no new crate without a treadmill justification. Bevy breaks every ~4 months and third-party crates lag. If this edit adds or bumps a dependency, state in the phase report: what it replaces, its support for the pinned Bevy version, and why hand-rolling is worse. pong_core must stay free of render, window, audio, and UI crates."},
  systemMessage:"Cargo.toml edited: dependency policy reminder issued."}'
exit 0
