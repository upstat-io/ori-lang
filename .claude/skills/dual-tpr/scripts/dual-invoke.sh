#!/usr/bin/env bash
# dual-invoke.sh — supervisor orchestrator for dual-source reviewers.
#
# USAGE
#   dual-invoke.sh \
#     --run "$RUN" \
#     --skill review-work \
#     --codex-prompt "$RUN/codex.prompt.md" \
#     --gemini-prompt "$RUN/gemini.prompt.md" \
#     [--schema .claude/skills/dual-tpr/findings-schema.json] \
#     [--mode {envelope|raw}]         # default: envelope
#
# WHAT IT DOES
#   Launches two supervisor.sh processes — one per reviewer — as backgrounded
#   siblings. Each supervisor runs its own retry loop on its own clock: a
#   fast-failing reviewer exhausts retries in 2-10 min independently of the
#   partner's 15-20 min wall time. `wait` on both supervisors — each returns
#   when it's done, not when the partner is done.
#
# CONTRACT (exit code)
#   0  — at least one reviewer succeeded (envelope.json materialized).
#        The operator (one-round.sh, /tpr-review) decides whether a single-
#        reviewer round is acceptable per its own policy.
#   1  — all launched reviewers gave up (every supervisor wrote .gave_up).
#   2  — usage error.
#
# HISTORY
#   Before 2026-04-16: dual-invoke.sh launched reviewers directly and watched
#   both PIDs from a shared retry wrapper (dual-invoke-with-retry.sh). Retry
#   was round-coupled — attempt N+1 started only when BOTH reviewers from
#   attempt N had settled. A fast-failing gemini waited on codex's wall time
#   before it could retry, and by the time circuit-breaker accumulated 5
#   failures, 45+ minutes had passed. Supervisors fix that by moving the
#   retry loop into a per-reviewer process. See supervisor.sh for details.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

RUN=""
SKILL=""
CODEX_PROMPT=""
GEMINI_PROMPT=""
SCHEMA=""
MODE="envelope"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --run)            RUN="$2"; shift 2 ;;
    --skill)          SKILL="$2"; shift 2 ;;
    --codex-prompt)   CODEX_PROMPT="$2"; shift 2 ;;
    --gemini-prompt)  GEMINI_PROMPT="$2"; shift 2 ;;
    --schema)         SCHEMA="$2"; shift 2 ;;
    --mode)           MODE="$2"; shift 2 ;;
    -h|--help)
      sed -n '3,30p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      echo "dual-invoke.sh: unknown arg: $1" >&2
      exit 2 ;;
  esac
done

[[ -z "$RUN"           ]] && { echo "dual-invoke.sh: --run required" >&2; exit 2; }
[[ -z "$CODEX_PROMPT"  ]] && { echo "dual-invoke.sh: --codex-prompt required" >&2; exit 2; }
[[ -z "$GEMINI_PROMPT" ]] && { echo "dual-invoke.sh: --gemini-prompt required" >&2; exit 2; }
# SKILL falls back to review-work for legacy callers. First-party callers always pass it.
[[ -z "$SKILL" ]] && SKILL="review-work"
case "$MODE" in envelope|raw) ;; *)
  echo "dual-invoke.sh: --mode must be envelope|raw (got '$MODE')" >&2; exit 2 ;;
esac
if [[ "$MODE" == "envelope" && -z "$SCHEMA" ]]; then
  echo "dual-invoke.sh: --schema required when --mode envelope" >&2; exit 2
fi

mkdir -p "$RUN"
: > "$RUN/round.log"  # truncate per-invocation log
echo "[$(date +%s)] dual-invoke start (skill=$SKILL run=$RUN mode=$MODE)" >> "$RUN/round.log"

# ── ORI_TPR_REVIEWERS validation ──────────────────────────────────────
# Supervisors themselves consult this env var and self-skip when excluded.
# We only validate the string here so invalid values fail fast.
REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
if [[ "$REVIEWERS" != "codex" && "$REVIEWERS" != "gemini" && "$REVIEWERS" != "both" ]]; then
  echo "invalid ORI_TPR_REVIEWERS: $REVIEWERS (must be codex|gemini|both)" >&2
  exit 2
fi

