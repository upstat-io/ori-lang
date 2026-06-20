#!/usr/bin/env bash
# TDD pin: test-all.sh emit_json must NOT enumerate per-test failures for a
# suite the harness classified `errored`. An errored suite (nonzero exit + 0
# parsed failures, e.g. the AOT artifact-identity gate) has untrustworthy
# per-test results by definition; its FAIL lines stay in the raw nextest log
# but MUST NOT reach build/test-all-summary.json `failures[]`, where
# per_suite[id].status="errored" + totals.failed already disown them. Emitting
# them contradicts the suite-level ERR verdict that every other emission path
# (the table, per_suite, totals) already honors.
#
# Pins:
#  (a) the status-guarded scrape helper exists (cure shipped);
#  (b) POSITIVE PIN — status `errored` => helper returns `[]` (the invariant);
#  (c) NEGATIVE PIN — status `failed` => helper returns the scraped entries
#      (no over-suppression; a suite that genuinely ran and failed keeps its
#      per-test failures, since the gate suppresses ONLY `errored`);
#  (d) status `passed` => pass-through (the gate suppresses ONLY `errored`);
#  (e) SSOT wiring — all FIVE rust legs route their scrape through the helper;
#  (f) litter-pickup — the failures[] aggregation includes rust_doctest (it is
#      scraped today but dropped from the aggregation loop).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTALL="$HERE/../../test-all.sh"

if [ ! -f "$TESTALL" ]; then
    echo "FAIL: test-all.sh not found at $TESTALL"
    exit 1
fi

# (a) The status-guarded helper must exist (test-all.sh has no BASH_SOURCE
# main-guard, so individual functions are extracted by sed + eval'd).
fn="$(sed -n '/^scrape_failures_unless_errored()/,/^}/p' "$TESTALL")"
if [ -z "$fn" ]; then
    echo "FAIL: scrape_failures_unless_errored() is not defined in test-all.sh (cure unshipped)"
    exit 1
fi
eval "$fn"

# Stub the underlying scraper so the STATUS GATE is the unit under test, not the
# log-to-JSON converter. The stub returns a known non-empty array for any input.
rust_failures_json() {
    printf '%s' '[{"suite":"'"$2"'","test":"t_phantom","kind":"failed"}]'
}

dummy_log="$(mktemp)"; printf 'FAIL stub::t_phantom (nextest)\n' > "$dummy_log"

# (b) POSITIVE PIN — errored => empty array, no per-test entries.
errored_out="$(scrape_failures_unless_errored "$dummy_log" "aot" "errored")"
# (c) NEGATIVE PIN — failed => the scraped entries pass through.
failed_out="$(scrape_failures_unless_errored "$dummy_log" "aot" "failed")"
# (d) passed => pass-through (gate suppresses errored ONLY).
passed_out="$(scrape_failures_unless_errored "$dummy_log" "rust_workspace" "passed")"
rm -f "$dummy_log"

python3 - "$errored_out" "$failed_out" "$passed_out" <<'PY'
import json, sys
errored, failed, passed = sys.argv[1], sys.argv[2], sys.argv[3]
e = json.loads(errored)
assert e == [], f"(b) errored suite must scrape NO failures, got: {e!r}"
f = json.loads(failed)
assert len(f) == 1 and f[0]["suite"] == "aot", f"(c) failed suite must enumerate its failures, got: {f!r}"
p = json.loads(passed)
assert len(p) == 1, f"(d) non-errored (passed) suite must pass through the scrape, got: {p!r}"
print("OK: status-gated scrape — errored=>[] (positive), failed=>enumerated (negative), passed=>pass-through")
PY

# (e) SSOT wiring: every rust leg's scrape in emit_json MUST route through the
# status-guarded helper, never bare rust_failures_json. Extract emit_json and
# assert each leg id appears on a scrape_failures_unless_errored call line.
emit_body="$(sed -n '/^emit_json()/,/^}/p' "$TESTALL")"
if [ -z "$emit_body" ]; then
    echo "FAIL: emit_json() is not defined in test-all.sh"
    exit 1
fi
for leg in rust_workspace rust_rt rust_llvm rust_doctest aot; do
    if ! printf '%s' "$emit_body" | grep -E 'scrape_failures_unless_errored' | grep -q "\"$leg\""; then
        echo "FAIL: emit_json leg '$leg' scrape does not route through scrape_failures_unless_errored (SSOT wiring missing)"
        exit 1
    fi
done
echo "OK: all 5 rust legs route their per-test scrape through scrape_failures_unless_errored"

# (f) Litter-pickup: rust_doctest failures must be aggregated into failures[].
# The doctest scrape result MUST be combined (it is computed today but the
# aggregation loop drops it). Assert the aggregation references a doctest scrape.
if ! printf '%s' "$emit_body" | grep -E 'all_failures|for failures in' | grep -q 'doctest'; then
    # Fall back to asserting the doctest leg's scrape feeds aggregation by id.
    if ! printf '%s' "$emit_body" | grep -A6 'for failures in' | grep -q 'doctest'; then
        echo "FAIL: rust_doctest failures are scraped but not aggregated into failures[] (dropped-leg defect unfixed)"
        exit 1
    fi
fi
echo "OK: rust_doctest failures are aggregated into failures[] (dropped-leg defect fixed)"

# (g) PRODUCTION-PATH PIN (L12): exercise the REAL scrape via the live
# rust_failures_json + json_array_inner on a real nextest FAIL-line fixture
# (NO stub) so the helper's status gate is proven against the production
# log-to-JSON path, not just isolated gate logic.
PARSE_TEST_JSON="$HERE/../../diagnostics/parse_test_json.py"
if [ ! -f "$PARSE_TEST_JSON" ]; then
    echo "FAIL: parse_test_json.py not found at $PARSE_TEST_JSON"
    exit 1
fi
# Restore the REAL rust_failures_json + json_array_inner (the stub above shadowed
# rust_failures_json for the gate-isolation cells); scrape_failures_unless_errored
# resolves them at call time, so it now drives the production scraper.
unset -f rust_failures_json
eval "$(sed -n '/^rust_failures_json()/,/^}/p; /^json_array_inner()/,/^}/p' "$TESTALL")"
nextest_log="$(mktemp)"
printf '    FAIL [   0.038s] (1/1) ori_llvm::aot t_phantom::real_case\n' > "$nextest_log"
real_errored="$(json_array_inner "$(scrape_failures_unless_errored "$nextest_log" "aot" "errored")")"
real_failed="$(json_array_inner "$(scrape_failures_unless_errored "$nextest_log" "aot" "failed")")"
rm -f "$nextest_log"
if [ -n "$real_errored" ]; then
    echo "FAIL: (g) errored leg yielded failures[] entries from the REAL scrape: $real_errored"
    exit 1
fi
if [ -z "$real_failed" ]; then
    echo "FAIL: (g) failed leg yielded NO failures[] entries from the REAL scrape (over-suppressed)"
    exit 1
fi
echo "OK: production-path — real rust_failures_json scrape gated by status (errored=>[], failed=>entry)"
