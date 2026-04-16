#!/usr/bin/env bash
# one-round.sh — canonical single-entry dual-source invocation primitive.
#
# PURPOSE
#   ONE script for every dual-source consumer (/tp-help, /tpr-review,
#   /review-work, /review-plan). Before this existed, /tp-help invoked
#   dual-invoke.sh directly — bypassing circuit-breaker state,
#   per-reviewer retry, worktree-guard composition, and every other
#   operational-resilience feature that dual-invoke-with-retry.sh
#   builds for /tpr-review. That was "operational blindness": a known-
#   broken gemini (tripped by a prior /tpr-review round) still cost
#   /tp-help the full 10-minute foreground timeout.
#
# MODES
#   --mode envelope
#       Findings-envelope output (JSON schema validated). Delegates to
#       dual-invoke-with-retry.sh — the retry wrapper already integrates
#       circuit-breaker, selective-retry, and envelope-aware parsing.
#       Consumers: /tpr-review, /review-work, /review-plan.
#
#   --mode raw-concat
#       Free-form reviewer prose concatenated with HTML-comment
#       attribution sentinels. No envelope, no schema. Consumers:
#       /tp-help (question/consultation flow).
#
# SHARED ACROSS MODES
#   - Circuit-breaker pre-check (auto-restrict ORI_TPR_REVIEWERS when a
#     reviewer is tripped; error if BOTH are tripped)
#   - Worktree snapshot BEFORE + worktree-guard compare AFTER
#   - Circuit-breaker state update per reviewer based on outcome
#   - ORI_TPR_REVIEWERS escape hatch honored
#
# ARGS
#   --mode {envelope|raw-concat}    REQUIRED
#   --run <dir>                     scratch dir (REQUIRED)
#   --codex-prompt <file>           REQUIRED
#   --gemini-prompt <file>          REQUIRED
#   --skill <name>                  active review mode (informational
#                                   for raw-concat; drives parser
#                                   --default-skill for envelope).
#                                   Default: review-work.
#   --schema <path>                 REQUIRED for envelope mode; ignored
#                                   for raw-concat.
#   --output <file>                 raw-concat only. Default: $RUN/concat.md.
#
# EXIT CODES
#   0  success
#   1  both reviewers failed (or were circuit-broken)
#   2  usage error
#   3  infra failure (worktree drift under ORI_TPR_STRICT_WORKTREE=1)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

MODE=""
RUN=""
CODEX_PROMPT=""
GEMINI_PROMPT=""
SKILL="review-work"
SCHEMA=""
OUTPUT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)           MODE="$2"; shift 2 ;;
    --run)            RUN="$2"; shift 2 ;;
    --codex-prompt)   CODEX_PROMPT="$2"; shift 2 ;;
    --gemini-prompt)  GEMINI_PROMPT="$2"; shift 2 ;;
    --skill)          SKILL="$2"; shift 2 ;;
    --schema)         SCHEMA="$2"; shift 2 ;;
    --output)         OUTPUT="$2"; shift 2 ;;
    --help|-h)
      sed -n '3,50p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      echo "one-round.sh: unknown arg: $1" >&2
      exit 2 ;;
  esac
done

# ── Validation ────────────────────────────────────────────────────────
if [[ -z "$MODE" || -z "$RUN" || -z "$CODEX_PROMPT" || -z "$GEMINI_PROMPT" ]]; then
  echo "usage: one-round.sh --mode {envelope|raw-concat} --run DIR --codex-prompt FILE --gemini-prompt FILE [--skill NAME] [--schema FILE] [--output FILE]" >&2
  exit 2
fi

case "$MODE" in
  envelope)
    if [[ -z "$SCHEMA" ]]; then
      echo "one-round.sh: --schema is required for --mode envelope" >&2
      exit 2
    fi
    ;;
  raw-concat)
    OUTPUT="${OUTPUT:-$RUN/concat.md}"
    ;;
  *)
    echo "one-round.sh: invalid --mode '$MODE' (must be envelope|raw-concat)" >&2
    exit 2
    ;;
esac

mkdir -p "$RUN"