# ── Supervisor dispatch ───────────────────────────────────────────────
# Launch each supervisor as a backgrounded child. The supervisor handles its
# own per-attempt launch, watchdog, parse/classify, retry, and circuit-breaker
# bookkeeping. We just wait on the PIDs.
CODEX_SUP_PID=""
GEMINI_SUP_PID=""

SUPERVISOR="$SCRIPT_DIR/supervisor.sh"

# Cleanup on abnormal exit — reap any still-running supervisor.
cleanup_supervisors() {
  for pid in "$CODEX_SUP_PID" "$GEMINI_SUP_PID"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill -TERM "$pid" 2>/dev/null || true
      sleep 0.5
      kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null || true
    fi
  done
}
trap cleanup_supervisors EXIT INT TERM

# Build supervisor arg arrays (mode-dependent parts omitted when empty).
codex_args=(--reviewer codex --run "$RUN" --prompt "$CODEX_PROMPT" --mode "$MODE" --skill "$SKILL")
gemini_args=(--reviewer gemini --run "$RUN" --prompt "$GEMINI_PROMPT" --mode "$MODE" --skill "$SKILL")
if [[ "$MODE" == "envelope" ]]; then
  codex_args+=(--schema "$SCHEMA")
  gemini_args+=(--schema "$SCHEMA")
fi

# Launch. Each supervisor's own ORI_TPR_REVIEWERS logic decides whether to
# actually run or self-skip.
"$SUPERVISOR" "${codex_args[@]}" &
CODEX_SUP_PID=$!
"$SUPERVISOR" "${gemini_args[@]}" &
GEMINI_SUP_PID=$!

echo "[$(date +%s)] dual-invoke dispatched supervisors (codex=$CODEX_SUP_PID gemini=$GEMINI_SUP_PID)" >> "$RUN/round.log"

# ── Wait barrier ──────────────────────────────────────────────────────
# Each supervisor returns independently on its own wall time. `set +e` so a
# non-zero from one supervisor doesn't abort before we wait on the partner.
set +e
wait "$CODEX_SUP_PID"
CODEX_SUP_RC=$?
wait "$GEMINI_SUP_PID"
GEMINI_SUP_RC=$?
set -e

CODEX_SUP_PID=""
GEMINI_SUP_PID=""

echo "[$(date +%s)] dual-invoke supervisors settled (codex_rc=$CODEX_SUP_RC gemini_rc=$GEMINI_SUP_RC)" >> "$RUN/round.log"

# ── Aggregate outcome ─────────────────────────────────────────────────
# supervisor.sh exit codes:
#   0  success
#   1  gave up
#   3  operator-filter skip (treat as success for aggregation)
#
# A round is "usable" iff at least one reviewer produced a materialized
# envelope.json (envelope mode) OR at least one produced a non-empty concat
# parse (raw mode). Envelope presence is checked by `-s <reviewer>.envelope.json`
# in envelope mode; raw mode callers (one-round.sh) perform their own post-
# processing via parse-*-raw.py, so we only check exit codes.
CODEX_OK=0
GEMINI_OK=0
case "$CODEX_SUP_RC" in 0|3) CODEX_OK=1 ;; esac
case "$GEMINI_SUP_RC" in 0|3) GEMINI_OK=1 ;; esac

# For envelope mode, also verify envelope.json exists for reviewers that
# weren't skipped. A supervisor that exited 0 MUST have written an envelope.
if [[ "$MODE" == "envelope" ]]; then
  if [[ "$CODEX_SUP_RC" == "0" && ! -s "$RUN/codex.envelope.json" ]]; then
    echo "[$(date +%s)] dual-invoke WARN: codex supervisor exited 0 but envelope.json is empty" >> "$RUN/round.log"
    CODEX_OK=0
  fi
  if [[ "$GEMINI_SUP_RC" == "0" && ! -s "$RUN/gemini.envelope.json" ]]; then
    echo "[$(date +%s)] dual-invoke WARN: gemini supervisor exited 0 but envelope.json is empty" >> "$RUN/round.log"
    GEMINI_OK=0
  fi
fi

if [[ $CODEX_OK -eq 1 || $GEMINI_OK -eq 1 ]]; then
  echo "[$(date +%s)] dual-invoke OK (codex_ok=$CODEX_OK gemini_ok=$GEMINI_OK)" >> "$RUN/round.log"
  exit 0
fi

echo "[$(date +%s)] dual-invoke FAIL — all reviewers gave up" >> "$RUN/round.log"
exit 1
