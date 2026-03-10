#!/usr/bin/env bash
# PreToolUse hook: block banned git commands even inside compound commands.
# The built-in deny patterns only match the first subcommand of a compound
# command (&&, ;, ||), so this hook inspects the full command string.
#
# No external dependencies — uses python3 for JSON parsing (no jq).

set -euo pipefail

INPUT=$(cat)
COMMAND=$(printf '%s' "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))")

# Same patterns as the deny list in .claude/settings.json, expressed as
# bash substring matches against the raw command string.
BANNED_PATTERNS=(
  "--no-verify"
  "--no-gpg-sign"
  "git stash"
  "git reset --hard"
  "git checkout ."
  "git checkout -- ."
  "git restore ."
  "git push --force"
  "git push -f "
  "git branch -D"
  "git rebase"
)

deny() {
  local reason="$1"
  python3 -c "
import json, sys
print(json.dumps({
    'hookSpecificOutput': {
        'hookEventName': 'PreToolUse',
        'permissionDecision': 'deny',
        'permissionDecisionReason': sys.argv[1]
    }
}))
" "$reason"
  exit 0
}

# git clean with -f anywhere after it
if [[ "$COMMAND" =~ git\ clean.*-f ]]; then
  deny "Blocked: command contains 'git clean -f'"
fi

# git push -f at end of string
if [[ "$COMMAND" =~ git\ push.*-f$ ]]; then
  deny "Blocked: command contains 'git push -f'"
fi

for pattern in "${BANNED_PATTERNS[@]}"; do
  if [[ "$COMMAND" == *"$pattern"* ]]; then
    deny "Blocked: command contains '$pattern'"
  fi
done

# No banned pattern found — no output so normal permission system applies.
exit 0
