#!/usr/bin/env bash
# worktree-guard.sh — snapshot or compare git working tree state.
#
# Purpose: detect reviewer-caused modifications to the working tree during a
# dual-source review run. Reviewers (codex, gemini) run under prompt-discipline
# contracts that forbid writing to tracked files; this script's `compare` mode
# exists to catch violations.
#
# CORRECT SEMANTICS (post-2026-04-08 fix): `compare` flags ONLY lines that are
# new in AFTER that weren't in BEFORE. A line that was in BEFORE and is now
# absent in AFTER means the drift was CLEANED UP during the run (e.g., Claude
# committed pre-existing uncommitted edits) — that is NOT a violation and
# MUST NOT trigger the guard.
#
# UNTRACKED FILES ARE NOT VIOLATIONS (post-2026-04-11 fix): lines with `??`
# prefix (untracked files) are filtered OUT of both snapshots before comparison.
# Reviewers (especially gemini) create temp files in the repo root during
# verification (e.g., `test_regex.rs`, `dummy.txt`) — these are NOT codebase
# modifications. The guard only cares about changes to TRACKED files (M, A, D,
# R, C status codes). Untracked files are cleaned up after the run.
#
# Usage:
#   worktree-guard.sh snapshot OUT_FILE
#     Saves `git status --porcelain` to OUT_FILE (tracked files only).
#
#   worktree-guard.sh compare BEFORE_FILE [SAVE_AFTER_FILE]
#     Compares current tracked-file status to BEFORE_FILE, flagging only
#     NEW drift (lines in AFTER not present in BEFORE). Exit 0 if no new drift,
#     exit 1 if new drift detected. On new drift: prints the new lines to
#     stderr. If SAVE_AFTER_FILE is provided, the current snapshot is also
#     saved to that path as a run artifact.

set -euo pipefail

MODE="${1:-}"
shift 2>/dev/null || true

case "$MODE" in
  snapshot)
    OUT="${1:-}"
    if [[ -z "$OUT" ]]; then
      echo "usage: worktree-guard.sh snapshot OUT_FILE" >&2
      exit 2
    fi
    # Filter out untracked files (??) — only track modifications to tracked files
    git status --porcelain | grep -v '^?? ' > "$OUT" || true
    ;;
  compare)
    BEFORE="${1:-}"
    SAVE_AFTER="${2:-}"
    if [[ -z "$BEFORE" ]]; then
      echo "usage: worktree-guard.sh compare BEFORE_FILE [SAVE_AFTER_FILE]" >&2
      exit 2
    fi
    if [[ ! -f "$BEFORE" ]]; then
      echo "worktree-guard: BEFORE_FILE does not exist: $BEFORE" >&2
      exit 2
    fi
    AFTER=$(mktemp)
    trap 'rm -f "$AFTER"' EXIT
    # Filter out untracked files (??) — same as snapshot mode
    git status --porcelain | grep -v '^?? ' > "$AFTER" || true
    if [[ -n "$SAVE_AFTER" ]]; then
      cp "$AFTER" "$SAVE_AFTER"
    fi
    # Flag ONLY new drift: lines in AFTER that weren't in BEFORE. A line
    # removed from BEFORE (cleaned up during the run) is NOT a violation.
    # `comm -13` with sorted inputs yields lines unique to the second file.
    NEW_DRIFT=$(comm -13 <(sort -u "$BEFORE") <(sort -u "$AFTER"))
    if [[ -z "$NEW_DRIFT" ]]; then
      exit 0
    else
      echo "dirty_worktree: reviewer-caused modifications detected during run" >&2
      echo "new lines in AFTER not present in BEFORE:" >&2
      printf '%s\n' "$NEW_DRIFT" >&2
      exit 1
    fi
    ;;
  *)
    echo "usage: worktree-guard.sh snapshot OUT_FILE" >&2
    echo "       worktree-guard.sh compare BEFORE_FILE [SAVE_AFTER_FILE]" >&2
    exit 2
    ;;
esac
