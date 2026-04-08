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
    # BUG-08-002: dirty_worktree is a deterministic reviewer-misbehavior
    # signal — a misbehaving reviewer will misbehave again on retry, and
    # the next attempt's fresh snapshot would re-baseline against the
    # already-dirty state, laundering the failure into a false success.
    # Treat it as a terminal failure: record, break out of the retry
    # loop, surface to the user. Other failure categories (launch_or_exit_fail,
    # codex_*, gemini_*) remain retry-eligible because they CAN be transient.
    echo "[$(date +%s)] dirty_worktree is deterministic — not retrying" >> "$RUN/round.log"
    break
  else
    # All checks passed
    echo "[$(date +%s)] round succeeded on attempt $ATTEMPT" >> "$RUN/round.log"
    exit 0
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
