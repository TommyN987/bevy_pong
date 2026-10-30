#!/usr/bin/env bash
# Pipe-tests for git-guard.sh. Run: bash .claude/hooks/test-git-guard.sh
# Each case: expected verdict, then the command string the Bash tool would carry.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
export CLAUDE_PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$here/../.." && pwd)}"
guard="$here/git-guard.sh"
fail=0

check() {
  local expect="$1" cmd="$2" out verdict
  out=$(printf '{"tool_name":"Bash","tool_input":{"command":%s}}' "$(printf '%s' "$cmd" | jq -Rs .)" | "$guard")
  if [ -n "$out" ]; then verdict=DENY; else verdict=allow; fi
  if [ "$verdict" = "$expect" ]; then
    printf 'ok    %-5s %s\n' "$verdict" "$cmd"
  else
    printf 'FAIL  got %-5s want %-5s %s\n' "$verdict" "$expect" "$cmd"
    fail=1
  fi
}

check allow 'git status'
check allow 'git log --oneline | head'
check allow 'git diff -- docs/'
check allow 'git show HEAD:docs/frame.md'
check allow 'git blame docs/design.md'
check allow 'git stash push -m wip'
check allow 'git stash pop'
check allow 'git branch'
check allow 'git branch --show-current'
check allow 'git tag -l'
check allow 'git remote -v'
check allow 'git worktree list'
check allow 'git config --get core.hooksPath'
check allow 'git rev-parse --show-toplevel'
check allow 'echo "git commit" > note.txt'
check allow 'gh pr view 1'
check allow 'ls'
check allow 'cargo build'

check DENY 'git branch feature/x'
check DENY 'git tag v1'
check DENY 'git config core.hooksPath .githooks'
check DENY 'git commit -m x'
check DENY 'git add .'
check DENY 'cd /tmp && git add .'
check DENY 'GIT_DIR=.git git push'
check DENY 'git -C /home/tommy/repos/bevy_pong commit -am x'
check DENY 'cargo build && git checkout main'
check DENY 'git switch -c x'
check DENY 'git rebase -i HEAD~2'
check DENY 'git reset --hard'
check DENY 'git restore docs/'
check DENY 'gh pr create --fill'
check DENY 'gh repo create x'
check DENY 'sudo git push'

echo
echo "deny payload sample:"
printf '{"tool_name":"Bash","tool_input":{"command":"git commit -m x"}}' | "$guard" | jq -c .

exit $fail
