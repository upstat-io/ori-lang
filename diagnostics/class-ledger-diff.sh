#!/bin/bash
# Class-Ledger Differential Harness: compare the class-ledger emitter (toggle
# ON) against the legacy burden path (toggle OFF) over a program corpus and
# emit a cutover worklist.
#
# Usage:
#   diagnostics/class-ledger-diff.sh [options] [corpus-path]
#
# Options:
#   --limit N          Max programs to compare (default: 100)
#   --family GLOB      Filter corpus by shell glob against the relative path
#                      (a plain token matches as a substring)
#   --json[=PATH]      Emit JSON report (default: build/class-ledger-diff.json)
#   --timeout N        Per-step timeout in seconds (default: 30)
#   -v, --verbose      Show every program result, not just divergences
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Both legs build under the gated burden-sole env
# (ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1).
# Leg B adds ORI_CLASS_LEDGER_EMITTER=1 and captures the per-function
# readiness trace from ORI_LOG=ori_arc::aims::class_ledger=debug.
#
# A fallback is NOT a failure — the ranked fallback-reason table IS the
# cutover worklist. A divergence (exit/stdout mismatch between legs) is a
# real cutover blocker.
#
# Environment:
#   ORI_BIN    Override path to the ori binary (must have LLVM support)
#
# Exit codes:
#   0 = No divergences (fallbacks are fine — they are the worklist)
#   1 = Divergences found (including per-program timeouts)
#   2 = Infrastructure error (binary not found, bad arguments)
#   3 = Zero programs compared (misleading "all clear")

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/_common.sh"

# --- Defaults ---
CORPUS_PATH="tests/spec"
LIMIT=100
FAMILY=""
EMIT_JSON=0
JSON_PATH="$ROOT_DIR/build/class-ledger-diff.json"
STEP_TIMEOUT=30
VERBOSE=0
USE_COLOR=auto

# The gated burden-sole env — the sanctioned verdict surface for both legs.
GATED_ENV=(ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1)
TRACE_TARGET="ori_arc::aims::class_ledger=debug"

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --limit) LIMIT="$2"; shift 2 ;;
        --limit=*) LIMIT="${1#--limit=}"; shift ;;
        --family) FAMILY="$2"; shift 2 ;;
        --family=*) FAMILY="${1#--family=}"; shift ;;
        --json) EMIT_JSON=1; shift ;;
        --json=*) EMIT_JSON=1; JSON_PATH="${1#--json=}"; shift ;;
        --timeout) STEP_TIMEOUT="$2"; shift 2 ;;
        --timeout=*) STEP_TIMEOUT="${1#--timeout=}"; shift ;;
        -v|--verbose) VERBOSE=1; shift ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        -*)
            echo "Error: unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
        *)
            if [[ -e "$1" ]]; then
                CORPUS_PATH="$1"
            elif [[ -e "$ROOT_DIR/$1" ]]; then
                CORPUS_PATH="$ROOT_DIR/$1"
            else
                echo "Error: path not found: $1" >&2
                exit 2
            fi
            shift
            ;;
    esac
done

find_ori_bin

# --- Resolve color mode ---
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then USE_COLOR=yes; else USE_COLOR=no; fi
fi
if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_CYAN='\033[0;36m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_YELLOW="" C_CYAN="" C_BOLD="" C_DIM="" C_NC=""
fi

# --- Temp files ---
WORK_DIR=$(mktemp -d)
cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT
RECORDS="$WORK_DIR/records.tsv"
> "$RECORDS"

# --- Collect corpus ---
collect_corpus() {
    local base="$CORPUS_PATH"
    [[ -e "$base" ]] || base="$ROOT_DIR/$CORPUS_PATH"
    if [[ -f "$base" ]]; then
        echo "$base"
        return
    fi
    grep -rl --include='*.ori' '@main' "$base" 2>/dev/null | sort
}

# Family filter: glob when the pattern carries glob metacharacters,
# substring otherwise.
family_match() {
    local rel="$1"
    [[ -z "$FAMILY" ]] && return 0
    if [[ "$FAMILY" == *[\*\?\[]* ]]; then
        # shellcheck disable=SC2254
        case "$rel" in
            $FAMILY) return 0 ;;
            *) return 1 ;;
        esac
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

# --- Counters ---
COMPARED=0
IDENTICAL=0
DIVERGENT=0
BUILD_FAIL_BOTH=0
TIMEOUTS=0
TOTAL_REPLACED=0
TOTAL_FALLBACK=0
declare -A REASON_TOTALS=()
declare -a DIVERGENT_DETAILS=()
TOGGLED_FIXES=0
declare -a TOGGLED_FIX_DETAILS=()

