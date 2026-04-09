#!/usr/bin/env bash
# lint-command-file.sh — verify the dual-tpr shared command file is reviewer-agnostic.
#
# command-file.md is the canonical, single-source description of the review
# methodology shared by all reviewers (any model, any tool) across all review
# skills (tpr-review, review-work, review-plan, tp-help). It must:
#
#   1. NOT contain any reviewer-specific terms (codex, gemini, --full-auto,
#      --output-schema, google_web_search, plan-write, envelope-only)
#   2. Contain all six load-bearing methodology sections (Scope Resolution,
#      Evidence Gathering, Deep Investigation, Mandatory Standards,
#      Finding Format, Verification Basis)
#
# This script is the permanent regression guard for command-file.md. The
# original one-shot grep checks lived in plans/dual-tpr-gemini/section-03's
# 03.1 subsection. Wrapping them as a script gives them durability — any
# future edit to command-file.md that breaks either invariant fails the lint.
#
# Usage:
#   lint-command-file.sh                    # lint the canonical file
#   lint-command-file.sh path/to/file.md    # lint an alternate file
#
# Exits 0 if all checks pass, non-zero otherwise.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_FILE="$SCRIPT_DIR/../command-file.md"
TARGET="${1:-$DEFAULT_FILE}"

PASS=0
FAIL=0
FAILED_CHECKS=()

check() {
  local name="$1"
  local actual_exit="$2"
  local expected_exit="$3"
  if [[ "$actual_exit" == "$expected_exit" ]]; then
    echo "  PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $name (expected exit=$expected_exit, got $actual_exit)"
    FAIL=$((FAIL + 1))
    FAILED_CHECKS+=("$name")
  fi
}

echo "=== command-file.md lint ==="
echo "target: $TARGET"
echo ""

if [[ ! -f "$TARGET" ]]; then
  echo "  FAIL: target file does not exist: $TARGET"
  echo ""
  echo "=== summary ==="
  echo "PASS: 0"
  echo "FAIL: 1"
  exit 1
fi

echo "=== check 1: no reviewer-specific terms ==="
# Banned terms: anything that names a specific reviewer, model, or tool flag.
# grep -i exits 0 on match (FAIL — banned term present), 1 on no match (PASS).
grep -i 'codex\|gemini\|--full-auto\|--output-schema\|google_web_search\|plan-write\|envelope-only' \
  "$TARGET" >/dev/null 2>&1
check "no reviewer-specific terms" "$?" "1"

echo ""
echo "=== check 2: required methodology sections present ==="
# Six load-bearing concepts. All must appear at least once.
for concept in \
  "Scope Resolution" \
  "Evidence Gathering" \
  "Deep Investigation" \
  "Mandatory Standards" \
  "Finding Format" \
  "Verification Basis"; do
  grep -q "$concept" "$TARGET"
  check "required concept: $concept" "$?" "0"
done

echo ""
echo "=== summary ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
if [[ $FAIL -gt 0 ]]; then
  echo "Failed checks:"
  for c in "${FAILED_CHECKS[@]}"; do
    echo "  - $c"
  done
  exit 1
fi
exit 0
