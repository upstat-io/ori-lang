#!/usr/bin/env bash
# supervisor.sh — per-reviewer retry supervisor.
#
# PURPOSE
#   Runs ONE reviewer (codex or gemini) with a self-contained retry loop.
#   Each supervisor has its own retry clock — it does not wait for a
#   partner reviewer to settle before retrying. A fast-failing reviewer
#   exhausts its 5 retries on its own wall time (typically 2-10 min) and
#   fires the circuit-breaker immediately on give-up, independent of what
#   the partner is doing.
#
# BEFORE THIS EXISTED
#   dual-invoke-with-retry.sh retried at ROUND granularity — attempt N+1
#   could not start until BOTH reviewers from attempt N had settled. A fast-
#   failing gemini was hostage to codex's 15-20 min wall clock. By the time
#   circuit-breaker accumulated 5 gemini failures, 45+ minutes had passed.
#   The NEXT round could finally auto-restrict, but the user already waited
#   through the damage. Supervisors fix that by decoupling retry timing.
#
# CONTRACT (artifacts under $RUN)
#   Inputs  (read):
#     <reviewer>.exit            — if present on entry AND non-zero at start,
#                                  prior-attempt state (selective retry) —
#                                  NOT relevant here; one-round.sh handles
#                                  operator-explicit filtering.
#   Outputs (written):
#     <reviewer>.jsonl           — final attempt's JSONL stream (stable path)
#     <reviewer>.stderr          — final attempt's stderr
#     <reviewer>.exit            — final attempt's process exit code
#     <reviewer>.walltime        — final attempt's wall seconds
#     <reviewer>.envelope.json   — on success: parsed envelope (envelope mode)
#                                  on failure: truncated to zero bytes
#     <reviewer>.parse-error     — parser stderr (advisory + category)
#     <reviewer>.stalled         — if watchdog killed the subprocess
#     <reviewer>.skipped         — if operator excluded via ORI_TPR_REVIEWERS
#     <reviewer>.gave_up         — on terminal failure after all retries
#     <reviewer>.attempts.log    — one TSV line per attempt: epoch\tN\tcat\tbackoff
#   Per-attempt intermediate files (kept for postmortem):
#     <reviewer>.jsonl.attemptN
#     <reviewer>.stderr.attemptN
#     <reviewer>.exit.attemptN
#
# ARGS
#   --reviewer {codex|gemini}        REQUIRED
#   --run <dir>                      REQUIRED
#   --prompt <file>                  REQUIRED
#   --mode {envelope|raw}            REQUIRED
#   --schema <file>                  required when --mode envelope
#   --skill <name>                   parser --default-skill (envelope only)
#                                    default: review-work
#   --max-attempts <N>               override MAX_RETRIES (testing only)
#
# EXIT CODES
#   0  success — envelope.json (or raw output) materialized
#   1  give-up — wrote .gave_up, fired circuit-breaker fail
#   2  usage error
#   3  skipped — wrote .skipped (operator filter; treat as success by caller)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=./lib-retry.sh
source "$SCRIPT_DIR/lib-retry.sh"

REVIEWER=""
RUN=""
PROMPT=""
MODE=""
SCHEMA=""
SKILL="review-work"
MAX_ATTEMPTS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --reviewer)      REVIEWER="$2"; shift 2 ;;
    --run)           RUN="$2"; shift 2 ;;
    --prompt)        PROMPT="$2"; shift 2 ;;
    --mode)          MODE="$2"; shift 2 ;;
    --schema)        SCHEMA="$2"; shift 2 ;;
    --skill)         SKILL="$2"; shift 2 ;;
    --max-attempts)  MAX_ATTEMPTS="$2"; shift 2 ;;
    -h|--help)
      sed -n '3,55p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      echo "supervisor.sh: unknown arg: $1" >&2
      exit 2 ;;
  esac
done

# ── Validation ────────────────────────────────────────────────────────
case "$REVIEWER" in codex|gemini) ;; *)
  echo "supervisor.sh: --reviewer must be codex|gemini (got: '$REVIEWER')" >&2; exit 2 ;;
esac
case "$MODE" in envelope|raw) ;; *)
  echo "supervisor.sh: --mode must be envelope|raw (got: '$MODE')" >&2; exit 2 ;;
