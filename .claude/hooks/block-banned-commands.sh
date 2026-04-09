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

# ── Guard timeouts on review (codex/gemini) commands ────────────────
# codex/gemini exec calls are review tasks, NOT tests. They take 20-35
# minutes in practice — reviews barely ever finish in under 10 minutes,
# and the operational sweet spot is 20-35 min. Block any timeout outside
# that window so a foreground review can't be killed mid-stream.
# Minimum allowed: 1200000 ms (20 minutes).
# Maximum allowed: 2100000 ms (35 minutes).
#
# BUG-08-001: The matcher must fire only on GENUINE top-level codex or
# gemini invocations — never on commands that merely mention the literal
# strings in a path, argument, message body, or quoted text.
#
# TPR-04-001-codex/gemini fix (commit pending): the previous regex-based
# REVIEW_CMD_RE approach leaked bypasses because shell is not a regular
# language. Seven distinct bypass forms were verified against the old
# regex (escaped double quotes, $(...) command substitution, backtick
# substitution, heredocs inside $(...), backslash-newline continuation,
# literal newline separators, env-var values with internal whitespace
# that the regex fragmented). Each new alternation opened up more edge
# cases.
#
# The correct architectural fix is shell-aware tokenization. We delegate
# classification to `.claude/hooks/classify-review-command.py`, which
# walks the command string with a character-level state machine tracking
# quote state, subshell depth, command substitution, and compound
# operators. The classifier returns exit 0 iff the command contains a
# top-level codex/gemini invocation at any command position.
#
# See `.claude/hooks/verify-hook.sh` for the full matrix and
# `plans/bug-tracker/fix-BUG-08-001.md` for the original design rationale.

CLASSIFY_REVIEW_CMD="$(dirname "${BASH_SOURCE[0]}")/classify-review-command.py"

is_review_command() {
  # Returns 0 if COMMAND invokes codex/gemini at a top-level command
  # position, 1 otherwise. Delegates to the Python classifier so shell
  # grammar edge cases are handled correctly (the earlier regex-based
  # approach kept leaking bypasses).
  printf '%s' "$COMMAND" | python3 "$CLASSIFY_REVIEW_CMD"
}

TIMEOUT=$(printf '%s' "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_input',{}).get('timeout',''))" 2>/dev/null || true)

if [[ -n "$TIMEOUT" && "$TIMEOUT" != "None" ]]; then
  if is_review_command; then
    # Require at least 20 minutes (1200000 ms) — anything shorter risks
    # killing the review mid-stream (reviews barely ever complete in 10
    # minutes, so 5- and 10-minute timeouts almost always fail).
    if [[ "$TIMEOUT" =~ ^[0-9]+$ ]] && (( TIMEOUT < 1200000 )); then
      deny "Blocked: timeout ($TIMEOUT ms) on codex/gemini command is too short. Reviews need 20-35 minutes — use at least 1200000 ms, up to 2100000 ms (35 min)."
    fi
    if [[ "$TIMEOUT" =~ ^[0-9]+$ ]] && (( TIMEOUT > 2100000 )); then
      deny "Blocked: timeout ($TIMEOUT ms) on codex/gemini command exceeds 35-minute ceiling (2100000 ms)."
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
