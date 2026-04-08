#!/usr/bin/env bash
# dual-invoke.sh — launch codex AND gemini in parallel for one review round
#
# Usage:
#   .claude/skills/dual-tpr/scripts/dual-invoke.sh \
#       --run "$RUN" \
#       --skill review-work \
#       --codex-prompt "$RUN/codex.prompt.md" \
#       --gemini-prompt "$RUN/gemini.prompt.md" \
#       --schema .claude/skills/dual-tpr/findings-schema.json
#
# Outputs (placed in $RUN):
#   $RUN/codex.jsonl       — codex's stdout (item.completed JSONL stream)
#   $RUN/gemini.jsonl      — gemini's stdout (stream-json JSONL stream)
#   $RUN/codex.exit        — codex exit code
#   $RUN/gemini.exit       — gemini exit code
#   $RUN/codex.walltime    — codex wall time in seconds
#   $RUN/gemini.walltime   — gemini wall time in seconds
#   $RUN/round.log         — orchestration log
#
# Returns: 0 if BOTH reviewers exited 0; non-zero if either failed.
#          Note: this script is launch-only; success is gated on parser
#          validation in 02.2/02.3, not just exit code 0.

set -euo pipefail

# Parse args (minimal flag handling, no getopts to keep it tiny)
RUN=""; SKILL=""; CODEX_PROMPT=""; GEMINI_PROMPT=""; SCHEMA=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --run)            RUN="$2"; shift 2 ;;
    --skill)          SKILL="$2"; shift 2 ;;
    --codex-prompt)   CODEX_PROMPT="$2"; shift 2 ;;
    --gemini-prompt)  GEMINI_PROMPT="$2"; shift 2 ;;
    --schema)         SCHEMA="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

[[ -z "$RUN" || -z "$SKILL" || -z "$CODEX_PROMPT" || -z "$GEMINI_PROMPT" || -z "$SCHEMA" ]] && {
  echo "usage: dual-invoke.sh --run DIR --skill NAME --codex-prompt FILE --gemini-prompt FILE --schema FILE" >&2
  exit 2
}

echo "[$(date +%s)] dual-invoke start (skill=$SKILL run=$RUN)" >> "$RUN/round.log"

# Launch codex in the background
(
  START=$(date +%s)
  codex exec --full-auto --json --output-schema "$SCHEMA" --ephemeral "$(cat "$CODEX_PROMPT")" 2>/dev/null > "$RUN/codex.jsonl"
  echo "$?" > "$RUN/codex.exit"
  echo "$(($(date +%s) - START))" > "$RUN/codex.walltime"
  echo "[$(date +%s)] codex finished" >> "$RUN/round.log"
) &
CODEX_PID=$!

# Launch gemini in the background
(
  START=$(date +%s)
  gemini --approval-mode yolo --output-format stream-json -p "$(cat "$GEMINI_PROMPT")" 2>/dev/null > "$RUN/gemini.jsonl"
  echo "$?" > "$RUN/gemini.exit"
  echo "$(($(date +%s) - START))" > "$RUN/gemini.walltime"
  echo "[$(date +%s)] gemini finished" >> "$RUN/round.log"
) &
GEMINI_PID=$!

# Wait for BOTH to complete
wait "$CODEX_PID"
wait "$GEMINI_PID"

CODEX_EXIT=$(cat "$RUN/codex.exit")
GEMINI_EXIT=$(cat "$RUN/gemini.exit")
echo "[$(date +%s)] dual-invoke done (codex=$CODEX_EXIT gemini=$GEMINI_EXIT)" >> "$RUN/round.log"

# Return non-zero if either failed at the launch level.
# Note: launch success is necessary but not sufficient — parser validation
# in 02.2/02.3 is the authoritative success check.
if [[ "$CODEX_EXIT" != "0" || "$GEMINI_EXIT" != "0" ]]; then
  exit 1
fi
exit 0