esac
[[ -z "$RUN"    ]] && { echo "supervisor.sh: --run required" >&2; exit 2; }
[[ -z "$PROMPT" ]] && { echo "supervisor.sh: --prompt required" >&2; exit 2; }
[[ ! -f "$PROMPT" ]] && { echo "supervisor.sh: prompt file not found: $PROMPT" >&2; exit 2; }
if [[ "$MODE" == "envelope" && -z "$SCHEMA" ]]; then
  echo "supervisor.sh: --schema required when --mode envelope" >&2; exit 2
fi

mkdir -p "$RUN"

MAX_ATTEMPTS="${MAX_ATTEMPTS:-$MAX_RETRIES}"

# ── Operator-filter skip ──────────────────────────────────────────────
# ORI_TPR_REVIEWERS explicitly excludes this reviewer → skip without
# running. Mirrors the current dual-invoke.sh behavior so one-round.sh's
# pre-check decisions still hold.
REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
if [[ "$REVIEWERS" != "both" && "$REVIEWERS" != "$REVIEWER" ]]; then
  # Preserve prior-attempt state if present (selective-retry compat).
  if [[ -f "$RUN/$REVIEWER.exit" ]]; then
    echo "[$(date +%s)] $REVIEWER preserved from prior attempt (.exit present, ORI_TPR_REVIEWERS=$REVIEWERS narrowed)" >> "$RUN/round.log"
  else
    echo "ORI_TPR_REVIEWERS=$REVIEWERS" > "$RUN/$REVIEWER.skipped"
    echo "[$(date +%s)] $REVIEWER supervisor skipped (ORI_TPR_REVIEWERS=$REVIEWERS)" >> "$RUN/round.log"
  fi
  exit 3
fi

# ── Watchdog config (same defaults as legacy dual-invoke.sh) ──────────
case "$REVIEWER" in
  codex)
    STALL_GRACE="${ORI_TPR_STALL_GRACE_CODEX:-${ORI_TPR_STALL_GRACE:-600}}"
    STALL_PATIENCE="${ORI_TPR_STALL_PATIENCE_CODEX:-${ORI_TPR_STALL_PATIENCE:-3}}"
    MAX_WALLTIME="${ORI_TPR_CODEX_MAX_WALLTIME:-1200}"
    ;;
  gemini)
    STALL_GRACE="${ORI_TPR_STALL_GRACE_GEMINI:-${ORI_TPR_STALL_GRACE:-600}}"
    STALL_PATIENCE="${ORI_TPR_STALL_PATIENCE_GEMINI:-${ORI_TPR_STALL_PATIENCE:-3}}"
    MAX_WALLTIME="${ORI_TPR_GEMINI_MAX_WALLTIME:-1200}"
    ;;
esac
STALL_CHECK_INTERVAL="${ORI_TPR_STALL_INTERVAL:-180}"
STALL_DETECT="$SCRIPT_DIR/reviewer-stall-detect.sh"

# Canonical paths (stable — downstream consumers poll these).
JSONL="$RUN/$REVIEWER.jsonl"
STDERR_FILE="$RUN/$REVIEWER.stderr"
EXIT_FILE="$RUN/$REVIEWER.exit"
WALLTIME_FILE="$RUN/$REVIEWER.walltime"
ENVELOPE_FILE="$RUN/$REVIEWER.envelope.json"
PARSE_ERR_FILE="$RUN/$REVIEWER.parse-error"
STALLED_FILE="$RUN/$REVIEWER.stalled"
ATTEMPTS_LOG="$RUN/$REVIEWER.attempts.log"

# Clear stale state from a prior supervisor invocation in the same $RUN.
rm -f "$EXIT_FILE" "$WALLTIME_FILE" "$PARSE_ERR_FILE" "$ENVELOPE_FILE" \
      "$RUN/$REVIEWER.skipped" "$STALLED_FILE" "$STDERR_FILE" \
      "$RUN/$REVIEWER.gave_up" "$ATTEMPTS_LOG"

# ── Parser command assembly (envelope vs raw) ─────────────────────────
if [[ "$MODE" == "envelope" ]]; then
  PARSER_SCRIPT="$SCRIPT_DIR/parse-${REVIEWER}.py"
  PARSER_EXTRA="--schema \"$SCHEMA\" --default-skill \"$SKILL\""
