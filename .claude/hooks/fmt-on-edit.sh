#!/usr/bin/env bash
# PostToolUse(Edit|Write): format after a Rust edit via the `just fmt` recipe.
# Guarded: no-op until the workspace (root Cargo.toml) and the recipe exist.
set -uo pipefail
f=$(jq -r '.tool_input.file_path // .tool_response.filePath // empty' 2>/dev/null) || exit 0
case "$f" in *.rs) ;; *) exit 0 ;; esac
root="${CLAUDE_PROJECT_DIR:-$PWD}"
[ -f "$root/Cargo.toml" ] || exit 0
command -v just >/dev/null 2>&1 || exit 0
(cd "$root" && just --list 2>/dev/null | grep -qE '^\s*fmt\b') || exit 0
(cd "$root" && just fmt) >/dev/null 2>&1 || true
exit 0
