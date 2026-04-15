#!/usr/bin/env bash
# dual-invoke-with-retry.sh — wraps dual-invoke.sh with infra retry logic.
#
# Usage: same args as dual-invoke.sh
#
# Retry policy:
#   - 5 attempts per reviewer per round (was 3; raised for capacity errors)
#   - Default backoff: 1s, 2s, 4s, 30s, 60s between attempts
#   - Capacity-aware backoff: 30s, 60s, 120s, 120s, 120s for API capacity errors
#   - Retries are SEPARATE from the wrapper's semantic iteration budget
#   - On failure: returns the failure category as the last line of stderr,
#     leaves $RUN intact for postmortem, exits 1
#
# Success criteria (all must hold):
#   - Every active reviewer (per ORI_TPR_REVIEWERS) produced a valid envelope
#     AS PARSED by parse-codex.py / parse-gemini.py. dual-invoke.sh's exit
#     code is NO LONGER the authoritative success signal — a non-zero exit
#     from dual-invoke only means at least one reviewer had a non-zero
#     process exit; the PARTNER may still have produced a valid envelope.
#     (Fix: per-reviewer independent parsing — 2026-04-15.)
#
# Per-reviewer independence (2026-04-15):
#   Prior to this change, the wrapper used a sequential if/elif/elif/else
#   cascade that short-circuited parsing as soon as any step failed:
#     1. dual-invoke.sh exits non-zero → SKIP all parsers
#     2. parse-codex.py fails         → SKIP parse-gemini.py
#   Either cascade left the partner reviewer's envelope file missing even
#   when its JSONL held a perfect envelope. On attempt 2, the selective-
#   retry logic (see below) uses `[[ -s <reviewer>.envelope.json ]]` as its
#   preservation signal — a missing envelope forces EFFECTIVE_REVIEWERS to
#   fall back to "both", and dual-invoke.sh's launch-time `rm -f` then wipes
#   the successful reviewer's jsonl/exit/envelope state. Net effect: the
#   operator saw a clean reviewer being needlessly re-invoked, paying the
#   full codex wall-time (~5-10 min) twice even though attempt 1's output
#   was intact.
#   The fix is to parse both reviewers INDEPENDENTLY after launch, materializing
#   each reviewer's envelope on its own merits. Only after both parses
#   complete do we classify the round's outcome. The selective-retry logic
#   is UNCHANGED — it was already correct, just starved of inputs.
#
# Worktree drift check (informational, non-blocking by default):
#   - worktree-guard.sh compare runs AFTER parsers succeed
#   - Drift is logged as a WARNING, not a failure — the user regularly runs
#     parallel agents and their edits should not kill the review round
#   - Only escalated to terminal failure with ORI_TPR_STRICT_WORKTREE=1
#   - Drift details saved to $RUN/worktree-drift.txt for user triage

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MAX_RETRIES=5
BACKOFFS=(1 2 4 30 60)
# Capacity-specific backoff: much longer delays for API capacity errors.
# The short 1/2/4s default backoffs just hammer a capacity-limited API.
CAPACITY_BACKOFFS=(30 60 120 120 120)

