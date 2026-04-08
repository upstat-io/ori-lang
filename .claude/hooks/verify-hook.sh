#!/usr/bin/env bash
# verify-hook.sh — regression test suite for block-banned-commands.sh
#
# Runs the full timeout-gate matrix against the hook and reports
# PASS/FAIL per scenario. Exits non-zero on any failure. Designed to
# run in <1 second so it can be invoked freely after any hook edit.
#
# Usage:
#   bash .claude/hooks/verify-hook.sh
#   bash .claude/hooks/verify-hook.sh --quiet   # only report failures
#
# Why this exists:
#   The hook gates timeout windows on codex/gemini commands. Any change
#   to its threshold constants, conditional structure, or deny messages
#   risks breaking review-time guarantees. Reconstructing the 9-test
#   matrix from the section plan every time someone touches the hook
#   was a 10-15 minute manual chore; this script makes it a single
#   command. Surfaced by the dual-tpr-gemini section-01.4 retrospective.
#
# Test matrix dimensions:
#   command in {codex, gemini}  ×  timeout in {1min, 10min, 25min, 60min}
#   plus a control (echo, 1min) proving the gate is scoped to codex/gemini.
#
# Keep this matrix in sync with the section file's documented matrix —
# they describe the same contract.

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK="$SCRIPT_DIR/block-banned-commands.sh"
QUIET=0

if [[ "${1:-}" == "--quiet" ]]; then
  QUIET=1
fi

if [[ ! -x "$HOOK" ]]; then
  printf 'error: hook not found or not executable: %s\n' "$HOOK" >&2
  exit 1
fi

# Color codes (no dependency on tput)
GREEN='\033[0;32m'
RED='\033[0;31m'
DIM='\033[0;2m'
RESET='\033[0m'
if [[ -n "${NO_COLOR:-}" ]] || [[ ! -t 1 ]]; then
  GREEN='' RED='' DIM='' RESET=''
fi

PASS=0
FAIL=0
FAILED_TESTS=()

# run_test NAME COMMAND TIMEOUT EXPECTED [DENY_SUBSTR]
#   EXPECTED: "deny" or "allow"
#   DENY_SUBSTR: optional substring that the deny message must contain
#                (only checked when EXPECTED=deny)
run_test() {
  local name="$1" cmd="$2" timeout="$3" expected="$4" deny_substr="${5:-}"

  local input
  input=$(printf '{"tool_name":"Bash","tool_input":{"command":"%s","timeout":%s}}' "$cmd" "$timeout")
  local output
  output=$(printf '%s' "$input" | bash "$HOOK" 2>&1)

  local actual="allow"
  if [[ -n "$output" ]] && printf '%s' "$output" | grep -q '"permissionDecision": *"deny"'; then
    actual="deny"
  fi

  local ok=0
  if [[ "$actual" == "$expected" ]]; then
    ok=1
    if [[ "$expected" == "deny" && -n "$deny_substr" ]]; then
      if ! printf '%s' "$output" | grep -q -- "$deny_substr"; then
        ok=0
      fi
    fi
  fi

  if (( ok )); then
    PASS=$((PASS + 1))
    if (( ! QUIET )); then
      printf '  %b[PASS]%b %s\n' "$GREEN" "$RESET" "$name"
    fi
  else
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("$name")
    printf '  %b[FAIL]%b %s\n' "$RED" "$RESET" "$name"
    printf '    %bcmd:%b      %s\n' "$DIM" "$RESET" "$cmd"
    printf '    %btimeout:%b  %s ms\n' "$DIM" "$RESET" "$timeout"
    printf '    %bexpected:%b %s' "$DIM" "$RESET" "$expected"
    if [[ -n "$deny_substr" ]]; then
      printf ' (containing "%s")' "$deny_substr"
    fi
    printf '\n'
    printf '    %bgot:%b      %s\n' "$DIM" "$RESET" "$actual"
    if [[ -n "$output" ]]; then
      printf '    %boutput:%b   %s\n' "$DIM" "$RESET" "$output"
    fi
  fi
}

if (( ! QUIET )); then
  printf '=== block-banned-commands.sh — timeout gate matrix ===\n'
fi

# Codex × {1min, 10min, 25min, 60min}
run_test "codex + 1 min   → DENY (under 20-min floor)"        "codex exec test" 60000   deny "20-35 minutes"
run_test "codex + 10 min  → DENY (negative pin: floor raised)" "codex exec test" 600000  deny "20-35 minutes"
run_test "codex + 25 min  → ALLOW (in sweet spot)"             "codex exec test" 1500000 allow
run_test "codex + 60 min  → DENY (over 35-min ceiling)"        "codex exec test" 3600000 deny "35-minute ceiling"

# Gemini × {1min, 10min, 25min, 60min}
run_test "gemini + 1 min  → DENY (under 20-min floor)"         "gemini -p test"  60000   deny "20-35 minutes"
run_test "gemini + 10 min → DENY (negative pin: floor raised)" "gemini -p test"  600000  deny "20-35 minutes"
run_test "gemini + 25 min → ALLOW (in sweet spot)"             "gemini -p test"  1500000 allow
run_test "gemini + 60 min → DENY (over 35-min ceiling)"        "gemini -p test"  3600000 deny "35-minute ceiling"

# Control: gate must NOT apply to non-codex/gemini commands
run_test "control: echo + 1 min → ALLOW (gate doesn't apply)"  "echo hello"      60000   allow

if (( ! QUIET )); then
  printf '\n'
fi

if (( FAIL == 0 )); then
  printf '%b[OK]%b %d/%d tests passed\n' "$GREEN" "$RESET" "$PASS" "$((PASS + FAIL))"
  exit 0
fi

printf '%b[FAIL]%b %d/%d tests passed (%d failed)\n' "$RED" "$RESET" "$PASS" "$((PASS + FAIL))" "$FAIL"
printf '\nFailed tests:\n'
for t in "${FAILED_TESTS[@]}"; do
  printf '  - %s\n' "$t"
done
exit 1
