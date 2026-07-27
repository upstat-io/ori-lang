#!/usr/bin/env bash
# class-ledger-census.sh — single-leg readiness census for the class-ledger emitter.
#
# Usage:
#   diagnostics/class-ledger-census.sh [options] [corpus-path]
#
# Builds every corpus program once under the verification env with the
# class-ledger readiness trace enabled, then tallies per-function
# mode="replaced" vs mode="fallback" counts and a ranked fallback_reason
# table — the drain worklist. Optionally runs each binary (plain + leak
# check) for a behavior verdict.
#
# Options:
#   --limit N          Max corpus programs (default 100)
#   --family PAT       Filter corpus by path substring (or shell glob)
#   --run              Also execute each built binary: plain run + ORI_CHECK_LEAKS=1 run
#   --timeout SECS     Per-step timeout (default 30)
#   -v, --verbose      Show every program result
#   -h, --help         This help
#
# Exit codes:
#   0  census completed (fallbacks are the worklist, not failures)
#   1  behavior failures under --run (non-zero plain exit or leak report)
#   2  infrastructure error (binary not found, bad arguments)
#   3  zero programs censused — misleading "all clear"

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
# shellcheck source=diagnostics/_common.sh
source "$SCRIPT_DIR/_common.sh"

LIMIT=100
FAMILY=""
CORPUS_PATH="$ROOT_DIR/tests/spec"
STEP_TIMEOUT=30
DO_RUN=0
VERBOSE=0

GATED_ENV=(ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1)
TRACE_TARGET="ori_arc::aims::class_ledger=debug"

while [[ $# -gt 0 ]]; do
    case $1 in
        --limit) LIMIT="$2"; shift 2 ;;
        --limit=*) LIMIT="${1#--limit=}"; shift ;;
        --family) FAMILY="$2"; shift 2 ;;
        --family=*) FAMILY="${1#--family=}"; shift ;;
        --run) DO_RUN=1; shift ;;
        --timeout) STEP_TIMEOUT="$2"; shift 2 ;;
        --timeout=*) STEP_TIMEOUT="${1#--timeout=}"; shift ;;
        -v|--verbose) VERBOSE=1; shift ;;
        -h|--help) sed -n '2,/^$/{ s/^# \?//; p }' "$0"; exit 0 ;;
        -*) echo "Error: unknown option: $1" >&2; exit 2 ;;
        *)
            if [[ -e "$1" ]]; then CORPUS_PATH="$1"
            elif [[ -e "$ROOT_DIR/$1" ]]; then CORPUS_PATH="$ROOT_DIR/$1"
            else echo "Error: path not found: $1" >&2; exit 2; fi
            shift ;;
    esac
done

find_ori_bin

WORK_DIR=$(mktemp -d)
cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT

# Immutable per-run binary snapshot (a parallel cargo build replacing
# target/ mid-run otherwise turns the corpus tail into phantom build fails).
cp "$ORI" "$WORK_DIR/ori-census"
ORI_DIR=$(dirname "$ORI")
[[ -f "$ORI_DIR/libori_rt.a" ]] && cp "$ORI_DIR/libori_rt.a" "$WORK_DIR/libori_rt.a"
ORI="$WORK_DIR/ori-census"

collect_corpus() {
    local base="$CORPUS_PATH"
    [[ -e "$base" ]] || base="$ROOT_DIR/$CORPUS_PATH"
    if [[ -f "$base" ]]; then echo "$base"; return; fi
    grep -rl --include='*.ori' '@main' "$base" 2>/dev/null | sort
}

family_match() {
    local rel="$1"
    [[ -z "$FAMILY" ]] && return 0
    if [[ "$FAMILY" == *[\*\?\[]* ]]; then
        # shellcheck disable=SC2254
        case "$rel" in $FAMILY) return 0 ;; *) return 1 ;; esac
    else
        [[ "$rel" == *"$FAMILY"* ]]
    fi
}