strip_ansi() { sed -e 's/\x1b\[[0-9;]*m//g'; }

# Normalize a run-output file for comparison: heap addresses vary per run
# (ASLR), so two identical crashes must not read as a divergence.
normalize_out() { sed -E 's/0x[0-9a-f]+/0xADDR/g' "$1"; }

# Run one leg: build + execute. Args: <file> <leg> (a|b).
# Writes: $WORK_DIR/$leg.build-exit, $leg.run-exit, $leg.out, $leg.trace (b only).
run_leg() {
    local file="$1" leg="$2"
    local bin="$WORK_DIR/prog-$leg.bin"
    local build_err="$WORK_DIR/$leg.build-err"
    rm -f "$bin"

    local -a env_vars=("${GATED_ENV[@]}")
    if [[ "$leg" == "b" ]]; then
        env_vars+=(ORI_CLASS_LEDGER_EMITTER=1 "ORI_LOG=$TRACE_TARGET")
    else
        env_vars+=(ORI_LOG=off)
    fi

    local build_exit
    timeout -k 5 "$STEP_TIMEOUT" env "${env_vars[@]}" \
        "$ORI" build "$file" -o "$bin" >/dev/null 2>"$build_err" \
        && build_exit=0 || build_exit=$?
    echo "$build_exit" > "$WORK_DIR/$leg.build-exit"

    if [[ "$leg" == "b" ]]; then
        strip_ansi < "$build_err" > "$WORK_DIR/b.trace"
    fi

    if [[ $build_exit -ne 0 ]]; then
        echo "" > "$WORK_DIR/$leg.out"
        echo "$build_exit" > "$WORK_DIR/$leg.run-exit"
        return
    fi

    local run_exit
    timeout -k 5 "$STEP_TIMEOUT" "$bin" > "$WORK_DIR/$leg.out" 2>&1 \
        && run_exit=0 || run_exit=$?
    echo "$run_exit" > "$WORK_DIR/$leg.run-exit"
    rm -f "$bin"
}

# Tally leg B's readiness trace into per-program counts.
# Sets: PROG_REPLACED, PROG_FALLBACK, PROG_REASONS ("reason=count,...").
tally_trace() {
    local trace="$WORK_DIR/b.trace"
    PROG_REPLACED=$(grep -c 'mode="replaced"' "$trace" 2>/dev/null || true)
    PROG_FALLBACK=$(grep -c 'mode="fallback"' "$trace" 2>/dev/null || true)
    : "${PROG_REPLACED:=0}" "${PROG_FALLBACK:=0}"
    PROG_REASONS=""
    while read -r count reason; do
        [[ -z "$reason" ]] && continue
        PROG_REASONS+="${PROG_REASONS:+,}${reason}=${count}"
        REASON_TOTALS[$reason]=$(( ${REASON_TOTALS[$reason]:-0} + count ))
    done < <(sed -n 's/.*fallback_reason="\([^"]\{1,\}\)".*/\1/p' "$trace" | sort | uniq -c)
}

is_timeout_exit() { [[ "$1" -eq 124 || "$1" -eq 137 ]]; }

record() {
    # path, status, exit_a, exit_b, replaced, fallback, reasons, detail
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$@" >> "$RECORDS"
}

echo ""
printf "${C_BOLD}Ori Class-Ledger Differential Harness${C_NC}\n"
printf "${C_DIM}Leg A: legacy burden path | Leg B: ORI_CLASS_LEDGER_EMITTER=1 (both burden-sole gated)${C_NC}\n"
printf "  Binary: %s\n" "$ORI"
printf "  Corpus: %s (%d programs" "$CORPUS_PATH" "${#CORPUS_FILES[@]}"
[[ -n "$FAMILY" ]] && printf ", family=%s" "$FAMILY"
printf ", limit=%d)\n\n" "$LIMIT"

