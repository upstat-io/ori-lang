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
#
# Worktree drift check (informational, non-blocking by default):
#   - worktree-guard.sh compare runs AFTER parsers succeed
#   - Drift is logged as a WARNING, not a failure — the user regularly runs
#     parallel agents and their edits should not kill the review round
#   - Only escalated to terminal failure with ORI_TPR_STRICT_WORKTREE=1
#   - Drift details saved to $RUN/worktree-drift.txt for user triage

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
#
# ORIGINAL_REVIEWERS captures the operator's explicit choice (or the default
# `both`) and is immutable across attempts. EFFECTIVE_REVIEWERS is recomputed
# per attempt by the selective-retry logic (2026-04-11 fix) — on attempt 2+
# it may be narrowed to ONLY the reviewers that failed on the prior attempt
# so successful reviewers aren't wastefully re-run. See the selective-retry
# block in the main loop below for the full rationale.
ORIGINAL_REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
if [[ "$ORIGINAL_REVIEWERS" != "codex" && "$ORIGINAL_REVIEWERS" != "gemini" && "$ORIGINAL_REVIEWERS" != "both" ]]; then
  echo "invalid ORI_TPR_REVIEWERS: $ORIGINAL_REVIEWERS (must be codex|gemini|both)" >&2
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
#   dirty_worktree                 — tracked-file drift detected AND
#                                    ORI_TPR_STRICT_WORKTREE=1 was set.
#                                    Without strict mode, drift is a non-
#                                    blocking warning (not a failure at all).
#                                    (Post-2026-04-12: default changed from
#                                    terminal to informational — user runs
#                                    parallel agents whose edits are expected).
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
#   gemini_missing_json_block      — sentinels present but no fenced JSON
#                                    block between them; skill output format
#                                    is broken.
#   gemini_failed_partial          — same as codex.
#
# Retryable categories — could be transient, worth another attempt:
#
#   gemini_schema_violation        — [EFFECTIVELY DEAD CODE as of 2026-04-13]
#                                    parse-gemini.py now RESCUES schema-invalid
#                                    envelopes (exits 0 with RESCUED warnings)
#                                    instead of emitting this category. Kept in
#                                    the classifier as a safety net in case a
#                                    future parser change re-introduces it.
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
#   gemini_missing_begin_sentinel  — no BEGIN sentinel AND sentinel-less
#                                    fallback found no fenced JSON block
#                                    matching the review-envelope shape.
#                                    Previously terminal on the theory that
#                                    this indicated skill misconfiguration,
#                                    but the real-world pattern is gemini
#                                    cutting off mid-emission after saying
#                                    "I'm ready to emit the envelope" and
#                                    never actually writing the fenced block
#                                    (ori-tpr-ODwpfyOd failure class). A
#                                    fresh gemini invocation has a real
#                                    chance of completing the emission, so
#                                    retry is load-bearing. Symmetric with
#                                    missing_end_sentinel (truncation inside
#                                    the envelope) and missing_terminator
#                                    (no result event) — all three failure
#                                    modes share a "gemini output was cut
#                                    short" root cause and all should retry.
#   gemini_missing_end_sentinel    — BEGIN found but END missing; could be
#                                    truncation (transient).
#   unknown_failure                — fall back to retry; if it's deterministic
#                                    the retry will surface the same category
#                                    three times and the operator will see it.
#
# Why this is the symmetric form of the original dirty_worktree fix: the
# original fix added a single-case `break`. This generalizes that to a
# classifier so we don't burn 3 attempts on deterministic failure modes. The
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
    reviewer_stalled_*)            return 0 ;;  # API-level hang — will recur on retry
    codex_missing_dependency)      return 0 ;;
    codex_missing_envelope)        return 0 ;;
    codex_schema_violation)        return 0 ;;
    codex_failed_partial)          return 0 ;;
    gemini_missing_dependency)     return 0 ;;
    gemini_missing_envelope)       return 0 ;;
    gemini_missing_json_block)     return 0 ;;
    gemini_failed_partial)         return 0 ;;
    *) return 1 ;;
  esac
}

