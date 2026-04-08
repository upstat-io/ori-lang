#!/usr/bin/env bash
# transport-tests.sh — run the dual-tpr transport test suite.
#
# Usage:
#   transport-tests.sh                          # run all unit tests (fast, no real CLI calls)
#   transport-tests.sh --integration            # also run integration tests (invokes real codex/gemini)
#   transport-tests.sh --test-only CATEGORY     # run ONLY the named category (skips everything else)
#
# Supported --test-only categories:
#   schema_optional  — the 4-cell matrix pinning dual-invoke.sh's --schema optional contract
#                      (added by §07.0 of the dual-tpr-gemini plan)
#
# Exits 0 if all tests pass, non-zero if any test fails.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FIXTURES="$SCRIPT_DIR/../fixtures"
SCHEMA="$SCRIPT_DIR/../findings-schema.json"

PASS=0
FAIL=0
FAILED_TESTS=()

# Parse args
INTEGRATION=false
TEST_ONLY=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --integration) INTEGRATION=true; shift ;;
    --test-only)   TEST_ONLY="${2:-}"; shift 2 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# Reject unknown --test-only values loudly so typos don't silently run no tests
if [[ -n "$TEST_ONLY" && "$TEST_ONLY" != "schema_optional" ]]; then
  echo "unsupported --test-only category: $TEST_ONLY (supported: schema_optional)" >&2
  exit 2
fi

# test_case helper used by every test category below. Declared before
# run_schema_optional_tests so the --test-only fast path can reach it.
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

run_schema_optional_tests() {
  # 4-cell matrix pinning dual-invoke.sh's --schema optional contract.
  # Added by §07.0 of the dual-tpr-gemini plan after discovering that
  # $SCHEMA is dead code inside dual-invoke.sh (BUG-08-003 removed the
  # --output-schema passthrough to codex). Cell 1 pins backward compat
  # for old callers; Cell 2 pins forward compat for concat-mode callers;
  # Cell 3 pins the round.log content invariant; Cell 4 pins the
  # retry-wrapper contract (dual-invoke-with-retry.sh still passes
  # --schema on behalf of envelope-mode callers).
  echo ""
  echo "=== schema_optional flag-parsing tests ==="

  local RUN1 RUN2 cell3_exit cell4_exit

  # Cell 1: backward compat — --schema present, must still launch.
  RUN1=$("$SCRIPT_DIR/scratch-dir.sh")
  printf 'respond with OK\n' > "$RUN1/c.md"
  printf 'respond with OK\n' > "$RUN1/g.md"
  PATH="$FIXTURES/stub-bin:$PATH" \
    bash "$SCRIPT_DIR/dual-invoke.sh" \
      --run "$RUN1" --skill schema-opt-cell1 \
      --codex-prompt "$RUN1/c.md" --gemini-prompt "$RUN1/g.md" \
      --schema "$SCHEMA" >/dev/null 2>&1
  test_case "schema_optional cell1 (--schema present, backward compat)" "$?" "0"

  # Cell 2: new caller path — --schema OMITTED, must parse and launch.
  RUN2=$("$SCRIPT_DIR/scratch-dir.sh")
  printf 'respond with OK\n' > "$RUN2/c.md"
  printf 'respond with OK\n' > "$RUN2/g.md"
  PATH="$FIXTURES/stub-bin:$PATH" \
    bash "$SCRIPT_DIR/dual-invoke.sh" \
      --run "$RUN2" --skill schema-opt-cell2 \
      --codex-prompt "$RUN2/c.md" --gemini-prompt "$RUN2/g.md" \
      >/dev/null 2>&1
  test_case "schema_optional cell2 (--schema omitted, new behavior)" "$?" "0"

  # Cell 3: round.log invariant — neither launch wrote a --schema reference
  # into round.log. If either does, dead-code drift has crept back in.
  if ! grep -qF -- '--schema' "$RUN1/round.log" 2>/dev/null && \
     ! grep -qF -- '--schema' "$RUN2/round.log" 2>/dev/null; then
    cell3_exit=0
  else
    cell3_exit=1
  fi
  test_case "schema_optional cell3 (round.log has no --schema references)" "$cell3_exit" "0"

  # Cell 4: retry-wrapper contract — dual-invoke-with-retry.sh still passes
  # --schema on behalf of envelope-mode callers. If this grep comes up empty,
  # the wrapper has silently dropped its schema passthrough and envelope-mode
  # reviewers (/tpr-review, /review-work) will lose their schema contract.
  if grep -q -- '--schema' "$SCRIPT_DIR/dual-invoke-with-retry.sh"; then
    cell4_exit=0
  else
    cell4_exit=1
  fi
  test_case "schema_optional cell4 (retry wrapper still passes --schema)" "$cell4_exit" "0"

  rm -rf "$RUN1" "$RUN2"
}

# --test-only fast path: run ONLY the requested category and exit.
if [[ "$TEST_ONLY" == "schema_optional" ]]; then
  run_schema_optional_tests
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
fi

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

echo ""
echo "=== envelope_invariants tests ==="
python3 "$SCRIPT_DIR/test_envelope_invariants.py"
test_case "envelope_invariants regression suite" "$?" "0"

# schema_optional category — runs in the default path too, not just via
# --test-only. Pins the dual-invoke.sh --schema optional contract
# (added by §07.0 of the dual-tpr-gemini plan).
run_schema_optional_tests

if [[ "$INTEGRATION" == "true" ]]; then
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
