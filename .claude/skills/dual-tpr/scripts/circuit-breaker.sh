#!/usr/bin/env bash
# circuit-breaker.sh — per-reviewer global circuit breaker.
#
# Goal: when a reviewer (codex | gemini) fails 3 times from API or transport
# errors inside a 1-hour sliding window, park it for 1 hour and let the
# surviving reviewer carry the round alone. Prevents the dual-source pipeline
# from burning wall time and quota spinning on a provider that is clearly
# degraded.
#
# State is **global per user** — stored under $HOME/.cache/ori-tpr-circuit/
# (overridable via $ORI_TPR_CIRCUIT_DIR). This is intentional:
#   - "global for the entire repo" per user intent (2026-04-15)
#   - Persists across Claude sessions, across skills (/tpr-review, /tp-help,
#     /review-work, /review-plan), and across multiple worktrees of the same
#     repo — the API itself is the shared failing resource, not the worktree.
#   - File-level flocking makes parallel agents safe.
#
# Subcommands:
#   check <reviewer>              exit 0 if available, 1 if in timeout.
#                                 On timeout: prints "timeout_remaining=<sec>" to stdout.
#   fail <reviewer> <category>    record a failure. No-op unless <category>
#                                 is api/transport. If the sliding-window
#                                 count reaches the threshold, writes the
#                                 timeout sentinel.
#   success <reviewer>            clear the fail counter (success resets).
#   status                        human-readable summary for both reviewers.
#   reset <reviewer|all>          manual override — clear fails + timeout.
#
# Exit codes:
#   0   ok / available
#   1   tripped (timeout active) — for `check`
#   2   usage error
#
# Tuning via env:
#   ORI_TPR_CIRCUIT_THRESHOLD   fails inside the window that trip the breaker (default 3)
#   ORI_TPR_CIRCUIT_WINDOW_SEC  sliding-window length in seconds (default 3600)
#   ORI_TPR_CIRCUIT_TIMEOUT_SEC how long the breaker stays tripped (default 3600)
#   ORI_TPR_CIRCUIT_DIR         state dir (default $HOME/.cache/ori-tpr-circuit)

set -euo pipefail

CIRCUIT_DIR="${ORI_TPR_CIRCUIT_DIR:-$HOME/.cache/ori-tpr-circuit}"
THRESHOLD="${ORI_TPR_CIRCUIT_THRESHOLD:-3}"
WINDOW_SEC="${ORI_TPR_CIRCUIT_WINDOW_SEC:-3600}"
TIMEOUT_SEC="${ORI_TPR_CIRCUIT_TIMEOUT_SEC:-3600}"

mkdir -p "$CIRCUIT_DIR"

now() { date +%s; }

valid_reviewer() {
  case "$1" in
    codex|gemini) return 0 ;;
    *) return 1 ;;
  esac
}

# is_api_or_transport — returns 0 iff the failure category represents an
# API-level or transport-level error (i.e. "the reviewer couldn't run"),
# not a semantic/content error from the reviewer itself.
#
# Counted (trip the breaker):
#   <reviewer>_api_capacity    — provider capacity / rate limit
#   <reviewer>_api_auth        — auth failure (terminal upstream, but still
#                                counts — repeated auth fails mean the key
#                                or entitlement is broken; skip the reviewer)
#   <reviewer>_api_error       — generic API rejection
#   <reviewer>_missing_jsonl   — reviewer subprocess died before emitting anything
#   reviewer_stalled_<r>       — watchdog killed a hung reviewer
#   launch_or_exit_fail        — legacy catch-all for launch / exit failures
#
# NOT counted (do not trip the breaker):
#   <reviewer>_parse_fail      — reviewer emitted non-JSON (content issue)
#   <reviewer>_schema_violation — envelope shape wrong (content issue)
#   <reviewer>_missing_envelope — reviewer said nothing (prompt issue)
#   <reviewer>_missing_*_sentinel — gemini format issue
#   <reviewer>_failed_partial  — reviewer self-reported it gave up (content)
#   <reviewer>_missing_dependency — local env issue, not the reviewer's fault
#   dirty_worktree             — drift check, not a reviewer failure
is_api_or_transport() {
  local cat="$1"
  case "$cat" in
    *_api_capacity|*_api_auth|*_api_error) return 0 ;;
    *_missing_jsonl)                        return 0 ;;
    reviewer_stalled_*)                     return 0 ;;
    launch_or_exit_fail)                    return 0 ;;
    *) return 1 ;;
  esac
}

