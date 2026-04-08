#!/usr/bin/env bash
# check-additive-diff.sh — verify a file's working-tree diff is additive-only.
#
# "Additive-only" means the diff contains ONLY insertions (`+` lines) and
# context (` ` lines) — zero deletions (`-` lines). This is the invariant
# the dual-tpr plan's Section 03.2 enforces when adding Step 0 blocks to
# the existing codex skill files: the mode switch must be added, not
# substituted for existing content. The same invariant applies whenever
# a skill/doc file grows by having a new block grafted on top of the
# existing content instead of replacing it.
#
# This script is the permanent regression guard for that invariant. It
# was surfaced by the per-subsection /improve-tooling retrospective for
# dual-tpr-gemini/section-03.2 — during the subsection, verifying each
# edit was additive required the inline pipeline
#   git diff FILE | grep -c '^-[^-]' || true
# which silently short-circuits in sandboxed bash environments when the
# count is zero (grep exits 1 on zero matches). Wrapping that pipeline
# as a script with explicit counter capture + human-readable output
# makes additive-only verification durable and reusable.
#
# Usage:
#   check-additive-diff.sh PATH [PATH ...]            # check working tree vs HEAD
#   check-additive-diff.sh --staged PATH [PATH ...]   # check index vs HEAD
#   check-additive-diff.sh --vs COMMIT PATH [PATH...] # check working tree vs COMMIT
#
# Exits 0 if every path's diff is additive-only, non-zero otherwise.
# A path with no diff at all passes (zero deletions is still zero deletions).

set -uo pipefail

SCRIPT_NAME="$(basename "$0")"
DIFF_ARGS=()
PATHS=()

usage() {
  cat <<EOF
usage: $SCRIPT_NAME [--staged|--vs COMMIT] PATH [PATH ...]

Verifies the specified path(s) have an additive-only diff (zero deletions).
Designed for verifying that a block of content was grafted onto an existing
file rather than substituted for existing content.

Options:
  --staged        Compare the index against HEAD instead of the working tree.
  --vs COMMIT     Compare the working tree against COMMIT instead of HEAD.
  -h, --help      Show this help.

Exit codes:
  0  all paths are additive-only
  1  one or more paths have deletions
  2  usage error
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --staged)
      DIFF_ARGS+=("--cached")
      shift
      ;;
    --vs)
      if [[ -z "${2:-}" ]]; then
        echo "error: --vs requires a COMMIT argument" >&2
        exit 2
      fi
      DIFF_ARGS+=("$2")
      shift 2
      ;;
    --)
      shift
      PATHS+=("$@")
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      PATHS+=("$1")
      shift
      ;;
  esac
done

if [[ ${#PATHS[@]} -eq 0 ]]; then
  echo "error: at least one PATH is required" >&2
  usage >&2
  exit 2
fi

PASS=0
FAIL=0
FAILED_PATHS=()

echo "=== additive-diff check ==="
for path in "${PATHS[@]}"; do
  if [[ ! -e "$path" ]]; then
    echo "  FAIL: $path (file does not exist)"
    FAIL=$((FAIL + 1))
    FAILED_PATHS+=("$path")
    continue
  fi

  # `git diff [--cached] [COMMIT] -- PATH` gives the unified diff.
  # `grep -c '^-[^-]'` counts deletion lines, excluding the `--- a/path` header.
  # `|| true` prevents `grep -c` from short-circuiting the script when the
  # count is zero (grep exits 1 on no match).
  removed=$(git diff "${DIFF_ARGS[@]}" -- "$path" 2>/dev/null | grep -c '^-[^-]' || true)
  added=$(git diff "${DIFF_ARGS[@]}" -- "$path" 2>/dev/null | grep -c '^+[^+]' || true)

  if [[ "$removed" == "0" ]]; then
    echo "  PASS: $path (additive: +$added -0)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $path (NOT additive: +$added -$removed — $removed line(s) removed)"
    FAIL=$((FAIL + 1))
    FAILED_PATHS+=("$path")
  fi
done

echo ""
echo "=== summary ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
if [[ $FAIL -gt 0 ]]; then
  echo "Non-additive paths:"
  for p in "${FAILED_PATHS[@]}"; do
    echo "  - $p"
  done
  exit 1
fi
exit 0