# ── Circuit-breaker pre-check ──────────────────────────────────────────
# Reads global per-user circuit-breaker state ($HOME/.cache/ori-tpr-circuit/).
# If a reviewer is currently tripped — regardless of which skill tripped it —
# auto-restrict ORI_TPR_REVIEWERS. Shared across /tpr-review, /tp-help, etc.
#
# Precedence: operator-supplied ORI_TPR_REVIEWERS wins over auto-restriction
# (operator intent always trumps circuit-breaker auto-drop).
ORIGINAL_REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
CODEX_TRIPPED=0
GEMINI_TRIPPED=0
if [[ "$ORIGINAL_REVIEWERS" == "both" || "$ORIGINAL_REVIEWERS" == "codex" ]]; then
  if ! "$SCRIPT_DIR/circuit-breaker.sh" check codex >/dev/null 2>&1; then
    CODEX_TRIPPED=1
    echo "[one-round] circuit-breaker: codex is in timeout — will skip" >&2
  fi
fi
if [[ "$ORIGINAL_REVIEWERS" == "both" || "$ORIGINAL_REVIEWERS" == "gemini" ]]; then
  if ! "$SCRIPT_DIR/circuit-breaker.sh" check gemini >/dev/null 2>&1; then
    GEMINI_TRIPPED=1
    echo "[one-round] circuit-breaker: gemini is in timeout — will skip" >&2
  fi
fi

# Auto-restrict only when operator said "both" (the default).
if [[ "$ORIGINAL_REVIEWERS" == "both" ]]; then
  if [[ "$CODEX_TRIPPED" -eq 1 && "$GEMINI_TRIPPED" -eq 1 ]]; then
    echo "[one-round] BOTH reviewers are in circuit-breaker timeout — aborting" >&2
    echo "[one-round] run \`circuit-breaker.sh reset all\` to override" >&2
    exit 1
  elif [[ "$CODEX_TRIPPED" -eq 1 ]]; then
    export ORI_TPR_REVIEWERS=gemini
    echo "[one-round] auto-restricted to ORI_TPR_REVIEWERS=gemini (codex tripped)" >&2
  elif [[ "$GEMINI_TRIPPED" -eq 1 ]]; then
    export ORI_TPR_REVIEWERS=codex
    echo "[one-round] auto-restricted to ORI_TPR_REVIEWERS=codex (gemini tripped)" >&2
  fi
elif [[ "$ORIGINAL_REVIEWERS" == "codex" && "$CODEX_TRIPPED" -eq 1 ]]; then
  echo "[one-round] operator requested codex only, but codex is tripped — aborting" >&2
  exit 1
elif [[ "$ORIGINAL_REVIEWERS" == "gemini" && "$GEMINI_TRIPPED" -eq 1 ]]; then
  echo "[one-round] operator requested gemini only, but gemini is tripped — aborting" >&2
  exit 1
fi

# ── Envelope mode — delegate to the existing retry wrapper ─────────────
# The retry wrapper already integrates circuit-breaker updates, selective
# retry, and envelope-aware parsing. We preserve its contract exactly.
if [[ "$MODE" == "envelope" ]]; then
  exec "$SCRIPT_DIR/dual-invoke-with-retry.sh" \
    --run "$RUN" \
    --skill "$SKILL" \
    --codex-prompt "$CODEX_PROMPT" \
    --gemini-prompt "$GEMINI_PROMPT" \
    --schema "$SCHEMA"
fi

# ── Raw-concat mode ────────────────────────────────────────────────────
# Unlike envelope mode, there's no schema to validate against and no
# findings to merge. We still want: circuit-breaker awareness (handled
# above), worktree-guard, and bounded retry on transient infra errors.

# Snapshot worktree BEFORE the reviewers run.
git status --porcelain > "$RUN/worktree.before" || true

# Launch dual-invoke.sh. One attempt. The /tp-help semantic contract is
# "one consultation" — retrying the consultation itself reshapes its
# meaning (you'd be getting a different question-answer pair). Transient
# API capacity errors are categorized through circuit-breaker.sh post-run,
# which means the NEXT /tp-help invocation auto-drops the degraded
# reviewer rather than this one retrying blind.
"$SCRIPT_DIR/dual-invoke.sh" \
  --run "$RUN" \
  --skill "$SKILL" \
  --codex-prompt "$CODEX_PROMPT" \
  --gemini-prompt "$GEMINI_PROMPT" >&2 || DUAL_EXIT=$?
