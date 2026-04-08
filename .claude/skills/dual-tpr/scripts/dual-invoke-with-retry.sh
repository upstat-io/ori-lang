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

# ORI_TPR_REVIEWERS runtime toggle (§07.2, moved from §08.2). When a
# reviewer is excluded, dual-invoke.sh skips launching it — so the
# retry wrapper must ALSO skip parsing its (absent) JSONL output.
# Default is `both` so every existing caller (§04/§05 wrappers)
# runs unchanged.
REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
if [[ "$REVIEWERS" != "codex" && "$REVIEWERS" != "gemini" && "$REVIEWERS" != "both" ]]; then
  echo "invalid ORI_TPR_REVIEWERS: $REVIEWERS (must be codex|gemini|both)" >&2
  exit 2
fi

# is_terminal_failure: classify a failure category as either terminal (no
# retry) or retryable (worth another attempt). Returns 0 (true) for terminal,
# 1 (false) for retryable.
#
# The failure category is formed by `dual-invoke-with-retry.sh` as either
# `launch_or_exit_fail` (dual-invoke.sh exited non-zero, meaning one or both
# reviewer subprocesses failed to run cleanly), or `<reviewer>_<first-stderr-
# line>` where <first-stderr-line> is the authoritative failure category
# emitted by `parse-codex.py` or `parse-gemini.py`. This classifier must stay
# in sync with the parsers' emitted category list — the canonical source is:
#
#   parse-codex.py:  missing_dependency | missing_envelope | parse_fail |
#                    schema_violation | failed_partial
#   parse-gemini.py: missing_dependency | missing_envelope | missing_terminator |
#                    missing_begin_sentinel | missing_end_sentinel |
#                    missing_json_block | parse_fail | schema_violation |
#                    failed_partial
#
# Terminal categories — deterministic, retry will produce the same result:
#
#   dirty_worktree                 — reviewer modified tracked files, will do
#                                    so again on retry (BUG-08-002 fix).
#   codex_missing_dependency       — jsonschema Python module not installed
#                                    on this host; won't install itself on
#                                    retry (TPR-04-002-codex fix).
#   codex_missing_envelope         — codex exited 0 but emitted no agent
#                                    message; the skill or prompt is broken.
#   codex_schema_violation         — codex emitted JSON that violates the
#                                    envelope schema; model/prompt issue.
#   codex_failed_partial           — envelope valid but status=failed_partial;
#                                    reviewer explicitly said it did not
#                                    finish — retry won't change that.
#   gemini_missing_dependency      — same as codex (env issue).
#   gemini_missing_envelope        — gemini exited 0 but emitted no assistant
#                                    message; the skill is broken.
#   gemini_missing_begin_sentinel  — assistant messages present but no BEGIN
#                                    sentinel; skill misconfigured or prompt
#                                    didn't include the activation phrase.
#   gemini_missing_json_block      — sentinels present but no fenced JSON
#                                    block between them; skill output format
#                                    is broken.
#   gemini_schema_violation        — same as codex.
#   gemini_failed_partial          — same as codex.
#
# Retryable categories — could be transient, worth another attempt:
#
#   launch_or_exit_fail            — dual-invoke.sh returned non-zero. This
#                                    collapses ALL launch-time failures into
#                                    one category because dual-invoke.sh
#                                    discards reviewer stderr (`2>/dev/null`).
#                                    We cannot distinguish a deterministic
#                                    OpenAI 400 schema rejection from a
#                                    transient network failure at this layer,
#                                    so we retry. The 3-attempt budget bounds
#                                    the waste for deterministic cases.
#   codex_parse_fail               — codex emitted text that isn't JSON.
#                                    Could be mid-stream truncation (transient)
#                                    or a model output bug (deterministic).
#                                    Retry to distinguish.
#   gemini_parse_fail              — same as codex_parse_fail.
#   gemini_missing_terminator      — assistant content present but no
#                                    result/success event; could be a
#                                    cancelled stream (transient).
#   gemini_missing_end_sentinel    — BEGIN found but END missing; could be
#                                    truncation (transient).
#   unknown_failure                — fall back to retry; if it's deterministic
#                                    the retry will surface the same category
#                                    three times and the operator will see it.
#
# Why this is the symmetric form of BUG-08-002: the dirty_worktree fix added
# a single-case `break`. This generalizes that to a classifier so we don't
# burn 3 attempts on every newly-discovered deterministic failure mode. The
# categories listed above are the RESULT of auditing what parse-codex.py and
# parse-gemini.py actually emit — earlier iterations of this function
# invented category names (`codex_invalid_*`, `codex_authentication_*`,
# `gemini_authentication_*`, `gemini_no_begin`) that were never produced by
# any code path and therefore had no effect. That was a SSOT violation per
# impl-hygiene.md (classifier encoded drifted knowledge about parser output
# instead of matching the actual parser emissions). TPR-04-001-codex.
is_terminal_failure() {
  local category="$1"
  case "$category" in
    dirty_worktree)                return 0 ;;
    codex_missing_dependency)      return 0 ;;
    codex_missing_envelope)        return 0 ;;
    codex_schema_violation)        return 0 ;;
    codex_failed_partial)          return 0 ;;
    gemini_missing_dependency)     return 0 ;;
    gemini_missing_envelope)       return 0 ;;
    gemini_missing_begin_sentinel) return 0 ;;
    gemini_missing_json_block)     return 0 ;;
    gemini_schema_violation)       return 0 ;;
    gemini_failed_partial)         return 0 ;;
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

  # Launch reviewers per ORI_TPR_REVIEWERS filter (default: both). Parser
  # calls are gated by the same filter — skipping an absent JSONL file
  # would otherwise produce a spurious "missing_envelope" failure.
  if ! "$SCRIPT_DIR/dual-invoke.sh" "${ARGS[@]}"; then
    FAILURE="launch_or_exit_fail"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif [[ ( "$REVIEWERS" == "codex" || "$REVIEWERS" == "both" ) ]] && ! "$SCRIPT_DIR/parse-codex.py" --jsonl "$RUN/codex.jsonl" --schema "$SCHEMA" > "$RUN/codex.envelope.json" 2> "$RUN/codex.parse-error"; then
    FAILURE="codex_$(head -1 "$RUN/codex.parse-error")"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif [[ ( "$REVIEWERS" == "gemini" || "$REVIEWERS" == "both" ) ]] && ! "$SCRIPT_DIR/parse-gemini.py" --jsonl "$RUN/gemini.jsonl" --schema "$SCHEMA" > "$RUN/gemini.envelope.json" 2> "$RUN/gemini.parse-error"; then
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
