#!/usr/bin/env bash
# dual-invoke-with-retry.sh — wraps dual-invoke.sh with infra retry logic.
#
# Usage: same args as dual-invoke.sh
#
# Retry policy:
#   - 3 attempts per reviewer per round
#   - Exponential backoff: 1s, 2s, 4s between attempts
#   - Retries are SEPARATE from the wrapper's semantic iteration budget
#   - On failure: returns the failure category as the last line of stderr,
#     leaves $RUN intact for postmortem, exits 1
#
# Success criteria (all must hold):
#   - dual-invoke.sh exits 0 (both reviewers exited cleanly)
#   - parse-codex.py succeeds on $RUN/codex.jsonl
#   - parse-gemini.py succeeds on $RUN/gemini.jsonl
#   - worktree-guard.sh compare passes (no dirty files)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MAX_RETRIES=3
BACKOFFS=(1 2 4)

# Pass through all args to dual-invoke.sh; we also need RUN and SCHEMA to know
# where outputs go and which schema to validate against.
RUN=""
SCHEMA=""
ARGS=("$@")
for ((i=0; i<${#ARGS[@]}; i++)); do
  if [[ "${ARGS[$i]}" == "--run" ]]; then
    RUN="${ARGS[$((i+1))]}"
  elif [[ "${ARGS[$i]}" == "--schema" ]]; then
    SCHEMA="${ARGS[$((i+1))]}"
  fi
done

[[ -z "$RUN" ]] && { echo "missing --run arg" >&2; exit 2; }
[[ -z "$SCHEMA" ]] && { echo "missing --schema arg" >&2; exit 2; }

# is_terminal_failure: classify a failure category as either terminal (no
# retry) or retryable (worth another attempt). Returns 0 (true) for terminal,
# 1 (false) for retryable.
#
# Terminal categories — deterministic, retry will produce the same result:
#   dirty_worktree           — reviewer is misbehaving (BUG-08-002 fix)
#   codex_invalid_*          — codex/OpenAI rejected our request structurally
#   codex_authentication_*   — auth failures don't fix themselves
#   codex_schema_violation   — codex emitted JSON that violates the envelope
#                              schema (deterministic given the same input)
#   gemini_no_begin          — gemini didn't emit BEGIN sentinel; the skill is
#                              misconfigured or the prompt is wrong
#   gemini_authentication_*  — auth failures
#   gemini_schema_violation  — same as codex
#   missing_envelope         — reviewer never emitted any envelope at all,
#                              indicating a fundamental skill/CLI mismatch
#
# Retryable categories — could be transient:
#   launch_or_exit_fail      — could be a launch race or transient cloud error
#                              (the underlying cause matters; we retry once
#                              and let codex/gemini sort themselves out)
#   codex_parse_error        — could be mid-stream truncation from a network
#                              hiccup
#   gemini_parse_error       — same
#   gemini_no_end            — could be a cancelled stream
#   gemini_missing_terminator — could be a cancelled stream
#   unknown_failure          — fall back to retry; if it's deterministic the
#                              caller will see the same category three times
#
# Why this is the symmetric form of BUG-08-002: the dirty_worktree fix added
# a single-case `break`. This generalizes that to a classifier so we don't
# burn 3 attempts on every newly-discovered deterministic failure mode.
is_terminal_failure() {
  local category="$1"
  case "$category" in
    dirty_worktree) return 0 ;;
    codex_invalid_*) return 0 ;;
    codex_authentication_*) return 0 ;;
    codex_schema_violation) return 0 ;;
    codex_missing_envelope) return 0 ;;
    gemini_no_begin) return 0 ;;
    gemini_authentication_*) return 0 ;;
    gemini_schema_violation) return 0 ;;
    gemini_missing_envelope) return 0 ;;
    *) return 1 ;;
  esac
}

ATTEMPT=0
FAILURE=""
while [[ $ATTEMPT -lt $MAX_RETRIES ]]; do
  ATTEMPT=$((ATTEMPT + 1))
  echo "[$(date +%s)] attempt $ATTEMPT/$MAX_RETRIES" >> "$RUN/round.log"

  # Snapshot worktree before reviewer run
  "$SCRIPT_DIR/worktree-guard.sh" snapshot "$RUN/worktree-before.txt"

  # Launch both reviewers
  if ! "$SCRIPT_DIR/dual-invoke.sh" "${ARGS[@]}"; then
    FAILURE="launch_or_exit_fail"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif ! "$SCRIPT_DIR/parse-codex.py" --jsonl "$RUN/codex.jsonl" --schema "$SCHEMA" > "$RUN/codex.envelope.json" 2> "$RUN/codex.parse-error"; then
    FAILURE="codex_$(head -1 "$RUN/codex.parse-error")"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif ! "$SCRIPT_DIR/parse-gemini.py" --jsonl "$RUN/gemini.jsonl" --schema "$SCHEMA" > "$RUN/gemini.envelope.json" 2> "$RUN/gemini.parse-error"; then
    FAILURE="gemini_$(head -1 "$RUN/gemini.parse-error")"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif ! "$SCRIPT_DIR/worktree-guard.sh" compare "$RUN/worktree-before.txt" 2> "$RUN/worktree-error"; then
    FAILURE="dirty_worktree"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  else
    # All checks passed
    echo "[$(date +%s)] round succeeded on attempt $ATTEMPT" >> "$RUN/round.log"
    exit 0
  fi

  # Generalized terminal-failure classifier (BUG-08-006). Originally only
  # dirty_worktree was treated as terminal (BUG-08-002 fix); the classifier
  # generalizes that to all deterministic failure categories so we don't
  # waste 3 attempts (plus exponential backoff plus partner reviewer quota)
  # on failures that will recur identically on retry.
  if is_terminal_failure "$FAILURE"; then
    echo "[$(date +%s)] $FAILURE is deterministic (terminal) — not retrying" >> "$RUN/round.log"
    break
  fi

  if [[ $ATTEMPT -lt $MAX_RETRIES ]]; then
    BACKOFF=${BACKOFFS[$((ATTEMPT - 1))]}
    echo "[$(date +%s)] sleeping ${BACKOFF}s before retry" >> "$RUN/round.log"
    sleep "$BACKOFF"
  fi
done

echo "infra_retries_exhausted: ${FAILURE:-unknown_failure}" >&2
echo "postmortem dir: $RUN" >&2
exit 1