# Pass through all args to dual-invoke.sh; we also need RUN and SCHEMA to know
# where outputs go and which schema to validate against.
RUN=""
SCHEMA=""
# SKILL is extracted so we can thread the transport's active review mode
# into parse-codex.py / parse-gemini.py as --default-skill. Without this,
# the parsers silently rewrite missing/invalid envelope `skill` fields to
# "review-work" regardless of what the transport is actually running,
# corrupting downstream provenance for `review-plan`, `tp-help`, or
# `custom` rounds (TPR-XX-001).
SKILL=""
ARGS=("$@")
for ((i=0; i<${#ARGS[@]}; i++)); do
  if [[ "${ARGS[$i]}" == "--run" ]]; then
    RUN="${ARGS[$((i+1))]}"
  elif [[ "${ARGS[$i]}" == "--schema" ]]; then
    SCHEMA="${ARGS[$((i+1))]}"
  elif [[ "${ARGS[$i]}" == "--skill" ]]; then
    SKILL="${ARGS[$((i+1))]}"
  fi
done

[[ -z "$RUN" ]] && { echo "missing --run arg" >&2; exit 2; }
[[ -z "$SCHEMA" ]] && { echo "missing --schema arg" >&2; exit 2; }
# SKILL falls back to review-work only when the caller omits --skill
# (legacy / bare invocations). All first-party callers pass --skill.
[[ -z "$SKILL" ]] && SKILL="review-work"

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

# ── Global circuit breaker (2026-04-15) ──────────────────────────────
#
# Before we commit to the operator's reviewer selection, consult the
# per-reviewer circuit breaker (state lives under $HOME/.cache/ori-tpr-circuit/
# — global per user, persists across Claude sessions and worktrees). If a
# reviewer has tripped its API/transport failure threshold within the
# sliding window, it is parked for 1 hour — skip it and use only the
# surviving reviewer. If BOTH are tripped, fail loud with a clear message
# so the operator can choose to wait or `reset all`.
#
# ORI_TPR_CIRCUIT_OFF=1 bypasses the breaker entirely (escape hatch for
# diagnostics). Otherwise the narrowing is silent and invisible to callers
# that already tolerate single-reviewer mode via ORI_TPR_REVIEWERS.
if [[ "${ORI_TPR_CIRCUIT_OFF:-0}" != "1" ]]; then
  CB="$SCRIPT_DIR/circuit-breaker.sh"
  CODEX_TRIPPED=0
  GEMINI_TRIPPED=0
  CODEX_CHECK="$("$CB" check codex 2>/dev/null)" || CODEX_TRIPPED=1
  GEMINI_CHECK="$("$CB" check gemini 2>/dev/null)" || GEMINI_TRIPPED=1

  case "$ORIGINAL_REVIEWERS" in
    both)
      if [[ $CODEX_TRIPPED -eq 1 && $GEMINI_TRIPPED -eq 1 ]]; then
        echo "circuit_breaker_both_tripped: $CODEX_CHECK / $GEMINI_CHECK" >&2
        echo "Both reviewers are in timeout. Wait for expiry or run:" >&2
        echo "  $CB reset all" >&2
        exit 1
      elif [[ $CODEX_TRIPPED -eq 1 ]]; then
        echo "[$(date +%s)] circuit-breaker: codex tripped ($CODEX_CHECK), narrowing to gemini-only" >> "$RUN/round.log"
        ORIGINAL_REVIEWERS="gemini"
      elif [[ $GEMINI_TRIPPED -eq 1 ]]; then
        echo "[$(date +%s)] circuit-breaker: gemini tripped ($GEMINI_CHECK), narrowing to codex-only" >> "$RUN/round.log"
        ORIGINAL_REVIEWERS="codex"
      fi
      ;;
    codex)
      if [[ $CODEX_TRIPPED -eq 1 ]]; then
        echo "circuit_breaker_tripped: codex $CODEX_CHECK (operator requested codex-only; no fallback possible)" >&2
        echo "Wait for expiry or run: $CB reset codex" >&2
        exit 1
      fi
      ;;
    gemini)
      if [[ $GEMINI_TRIPPED -eq 1 ]]; then
        echo "circuit_breaker_tripped: gemini $GEMINI_CHECK (operator requested gemini-only; no fallback possible)" >&2
        echo "Wait for expiry or run: $CB reset gemini" >&2
        exit 1
      fi
      ;;
  esac
fi

