#!/usr/bin/env bash
# lib-retry.sh — shared retry/classification helpers for dual-source reviewers.
#
# SCOPE
#   Extracted verbatim from dual-invoke-with-retry.sh (the round-level retry
#   wrapper) so the same classification + backoff logic can be reused by
#   per-reviewer supervisors without copy/paste. All consumers `source` this
#   file; nothing is exec'd.
#
# CONSUMERS
#   - supervisor.sh (per-reviewer retry loop)
#   - dual-invoke-with-retry.sh (legacy round-level retry — kept as shim)
#
# HISTORY
#   Every function here was a per-reviewer pure function before extraction —
#   the move is pure reorganization, no logic changes. See the original
#   comments in dual-invoke-with-retry.sh (git blame) for each function's
#   rationale.

# Guard against double-source. `set -u`-safe variable check.
if [[ -n "${_LIB_RETRY_SOURCED:-}" ]]; then
  return 0 2>/dev/null || exit 0
fi
_LIB_RETRY_SOURCED=1

# ── Backoff schedules ─────────────────────────────────────────────────────
# Default backoff between retry attempts (seconds). Indexed by attempt
# number - 1 (attempt 1 → BACKOFFS[0] = 1s, attempt 5 → BACKOFFS[4] = 60s).
# Aggressive on the first attempts because transient network hiccups
# typically resolve in 1-4s; extended on attempts 4-5 for slower provider
# recoveries.
BACKOFFS=(1 2 4 30 60)

# Capacity-specific backoff: much longer delays for API capacity errors.
# The short 1/2/4s default backoffs just hammer a capacity-limited API.
CAPACITY_BACKOFFS=(30 60 120 120 120)

# Max retry attempts per reviewer. Kept here so supervisors and the legacy
# wrapper share the same cap — operators tune via MAX_RETRIES env override
# where supported, or via supervisor.sh's --max-attempts flag.
MAX_RETRIES="${MAX_RETRIES:-5}"

# ── Classification helpers ────────────────────────────────────────────────

# is_terminal_failure — returns 0 (true) if the failure category is
# deterministic and retrying cannot help. Returns 1 (false) for retryable.
#
# The list of terminal categories comes from auditing what parse-codex.py
# and parse-gemini.py actually emit — earlier iterations of this function
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
    # Raw-mode categories (parse-*-raw.py). missing_agent_message /
    # missing_assistant_content mean the reviewer produced no prose at all —
    # retrying cannot recover a response that was never emitted. Matches
    # the envelope-mode analogues (missing_envelope). missing_terminator is
    # a stream-truncation signal; treat as retryable (classified elsewhere).
    *_missing_agent_message)       return 0 ;;
    *_missing_assistant_content)   return 0 ;;
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
#
# NOTE: IS_CAPACITY_FAILURE is a shared flag set by classify_reviewer_outcome
# when it detects a capacity error via JSONL inspection. Callers check this
# via is_capacity_error on the returned category string; both paths agree.
IS_CAPACITY_FAILURE=false
is_capacity_error() {
  local category="$1"
  [[ "$category" == *"_api_capacity"* ]] && return 0
  return 1
}

# pick_backoff — choose a backoff duration in seconds given the attempt
# number (1-indexed) and the prior attempt's failure category. Returns the
# seconds via stdout. Bounds-checks the index (clamps to last element of
# the array if attempt > array length).
#
# This function is the SSOT for "which schedule applies when" — callers
# should NEVER index BACKOFFS/CAPACITY_BACKOFFS directly.
pick_backoff() {
  local attempt="$1"
  local category="$2"
  local idx=$((attempt - 1))
  local arr_name
  if is_capacity_error "$category"; then
    arr_name=CAPACITY_BACKOFFS
  else
    arr_name=BACKOFFS
  fi
  # Indirect array expansion — `${!arr_name[@]}` gives the keys; we compute
  # length via the "@" indexing trick.
  local -n arr_ref="$arr_name"
  local len=${#arr_ref[@]}
  (( idx >= len )) && idx=$((len - 1))
  printf '%s' "${arr_ref[$idx]}"
}

# classify_reviewer_outcome — inspect a single reviewer's post-launch state
# and attempt to parse its JSONL into an envelope (or raw output).
#
# Arguments:
#   $1 — reviewer name (codex|gemini)
#   $2 — path to reviewer's jsonl file
#   $3 — path to reviewer's parse-error file
#   $4 — path to reviewer's envelope.json output file
#   $5 — path to reviewer's stalled marker file (if present, reviewer was
#        killed by watchdog and must NOT be retried)
#   $6 — path to parser script (absolute or relative to SCRIPT_DIR). For
#        envelope mode: parse-codex.py / parse-gemini.py. For raw mode:
#        parse-codex-raw.py / parse-gemini-raw.py.
#   $7 — extra parser args as a single pre-quoted string. For envelope mode:
#        "--schema <path> --default-skill <name>". For raw mode: empty.
#        Parsed via `eval` so the caller is responsible for quoting paths
#        (standard shell-string discipline).
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
  local parser_script="$6"
  local parser_extra_args="${7:-}"

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
  # shellcheck disable=SC2294  # eval is intentional — caller-supplied args string
  if eval "$parser_script" --jsonl "\"$jsonl\"" $parser_extra_args \
      > "$envelope" 2> "$parse_err"; then
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