for file in "${CORPUS_FILES[@]}"; do
    rel="${file#$ROOT_DIR/}"

    run_leg "$file" a
    run_leg "$file" b
    tally_trace
    TOTAL_REPLACED=$((TOTAL_REPLACED + PROG_REPLACED))
    TOTAL_FALLBACK=$((TOTAL_FALLBACK + PROG_FALLBACK))

    build_a=$(cat "$WORK_DIR/a.build-exit")
    build_b=$(cat "$WORK_DIR/b.build-exit")
    exit_a=$(cat "$WORK_DIR/a.run-exit")
    exit_b=$(cat "$WORK_DIR/b.run-exit")

    # Timeouts on either leg wedge the comparison — count as divergence.
    if is_timeout_exit "$build_a" || is_timeout_exit "$build_b" \
        || is_timeout_exit "$exit_a" || is_timeout_exit "$exit_b"; then
        ((COMPARED++)); ((DIVERGENT++)); ((TIMEOUTS++))
        detail="timeout (build_a=$build_a build_b=$build_b run_a=$exit_a run_b=$exit_b)"
        DIVERGENT_DETAILS+=("$rel — $detail")
        record "$rel" "timeout" "$exit_a" "$exit_b" "$PROG_REPLACED" "$PROG_FALLBACK" "$PROG_REASONS" "$detail"
        printf "  %s ... ${C_RED}TIMEOUT${C_NC}\n" "$rel"
        continue
    fi

    # Both legs fail the build identically: not a runtime comparison.
    if [[ $build_a -ne 0 && $build_b -ne 0 ]]; then
        ((BUILD_FAIL_BOTH++))
        record "$rel" "build-fail-both" "$build_a" "$build_b" "$PROG_REPLACED" "$PROG_FALLBACK" "$PROG_REASONS" ""
        [[ $VERBOSE -eq 1 ]] && printf "  %s ... ${C_DIM}build fail (both legs)${C_NC}\n" "$rel"
        continue
    fi

    ((COMPARED++))

    # One leg builds, the other does not: a real divergence.
    if [[ $build_a -ne 0 || $build_b -ne 0 ]]; then
        ((DIVERGENT++))
        detail="build divergence: legacy=$build_a toggled=$build_b"
        DIVERGENT_DETAILS+=("$rel — $detail")
        record "$rel" "divergent" "$exit_a" "$exit_b" "$PROG_REPLACED" "$PROG_FALLBACK" "$PROG_REASONS" "$detail"
        printf "  %s ... ${C_RED}DIVERGENT (build)${C_NC}\n" "$rel"
        continue
    fi

    normalize_out "$WORK_DIR/a.out" > "$WORK_DIR/a.norm"
    normalize_out "$WORK_DIR/b.out" > "$WORK_DIR/b.norm"
    if [[ "$exit_a" -eq "$exit_b" ]] && cmp -s "$WORK_DIR/a.norm" "$WORK_DIR/b.norm"; then
        ((IDENTICAL++))
        record "$rel" "identical" "$exit_a" "$exit_b" "$PROG_REPLACED" "$PROG_FALLBACK" "$PROG_REASONS" ""
        [[ $VERBOSE -eq 1 ]] && printf "  %s ... ${C_GREEN}identical${C_NC} ${C_DIM}(replaced=%d fallback=%d)${C_NC}\n" \
            "$rel" "$PROG_REPLACED" "$PROG_FALLBACK"
    elif [[ "$exit_a" -ne 0 && "$exit_b" -eq 0 ]]; then
        # The legacy leg crashes and the toggled leg runs clean: the toggled
        # emitter FIXES a legacy defect on this cell. Reported separately —
        # informative, not a cutover blocker.
        ((TOGGLED_FIXES++))
        detail="legacy=$exit_a toggled=0: legacy=[$(head -c 120 "$WORK_DIR/a.out" | tr '\n\t' '  ')]"
        TOGGLED_FIX_DETAILS+=("$rel — $detail")
        record "$rel" "toggled-fixes-legacy" "$exit_a" "$exit_b" "$PROG_REPLACED" "$PROG_FALLBACK" "$PROG_REASONS" "$detail"
        printf "  %s ... ${C_YELLOW}TOGGLED FIXES LEGACY${C_NC}\n" "$rel"
    else
        ((DIVERGENT++))
        detail="exit legacy=$exit_a toggled=$exit_b"
        if ! cmp -s "$WORK_DIR/a.norm" "$WORK_DIR/b.norm"; then
            detail+="; stdout differs: legacy=[$(head -c 120 "$WORK_DIR/a.out" | tr '\n\t' '  ')] toggled=[$(head -c 120 "$WORK_DIR/b.out" | tr '\n\t' '  ')]"
        fi
        DIVERGENT_DETAILS+=("$rel — $detail")
        record "$rel" "divergent" "$exit_a" "$exit_b" "$PROG_REPLACED" "$PROG_FALLBACK" "$PROG_REASONS" "$detail"
        printf "  %s ... ${C_RED}DIVERGENT${C_NC}\n" "$rel"
    fi
done

# --- Human summary ---
echo ""
printf "${C_BOLD}=== Class-Ledger Differential Summary ===${C_NC}\n\n"
printf "  Programs compared:     %d\n" "$COMPARED"
printf "  ${C_GREEN}Identical${C_NC}:             %d\n" "$IDENTICAL"
if [[ $DIVERGENT -gt 0 ]]; then
    printf "  ${C_RED}${C_BOLD}DIVERGENT${C_NC}:             %d (%d timeout)\n" "$DIVERGENT" "$TIMEOUTS"
