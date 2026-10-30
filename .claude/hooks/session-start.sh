#!/usr/bin/env bash
# SessionStart: orient the session. Stdout becomes context for Claude.
set -uo pipefail
root="${CLAUDE_PROJECT_DIR:-$PWD}"
cd "$root" || exit 0

bevy=$(grep -hE '^bevy\s*=' Cargo.toml pong_core/Cargo.toml pong/Cargo.toml 2>/dev/null | head -1)
echo "bevy_pong session"
echo "Bevy: ${bevy:-not yet pinned (arrives in plan phase 2); target 0.20}"
if [ -f docs/specs/CURRENT ]; then
  echo "Current phase: $(cat docs/specs/CURRENT)"
fi
echo "Git:"
git status --short --branch 2>/dev/null | head -15 | sed 's/^/  /'
echo "Reminder: git is the user's domain (read-only + stash for Claude). Run \`just dev-check\` before any handoff."
exit 0
