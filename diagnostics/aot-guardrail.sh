#!/bin/bash
# Corpus-wide AOT guardrail-2 runner for the AIMS burden-emission grind.
#
# Usage:
#   diagnostics/aot-guardrail.sh [options]
#
# Options:
#   --baseline FILE    Compare failing-ID set against FILE: prints NEW (comm -13)
#                      and FIXED (comm -23). Exit 1 if NEW is non-empty (a
#                      guardrail-2 regression). No baseline => report count only.
#   --out FILE         Write the sorted failing-ID set here (default:
#                      /tmp/aot-guardrail-fails.txt).
#   --log FILE         Keep the FULL cargo-test run log at FILE (default: a
#                      mktemp path, echoed). The log carries each failing
#                      cell's compile stderr incl. the VF-1 attribution
#                      lines (`[def=.. exit=.. repr=.. ops=..]`) consumed by
#                      burden-imbalance classification.
#   --env "V=1 W=2"    Extra env vars exported into the AOT run (e.g.
#                      "ORI_DISABLE_BURDEN_OPS=1 ORI_DISABLE_PREDICATE_STACK_RC=0"
#                      for the predicate-stack path). Default: burden default.
#
# METRIC CONTRACT: the run ALWAYS exports ORI_VERIFY_ARC=1 + ORI_VERIFY_EACH=1
# (the same verification gates test-all.sh and CI export), so the failing-ID
# set this script captures is the SAME metric the full suite reports. A bare
# `cargo test -p ori_llvm --test aot` without the gates counts only behavioral
# failures and silently excludes every VF-1 burden-imbalance verification
# failure -- an under-count that MUST NOT be used as a floor or baseline.
# Diagnostic-bisection override: --env "ORI_VERIFY_ARC=0 ORI_VERIFY_EACH=0".
#   --floor            Floor-validation preset for the corpus_under_flag_gate
#                      baseline. Prepends ORI_DISABLE_PREDICATE_STACK_RC=1 to the
#                      run env (the burden-path-sole-emitter probe the baseline was
#                      captured under) and, when --baseline is unset, defaults it to
#                      compiler/ori_llvm/tests/aot/fixtures/corpus_under_flag_gate/baseline_failing_ids.txt.
#                      The FIXED set is relabeled "STALE (prune from baseline)": a
#                      baseline cell no longer failing under the gated env is NOT
#                      live floor. THE floor env is
#                      ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1;
#                      a plain `cargo test`/`ori build` runs the DEFAULT path
#                      (predicate stack emits RC) where floor cells PASS (false-green).
#   --threads N        cargo test --test-threads (default 8).
#   --no-build         Skip the oric+ori_rt rebuild + staticlib confirm (caller
#                      already built this cycle).
#   -h, --help         Show this help.
#
# What it does (the dance every burden-emission cycle repeats):
#   1. Rebuild oric + ori_rt; confirm target/debug/libori_rt.a is present
#      (the §B.3 staticlib-abort race: a parallel `cargo build` clobbering the
#      staticlib yields spurious E5005 false-RED — this serializes + verifies).
#   2. Run the AOT suite; capture failing IDs to --out.
#   3. Count E5005 "runtime library not found" aborts; >0 => false-RED, exit 3.
#   4. With --baseline: emit NEW + FIXED via comm; exit 1 on any NEW.
#
# Exit codes:
#   0 = clean run (0 aborts) AND (no baseline OR zero NEW failures)
#   1 = guardrail-2 regression: NEW failures vs baseline
#   2 = usage error
#   3 = staticlib-abort false-RED (re-run after a clean rebuild)

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BASELINE=""
OUT="/tmp/aot-guardrail-fails.txt"
EXTRA_ENV=""
THREADS=8
DO_BUILD=1
RUN_LOG=""
FLOOR=0

while [[ $# -gt 0 ]]; do
    case $1 in
        --baseline) BASELINE="${2:-}"; shift 2 ;;
        --out) OUT="${2:-}"; shift 2 ;;
        --env) EXTRA_ENV="${2:-}"; shift 2 ;;
        --floor) FLOOR=1; shift ;;
        --threads) THREADS="${2:-}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        --log) RUN_LOG="${2:-}"; shift 2 ;;
        -h|--help) sed -n '2,/^$/{ s/^# \?//; p }' "$0"; exit 0 ;;
        *) echo "Error: unknown option: $1" >&2; echo "Run with --help." >&2; exit 2 ;;
    esac
done

# --floor preset: validate against the corpus_under_flag_gate baseline under the
# burden-path-sole-emitter probe env. Prepend the probe flag (explicit --env still
# wins, since --env entries follow these in the `env` invocation) and default the
# baseline to the checked-in corpus floor when unset.
FLOOR_BASELINE="compiler/ori_llvm/tests/aot/fixtures/corpus_under_flag_gate/baseline_failing_ids.txt"
if [[ "$FLOOR" == "1" ]]; then
    EXTRA_ENV="ORI_DISABLE_PREDICATE_STACK_RC=1 ${EXTRA_ENV}"
    [[ -z "$BASELINE" ]] && BASELINE="$FLOOR_BASELINE"
fi