# with_lock <statefile> <function> — serialize state mutations for a reviewer
# so concurrent agents don't corrupt the fails/timeout files.
with_lock() {
  local lockfile="$1"
  shift
  (
    # shellcheck disable=SC2094
    flock -x 9
    "$@"
  ) 9>"$lockfile"
}

# prune_fails_inplace <fails_file> <cutoff_epoch> — rewrite fails file
# keeping only lines whose epoch >= cutoff.
prune_fails_inplace() {
  local fails_file="$1"
  local cutoff="$2"
  [[ -f "$fails_file" ]] || return 0
  local tmp
  tmp="$(mktemp "${fails_file}.XXXXXX")"
  awk -v cutoff="$cutoff" '$0 ~ /^[0-9]+$/ && $0 >= cutoff' "$fails_file" > "$tmp" || true
  mv "$tmp" "$fails_file"
}

cmd_check() {
  local reviewer="$1"
  valid_reviewer "$reviewer" || { echo "invalid reviewer: $reviewer" >&2; exit 2; }

  local timeout_file="$CIRCUIT_DIR/$reviewer.timeout"
  [[ -f "$timeout_file" ]] || return 0

  local expiry
  expiry="$(cat "$timeout_file" 2>/dev/null || echo 0)"
  local t
  t="$(now)"

  if [[ "$expiry" -gt "$t" ]]; then
    printf 'timeout_remaining=%d\n' "$((expiry - t))"
    return 1
  fi

  # Expired — clean up so next check is cheap.
  rm -f "$timeout_file"
  # Also clear fails on natural expiry — a fresh 1hr window starts.
  rm -f "$CIRCUIT_DIR/$reviewer.fails"
  return 0
}

# _record_fail_locked — must run under with_lock.
_record_fail_locked() {
  local reviewer="$1"
  local fails_file="$CIRCUIT_DIR/$reviewer.fails"
  local timeout_file="$CIRCUIT_DIR/$reviewer.timeout"
  local t
  t="$(now)"
  local cutoff=$((t - WINDOW_SEC))

  prune_fails_inplace "$fails_file" "$cutoff"
  printf '%s\n' "$t" >> "$fails_file"

  local count
  count="$(wc -l < "$fails_file" 2>/dev/null || echo 0)"
  count="${count// /}"  # trim any padding wc adds

  if [[ "$count" -ge "$THRESHOLD" ]]; then
    local expiry=$((t + TIMEOUT_SEC))
    printf '%s\n' "$expiry" > "$timeout_file"
    printf 'tripped: %s in timeout until epoch %s (%d fails in last %ds)\n' \
      "$reviewer" "$expiry" "$count" "$WINDOW_SEC" >&2
  else
    printf 'recorded: %s fail %d/%d in last %ds\n' \
      "$reviewer" "$count" "$THRESHOLD" "$WINDOW_SEC" >&2
  fi
}

cmd_fail() {
  local reviewer="$1"
  local category="${2:-unknown}"
  valid_reviewer "$reviewer" || { echo "invalid reviewer: $reviewer" >&2; exit 2; }

  if ! is_api_or_transport "$category"; then
    # Semantic/content failure — not our problem. Silent no-op so callers
    # can invoke unconditionally.
    return 0
  fi

  with_lock "$CIRCUIT_DIR/$reviewer.lock" _record_fail_locked "$reviewer"
}

