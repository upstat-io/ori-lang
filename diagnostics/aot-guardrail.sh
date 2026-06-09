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
#   --env "V=1 W=2"    Extra env vars exported into the AOT run (e.g.
#                      "ORI_DISABLE_BURDEN_OPS=1 ORI_DISABLE_PREDICATE_STACK_RC=0"
#                      for the predicate-stack path). Default: burden default.
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

while [[ $# -gt 0 ]]; do
    case $1 in
        --baseline) BASELINE="${2:-}"; shift 2 ;;
        --out) OUT="${2:-}"; shift 2 ;;
        --env) EXTRA_ENV="${2:-}"; shift 2 ;;
        --threads) THREADS="${2:-}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        -h|--help) sed -n '2,/^$/{ s/^# \?//; p }' "$0"; exit 0 ;;
        *) echo "Error: unknown option: $1" >&2; echo "Run with --help." >&2; exit 2 ;;
    esac
done

cd "$REPO_ROOT" || exit 2
RUN_LOG="$(mktemp /tmp/aot-guardrail-run.XXXXXX.log)"

if [[ "$DO_BUILD" == "1" ]]; then
    echo "=== rebuild oric + ori_rt (serialize staticlib) ==="
    cargo build -p oric -p ori_rt 2>&1 | tail -2
    if [[ ! -f target/debug/libori_rt.a ]]; then
        echo "STATICLIB MISSING — abort (rebuild ori_rt before running)" >&2
        exit 3
    fi
    echo "STATICLIB OK ($(stat -c%s target/debug/libori_rt.a) bytes)"
    cargo test -p ori_llvm --test aot --no-run 2>&1 | tail -1
fi

echo "=== AOT suite (env: ${EXTRA_ENV:-<burden default>}; threads $THREADS) ==="
# shellcheck disable=SC2086
env $EXTRA_ENV cargo test -p ori_llvm --test aot -- --test-threads "$THREADS" 2>&1 \
    | tee "$RUN_LOG" | tail -3

grep -E '^test .* \.\.\. FAILED' "$RUN_LOG" \
    | sed -E 's/^test //; s/ \.\.\. FAILED$//' | sort -u > "$OUT"

ABORTS="$(grep -c 'E5005 runtime library not found' "$RUN_LOG")"
COUNT="$(wc -l < "$OUT")"
echo "=== FAIL COUNT: $COUNT | E5005-aborts: $ABORTS | failing-ids: $OUT ==="

if [[ "$ABORTS" -gt 0 ]]; then
    echo "STATICLIB-ABORT FALSE-RED ($ABORTS) — result UNTRUSTABLE; rebuild + re-run." >&2
    exit 3
fi

if [[ -n "$BASELINE" ]]; then
    if [[ ! -f "$BASELINE" ]]; then
        echo "Error: baseline file not found: $BASELINE" >&2
        exit 2
    fi
    NEW="$(comm -13 "$BASELINE" "$OUT")"
    FIXED="$(comm -23 "$BASELINE" "$OUT")"
    echo "=== NEW failures vs baseline (guardrail-2: MUST be empty): ==="
    echo "${NEW:-<none>}"
    echo "=== FIXED vs baseline: ==="
    echo "${FIXED:-<none>}"
    if [[ -n "$NEW" ]]; then
        echo "GUARDRAIL-2 REGRESSION: $(echo "$NEW" | grep -c .) new failure(s)." >&2
        exit 1
    fi
fi
exit 0