else
  PARSER_SCRIPT="$SCRIPT_DIR/parse-${REVIEWER}-raw.py"
  PARSER_EXTRA=""
fi

# ── Subprocess cleanup trap ───────────────────────────────────────────
# Any trailing children (reviewer, watchdog) on abnormal exit get reaped
# so we don't orphan a process writing to $RUN files.
CHILD_PID=""
WATCHDOG_PID=""
cleanup() {
  for pid in "$WATCHDOG_PID" "$CHILD_PID"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill -TERM "$pid" 2>/dev/null || true
      sleep 0.5
      kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null || true
    fi
  done
}
trap cleanup EXIT INT TERM

# ── Watchdog (per-reviewer, scoped to this supervisor's subprocess) ───
# Lifted from dual-invoke.sh watchdog_monitor (lines 282-397) with scope
# narrowed from "any reviewer PID" to "this supervisor's CHILD_PID".
# Same stall-detect binary, same tunables, same IDLE/ACTIVE/DEAD classifier.
watchdog_monitor() {
  local pid="$1" jsonl="$2"
  local idle_count=0
  local snapshot_file="$RUN/$REVIEWER.stall-snapshot"
  local start_time
  start_time=$(date +%s)

  check_absolute_cap() {
    local elapsed=$(( $(date +%s) - start_time ))
    if [[ $elapsed -ge $MAX_WALLTIME ]]; then
      echo "[$(date +%s)] watchdog: $REVIEWER WALLTIME CAP — killing (elapsed=${elapsed}s >= cap=${MAX_WALLTIME}s)" >> "$RUN/round.log"
      echo "stalled_after=${elapsed}s reason=absolute_walltime_cap cap=${MAX_WALLTIME}s" > "$STALLED_FILE"
      kill -TERM "$pid" 2>/dev/null || true
      sleep 2
      kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null || true
      return 0
    fi
    return 1
  }

  # Grace period — absolute cap still fires, stall classification suppressed.
  while kill -0 "$pid" 2>/dev/null; do
    local elapsed=$(( $(date +%s) - start_time ))
    [[ $elapsed -ge $STALL_GRACE ]] && break
    check_absolute_cap && return
    sleep "$STALL_CHECK_INTERVAL"
  done
  kill -0 "$pid" 2>/dev/null || return

  echo "[$(date +%s)] watchdog: $REVIEWER grace period elapsed (${STALL_GRACE}s) — stall detection active (interval=${STALL_CHECK_INTERVAL}s, patience=${STALL_PATIENCE}, absolute cap=${MAX_WALLTIME}s)" >> "$RUN/round.log"
  "$STALL_DETECT" --snapshot --pid "$pid" --jsonl "$jsonl" --out "$snapshot_file" 2>/dev/null || true

  while kill -0 "$pid" 2>/dev/null; do
    sleep "$STALL_CHECK_INTERVAL"
    kill -0 "$pid" 2>/dev/null || break
    check_absolute_cap && break
    local result
    result=$("$STALL_DETECT" --compare --previous "$snapshot_file" --pid "$pid" --jsonl "$jsonl" 2>/dev/null) || true
    case "$result" in
      ACTIVE|RECEIVING|COMPUTING) idle_count=0 ;;
      IDLE)
        idle_count=$((idle_count + 1))
        echo "[$(date +%s)] watchdog: $REVIEWER IDLE ($idle_count/$STALL_PATIENCE)" >> "$RUN/round.log"
        if [[ $idle_count -ge $STALL_PATIENCE ]]; then
          echo "[$(date +%s)] watchdog: $REVIEWER STALLED — killing (grace=${STALL_GRACE}s + ${STALL_CHECK_INTERVAL}s × ${idle_count} = $((STALL_GRACE + STALL_CHECK_INTERVAL * idle_count))s total)" >> "$RUN/round.log"
          echo "stalled_after=$((STALL_GRACE + STALL_CHECK_INTERVAL * idle_count))s" > "$STALLED_FILE"
          kill -TERM "$pid" 2>/dev/null || true
          sleep 2
          kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null || true
          break
        fi
        ;;
      DEAD) break ;;
    esac
  done
}