else
    printf "  Divergent:             0\n"
fi
[[ $TOGGLED_FIXES -gt 0 ]] && printf "  ${C_YELLOW}Toggled fixes legacy${C_NC}:  %d\n" "$TOGGLED_FIXES"
[[ $BUILD_FAIL_BOTH -gt 0 ]] && printf "  ${C_DIM}Build fail (both)${C_NC}:     %d\n" "$BUILD_FAIL_BOTH"
printf "  Replaced functions:    %d\n" "$TOTAL_REPLACED"
printf "  Fallback functions:    %d\n" "$TOTAL_FALLBACK"

if [[ ${#DIVERGENT_DETAILS[@]} -gt 0 ]]; then
    echo ""
    printf "  ${C_RED}${C_BOLD}Divergences (cutover blockers):${C_NC}\n"
    for d in "${DIVERGENT_DETAILS[@]}"; do
        printf "    %s\n" "$d"
    done
fi

if [[ ${#TOGGLED_FIX_DETAILS[@]} -gt 0 ]]; then
    echo ""
    printf "  ${C_YELLOW}${C_BOLD}Toggled fixes legacy (informative, not blockers):${C_NC}\n"
    for d in "${TOGGLED_FIX_DETAILS[@]}"; do
        printf "    %s\n" "$d"
    done
fi

echo ""
printf "  ${C_BOLD}Cutover worklist — ranked fallback reasons:${C_NC}\n"
if [[ ${#REASON_TOTALS[@]} -eq 0 ]]; then
    printf "    ${C_DIM}(no fallbacks recorded)${C_NC}\n"
else
    for reason in "${!REASON_TOTALS[@]}"; do
        printf "%d\t%s\n" "${REASON_TOTALS[$reason]}" "$reason"
    done | sort -rn | while IFS=$'\t' read -r count reason; do
        printf "    ${C_CYAN}%6d${C_NC}  %s\n" "$count" "$reason"
    done
fi
echo ""

# --- JSON report ---
if [[ $EMIT_JSON -eq 1 ]]; then
    mkdir -p "$(dirname "$JSON_PATH")"
    python3 - "$RECORDS" "$JSON_PATH" "$COMPARED" "$IDENTICAL" "$DIVERGENT" \
        "$BUILD_FAIL_BOTH" "$TIMEOUTS" "$TOTAL_REPLACED" "$TOTAL_FALLBACK" \
        "$TOGGLED_FIXES" <<'PYEOF'
import json, sys, datetime

records_path, json_path = sys.argv[1], sys.argv[2]
compared, identical, divergent, build_fail_both, timeouts, replaced, fallback, toggled_fixes = (
    int(x) for x in sys.argv[3:11]
)

programs = []
ranking: dict[str, int] = {}
with open(records_path) as fh:
    for line in fh:
        line = line.rstrip("\n")
        if not line:
            continue
        path, status, exit_a, exit_b, prog_replaced, prog_fallback, reasons, detail = (
            line.split("\t") + [""] * 8
        )[:8]
        fallbacks = {}
        if reasons:
            for pair in reasons.split(","):
                reason, _, count = pair.partition("=")
                fallbacks[reason] = int(count or 0)
                ranking[reason] = ranking.get(reason, 0) + int(count or 0)
        programs.append({
            "path": path,
            "status": status,
            "identical": status == "identical",
            "exit_a": int(exit_a),
            "exit_b": int(exit_b),
            "replaced": int(prog_replaced),
            "fallback_functions": int(prog_fallback),
            "fallbacks": fallbacks,
            **({"detail": detail} if detail else {}),
        })

report = {
    "timestamp": datetime.datetime.now(datetime.timezone.utc)
        .strftime("%Y-%m-%dT%H:%M:%SZ"),
    "programs": programs,
    "summary": {
        "programs_compared": compared,
        "identical": identical,
        "divergent": divergent,
        "toggled_fixes_legacy": toggled_fixes,
        "build_fail_both": build_fail_both,
        "timeouts": timeouts,
        "replaced_functions": replaced,
        "fallback_functions": fallback,
        "fallback_ranking": [
            {"reason": r, "count": c}
            for r, c in sorted(ranking.items(), key=lambda kv: -kv[1])
        ],
    },
}
with open(json_path, "w") as fh:
    json.dump(report, fh, indent=2)
    fh.write("\n")
print(f"JSON report written to {json_path}")
PYEOF
fi

if [[ $DIVERGENT -gt 0 ]]; then
    exit 1
elif [[ $COMPARED -eq 0 ]]; then
    printf "${C_YELLOW}${C_BOLD}WARNING: zero programs compared${C_NC}\n"
    exit 3
else
    exit 0
fi