# _reset_fails_locked — must run under with_lock.
_reset_fails_locked() {
  local reviewer="$1"
  # NOTE: a successful round clears the fails counter but does NOT clear an
  # active timeout — if the breaker is tripped, the reviewer was skipped
  # entirely, so any "success" reported by the caller is about the partner.
  # Timeout must run its full duration (or be cleared via `reset`).
  rm -f "$CIRCUIT_DIR/$reviewer.fails"
}

cmd_success() {
  local reviewer="$1"
  valid_reviewer "$reviewer" || { echo "invalid reviewer: $reviewer" >&2; exit 2; }
  with_lock "$CIRCUIT_DIR/$reviewer.lock" _reset_fails_locked "$reviewer"
}

cmd_reset() {
  local target="$1"
  case "$target" in
    codex|gemini)
      rm -f "$CIRCUIT_DIR/$target.fails" "$CIRCUIT_DIR/$target.timeout"
      printf 'reset %s\n' "$target"
      ;;
    all)
      rm -f "$CIRCUIT_DIR"/codex.fails "$CIRCUIT_DIR"/codex.timeout \
            "$CIRCUIT_DIR"/gemini.fails "$CIRCUIT_DIR"/gemini.timeout
      printf 'reset all reviewers\n'
      ;;
    *)
      echo "usage: circuit-breaker.sh reset {codex|gemini|all}" >&2
      exit 2
      ;;
  esac
}

cmd_status() {
  local t
  t="$(now)"
  printf 'circuit-breaker state (dir=%s, threshold=%d, window=%ds, timeout=%ds):\n' \
    "$CIRCUIT_DIR" "$THRESHOLD" "$WINDOW_SEC" "$TIMEOUT_SEC"
  for r in codex gemini; do
    local fails_file="$CIRCUIT_DIR/$r.fails"
    local timeout_file="$CIRCUIT_DIR/$r.timeout"
    local cutoff=$((t - WINDOW_SEC))
    local recent=0
    if [[ -f "$fails_file" ]]; then
      recent="$(awk -v c="$cutoff" '$0 ~ /^[0-9]+$/ && $0 >= c' "$fails_file" | wc -l)"
      recent="${recent// /}"
    fi

    local state="available"
    local detail=""
    if [[ -f "$timeout_file" ]]; then
      local expiry
      expiry="$(cat "$timeout_file" 2>/dev/null || echo 0)"
      if [[ "$expiry" -gt "$t" ]]; then
        state="TRIPPED"
        detail=" (${recent} recent fails, $((expiry - t))s remaining)"
      fi
    fi
    if [[ "$state" == "available" ]]; then
      detail=" (${recent} recent fails in ${WINDOW_SEC}s window)"
    fi
    printf '  %-7s %-9s%s\n' "$r" "$state" "$detail"
  done
}

usage() {
  cat >&2 <<EOF
usage: circuit-breaker.sh <subcommand> [args...]

  check <codex|gemini>            exit 1 if tripped, 0 otherwise
  fail  <codex|gemini> <category> record a failure (no-op if category
                                  isn't api/transport)
  success <codex|gemini>          clear fails counter for that reviewer
  reset <codex|gemini|all>        manual override — clear state
  status                          human-readable summary

Env:
  ORI_TPR_CIRCUIT_DIR         default \$HOME/.cache/ori-tpr-circuit
  ORI_TPR_CIRCUIT_THRESHOLD   default 3
  ORI_TPR_CIRCUIT_WINDOW_SEC  default 3600
  ORI_TPR_CIRCUIT_TIMEOUT_SEC default 3600
EOF
  exit 2
}

[[ $# -lt 1 ]] && usage

SUB="$1"; shift
case "$SUB" in
  check)   [[ $# -eq 1 ]] || usage; cmd_check "$1" ;;
  fail)    [[ $# -ge 1 ]] || usage; cmd_fail "$1" "${2:-unknown}" ;;
  success) [[ $# -eq 1 ]] || usage; cmd_success "$1" ;;
  reset)   [[ $# -eq 1 ]] || usage; cmd_reset "$1" ;;
  status)  cmd_status ;;
  *)       usage ;;
esac