# ── Reviewer launch (per-attempt) ─────────────────────────────────────
# Returns the launched PID in CHILD_PID global. Writes per-attempt
# intermediate files (<reviewer>.jsonl.attemptN etc.); the final attempt's
# intermediates are promoted to the canonical paths after the loop.
launch_reviewer() {
  local attempt="$1"
  local jsonl_file="$RUN/$REVIEWER.jsonl.attempt$attempt"
  local stderr_file="$RUN/$REVIEWER.stderr.attempt$attempt"
  local exit_file="$RUN/$REVIEWER.exit.attempt$attempt"
  local walltime_file="$RUN/$REVIEWER.walltime.attempt$attempt"

  case "$REVIEWER" in
    codex)
      (
        set +e
        START=$(date +%s)
        codex exec --full-auto --json --ephemeral "$(cat "$PROMPT")" \
          2>"$stderr_file" > "$jsonl_file"
        RC=$?
        echo "$RC" > "$exit_file"
        echo "$(($(date +%s) - START))" > "$walltime_file"
        echo "[$(date +%s)] codex attempt $attempt finished (rc=$RC)" >> "$RUN/round.log"
        if [[ "$RC" != "0" && -s "$stderr_file" ]]; then
          echo "[$(date +%s)] codex attempt $attempt stderr (first 500 chars): $(head -c 500 "$stderr_file")" >> "$RUN/round.log"
        fi
        exit "$RC"
      ) &
      CHILD_PID=$!
      ;;
    gemini)
      (
        set +e
        START=$(date +%s)
        gemini -m gemini-3.1-pro-preview --approval-mode yolo \
          --output-format stream-json -p "$(cat "$PROMPT")" \
          2>"$stderr_file" > "$jsonl_file"
        RC=$?
        echo "$RC" > "$exit_file"
        echo "$(($(date +%s) - START))" > "$walltime_file"
        echo "[$(date +%s)] gemini attempt $attempt finished (rc=$RC)" >> "$RUN/round.log"
        if [[ "$RC" != "0" && -s "$stderr_file" ]]; then
          echo "[$(date +%s)] gemini attempt $attempt stderr (first 500 chars): $(head -c 500 "$stderr_file")" >> "$RUN/round.log"
        fi
        exit "$RC"
      ) &
      CHILD_PID=$!
      ;;
  esac
}

# Promote an attempt's intermediate files to canonical (stable) paths.
promote_attempt() {
  local attempt="$1"
  cp -f "$RUN/$REVIEWER.jsonl.attempt$attempt"     "$JSONL"         2>/dev/null || true
  cp -f "$RUN/$REVIEWER.stderr.attempt$attempt"    "$STDERR_FILE"   2>/dev/null || true
  cp -f "$RUN/$REVIEWER.exit.attempt$attempt"      "$EXIT_FILE"     2>/dev/null || true
  cp -f "$RUN/$REVIEWER.walltime.attempt$attempt"  "$WALLTIME_FILE" 2>/dev/null || true
}

# ── Retry loop (per-reviewer, independent of partner) ─────────────────
echo "[$(date +%s)] $REVIEWER supervisor start (mode=$MODE skill=$SKILL max_attempts=$MAX_ATTEMPTS)" >> "$RUN/round.log"

