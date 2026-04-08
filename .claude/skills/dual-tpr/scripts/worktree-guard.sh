#!/usr/bin/env bash
# worktree-guard.sh — snapshot or compare git working tree state.
#
# Usage:
#   worktree-guard.sh snapshot OUT_FILE
#     Saves `git status --porcelain` to OUT_FILE
#
#   worktree-guard.sh compare BEFORE_FILE
#     Compares current `git status --porcelain` to BEFORE_FILE.
#     Exit 0 if identical (clean), exit 1 if different (dirty).
#     On dirty: prints the diff to stderr.

set -euo pipefail

MODE="$1"
shift

case "$MODE" in
  snapshot)
    OUT="$1"
    git status --porcelain > "$OUT"
    ;;
  compare)
    BEFORE="$1"
    AFTER=$(mktemp)
    git status --porcelain > "$AFTER"
    if diff -q "$BEFORE" "$AFTER" >/dev/null; then
      rm -f "$AFTER"
      exit 0
    else
      echo "dirty_worktree: tracked files changed during reviewer run" >&2
      diff "$BEFORE" "$AFTER" >&2
      rm -f "$AFTER"
      exit 1
    fi
    ;;
  *)
    echo "usage: worktree-guard.sh snapshot OUT_FILE | compare BEFORE_FILE" >&2
    exit 2
    ;;
esac
