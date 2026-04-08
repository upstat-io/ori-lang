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
#   raw_parsers      — the 13-cell matrix pinning parse-codex-raw.py + parse-gemini-raw.py
#                      behavior (added by §07.2 of the dual-tpr-gemini plan). 6 codex cells
#                      (C1-C6) + 7 gemini cells (G1-G7). Includes semantic pins (C2 last-wins,
#                      G1 multi-chunk concatenation) and negative pins (C5 no agent_message,
#                      G3 no terminator, G7 result=failure).
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
if [[ -n "$TEST_ONLY" && "$TEST_ONLY" != "schema_optional" && "$TEST_ONLY" != "raw_parsers" ]]; then
  echo "unsupported --test-only category: $TEST_ONLY (supported: schema_optional, raw_parsers)" >&2
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

# raw_parsers_cell helper — runs a single raw-mode parser cell and verifies
# stdout, stderr substring, and exit code all match expectations. Added by
# §07.2 of the dual-tpr-gemini plan. Unlike test_case (which only checks
# exit code), raw parsers MUST pin their full output contract: the stdout
# is load-bearing (it's the concatenated reviewer response), and the stderr
# category names (missing_agent_message, missing_terminator, parse_fail,
# missing_assistant_content) are consumed by downstream error classifiers
# in dual-invoke-with-retry.sh. A cell that passes the exit code check
# but drifts on stderr text would silently break retry classification.
raw_parsers_cell() {
  local name="$1" script="$2" fixture="$3"
  local expected_stdout="$4" expected_stderr="$5" expected_rc="$6"
  local got_stdout got_stderr got_rc stderr_tmp
  stderr_tmp=$(mktemp)
  got_stdout=$("$script" --jsonl "$fixture" 2>"$stderr_tmp")
  got_rc=$?
  got_stderr=$(cat "$stderr_tmp")
  rm -f "$stderr_tmp"

  local stdout_ok=1 stderr_ok=1 rc_ok=1
  [[ "$got_stdout" == "$expected_stdout" ]] || stdout_ok=0
  if [[ -n "$expected_stderr" ]]; then
    [[ "$got_stderr" == *"$expected_stderr"* ]] || stderr_ok=0
  fi
  [[ "$got_rc" == "$expected_rc" ]] || rc_ok=0

  if [[ $stdout_ok -eq 1 && $stderr_ok -eq 1 && $rc_ok -eq 1 ]]; then
    echo "  PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $name"
    [[ $stdout_ok -eq 0 ]] && echo "         stdout: expected=$(printf '%q' "$expected_stdout") got=$(printf '%q' "$got_stdout")"
    [[ $stderr_ok -eq 0 ]] && echo "         stderr: expected substring=$(printf '%q' "$expected_stderr") got=$(printf '%q' "$got_stderr")"
    [[ $rc_ok     -eq 0 ]] && echo "         exit  : expected=$expected_rc got=$got_rc"
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("$name")
  fi
}

run_raw_parsers_tests() {
  # 13-cell matrix pinning parse-codex-raw.py + parse-gemini-raw.py behavior.
  # Added by §07.2 of the dual-tpr-gemini plan. TDD ordering: this function
  # is called BEFORE the parsers exist (to verify all 13 cells FAIL with
  # "script not found"), THEN the parsers are implemented, THEN re-run to
  # verify all 13 cells PASS. Cell IDs C1-C6 are codex; G1-G7 are gemini.
  #
  # Semantic pins:
  #   C2 — "last agent_message wins" (would fail if parser returned first)
  #   G1 — "multi-chunk concatenation in arrival order" (would fail if parser
  #        dropped or reordered chunks)
  #
  # Negative pins:
  #   C5 — "no agent_message → exit 1 + missing_agent_message"
  #   G3 — "no terminator → exit 1 + missing_terminator"
  #   G7 — "result status=failure → exit 1 + missing_terminator" (proves
  #        the parser only accepts status=success as terminator, not any
  #        result event)
  echo ""
  echo "=== raw_parsers matrix (13 cells: 6 codex + 7 gemini) ==="

  local CODEX_RAW="$SCRIPT_DIR/parse-codex-raw.py"
  local GEMINI_RAW="$SCRIPT_DIR/parse-gemini-raw.py"
  local F="$SCRIPT_DIR/../fixtures/raw-mode"

  # --- codex cells (C1-C6) ---
  raw_parsers_cell "C1 codex-raw-happy (single agent_message)" \
    "$CODEX_RAW" "$F/codex-raw-happy.jsonl" \
    "hello world" "" "0"

  raw_parsers_cell "C2 codex-raw-multi-message (last wins — semantic pin)" \
    "$CODEX_RAW" "$F/codex-raw-multi-message.jsonl" \
    "third" "" "0"

  raw_parsers_cell "C3 codex-raw-empty (no content)" \
    "$CODEX_RAW" "$F/codex-raw-empty.jsonl" \
    "" "missing_agent_message" "1"

  raw_parsers_cell "C4 codex-raw-malformed (skip bad line, keep going)" \
    "$CODEX_RAW" "$F/codex-raw-malformed.jsonl" \
    "recovered text" "" "0"

  raw_parsers_cell "C5 codex-raw-no-agent-message (negative pin)" \
    "$CODEX_RAW" "$F/codex-raw-no-agent-message.jsonl" \
    "" "missing_agent_message" "1"

  raw_parsers_cell "C6 codex-raw-truncated (tolerate half-line at tail)" \
    "$CODEX_RAW" "$F/codex-raw-truncated.jsonl" \
    "complete text" "" "0"

  # --- gemini cells (G1-G7) ---
  raw_parsers_cell "G1 gemini-raw-happy (concat in order — semantic pin)" \
    "$GEMINI_RAW" "$F/gemini-raw-happy.jsonl" \
    "hello world!" "" "0"

  raw_parsers_cell "G2 gemini-raw-single-chunk" \
    "$GEMINI_RAW" "$F/gemini-raw-single-chunk.jsonl" \
    "complete answer" "" "0"

  raw_parsers_cell "G3 gemini-raw-no-terminator (negative pin)" \
    "$GEMINI_RAW" "$F/gemini-raw-no-terminator.jsonl" \
    "" "missing_terminator" "1"

  raw_parsers_cell "G4 gemini-raw-empty-content (result but no chunks)" \
    "$GEMINI_RAW" "$F/gemini-raw-empty-content.jsonl" \
    "" "missing_assistant_content" "1"

  raw_parsers_cell "G5 gemini-raw-malformed (invalid JSON line)" \
    "$GEMINI_RAW" "$F/gemini-raw-malformed.jsonl" \
    "" "parse_fail" "1"

  raw_parsers_cell "G6 gemini-raw-non-delta (only delta:true concatenated)" \
    "$GEMINI_RAW" "$F/gemini-raw-non-delta.jsonl" \
    "delta true chunk Adelta true chunk B" "" "0"

  raw_parsers_cell "G7 gemini-raw-result-failure (negative pin — only success terminates)" \
    "$GEMINI_RAW" "$F/gemini-raw-result-failure.jsonl" \
    "" "missing_terminator" "1"
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

if [[ "$TEST_ONLY" == "raw_parsers" ]]; then
  run_raw_parsers_tests
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

# raw_parsers category — runs in the default path too, not just via
# --test-only. Pins parse-codex-raw.py + parse-gemini-raw.py behavior
# (added by §07.2 of the dual-tpr-gemini plan).
run_raw_parsers_tests

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