ATTEMPT=0
FINAL_CATEGORY=""
while [[ $ATTEMPT -lt $MAX_ATTEMPTS ]]; do
  ATTEMPT=$((ATTEMPT + 1))
  FINAL_CATEGORY=""

  # Launch subprocess; spawn watchdog monitoring it.
  launch_reviewer "$ATTEMPT"
  watchdog_monitor "$CHILD_PID" "$RUN/$REVIEWER.jsonl.attempt$ATTEMPT" &
  WATCHDOG_PID=$!

  # Wait for this one reviewer. `set +e` so non-zero exits don't abort.
  set +e
  wait "$CHILD_PID"
  CHILD_RC=$?
  set -e
  CHILD_PID=""

  # Reap watchdog (it should exit naturally when kill -0 fails on the
  # departed reviewer PID, but defensive cleanup).
  if [[ -n "$WATCHDOG_PID" ]] && kill -0 "$WATCHDOG_PID" 2>/dev/null; then
    kill "$WATCHDOG_PID" 2>/dev/null || true
    wait "$WATCHDOG_PID" 2>/dev/null || true
  fi
  WATCHDOG_PID=""

  # Promote this attempt's intermediates so the classifier runs against
  # the stable paths (lib-retry.classify_reviewer_outcome reads JSONL +
  # parse-error + envelope files by their canonical names).
  promote_attempt "$ATTEMPT"

  # Classify outcome. On success: envelope.json materialized → record
  # circuit-breaker success and exit 0.
  # shellcheck disable=SC2310  # set -e with boolean function — intentional
  if FINAL_CATEGORY=$(classify_reviewer_outcome "$REVIEWER" \
        "$JSONL" "$PARSE_ERR_FILE" "$ENVELOPE_FILE" "$STALLED_FILE" \
        "$PARSER_SCRIPT" "$PARSER_EXTRA"); then
    echo "[$(date +%s)] $REVIEWER supervisor SUCCESS on attempt $ATTEMPT" >> "$RUN/round.log"
    printf '%s\t%s\t%s\t%s\n' "$(date +%s)" "$ATTEMPT" "success" "0" >> "$ATTEMPTS_LOG"
    if [[ "${ORI_TPR_CIRCUIT_OFF:-0}" != "1" ]]; then
      "$SCRIPT_DIR/circuit-breaker.sh" success "$REVIEWER" 2>/dev/null || true
    fi
    exit 0
  fi

  # Failure. FINAL_CATEGORY holds the classification (reviewer-prefixed).
  echo "[$(date +%s)] $REVIEWER attempt $ATTEMPT failed: $FINAL_CATEGORY" >> "$RUN/round.log"

  # Terminal? Give up immediately. Record circuit-breaker fail with the
  # specific category.
  if is_terminal_failure "$FINAL_CATEGORY"; then
    echo "[$(date +%s)] $REVIEWER supervisor TERMINAL failure: $FINAL_CATEGORY — giving up" >> "$RUN/round.log"
    printf '%s\t%s\t%s\t%s\n' "$(date +%s)" "$ATTEMPT" "$FINAL_CATEGORY" "terminal" >> "$ATTEMPTS_LOG"
    echo "$FINAL_CATEGORY" > "$RUN/$REVIEWER.gave_up"
    if [[ "${ORI_TPR_CIRCUIT_OFF:-0}" != "1" ]]; then
      "$SCRIPT_DIR/circuit-breaker.sh" fail "$REVIEWER" "$FINAL_CATEGORY" 2>/dev/null || true
    fi
    exit 1
  fi

  # Retryable. If we have budget left, backoff and re-attempt.
  if [[ $ATTEMPT -lt $MAX_ATTEMPTS ]]; then
    BACKOFF=$(pick_backoff "$ATTEMPT" "$FINAL_CATEGORY")
    echo "[$(date +%s)] $REVIEWER backoff ${BACKOFF}s before attempt $((ATTEMPT + 1)) (category=$FINAL_CATEGORY)" >> "$RUN/round.log"
    printf '%s\t%s\t%s\t%s\n' "$(date +%s)" "$ATTEMPT" "$FINAL_CATEGORY" "$BACKOFF" >> "$ATTEMPTS_LOG"
    sleep "$BACKOFF"
  else
    # Last attempt failed — exit loop and handle give-up below.
    printf '%s\t%s\t%s\t%s\n' "$(date +%s)" "$ATTEMPT" "$FINAL_CATEGORY" "exhausted" >> "$ATTEMPTS_LOG"
  fi
done

# ── Exhausted all retries without success ─────────────────────────────
echo "[$(date +%s)] $REVIEWER supervisor EXHAUSTED ($MAX_ATTEMPTS attempts) last_category=$FINAL_CATEGORY" >> "$RUN/round.log"
echo "${FINAL_CATEGORY:-unknown_failure}" > "$RUN/$REVIEWER.gave_up"
if [[ "${ORI_TPR_CIRCUIT_OFF:-0}" != "1" ]]; then
  "$SCRIPT_DIR/circuit-breaker.sh" fail "$REVIEWER" "${FINAL_CATEGORY:-unknown_failure}" 2>/dev/null || true
fi
exit 1
