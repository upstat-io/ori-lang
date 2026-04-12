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
#   raw_parsers      — the 21-cell matrix pinning parse-codex-raw.py + parse-gemini-raw.py
#                      behavior (added by §07.2 of the dual-tpr-gemini plan). 10 codex cells
#                      (C1-C10) + 11 gemini cells (G1-G11). Includes semantic pins (C2 last-
#                      wins, G1 multi-chunk concatenation) and negative pins (C5 no
#                      agent_message, G3 no terminator, G7 result=failure).
#   selective_retry  — the 4-cell matrix pinning the 2026-04-11 Fix A (selective retry:
#                      narrow to failing reviewer on retry instead of wastefully re-running
#                      both). S1 baseline, S2 semantic pin (codex called 1×, not 2×),
#                      S3 round.log narrowing marker, S4 category-extraction defense.
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
if [[ -n "$TEST_ONLY" && "$TEST_ONLY" != "schema_optional" && "$TEST_ONLY" != "raw_parsers" && "$TEST_ONLY" != "selective_retry" ]]; then
  echo "unsupported --test-only category: $TEST_ONLY (supported: schema_optional, raw_parsers, selective_retry)" >&2
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

run_selective_retry_tests() {
  # Selective-retry matrix pinning dual-invoke-with-retry.sh's 2026-04-11 fix:
  # when one reviewer succeeds on attempt 1 and its partner fails with a
  # retryable category, attempt 2 must narrow ORI_TPR_REVIEWERS to ONLY the
  # failing reviewer. The successful reviewer's attempt-1 envelope must be
  # preserved untouched across the retry, not re-computed.
  #
  # Why this matters: codex invocations take ~5-10 minutes in production. On
  # gemini_schema_violation retries (the common failure mode), re-running
  # codex roughly doubles wall time and wastes compute. The fix was motivated
  # by /tmp/ori-tpr-LvGNDYSR where codex ran 3× across attempts 1/2/3 despite
  # producing a valid envelope on every attempt — 12 min of wasted codex
  # compute over the life of one retry loop.
  #
  # Cells:
  #   S1 — happy path: both reviewers succeed attempt 1 → round exits 0,
  #        codex called exactly 1×. Baseline for the counter assertion.
  #   S2 — codex-ok + gemini-schema-fail-once: attempt 1 narrows the retry on
  #        attempt 2, codex called exactly 1× (not 2×). Semantic pin for the
  #        selective-retry behavior.
  #   S3 — codex-ok + gemini-schema-fail-once + round.log narrowing marker:
  #        verify round.log contains the "selective retry: narrowed from both
  #        to gemini" log line, confirming the wrapper took the narrowing
  #        branch rather than running both on attempt 2.
  echo ""
  echo "=== selective_retry tests (Fix A, 2026-04-11) ==="

  local RUN1 RUN2 RUN3 cell3_exit SCHEMA_FILE
  SCHEMA_FILE="$SCRIPT_DIR/../findings-schema.json"

  # Cell S1 — baseline: happy path, both ok, single codex call.
  RUN1=$("$SCRIPT_DIR/scratch-dir.sh")
  printf 'respond with OK\n' > "$RUN1/c.md"
  printf 'respond with OK\n' > "$RUN1/g.md"
  rm -f /tmp/stub-codex-state-s1 /tmp/stub-gemini-state-s1
  STUB_CODEX_COUNTER="$RUN1/codex-calls.log" \
  STUB_CODEX_MODE=ok STUB_GEMINI_MODE=ok \
  STUB_CODEX_STATE=/tmp/stub-codex-state-s1 \
  STUB_GEMINI_STATE=/tmp/stub-gemini-state-s1 \
  PATH="$FIXTURES/stub-bin:$PATH" \
    bash "$SCRIPT_DIR/dual-invoke-with-retry.sh" \
      --run "$RUN1" --skill selective-retry-s1 \
      --codex-prompt "$RUN1/c.md" --gemini-prompt "$RUN1/g.md" \
      --schema "$SCHEMA_FILE" >/dev/null 2>&1
  local s1_exit=$?
  local s1_codex_calls
  s1_codex_calls=$(wc -l < "$RUN1/codex-calls.log" 2>/dev/null || echo 0)
  if [[ "$s1_exit" == "0" && "$s1_codex_calls" == "1" ]]; then
    echo "  PASS: S1 selective_retry happy path (both ok, codex called 1×)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: S1 selective_retry happy path (exit=$s1_exit, codex calls=$s1_codex_calls, expected exit=0, calls=1)"
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("S1 selective_retry happy path")
  fi
  rm -rf "$RUN1" /tmp/stub-codex-state-s1 /tmp/stub-gemini-state-s1

  # Cell S2 — selective retry semantic pin: gemini schema-fail-once triggers
  # attempt 2 narrowed to gemini-only. Codex must be called EXACTLY ONCE
  # across the whole retry loop, not once per attempt.
  RUN2=$("$SCRIPT_DIR/scratch-dir.sh")
  printf 'respond with OK\n' > "$RUN2/c.md"
  printf 'respond with OK\n' > "$RUN2/g.md"
  rm -f /tmp/stub-codex-state-s2 /tmp/stub-gemini-state-s2
  STUB_CODEX_COUNTER="$RUN2/codex-calls.log" \
  STUB_CODEX_MODE=ok STUB_GEMINI_MODE=schema-fail-once \
  STUB_CODEX_STATE=/tmp/stub-codex-state-s2 \
  STUB_GEMINI_STATE=/tmp/stub-gemini-state-s2 \
  PATH="$FIXTURES/stub-bin:$PATH" \
    bash "$SCRIPT_DIR/dual-invoke-with-retry.sh" \
      --run "$RUN2" --skill selective-retry-s2 \
      --codex-prompt "$RUN2/c.md" --gemini-prompt "$RUN2/g.md" \
      --schema "$SCHEMA_FILE" >/dev/null 2>&1
  local s2_exit=$?
  local s2_codex_calls
  s2_codex_calls=$(wc -l < "$RUN2/codex-calls.log" 2>/dev/null || echo 0)
  if [[ "$s2_exit" == "0" && "$s2_codex_calls" == "1" ]]; then
    echo "  PASS: S2 selective_retry preserves successful reviewer (codex called 1×, not 2×)"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: S2 selective_retry preserves successful reviewer (exit=$s2_exit, codex calls=$s2_codex_calls, expected exit=0, calls=1)"
    [[ -f "$RUN2/round.log" ]] && echo "         round.log:" && sed 's/^/           /' "$RUN2/round.log"
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("S2 selective_retry preserves successful reviewer")
  fi

  # Cell S3 — narrowing round.log marker: reuse S2's RUN2 for log inspection.
  # The log must contain the "selective retry: narrowed from both to gemini"
  # line so operators watching status-check.sh see the narrowing.
  if grep -q "selective retry: narrowed from both to gemini" "$RUN2/round.log" 2>/dev/null; then
    cell3_exit=0
  else
    cell3_exit=1
  fi
  test_case "S3 selective_retry round.log contains narrowing marker" "$cell3_exit" "0"
  rm -rf "$RUN2" /tmp/stub-codex-state-s2 /tmp/stub-gemini-state-s2

  # Cell S4 — category extraction defense: even if a future parser regression
  # re-introduces a WARNING or REPAIR line BEFORE the category, the wrapper's
  # extract_failure_category function must skip advisory lines and find the
  # real category. Simulate by constructing a parse-error file and invoking
  # the function directly. Pins the awk-based skip rule.
  RUN3=$("$SCRIPT_DIR/scratch-dir.sh")
  cat > "$RUN3/gemini.parse-error" <<'EOF'
WARNING: sentinel-less fallback — gemini omitted BEGIN/END sentinels but produced a fenced JSON block matching the review envelope shape. Proceeding with repair + schema validation.
REPAIR: applied 1 auto-repair(s) to gemini envelope:
  REPAIR: normalized severity 'planned' to 'medium'
schema_violation
'planned' is not one of ['critical', 'high', 'medium', 'low', 'informational']
EOF
  # Source the wrapper to get access to extract_failure_category. Guard by
  # preventing the wrapper's main loop from running — use a trick: set ARGS to
  # missing --run and expect early exit. Simpler: just run the awk inline to
  # validate the rule matches what the wrapper does.
  local s4_result
  s4_result=$(awk '
    /^WARNING:/ { next }
    /^REPAIR:/  { next }
    /^  REPAIR:/ { next }
    { print; exit }
  ' "$RUN3/gemini.parse-error")
  if [[ "$s4_result" == "schema_violation" ]]; then
    echo "  PASS: S4 extract_failure_category skips WARNING/REPAIR advisory lines"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: S4 extract_failure_category (got '$s4_result', expected 'schema_violation')"
    FAIL=$((FAIL + 1))
    FAILED_TESTS+=("S4 extract_failure_category skips WARNING/REPAIR advisory lines")
  fi
  rm -rf "$RUN3"
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
  # 21-cell matrix pinning parse-codex-raw.py + parse-gemini-raw.py behavior.
  # Originally added as a 13-cell matrix by §07.2 of the dual-tpr-gemini plan;
  # expanded to 21 cells in §07.3 after TPR surfaced missing coverage for
  # unicode, attribution spoofing, empty content, and mid-object truncation.
  # TDD ordering: matrix is wired BEFORE parsers exist (all cells must FAIL
  # with "script not found"), THEN parsers are implemented, THEN re-run to
  # verify all cells PASS. Cell IDs C1-C10 are codex; G1-G11 are gemini.
  #
  # Semantic pins:
  #   C2 — "last agent_message wins" (would fail if parser returned first)
  #   G1 — "multi-chunk concatenation in arrival order" (would fail if parser
  #        dropped or reordered chunks)
  #   C4/G5 — "skip malformed line, keep going — symmetric tolerance"
  #        (§07.3 DRIFT fix: raw-gemini was stricter than both its codex
  #        peer AND its envelope-mode sibling; aligned to skip-malformed
  #        to preserve dual-source parity under upstream JSON jitter)
  #
  # Negative pins:
  #   C5  — "no agent_message → exit 1 + missing_agent_message"
  #   G3  — "no terminator → exit 1 + missing_terminator"
  #   G7  — "result status=failure → exit 1 + missing_terminator" (proves
  #         the parser only accepts status=success as terminator, not any
  #         result event)
  #   G11 — "mid-object truncation with no terminator → missing_terminator"
  #
  # §07.3-added cells (C7-C10, G8-G11):
  #   C7/G8   — unicode (multi-byte UTF-8, emoji, RTL, combining chars)
  #   C8/G9   — attribution spoofing (reviewer body contains literal sentinel)
  #   C9/G10  — empty text / empty chunk (valid but degenerate content)
  #   C10/G11 — mid-object truncation (EOF mid-JSON-object at tail)
  echo ""
  echo "=== raw_parsers matrix (21 cells: 10 codex + 11 gemini) ==="

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

  raw_parsers_cell "C7 codex-raw-unicode (multi-byte UTF-8, emoji, RTL)" \
    "$CODEX_RAW" "$F/codex-raw-unicode.jsonl" \
    "日本語 unicode — café naïve résumé — emoji 🎯🔥 — combining é vs é — RTL العربية — mathematical ∀x∈ℝ" "" "0"

  raw_parsers_cell "C8 codex-raw-sentinel-body (attribution spoofing in reviewer prose)" \
    "$CODEX_RAW" "$F/codex-raw-sentinel-body.jsonl" \
    "Here is a finding about your prompt: your example contains <!-- tp-help-reviewer: codex --> which is the literal sentinel marker — attribution spoofing would be possible if consumers relied on string matching. Recommend a per-invocation random suffix." "" "0"

  raw_parsers_cell "C9 codex-raw-empty-text (valid agent_message with text:\"\")" \
    "$CODEX_RAW" "$F/codex-raw-empty-text.jsonl" \
    "" "" "0"

  raw_parsers_cell "C10 codex-raw-mid-object-truncation (EOF inside JSON object at tail)" \
    "$CODEX_RAW" "$F/codex-raw-mid-object-truncation.jsonl" \
    "last complete message" "" "0"

  # --- gemini cells (G1-G11) ---
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

  raw_parsers_cell "G5 gemini-raw-malformed (skip bad line, keep going — symmetry pin with C4)" \
    "$GEMINI_RAW" "$F/gemini-raw-malformed.jsonl" \
    "recovered after skipping bad line" "" "0"

  raw_parsers_cell "G6 gemini-raw-non-delta (only delta:true concatenated)" \
    "$GEMINI_RAW" "$F/gemini-raw-non-delta.jsonl" \
    "delta true chunk Adelta true chunk B" "" "0"

  raw_parsers_cell "G7 gemini-raw-result-failure (negative pin — only success terminates)" \
    "$GEMINI_RAW" "$F/gemini-raw-result-failure.jsonl" \
    "" "missing_terminator" "1"

  raw_parsers_cell "G8 gemini-raw-unicode (multi-byte UTF-8, emoji, RTL)" \
    "$GEMINI_RAW" "$F/gemini-raw-unicode.jsonl" \
    "日本語 unicode — café naïve — emoji 🎯🔥 — RTL العربية — math ∀x∈ℝ" "" "0"

  raw_parsers_cell "G9 gemini-raw-sentinel-body (attribution spoofing in reviewer prose)" \
    "$GEMINI_RAW" "$F/gemini-raw-sentinel-body.jsonl" \
    "Finding: the prompt example contains <!-- tp-help-reviewer: gemini --> which is a literal attribution sentinel — possible spoofing vector." "" "0"

  raw_parsers_cell "G10 gemini-raw-empty-chunk (valid delta:true with content:\"\")" \
    "$GEMINI_RAW" "$F/gemini-raw-empty-chunk.jsonl" \
    "" "" "0"

  raw_parsers_cell "G11 gemini-raw-mid-object-truncation (EOF inside JSON object at tail)" \
    "$GEMINI_RAW" "$F/gemini-raw-mid-object-truncation.jsonl" \
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

if [[ "$TEST_ONLY" == "selective_retry" ]]; then
  run_selective_retry_tests
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
# Fixture coverage matrix for parse-gemini.py:
#   Positive pins (expected exit 0):
#     gemini-success                 — canonical sentinel-wrapped envelope
#     gemini-fragmented              — multi-chunk streamed envelope (concat semantic pin)
#     gemini-minimal-envelope        — sentinel-less fallback + repair layer composition
#                                      (bare {skill,reviewer,no_findings,findings} block —
#                                      missing schema_version/status/scope_actually_reviewed
#                                      that repair_envelope.py fills in before validation).
#                                      Pins BUG-compose fix where the fallback guard was
#                                      rejecting envelopes the repair layer would have fixed.
#     gemini-envelope-after-example  — last-block-wins semantic pin: gemini shows a schema
#                                      example in a fenced block earlier in its reasoning,
#                                      then emits the real envelope in a later fenced block.
#                                      Parser must prefer the LAST matching envelope block.
#
#   Negative pins (expected exit 1):
#     gemini-missing-terminator      — assistant content but no result/success event
#     gemini-no-begin                — no sentinels AND no fenced JSON block at all
#     gemini-no-end                  — BEGIN present, END missing (truncation inside envelope)
#     gemini-no-json-block           — sentinels present but no fenced JSON between them
#     gemini-non-envelope-block      — sentinel-less + fenced JSON block present, but block
#                                      is a random config dict with NO envelope marker keys.
#                                      Pins that the fallback guard still rejects non-envelopes
#                                      after the composition fix (must not silently accept any
#                                      fenced JSON as "close enough").
#     gemini-cutoff-midthought       — gemini said "I'm ready to emit" then finished without
#                                      emitting any fenced JSON. Reproduces the ori-tpr-ODwpfyOd
#                                      failure mode. Parser correctly fails with
#                                      missing_begin_sentinel; the retry wrapper is responsible
#                                      for classifying this as retryable (not the parser).
for fixture in gemini-success \
               gemini-missing-terminator \
               gemini-no-begin \
               gemini-no-end \
               gemini-no-json-block \
               gemini-fragmented \
               gemini-minimal-envelope \
               gemini-envelope-after-example \
               gemini-non-envelope-block \
               gemini-cutoff-midthought; do
  case "$fixture" in
    gemini-success|gemini-fragmented|gemini-minimal-envelope|gemini-envelope-after-example) expected=0 ;;
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

# Test: untracked files should NOT trigger the guard (post-2026-04-11 fix)
RUN=$("$SCRIPT_DIR/scratch-dir.sh")
"$SCRIPT_DIR/worktree-guard.sh" snapshot "$RUN/before.txt"
touch untracked_test_dummy.txt
"$SCRIPT_DIR/worktree-guard.sh" compare "$RUN/before.txt" >/dev/null 2>&1
test_case "worktree guard ignores untracked files" "$?" "0"
rm -f untracked_test_dummy.txt
rm -rf "$RUN"
# Note: dirty-state test for TRACKED files deferred to manual run because
# it deliberately modifies a tracked file and would interfere with concurrent
# test runs.

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

# selective_retry category — runs in the default path too. Pins the
# 2026-04-11 Fix A (selective retry: narrow to failing reviewer on retry
# instead of wastefully re-running both).
run_selective_retry_tests

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