# extract_failure_category — read the first NON-advisory line of a parse-error
# file and prefix it with the reviewer name. Skip lines starting with
# "WARNING:" or "REPAIR:" / "  REPAIR:" (advisory output from the parsers).
# This is defense-in-depth: parse-codex.py and parse-gemini.py both buffer
# their advisory lines and flush them AFTER the category (2026-04-11 fix), so
# the first line SHOULD already be the category. This function is a belt-and-
# suspenders check that survives future parser regressions — if either parser
# ever re-introduces a pre-category advisory line, the retry classifier still
# gets the real category instead of being mangled by e.g.
# `gemini_WARNING: sentinel-less fallback ...`.
extract_failure_category() {
  local reviewer="$1"
  local parse_error_file="$2"
  local category
  # awk: print the first line that doesn't start with WARNING:, REPAIR:, or
  # "  REPAIR:", then exit. If no such line exists, print empty string.
  category=$(awk '
    /^WARNING:/ { next }
    /^REPAIR:/  { next }
    /^  REPAIR:/ { next }
    { print; exit }
  ' "$parse_error_file" 2>/dev/null)
  printf '%s_%s' "$reviewer" "${category:-unknown_failure}"
}

ATTEMPT=0
FAILURE=""
while [[ $ATTEMPT -lt $MAX_RETRIES ]]; do
  ATTEMPT=$((ATTEMPT + 1))
  FAILURE=""  # Reset for this attempt — stale value from prior attempt must not leak

  # ── Selective retry: narrow to failing reviewers only ─────────────
  #
  # On attempt 2+, if the operator asked for BOTH reviewers, narrow the retry
  # to ONLY the reviewers that still lack a valid envelope. Preserving a
  # successful reviewer's attempt-N envelope across retries saves substantial
  # wall time on the common failure mode (gemini schema violations while codex
  # succeeds). Codex invocations take ~5-10 minutes each, so re-running codex
  # on a gemini-only failure roughly doubles the total review wall time.
  #
  # Success detection: a reviewer is "done" if its envelope file is non-empty.
  # parse-codex.py and parse-gemini.py both emit the envelope to stdout ONLY
  # on success — on failure they exit before any stdout write, leaving the
  # redirected envelope file empty. The parse-error file may be non-empty even
  # on success because the advisory-flush pattern (2026-04-11 fix) writes
  # REPAIR/WARNING lines to stderr AFTER the envelope is on stdout, so
  # parse-error size is NOT a reliable success discriminant. Only
  # envelope.json size is.
  #
  # Operator filter composition: if the operator explicitly set
  # ORI_TPR_REVIEWERS=codex (or gemini), narrowing is a no-op — the attempt
  # already runs only the chosen reviewer.
  #
  # Both-successful edge case: if both envelopes are valid but the wrapper
  # reached this point anyway, the failure category must have been
  # dirty_worktree in strict mode or a future category that somehow leaves
  # both envelopes untouched. Fall through to running both on the retry —
  # conservative default that matches pre-fix behavior.
  EFFECTIVE_REVIEWERS="$ORIGINAL_REVIEWERS"
  if [[ $ATTEMPT -gt 1 && "$ORIGINAL_REVIEWERS" == "both" ]]; then
    CODEX_OK=0
    GEMINI_OK=0
    if [[ -s "$RUN/codex.envelope.json" ]]; then
      CODEX_OK=1
    fi
    if [[ -s "$RUN/gemini.envelope.json" ]]; then
      GEMINI_OK=1
    fi

    if [[ $CODEX_OK -eq 1 && $GEMINI_OK -eq 0 ]]; then
      EFFECTIVE_REVIEWERS="gemini"
    elif [[ $CODEX_OK -eq 0 && $GEMINI_OK -eq 1 ]]; then
      EFFECTIVE_REVIEWERS="codex"
    else
      # Both failed (common case on transient network) OR both succeeded
      # (edge case — fall through to retry both rather than hit a dead end).
      EFFECTIVE_REVIEWERS="both"
    fi
  fi

  if [[ "$EFFECTIVE_REVIEWERS" == "$ORIGINAL_REVIEWERS" ]]; then
    echo "[$(date +%s)] attempt $ATTEMPT/$MAX_RETRIES (reviewers=$EFFECTIVE_REVIEWERS)" >> "$RUN/round.log"
  else
    echo "[$(date +%s)] attempt $ATTEMPT/$MAX_RETRIES (selective retry: narrowed from $ORIGINAL_REVIEWERS to $EFFECTIVE_REVIEWERS — preserving prior-attempt success)" >> "$RUN/round.log"
  fi

  # Snapshot worktree before reviewer run
  "$SCRIPT_DIR/worktree-guard.sh" snapshot "$RUN/worktree-before.txt"

  # Launch reviewers per EFFECTIVE_REVIEWERS. dual-invoke.sh reads
  # ORI_TPR_REVIEWERS from the environment — exporting it ONLY for this
  # sub-command call scopes the narrowing to the retry iteration without
  # polluting our own shell state.
  if ! ORI_TPR_REVIEWERS="$EFFECTIVE_REVIEWERS" "$SCRIPT_DIR/dual-invoke.sh" "${ARGS[@]}"; then
    # Check if failure was due to watchdog killing a stalled reviewer
    if [[ -f "$RUN/codex.stalled" || -f "$RUN/gemini.stalled" ]]; then
      stalled_reviewers=""
      [[ -f "$RUN/codex.stalled" ]] && stalled_reviewers="codex"
      [[ -f "$RUN/gemini.stalled" ]] && stalled_reviewers="${stalled_reviewers:+$stalled_reviewers+}gemini"
      FAILURE="reviewer_stalled_${stalled_reviewers}"
    else
      FAILURE="launch_or_exit_fail"
    fi
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif [[ ( "$EFFECTIVE_REVIEWERS" == "codex" || "$EFFECTIVE_REVIEWERS" == "both" ) ]] && ! "$SCRIPT_DIR/parse-codex.py" --jsonl "$RUN/codex.jsonl" --schema "$SCHEMA" > "$RUN/codex.envelope.json" 2> "$RUN/codex.parse-error"; then
    FAILURE="$(extract_failure_category codex "$RUN/codex.parse-error")"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  elif [[ ( "$EFFECTIVE_REVIEWERS" == "gemini" || "$EFFECTIVE_REVIEWERS" == "both" ) ]] && ! "$SCRIPT_DIR/parse-gemini.py" --jsonl "$RUN/gemini.jsonl" --schema "$SCHEMA" > "$RUN/gemini.envelope.json" 2> "$RUN/gemini.parse-error"; then
    FAILURE="$(extract_failure_category gemini "$RUN/gemini.parse-error")"
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
  else
    # ── Launch + parse succeeded — run worktree guard as INFORMATIONAL check ──
    #
    # Post-2026-04-12 fix: worktree drift is NO LONGER a failure condition by
    # default. The user regularly runs parallel agents whose edits produce drift
    # that is NOT a reviewer violation. Drift is logged as a warning and saved
    # to $RUN/worktree-drift.txt for the skill layer to surface if desired.
    #
    # ORI_TPR_STRICT_WORKTREE=1 restores the old terminal behavior for contexts
    # where the user is NOT running parallel work and wants strict enforcement.
    if ! "$SCRIPT_DIR/worktree-guard.sh" compare "$RUN/worktree-before.txt" "$RUN/worktree-after.txt" 2> "$RUN/worktree-drift.txt"; then
      if [[ "${ORI_TPR_STRICT_WORKTREE:-0}" == "1" ]]; then
        # Strict mode: treat drift as terminal failure (old behavior)
        FAILURE="dirty_worktree"
        echo "[$(date +%s)] dirty_worktree on attempt $ATTEMPT (ORI_TPR_STRICT_WORKTREE=1 — escalated to terminal)" >> "$RUN/round.log"
      else
        # Default: warn and continue — drift is expected from parallel work
        echo "[$(date +%s)] WARNING: worktree drift detected during run (non-blocking — assumed parallel agent or user edits)" >> "$RUN/round.log"
        if [[ -s "$RUN/worktree-drift.txt" ]]; then
          echo "--- drift details ---" >> "$RUN/round.log"
          cat "$RUN/worktree-drift.txt" >> "$RUN/round.log"
          echo "--- end drift details ---" >> "$RUN/round.log"
        fi
      fi
    fi

    # If no failure was set (either no drift, or drift was non-blocking), succeed
    if [[ -z "$FAILURE" ]]; then
      echo "[$(date +%s)] round succeeded on attempt $ATTEMPT" >> "$RUN/round.log"
      exit 0
    fi
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