mapfile -t ALL_FILES < <(collect_corpus)
CORPUS_FILES=()
for f in "${ALL_FILES[@]}"; do
    rel="${f#$ROOT_DIR/}"
    family_match "$rel" || continue
    CORPUS_FILES+=("$f")
    [[ ${#CORPUS_FILES[@]} -ge $LIMIT ]] && break
done

CENSUSED=0
BUILD_FAILS=0
RUN_FAILS=0
TOTAL_REPLACED=0
TOTAL_FALLBACK=0
declare -A REASON_TOTALS=()
declare -a FALLBACK_DETAILS=()
declare -a RUN_FAIL_DETAILS=()

strip_ansi() { sed -e 's/\x1b\[[0-9;]*m//g'; }

echo ""
echo "Ori Class-Ledger Readiness Census (single leg; emitter unconditional)"
printf "  Binary: %s\n" "$ORI"
printf "  Corpus: %s (%d programs" "$CORPUS_PATH" "${#CORPUS_FILES[@]}"
[[ -n "$FAMILY" ]] && printf ", family=%s" "$FAMILY"
printf ", limit=%d)\n\n" "$LIMIT"

for file in "${CORPUS_FILES[@]}"; do
    rel="${file#$ROOT_DIR/}"
    bin="$WORK_DIR/prog.bin"
    trace="$WORK_DIR/trace"
    rm -f "$bin"

    if ! timeout -k 5 "$STEP_TIMEOUT" env "${GATED_ENV[@]}" "ORI_LOG=$TRACE_TARGET" \
        "$ORI" build "$file" -o "$bin" >/dev/null 2>"$WORK_DIR/build-err"; then
        ((BUILD_FAILS++))
        [[ $VERBOSE -eq 1 ]] && printf "  %s ... build fail\n" "$rel"
        continue
    fi
    strip_ansi < "$WORK_DIR/build-err" > "$trace"
    ((CENSUSED++))

    prog_replaced=$(grep -c 'mode="replaced"' "$trace" || true)
    prog_fallback=$(grep -c 'mode="fallback"' "$trace" || true)
    TOTAL_REPLACED=$((TOTAL_REPLACED + prog_replaced))
    TOTAL_FALLBACK=$((TOTAL_FALLBACK + prog_fallback))
    if [[ $prog_fallback -gt 0 ]]; then
        while read -r count reason; do
            [[ -z "$reason" ]] && continue
            REASON_TOTALS[$reason]=$(( ${REASON_TOTALS[$reason]:-0} + count ))
        done < <(sed -n 's/.*fallback_reason="\([^"]\{1,\}\)".*/\1/p' "$trace" | sort | uniq -c)
        while read -r fn reason; do
            FALLBACK_DETAILS+=("$rel — $fn ($reason)")
        done < <(sed -n 's/.*function="\([^"]*\)".*mode="fallback".*fallback_reason="\([^"]*\)".*/\1 \2/p' "$trace")
    fi

    if [[ $DO_RUN -eq 1 ]]; then
        plain_exit=0
        timeout -k 5 "$STEP_TIMEOUT" "$bin" >/dev/null 2>&1 || plain_exit=$?
        leak_out="$WORK_DIR/leak-out"
        leak_exit=0
        timeout -k 5 "$STEP_TIMEOUT" env ORI_CHECK_LEAKS=1 "$bin" >"$leak_out" 2>&1 || leak_exit=$?
        leak_hit=0
        grep -qiE "not freed|leak" "$leak_out" && leak_hit=1
        # Verdict: crash signal (>=124 covers timeout/SIGABRT/SIGSEGV via
        # shell 128+N encoding), a leak report, or plain/leak exit divergence.
        # A program whose @main deliberately returns nonzero exits identically
        # on both legs and is NOT a failure.
        if [[ $plain_exit -ge 124 || $leak_exit -ge 124 || $leak_hit -eq 1 || $plain_exit -ne $leak_exit ]]; then
            ((RUN_FAILS++))
            RUN_FAIL_DETAILS+=("$rel — plain=$plain_exit leak_exit=$leak_exit leak_report=$leak_hit")
            printf "  %s ... RUN FAIL (plain=%s leak=%s report=%s)\n" "$rel" "$plain_exit" "$leak_exit" "$leak_hit"
            continue
        fi
    fi
    [[ $VERBOSE -eq 1 ]] && printf "  %s ... ok (replaced=%s fallback=%s)\n" "$rel" "$prog_replaced" "$prog_fallback"
done

echo ""
printf "Censused: %d | build-fails: %d | functions replaced: %d | fallbacks: %d\n" \
    "$CENSUSED" "$BUILD_FAILS" "$TOTAL_REPLACED" "$TOTAL_FALLBACK"
if [[ ${#REASON_TOTALS[@]} -gt 0 ]]; then
    echo "Fallback reasons (ranked):"
    for reason in "${!REASON_TOTALS[@]}"; do
        printf "  %6d  %s\n" "${REASON_TOTALS[$reason]}" "$reason"
    done | sort -rn
fi
if [[ ${#FALLBACK_DETAILS[@]} -gt 0 ]]; then
    echo "Fallback sites:"
    for d in "${FALLBACK_DETAILS[@]}"; do printf "  %s\n" "$d"; done
fi
if [[ $DO_RUN -eq 1 && ${#RUN_FAIL_DETAILS[@]} -gt 0 ]]; then
    echo "Run failures:"
    for d in "${RUN_FAIL_DETAILS[@]}"; do printf "  %s\n" "$d"; done
fi

[[ $CENSUSED -eq 0 ]] && exit 3
[[ $DO_RUN -eq 1 && $RUN_FAILS -gt 0 ]] && exit 1
exit 0
