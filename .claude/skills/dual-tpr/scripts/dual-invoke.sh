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
    --schema)         SCHEMA="$2"; shift 2 ;;  # lint-transport-contract: known-dead BUG-08-003 (--output-schema removed, flag kept for caller backward compat)
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# --schema is OPTIONAL. BUG-08-003 removed the `--output-schema` passthrough
# to codex (see the comment block at lines 82-94 below), making $SCHEMA dead
# code inside this script. The flag is preserved in the arg parser above for
# caller-signature backward compatibility (dual-invoke-with-retry.sh still
# passes it), but it is NOT enforced here.
[[ -z "$RUN" || -z "$SKILL" || -z "$CODEX_PROMPT" || -z "$GEMINI_PROMPT" ]] && {
  echo "usage: dual-invoke.sh --run DIR --skill NAME --codex-prompt FILE --gemini-prompt FILE [--schema FILE]" >&2
  exit 2
}

# ORI_TPR_REVIEWERS runtime toggle (§07.2, moved from §08.2). Lets callers
# restrict the round to a single reviewer for faster iteration (dual-source
# wall time ~10x single-source because gemini is the bottleneck). Default is
# `both` so every existing caller (§04/§05 wrappers, dual-invoke-with-retry.sh)
# runs unchanged. Backward-compat invariant: `unset ORI_TPR_REVIEWERS` MUST
# behave identically to pre-toggle dual-invoke.sh.
REVIEWERS="${ORI_TPR_REVIEWERS:-both}"
if [[ "$REVIEWERS" != "codex" && "$REVIEWERS" != "gemini" && "$REVIEWERS" != "both" ]]; then
  echo "invalid ORI_TPR_REVIEWERS: $REVIEWERS (must be codex|gemini|both)" >&2
  exit 2
fi

echo "[$(date +%s)] dual-invoke start (skill=$SKILL run=$RUN reviewers=$REVIEWERS)" >> "$RUN/round.log"

# Track child PIDs so we can clean them up on early exit (BUG-08-005). Bash
# inherits this script's traps into subshells, but the parent tracks the PIDs
# explicitly and the EXIT trap below kills any survivors.
CODEX_PID=""
GEMINI_PID=""

# On any exit (success, failure, signal), reap any still-running children to
# prevent orphaned reviewer subprocesses from continuing past dual-invoke.sh's
# lifetime (BUG-08-005). The orphan would otherwise keep writing to
# $RUN/{codex,gemini}.jsonl after the parent exited, racing with subsequent
# retry attempts.
cleanup_children() {
  local pid
  for pid in "$CODEX_PID" "$GEMINI_PID"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill -TERM "$pid" 2>/dev/null || true
      # Give the child a moment to exit cleanly, then escalate to KILL
      sleep 0.5
      if kill -0 "$pid" 2>/dev/null; then
        kill -KILL "$pid" 2>/dev/null || true
      fi
    fi
  done
}
trap cleanup_children EXIT INT TERM

# Launch codex in the background.
#
# BUG-08-004 fix: disable `set -e` inside the subshell so the trailing echo
# statements ALWAYS run, even when the codex command exits non-zero. Without
# this, a fast codex failure (e.g. OpenAI API rejection in <10s) would abort
# the subshell at the failed `codex exec` line and never record the exit code,
# walltime, or "codex finished" log entry. The trap pattern is the canonical
# way to capture exit codes from a subshell that may abort.
#
# BUG-08-003 Option B: --output-schema is intentionally NOT passed. Passing
# the schema causes codex to forward it to OpenAI's Structured Outputs API as
# response_format.json_schema, which OpenAI enforces in strict mode — strict
# mode requires `additionalProperties: false` on every object and every
# property in the `required` array, a substantial rewrite that would make
# codex's validation path asymmetric with gemini's (gemini uses Google's
# Gemini API, which doesn't share the OpenAI strict-mode subset). Instead,
# codex emits free-form JSON driven by the prompt template, and our
# parse-codex.py + envelope_invariants.py validate it at the parser layer.
# This keeps codex and gemini symmetric: both validated only at the parser
# level, same failure modes, same retry classifier treatment. The schema
# file remains one SSOT for envelope structure, and its code-level
# invariants (in envelope_invariants.py) apply uniformly to both reviewers.
# TPR-04-002-gemini: the subshell's final command must `exit "$CODEX_RC"`
# so that the `wait $CODEX_PID` return code in the parent matches the real
# codex exit code, rather than always being 0 from the last echo. The
# parent script still falls back to reading codex.exit if the subshell is
# killed before it can exit, but the wait RC is now a second redundant
# signal — defense in depth against a corrupted .exit file.
if [[ "$REVIEWERS" == "codex" || "$REVIEWERS" == "both" ]]; then
  (
    set +e
    START=$(date +%s)
    codex exec --full-auto --json --ephemeral "$(cat "$CODEX_PROMPT")" 2>/dev/null > "$RUN/codex.jsonl"
    CODEX_RC=$?
    echo "$CODEX_RC" > "$RUN/codex.exit"
    echo "$(($(date +%s) - START))" > "$RUN/codex.walltime"
    echo "[$(date +%s)] codex finished (rc=$CODEX_RC)" >> "$RUN/round.log"
    exit "$CODEX_RC"
  ) &
  CODEX_PID=$!
