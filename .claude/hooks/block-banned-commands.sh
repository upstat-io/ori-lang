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

# ── Guard timeouts on review/codex commands ─────────────────────────
# codex exec calls are review tasks, NOT tests. They take 5-35 minutes.
# Block only *short* timeouts that would kill a review mid-stream.
# Maximum allowed: 2100000 ms (35 minutes).
TIMEOUT=$(printf '%s' "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_input',{}).get('timeout',''))" 2>/dev/null || true)

if [[ -n "$TIMEOUT" && "$TIMEOUT" != "None" ]]; then
  if [[ "$COMMAND" == *"codex"* ]]; then
    # Require at least 5 minutes (300000 ms) on codex — anything shorter
    # will kill the review mid-stream.
    if [[ "$TIMEOUT" =~ ^[0-9]+$ ]] && (( TIMEOUT < 300000 )); then
      deny "Blocked: timeout ($TIMEOUT ms) on codex command is too short. Reviews need 5-35 minutes — use at least 300000 ms, up to 2100000 ms (35 min)."
    fi
    if [[ "$TIMEOUT" =~ ^[0-9]+$ ]] && (( TIMEOUT > 2100000 )); then
      deny "Blocked: timeout ($TIMEOUT ms) on codex command exceeds 35-minute ceiling (2100000 ms)."
    fi
  fi
fi

# ── Allow background execution on codex commands ────────────────────
# The Bash tool's foreground timeout cap (600000 ms / 10 min) is shorter
# than the 35-minute upper bound for codex reviews, so background
# execution is the only mechanism that can accommodate long reviews.
# No block here.

# No banned pattern found — no output so normal permission system applies.
exit 0