# is_terminal_failure: classify a failure category as either terminal (no
# retry) or retryable (worth another attempt). Returns 0 (true) for terminal,
# 1 (false) for retryable.
#
# The failure category is formed by `dual-invoke-with-retry.sh` as either
# `launch_or_exit_fail` (legacy — kept for backward compat with historical
# round.log entries; the new per-reviewer parse always refines to a specific
# category), or `<reviewer>_<first-stderr-line>` where <first-stderr-line> is
# the authoritative failure category emitted by `parse-codex.py` or
# `parse-gemini.py`. This classifier must stay in sync with the parsers'
# emitted category list — the canonical source is:
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
#   launch_or_exit_fail            — legacy category. Pre-2026-04-15 this was
#                                    emitted whenever dual-invoke.sh returned
#                                    non-zero. After the per-reviewer
#                                    independence fix, the wrapper always
#                                    refines to a specific <reviewer>_<cat>
#                                    using the reviewer's JSONL + parser
#                                    output, so launch_or_exit_fail should no
#                                    longer be produced. Kept in the classifier
#                                    as a safety net in case the per-reviewer
#                                    classifier ever fails to produce a
#                                    category.
#   codex_parse_fail               — codex emitted text that isn't JSON.
#                                    Could be mid-stream truncation (transient)
#                                    or a model output bug (deterministic).
#                                    Retry to distinguish.
#   codex_missing_jsonl            — codex.jsonl is empty or absent. Typically
#                                    means the codex subprocess crashed before
#                                    emitting anything. Usually transient.
#   gemini_missing_jsonl           — same as codex_missing_jsonl.
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
    *_api_auth)                    return 0 ;;  # authentication errors won't self-heal
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

# extract_jsonl_api_error — extract the API error message from a reviewer's
# JSONL stream. Gemini (and codex) emit a {"type":"result","status":"error",
# "error":{"message":"..."}} event when the API rejects the request. This
# function extracts that message so the retry classifier can distinguish
# capacity errors from other failures — the only information previously
# available was the generic "launch_or_exit_fail" category because stderr
# was discarded (now captured in $RUN/<reviewer>.stderr).
#
# Returns: the error message on stdout, or empty string if no error event found.
extract_jsonl_api_error() {
  local jsonl_file="$1"
  [[ -f "$jsonl_file" ]] || return 0
  python3 -c "
import json, sys
with open('$jsonl_file') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            obj = json.loads(line)
            if obj.get('type') == 'result' and obj.get('status') == 'error':
                err = obj.get('error', {})
                print(err.get('message', ''), end='')
                sys.exit(0)
        except: pass
" 2>/dev/null || true
}

# classify_api_error — map an API error message to a specific failure
# category. Shared between codex and gemini so both reviewers route through
# the same capacity/auth/error taxonomy. Returns the category via stdout.
classify_api_error() {
  local reviewer="$1"
  local api_err="$2"
  if [[ "$api_err" == *"No capacity"* || "$api_err" == *"rate limit"* || "$api_err" == *"overloaded"* || "$api_err" == *"quota"* ]]; then
    printf '%s_api_capacity' "$reviewer"
  elif [[ "$api_err" == *"authentication"* || "$api_err" == *"unauthorized"* || "$api_err" == *"permission"* ]]; then
    printf '%s_api_auth' "$reviewer"
  else
    printf '%s_api_error' "$reviewer"
  fi
}

# is_capacity_error — returns 0 (true) if the failure looks like an API
# capacity/rate-limit error. These need much longer backoff (30-120s) instead
# of the default 1-4s.
IS_CAPACITY_FAILURE=false
is_capacity_error() {
  local category="$1"
  [[ "$category" == *"_api_capacity"* ]] && return 0
  return 1
}