else
  # Skipped reviewer: write an explicit .skipped marker so status-check.sh
  # and downstream consumers can distinguish "skipped" from "still running"
  # (§07.3 TPR finding: codex surfaced that ORI_TPR_REVIEWERS=codex left
  # gemini showing 'running' forever because no .exit file was written).
  # The marker content is the env value for postmortem traceability.
  echo "ORI_TPR_REVIEWERS=$REVIEWERS" > "$RUN/codex.skipped"
  echo "[$(date +%s)] codex skipped (ORI_TPR_REVIEWERS=$REVIEWERS)" >> "$RUN/round.log"
fi

# Launch gemini in the background. Same BUG-08-004 + TPR-04-002-gemini fix.
if [[ "$REVIEWERS" == "gemini" || "$REVIEWERS" == "both" ]]; then
  (
    set +e
    START=$(date +%s)
    gemini --approval-mode yolo --output-format stream-json -p "$(cat "$GEMINI_PROMPT")" 2>/dev/null > "$RUN/gemini.jsonl"
    GEMINI_RC=$?
    echo "$GEMINI_RC" > "$RUN/gemini.exit"
    echo "$(($(date +%s) - START))" > "$RUN/gemini.walltime"
    echo "[$(date +%s)] gemini finished (rc=$GEMINI_RC)" >> "$RUN/round.log"
    exit "$GEMINI_RC"
  ) &
  GEMINI_PID=$!
else
  # Skipped reviewer: see the codex-side comment above for rationale.
  echo "ORI_TPR_REVIEWERS=$REVIEWERS" > "$RUN/gemini.skipped"
  echo "[$(date +%s)] gemini skipped (ORI_TPR_REVIEWERS=$REVIEWERS)" >> "$RUN/round.log"
fi

# Wait for whichever children were launched. BUG-08-005 fix: with `set -e`,
# the original `wait $CODEX_PID; wait $GEMINI_PID` aborted the script when
# the first wait returned non-zero, skipping the second wait and leaking
# the other reviewer subprocess. Disable `set -e` around the waits so we
# always collect both exit codes, then re-enable it after.
#
# §07.2: wait calls are gated on non-empty PIDs because ORI_TPR_REVIEWERS
# may have skipped one reviewer entirely. `wait ""` is a no-op but some
# bash versions error — safer to skip the call explicitly.
CODEX_WAIT_RC=0
GEMINI_WAIT_RC=0
set +e
if [[ -n "$CODEX_PID" ]]; then
  wait "$CODEX_PID"
  CODEX_WAIT_RC=$?
fi
if [[ -n "$GEMINI_PID" ]]; then
  wait "$GEMINI_PID"
  GEMINI_WAIT_RC=$?
fi
set -e

# Read the recorded exit codes (written by the subshells via their own
# trap-style capture). If a subshell was killed before recording its exit
# file, fall back to the wait return code so we never report empty.
#
# §07.2: when a reviewer was NOT launched (ORI_TPR_REVIEWERS filter), its
# exit code is synthesized as "0" so the aggregation check at the bottom
# only fires on real failures from launched reviewers. There is no .exit
# file to read in that case.
if [[ "$REVIEWERS" == "codex" || "$REVIEWERS" == "both" ]]; then
  if [[ -f "$RUN/codex.exit" ]]; then
    CODEX_EXIT=$(cat "$RUN/codex.exit")
  else
    CODEX_EXIT="$CODEX_WAIT_RC"
    echo "$CODEX_EXIT" > "$RUN/codex.exit"
    echo "[$(date +%s)] codex.exit was missing; recorded wait rc=$CODEX_EXIT" >> "$RUN/round.log"
  fi
else
  CODEX_EXIT="0"
fi
if [[ "$REVIEWERS" == "gemini" || "$REVIEWERS" == "both" ]]; then
  if [[ -f "$RUN/gemini.exit" ]]; then
    GEMINI_EXIT=$(cat "$RUN/gemini.exit")
  else
    GEMINI_EXIT="$GEMINI_WAIT_RC"
    echo "$GEMINI_EXIT" > "$RUN/gemini.exit"
    echo "[$(date +%s)] gemini.exit was missing; recorded wait rc=$GEMINI_EXIT" >> "$RUN/round.log"
  fi
else
  GEMINI_EXIT="0"
fi

echo "[$(date +%s)] dual-invoke done (codex=$CODEX_EXIT gemini=$GEMINI_EXIT)" >> "$RUN/round.log"

# Both children have exited (or been waited on). Clear the PID variables so
# the EXIT trap doesn't try to kill already-dead processes.
CODEX_PID=""
GEMINI_PID=""

# Return non-zero if either failed at the launch level.
# Note: launch success is necessary but not sufficient — parser validation
# in 02.2/02.3 is the authoritative success check.
if [[ "$CODEX_EXIT" != "0" || "$GEMINI_EXIT" != "0" ]]; then
  exit 1
fi
exit 0