cd "$REPO_ROOT" || exit 2
if [[ -z "$RUN_LOG" ]]; then
    RUN_LOG="$(mktemp /tmp/aot-guardrail-run.XXXXXX.log)"
fi
echo "run log: $RUN_LOG"

# Resolve the cargo target dir so the staticlib confirm works under an isolated
# CARGO_TARGET_DIR (worktree-isolated builds) as well as the default `target/`.
CARGO_TARGET="${CARGO_TARGET_DIR:-target}"
STATICLIB="$CARGO_TARGET/debug/libori_rt.a"

if [[ "$DO_BUILD" == "1" ]]; then
    echo "=== rebuild oric + ori_rt (serialize staticlib) ==="
    cargo build -p oric -p ori_rt 2>&1 | tail -2
    if [[ ! -f "$STATICLIB" ]]; then
        echo "STATICLIB MISSING ($STATICLIB) — abort (rebuild ori_rt before running)" >&2
        exit 3
    fi
    echo "STATICLIB OK ($(stat -c%s "$STATICLIB") bytes)"
    cargo test -p ori_llvm --test aot --no-run 2>&1 | tail -1
fi

# test-all.sh metric parity (see METRIC CONTRACT in the header). --env entries
# follow these in the `env` invocation, so an explicit override still wins.
export ORI_VERIFY_ARC=1
export ORI_VERIFY_EACH=1

echo "=== AOT suite (env: ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1 ${EXTRA_ENV:-<burden default>}; threads $THREADS) ==="
# shellcheck disable=SC2086
env $EXTRA_ENV cargo test -p ori_llvm --test aot -- --test-threads "$THREADS" 2>&1 \
    | tee "$RUN_LOG" | tail -3

grep -E '^test .* \.\.\. FAILED' "$RUN_LOG" \
    | sed -E 's/^test //; s/ \.\.\. FAILED$//' | sort -u > "$OUT"

# Two surface forms of the same staticlib race (a parallel `cargo build`
# clobbering target/debug/libori_rt.a mid-run): E5005 fires when oric cannot
# FIND the lib pre-link; E5006 `cannot find -lori_rt` fires when `cc` cannot.
ABORTS="$(grep -c 'E5005 runtime library not found' "$RUN_LOG")"
LINK_ABORTS="$(grep -c 'cannot find -lori_rt' "$RUN_LOG")"
# Build-failure guard: a cargo/rustc compile failure (e.g. a parallel session
# mid-editing a workspace crate) produces ZERO parseable `... FAILED` IDs, which
# the FAIL COUNT below would read as a FALSE "0 failures" (the corpus never ran,
# it did not pass). Detect the rustc/cargo build-failure markers — distinct from
# Ori test output, which emits `ENNNN` Ori codes, never `error: could not
# compile`. UNTRUSTABLE => exit 3, never a green count.
BUILD_FAILS="$(grep -cE 'error: could not compile|error: build failed|error: aborting due to' "$RUN_LOG")"
COUNT="$(wc -l < "$OUT")"
echo "=== FAIL COUNT: $COUNT | E5005-aborts: $ABORTS | linker-aborts: $LINK_ABORTS | build-fails: $BUILD_FAILS | failing-ids: $OUT ==="

if [[ "$BUILD_FAILS" -gt 0 ]]; then
    echo "BUILD FAILURE in the AOT run ($BUILD_FAILS compile-error marker(s)) — result UNTRUSTABLE; the failing-ID set is empty because the workspace did NOT compile, NOT because the corpus is green. Fix the build (check for a parallel session mid-edit) + re-run." >&2
    exit 3
fi

if [[ "$ABORTS" -gt 0 || "$LINK_ABORTS" -gt 0 ]]; then
    echo "STATICLIB-ABORT FALSE-RED (E5005: $ABORTS, linker: $LINK_ABORTS) — result UNTRUSTABLE; rebuild + re-run." >&2
    exit 3
fi

if [[ -n "$BASELINE" ]]; then
    if [[ ! -f "$BASELINE" ]]; then
        echo "Error: baseline file not found: $BASELINE" >&2
        exit 2
    fi
    # Baseline files MAY carry a `#` doc header; comm needs comment-free
    # sorted input on both sides.
    BASELINE_CLEAN="$(mktemp /tmp/aot-guardrail-baseline.XXXXXX.txt)"
    grep -vE '^(#|$)' "$BASELINE" | sort -u > "$BASELINE_CLEAN"
    NEW="$(comm -13 "$BASELINE_CLEAN" "$OUT")"
    FIXED="$(comm -23 "$BASELINE_CLEAN" "$OUT")"
    rm -f "$BASELINE_CLEAN"
    echo "=== NEW failures vs baseline (guardrail-2: MUST be empty): ==="
    echo "${NEW:-<none>}"
    if [[ "$FLOOR" == "1" ]]; then
        echo "=== STALE baseline entries (prune from baseline — now PASSING under the gated env): ==="
    else
        echo "=== FIXED vs baseline: ==="
    fi
    echo "${FIXED:-<none>}"
    if [[ -n "$NEW" ]]; then
        echo "GUARDRAIL-2 REGRESSION: $(echo "$NEW" | grep -c .) new failure(s)." >&2
        exit 1
    fi
fi
exit 0