# classify_reviewer_outcome — inspect a single reviewer's post-launch state
# and attempt to parse its JSONL into an envelope. Runs INDEPENDENTLY of the
# other reviewer so a failure on one side never prevents envelope
# materialization on the other. This is the core of the 2026-04-15
# per-reviewer independence fix.
#
# Arguments:
#   $1 — reviewer name (codex|gemini)
#   $2 — path to reviewer's jsonl file
#   $3 — path to reviewer's parse-error file
#   $4 — path to reviewer's envelope.json output file
#   $5 — path to reviewer's stalled marker file (if present, reviewer was
#        killed by watchdog and must NOT be retried)
#
# On success: sets parser-produced envelope.json, returns 0.
# On failure: truncates envelope.json to zero bytes (so selective-retry's
# `-s` preservation check correctly returns false), prints the failure
# category to stdout, returns 1.
#
# Why truncate on failure: parse-*.py writes to stdout ONLY on success (they
# exit before any stdout write on failure). But if a PRIOR attempt wrote a
# valid envelope and THIS attempt's jsonl is stale/bad, the stale envelope
# from the prior attempt could still be on disk. dual-invoke.sh's launch-time
# `rm -f` handles this for the re-launch case, but when parse-*.py fails on
# pre-existing content we must clear it ourselves so selective-retry doesn't
# mistake the stale file for a fresh success.
classify_reviewer_outcome() {
  local reviewer="$1"
  local jsonl="$2"
  local parse_err="$3"
  local envelope="$4"
  local stalled_marker="$5"

  # Case 1: watchdog killed this reviewer → terminal stall.
  if [[ -f "$stalled_marker" ]]; then
    : > "$envelope"
    printf 'reviewer_stalled_%s' "$reviewer"
    return 1
  fi

  # Case 2: jsonl missing or empty → reviewer subprocess crashed before
  # emitting anything. Cannot parse. Retryable (usually transient).
  if [[ ! -s "$jsonl" ]]; then
    : > "$envelope"
    printf '%s_missing_jsonl' "$reviewer"
    return 1
  fi

  # Case 3: attempt the parse. On success, envelope.json is written and we
  # return 0. On failure, classify via API error (preferred, richer) or
  # parser-reported category (fallback).
  if "$SCRIPT_DIR/parse-${reviewer}.py" --jsonl "$jsonl" --schema "$SCHEMA" \
      --default-skill "$SKILL" > "$envelope" 2> "$parse_err"; then
    return 0
  fi

  # Parse failed. Clear the envelope so -s preservation check returns false.
  : > "$envelope"

  # Try to refine using the JSONL-level API error event — this is the most
  # specific classification when the reviewer's API call itself was
  # rejected. If present, it overrides the parser-reported category because
  # API errors (capacity/auth) dictate retry strategy (backoff, terminal).
  local api_err
  api_err=$(extract_jsonl_api_error "$jsonl")
  if [[ -n "$api_err" ]]; then
    local cat
    cat=$(classify_api_error "$reviewer" "$api_err")
    if [[ "$cat" == *"_api_capacity" ]]; then
      IS_CAPACITY_FAILURE=true
    fi
    printf '%s' "$cat"
    return 1
  fi

  # Fallback: use the parser's own first-line category.
  extract_failure_category "$reviewer" "$parse_err"
  return 1
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
  # CRITICAL dependency on per-reviewer independence (2026-04-15): this
  # detection ONLY works because the main loop now parses each reviewer
  # independently, so a successful reviewer's envelope is ALWAYS materialized
  # on disk even if its partner failed. Before the per-reviewer fix, a
  # launch-level failure of one reviewer prevented the partner's envelope
  # from ever being written — this check would then return 0 for the partner
  # even though its JSONL held valid output, forcing EFFECTIVE_REVIEWERS to
  # "both" and wiping the good JSONL at dual-invoke.sh's launch-time `rm -f`.
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

  # ── Step 1: launch reviewers ───────────────────────────────────────
  #
  # dual-invoke.sh reads ORI_TPR_REVIEWERS from the environment — exporting it
  # ONLY for this sub-command call scopes the narrowing to the retry iteration
  # without polluting our own shell state. We DO NOT gate subsequent steps on
  # dual-invoke.sh's exit code (2026-04-15 per-reviewer independence fix): a
  # non-zero exit means AT LEAST ONE reviewer had a non-zero process exit, but
  # the PARTNER may still have produced a valid envelope. The per-reviewer
  # parse step below handles both success and failure cases uniformly.
  DUAL_INVOKE_RC=0
  ORI_TPR_REVIEWERS="$EFFECTIVE_REVIEWERS" "$SCRIPT_DIR/dual-invoke.sh" "${ARGS[@]}" || DUAL_INVOKE_RC=$?
  if [[ $DUAL_INVOKE_RC -ne 0 ]]; then
    echo "[$(date +%s)] dual-invoke.sh returned rc=$DUAL_INVOKE_RC (partial-success path — parsers will classify per reviewer)" >> "$RUN/round.log"
  fi

  # ── Step 2: parse each active reviewer INDEPENDENTLY ───────────────
  #
  # Per-reviewer independence (2026-04-15 fix): parse both reviewers' output
  # regardless of dual-invoke.sh's exit code and regardless of whether the
  # sibling reviewer succeeded or failed. Each reviewer's parse produces its
  # own envelope on success or its own failure category on failure. Only
  # AFTER both parses complete do we classify the round's overall outcome.
  #
  # This is load-bearing for selective-retry correctness: if codex succeeded
  # but gemini's API call was rejected at launch time, codex.jsonl holds a
  # valid envelope but dual-invoke.sh exits non-zero. Prior to this fix the
  # wrapper skipped ALL parsers in that case, leaving codex.envelope.json
  # empty and forcing attempt 2 to retry both reviewers (wasting ~10 min of
  # codex compute). Now codex.envelope.json is written on attempt 1 exactly
  # when codex's output was valid, regardless of gemini's fate.
  CODEX_OK=1
  GEMINI_OK=1
  CODEX_FAIL_CAT=""
  GEMINI_FAIL_CAT=""

  if [[ "$EFFECTIVE_REVIEWERS" == "codex" || "$EFFECTIVE_REVIEWERS" == "both" ]]; then
    CODEX_OK=0
    if CODEX_FAIL_CAT=$(classify_reviewer_outcome codex \
        "$RUN/codex.jsonl" "$RUN/codex.parse-error" \
        "$RUN/codex.envelope.json" "$RUN/codex.stalled"); then
      CODEX_OK=1
      CODEX_FAIL_CAT=""
    fi
  fi

  if [[ "$EFFECTIVE_REVIEWERS" == "gemini" || "$EFFECTIVE_REVIEWERS" == "both" ]]; then
    GEMINI_OK=0
    if GEMINI_FAIL_CAT=$(classify_reviewer_outcome gemini \
        "$RUN/gemini.jsonl" "$RUN/gemini.parse-error" \
        "$RUN/gemini.envelope.json" "$RUN/gemini.stalled"); then
      GEMINI_OK=1
      GEMINI_FAIL_CAT=""
    fi
  fi

  # ── Step 3: classify round outcome ─────────────────────────────────
  #
  # ROUND_OK iff every ACTIVE reviewer (per EFFECTIVE_REVIEWERS) succeeded.
  # For narrowed retries (codex-only or gemini-only), the other reviewer's
  # envelope was preserved from a prior attempt and is trusted on disk.
  ROUND_OK=0
  case "$EFFECTIVE_REVIEWERS" in
    both)   [[ $CODEX_OK -eq 1 && $GEMINI_OK -eq 1 ]] && ROUND_OK=1 ;;
    codex)  [[ $CODEX_OK -eq 1 ]]                       && ROUND_OK=1 ;;
    gemini) [[ $GEMINI_OK -eq 1 ]]                      && ROUND_OK=1 ;;
  esac

  if [[ $ROUND_OK -eq 1 ]]; then
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
      echo "[$(date +%s)] round succeeded on attempt $ATTEMPT (codex_ok=$CODEX_OK gemini_ok=$GEMINI_OK)" >> "$RUN/round.log"
      # Reset circuit-breaker fail counters for every reviewer that delivered
      # a valid envelope this round. Success within the window clears the
      # sliding-window count (but does NOT clear an active timeout — that
      # still runs its full duration).
      if [[ "${ORI_TPR_CIRCUIT_OFF:-0}" != "1" ]]; then
        [[ $CODEX_OK -eq 1 ]]  && "$SCRIPT_DIR/circuit-breaker.sh" success codex  2>/dev/null || true
        [[ $GEMINI_OK -eq 1 ]] && "$SCRIPT_DIR/circuit-breaker.sh" success gemini 2>/dev/null || true
      fi
      exit 0
    fi
  else
    # ── Round failed — pick a representative FAILURE category ─────────
    #
    # Prefer codex's category when both failed (consistent with pre-fix
    # ordering: the old elif-chain checked codex before gemini). This only
    # affects the round.log label and the terminal-failure classifier's
    # retry decision — the per-reviewer envelopes/categories are preserved
    # on disk for selective retry to pick up on the next attempt.
    if [[ $CODEX_OK -eq 0 && -n "$CODEX_FAIL_CAT" ]]; then
      FAILURE="$CODEX_FAIL_CAT"
    elif [[ $GEMINI_OK -eq 0 && -n "$GEMINI_FAIL_CAT" ]]; then
      FAILURE="$GEMINI_FAIL_CAT"
    else
      # Defensive fallback: neither reviewer reported a category but ROUND_OK
      # is 0. Shouldn't happen after the per-reviewer refactor, but keep the
      # legacy launch_or_exit_fail label so round.log stays parseable.
      FAILURE="launch_or_exit_fail"
    fi
    echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT (codex_ok=$CODEX_OK gemini_ok=$GEMINI_OK codex_cat=${CODEX_FAIL_CAT:-n/a} gemini_cat=${GEMINI_FAIL_CAT:-n/a})" >> "$RUN/round.log"
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
    if is_capacity_error "$FAILURE"; then
      BACKOFF=${CAPACITY_BACKOFFS[$((ATTEMPT - 1))]}
      echo "[$(date +%s)] capacity error — sleeping ${BACKOFF}s before retry (capacity-aware backoff)" >> "$RUN/round.log"
    else
      BACKOFF=${BACKOFFS[$((ATTEMPT - 1))]}
      echo "[$(date +%s)] sleeping ${BACKOFF}s before retry" >> "$RUN/round.log"
    fi
    sleep "$BACKOFF"
  fi
