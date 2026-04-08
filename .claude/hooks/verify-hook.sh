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
#
# The JSON input is encoded via python3 (a hook dependency) so that
# commands containing double quotes, backslashes, newlines, or other
# JSON-sensitive characters round-trip correctly. A printf-based encoder
# would corrupt any such command and silently produce the wrong input.
run_test() {
  local name="$1" cmd="$2" timeout="$3" expected="$4" deny_substr="${5:-}"

  local input
  input=$(CMD="$cmd" TIMEOUT="$timeout" python3 -c '
import json, os, sys
sys.stdout.write(json.dumps({
    "tool_name": "Bash",
    "tool_input": {
        "command": os.environ["CMD"],
        "timeout": int(os.environ["TIMEOUT"]),
    },
}))
')
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

# ── False-positive suppression (BUG-08-001) ──────────────────────────
# The hook originally used a naive substring match — any command whose
# arguments, paths, or message body contained the literal strings
# "codex" or "gemini" was denied. The fix narrows the match to genuine
# invocations at shell-command position. These tests prove the false
# positives are gone. All use a short (60000 ms) timeout; ALL must
# return ALLOW because none of them actually invoke codex or gemini.

run_test "git commit with 'gemini' in message → ALLOW (message body, not invocation)" \
  'git commit -m "fix dual-tpr-gemini work"' 60000 allow
run_test "git commit with 'codex' in message → ALLOW (message body, not invocation)" \
  'git commit -m "refactor codex parser"' 60000 allow
run_test "grep codex as arg → ALLOW (codex is grep's argument, not a command)" \
  "grep codex .claude/" 60000 allow
run_test "grep gemini as arg → ALLOW (gemini is grep's argument, not a command)" \
  "grep gemini .claude/" 60000 allow
run_test "ls .codex/skills/ → ALLOW (.codex is a path component preceded by .)" \
  "ls .codex/skills/" 60000 allow
run_test "ls .gemini/skills/ → ALLOW (.gemini is a path component preceded by .)" \
  "ls .gemini/skills/" 60000 allow
run_test "cat dual-tpr-gemini section file → ALLOW (gemini in a path, preceded by -)" \
  "cat plans/dual-tpr-gemini/section-04-tpr-review.md" 60000 allow
run_test "bash dual-invoke.sh script → ALLOW (top-level command is bash, not codex/gemini)" \
  "bash .claude/skills/dual-tpr/scripts/dual-invoke.sh arg1" 60000 allow
run_test "./my-gemini-wrapper.sh → ALLOW (gemini inside a script name, preceded by -)" \
  "./my-gemini-wrapper.sh test" 60000 allow
run_test "echo 'fix codex and gemini' → ALLOW (both substrings in a quoted arg)" \
  "echo 'fix codex and gemini bugs'" 60000 allow

# ── Bypass closure — compound invocations (BUG-08-001) ───────────────
# A word-boundary-only fix would regress the gate on env-var prefixes,
# pipelines, sequences, &&, and subshells. These tests pin the gate
# shut: each command is a GENUINE codex/gemini invocation reached via
# a compound-command form, and all must DENY at 60000 ms.

run_test "ORI_LOG=debug codex exec → DENY (env-var prefix bypass closed)" \
  "ORI_LOG=debug codex exec test" 60000 deny "20-35 minutes"
run_test "multiple env-var prefixes + codex → DENY (multi-env bypass closed)" \
  "TARGET=x86_64 ORI_LOG=debug codex exec test" 60000 deny "20-35 minutes"
run_test "pipeline into codex exec → DENY (| bypass closed)" \
  "cat prompt.md | codex exec test" 60000 deny "20-35 minutes"
run_test "&& codex exec → DENY (logical AND bypass closed)" \
  "some-prep && codex exec test" 60000 deny "20-35 minutes"
run_test "; gemini -p → DENY (sequence bypass closed)" \
  "cleanup; gemini -p test" 60000 deny "20-35 minutes"
run_test "(codex exec) subshell → DENY (subshell bypass closed)" \
  "(codex exec test)" 60000 deny "20-35 minutes"
run_test "gemini --approval-mode → DENY (flag-form invocation still denied)" \
  "gemini --approval-mode yolo" 60000 deny "20-35 minutes"
run_test "gemini --output-format stream-json -p → DENY (flag-form invocation still denied)" \
  "gemini --output-format stream-json -p test" 60000 deny "20-35 minutes"

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