DUAL_EXIT="${DUAL_EXIT:-0}"

# Per-reviewer outcome → circuit-breaker state update.
# dual-invoke.sh writes per-reviewer exit codes to $RUN/<reviewer>.exit.
# Missing .exit file means the reviewer was skipped (ORI_TPR_REVIEWERS
# filter or circuit-breaker-driven auto-restriction above).
for rev in codex gemini; do
  [[ -f "$RUN/$rev.skipped" ]] && continue
  if [[ -f "$RUN/$rev.exit" ]]; then
    code="$(cat "$RUN/$rev.exit" 2>/dev/null || echo 99)"
    if [[ "$code" == "0" ]]; then
      "$SCRIPT_DIR/circuit-breaker.sh" success "$rev" >/dev/null 2>&1 || true
    else
      # Classify as transport — reviewer process non-zero OR infra error.
      # The distinction between api/transport/semantic lives in the
      # envelope parsers, but raw-concat mode has no envelope. Treat
      # any non-zero exit as "transport-ish" for circuit-breaker
      # accounting — the threshold is 3-in-1-hour which is lenient
      # enough to tolerate occasional flakes while still catching
      # genuinely degraded providers.
      "$SCRIPT_DIR/circuit-breaker.sh" fail "$rev" transport >/dev/null 2>&1 || true
    fi
  fi
done

# Worktree-guard compare (delegates to canonical helper).
git status --porcelain > "$RUN/worktree.after" || true
if ! "$SCRIPT_DIR/worktree-guard.sh" compare \
     "$RUN/worktree.before" "$RUN/worktree.after" 2>"$RUN/worktree-drift.txt"; then
  echo "[one-round] WORKTREE DRIFT DETECTED — at least one reviewer modified the working tree" >&2
  echo "[one-round] see $RUN/worktree-drift.txt for details" >&2
  if [[ "${ORI_TPR_STRICT_WORKTREE:-0}" == "1" ]]; then
    echo "[one-round] ORI_TPR_STRICT_WORKTREE=1 — treating as failure" >&2
    exit 3
  fi
fi

# Parse both JSONL streams with the raw parsers.
codex_raw=""
gemini_raw=""
if [[ ! -f "$RUN/codex.skipped" ]]; then
  if ! codex_raw="$(python3 "$SCRIPT_DIR/parse-codex-raw.py" --jsonl "$RUN/codex.jsonl" 2>"$RUN/codex.parse.err")"; then
    codex_raw="(codex response unavailable — see $RUN/codex.jsonl and $RUN/codex.parse.err)"
  fi
fi
if [[ ! -f "$RUN/gemini.skipped" ]]; then
  if ! gemini_raw="$(python3 "$SCRIPT_DIR/parse-gemini-raw.py" --jsonl "$RUN/gemini.jsonl" 2>"$RUN/gemini.parse.err")"; then
    gemini_raw="(gemini response unavailable — see $RUN/gemini.jsonl and $RUN/gemini.parse.err)"
  fi
fi

# Concatenate with per-invocation sentinel tokens (canonical format from
# tp-help-sentinels.sh — consumers grep for TP_HELP_SENTINEL_PREFIX to
# detect leakage).
# shellcheck source=./tp-help-sentinels.sh
source "$SCRIPT_DIR/tp-help-sentinels.sh"
token="$(tp_help_make_token)"
{
  [[ -z "$codex_raw"  ]] || tp_help_emit_block codex  "$token" "$codex_raw"
  [[ -z "$codex_raw" || -z "$gemini_raw" ]] || printf '\n'
  [[ -z "$gemini_raw" ]] || tp_help_emit_block gemini "$token" "$gemini_raw"
} > "$OUTPUT"

# Success signal — both-side failure is only total blackout.
if [[ -z "$codex_raw" && -z "$gemini_raw" ]]; then
  echo "[one-round] both reviewers produced no usable output" >&2
  exit 1
fi

# Echo the canonical output path so callers can read it without re-deriving.
echo "$OUTPUT"
exit 0