done

echo "infra_retries_exhausted: ${FAILURE:-unknown_failure}" >&2
echo "postmortem dir: $RUN" >&2

# Record failures against the circuit breaker. circuit-breaker.sh filters
# out non-api/transport categories internally, so we can pass the last
# iteration's categories unconditionally. This is what eventually trips
# the per-reviewer 1-hour timeout after 3 such failures in a sliding
# 1-hour window.
if [[ "${ORI_TPR_CIRCUIT_OFF:-0}" != "1" ]]; then
  if [[ -n "${CODEX_FAIL_CAT:-}" ]]; then
    "$SCRIPT_DIR/circuit-breaker.sh" fail codex "$CODEX_FAIL_CAT" 2>>"$RUN/round.log" || true
  fi
  if [[ -n "${GEMINI_FAIL_CAT:-}" ]]; then
    "$SCRIPT_DIR/circuit-breaker.sh" fail gemini "$GEMINI_FAIL_CAT" 2>>"$RUN/round.log" || true
  fi
  # launch_or_exit_fail has no reviewer prefix — attribute to whichever
  # reviewer(s) were active this round so the breaker can still react.
  if [[ "${FAILURE:-}" == "launch_or_exit_fail" ]]; then
    case "$EFFECTIVE_REVIEWERS" in
      both|codex)  "$SCRIPT_DIR/circuit-breaker.sh" fail codex  launch_or_exit_fail 2>>"$RUN/round.log" || true ;;
    esac
    case "$EFFECTIVE_REVIEWERS" in
      both|gemini) "$SCRIPT_DIR/circuit-breaker.sh" fail gemini launch_or_exit_fail 2>>"$RUN/round.log" || true ;;
    esac
  fi
fi

exit 1
