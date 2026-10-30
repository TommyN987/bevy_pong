#!/usr/bin/env bash
# PreToolUse(Bash) guard. Claude may run read-only git and `git stash`; every
# mutative git/gh operation belongs to the user (docs/claude-code-setup.md).
# Reads the hook JSON on stdin, denies with a reason, never blocks on error.
set -uo pipefail

cmd=$(jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
[ -z "$cmd" ] && exit 0

norm=$(printf '%s' "$cmd" | tr '\n' ' ' | sed -E 's/[[:space:]]+/ /g')
mapfile -t parts < <(printf '%s\n' "$norm" | sed -E 's/(\|\||&&|;|\||\$\(|`)/\n/g')

blocked=""
for p in "${parts[@]}"; do
  p=$(printf '%s' "$p" | sed -E 's/^[[:space:]]+//; s/^([A-Za-z_][A-Za-z0-9_]*=[^ ]* )*//; s/^sudo( -[A-Za-z]+)* //')
  case "$p" in
    git|git\ *)
      sub=$(printf '%s' "$p" | sed -E 's/^git( (-C [^ ]+|-c [^ ]+|--git-dir=[^ ]+|--work-tree=[^ ]+|--no-pager|-p|-P|--no-optional-locks))*//; s/^ //')
      verb=${sub%% *}
      rest=" ${sub#"$verb"}"
      case "$verb" in
        ""|status|log|diff|show|blame|stash|rev-parse|ls-files|ls-tree|describe|grep|shortlog|for-each-ref|cat-file|name-rev|merge-base|rev-list|count-objects|version|help|reflog) ;;
        branch)  printf '%s' "$rest" | grep -qE -- '^ *$|^ +(-a|-r|--all|-l|--list|--show-current|-v|-vv|--contains|--merged|--no-merged)( |$)' || blocked="$p" ;;
        tag)     printf '%s' "$rest" | grep -qE -- '^ *$|^ +(-l|--list|-n[0-9]*|--contains)( |$)' || blocked="$p" ;;
        remote)  printf '%s' "$rest" | grep -qE -- '^ *$|^ +(-v|show|get-url)( |$)' || blocked="$p" ;;
        config)  printf '%s' "$rest" | grep -qE -- '^ +(--get|--get-all|--get-regexp|-l|--list|--show-origin)( |$)' || blocked="$p" ;;
        worktree) printf '%s' "$rest" | grep -qE -- '^ +list( |$)' || blocked="$p" ;;
        *) blocked="$p" ;;
      esac ;;
    gh\ *)
      printf '%s' "$p" | grep -qE -- '^gh (pr|issue|release|repo) (create|merge|close|edit|delete|reopen|comment|review|ready|checkout|fork|sync|rename|archive|upload)( |$)' && blocked="$p" ;;
  esac
  [ -n "$blocked" ] && break
done

if [ -n "$blocked" ]; then
  reason="Blocked: [$blocked] is a mutative git/gh operation, which belongs to the user. Claude may run: git status, log, diff, show, blame, stash, rev-parse, ls-files, and read-only branch/tag/remote/config listings. Ask the user to run it."
  jq -n --arg r "$reason" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:$r}}'
fi
exit 0
