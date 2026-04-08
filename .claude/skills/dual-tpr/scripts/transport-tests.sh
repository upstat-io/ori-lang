#!/usr/bin/env bash
# transport-tests.sh — run the dual-tpr transport test suite.
#
# Usage:
#   transport-tests.sh                     # run all unit tests (fast, no real CLI calls)
#   transport-tests.sh --integration       # also run integration tests (invokes real codex/gemini)
#
# Exits 0 if all tests pass, non-zero if any test fails.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FIXTURES="$SCRIPT_DIR/../fixtures"
SCHEMA="$SCRIPT_DIR/../findings-schema.json"

PASS=0
FAIL=0
FAILED_TESTS=()

test_case() {
  local name="$1"
  local actual_exit="$2"
  local expected_exit="$3"
  if [[ "$actual_exit" == "$expected_exit" ]]; then
    echo "  PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $name (expected exit=$expected_exit, got $actual_exit)"
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("$name")
  fi
}

echo "=== validator fixture tests ==="
for fixture in codex-with-findings gemini-with-grounded-citation no-findings; do
  "$SCRIPT_DIR/validate-envelope.py" --envelope "$FIXTURES/$fixture.json" --schema "$SCHEMA" >/dev/null 2>&1
  test_case "validator $fixture" "$?" "0"
done
"$SCRIPT_DIR/validate-envelope.py" --envelope "$FIXTURES/invalid-location.json" --schema "$SCHEMA" >/dev/null 2>&1
test_case "validator rejects invalid-location" "$?" "1"

echo ""
echo "=== codex parser fixture tests ==="
for fixture in codex-success codex-missing codex-parse-fail codex-schema-violation codex-failed-partial; do
  case "$fixture" in
    codex-success) expected=0 ;;
    *) expected=1 ;;
  esac
  "$SCRIPT_DIR/parse-codex.py" --jsonl "$FIXTURES/$fixture.jsonl" --schema "$SCHEMA" >/dev/null 2>&1
  test_case "codex parser $fixture" "$?" "$expected"
done

echo ""
echo "=== gemini parser fixture tests ==="
for fixture in gemini-success gemini-missing-terminator gemini-no-begin gemini-no-end gemini-no-json-block gemini-fragmented; do
  case "$fixture" in
    gemini-success|gemini-fragmented) expected=0 ;;
    *) expected=1 ;;
  esac
  "$SCRIPT_DIR/parse-gemini.py" --jsonl "$FIXTURES/$fixture.jsonl" --schema "$SCHEMA" >/dev/null 2>&1
  test_case "gemini parser $fixture" "$?" "$expected"
done

echo ""
echo "=== worktree guard tests ==="
RUN=$("$SCRIPT_DIR/scratch-dir.sh")
"$SCRIPT_DIR/worktree-guard.sh" snapshot "$RUN/before.txt"
"$SCRIPT_DIR/worktree-guard.sh" compare "$RUN/before.txt" >/dev/null 2>&1
test_case "worktree guard clean state" "$?" "0"
rm -rf "$RUN"
# Note: dirty-state test deferred to manual run because it deliberately
# modifies a tracked file and would interfere with concurrent test runs.

echo ""
echo "=== merger fixture tests ==="
"$SCRIPT_DIR/merge-findings.py" \
  --codex "$FIXTURES/codex-merge-test.json" \
  --gemini "$FIXTURES/gemini-merge-test.json" \
  --section 02 > /tmp/merge-test-out.json 2>&1
test_case "merger basic invocation" "$?" "0"
python3 -c "
import json, sys
d = json.load(open('/tmp/merge-test-out.json'))
assert d['summary']['agreements'] == 1, f'expected 1 agreement, got {d[\"summary\"][\"agreements\"]}'
assert d['summary']['codex_findings'] == 3, f'expected 3 codex findings'
assert d['summary']['gemini_findings'] == 3, f'expected 3 gemini findings'
print('OK')
" >/dev/null 2>&1
test_case "merger summary correctness" "$?" "0"
rm -f /tmp/merge-test-out.json

if [[ "${1:-}" == "--integration" ]]; then
  echo ""
  echo "=== integration tests (invokes real codex/gemini) ==="
  RUN=$("$SCRIPT_DIR/scratch-dir.sh")
  echo "respond with PING" > "$RUN/codex.prompt.md"
  echo "respond with PING" > "$RUN/gemini.prompt.md"
  "$SCRIPT_DIR/dual-invoke.sh" \
    --run "$RUN" --skill review-work \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md" \
    --schema "$SCHEMA" >/dev/null 2>&1
  test_case "dual-invoke smoke test" "$?" "0"
  rm -rf "$RUN"
fi

echo ""
echo "=== summary ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
if [[ $FAIL -gt 0 ]]; then
  echo "Failed tests:"
  for t in "${FAILED_TESTS[@]}"; do
    echo "  - $t"
  done
  exit 1
fi
exit 0
