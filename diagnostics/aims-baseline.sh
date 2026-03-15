#!/usr/bin/env bash
# aims-baseline.sh — Collect AIMS SynergyMetrics baseline measurements
#
# Builds every @main program, captures tracing output,
# and aggregates SynergyMetrics across three categories:
#   1. Golden Corpus (tests/aims/ + tests/aims/synergy/)
#   2. Full Spec Suite (tests/spec/ @main programs)
#   3. Benchmarks (tests/benchmarks/ @main programs)
#
# Usage: diagnostics/aims-baseline.sh [--no-color] [--verbose] [--json]

set -euo pipefail

# --- Colors ---
NO_COLOR=false
VERBOSE=false
JSON_OUTPUT=false
for arg in "$@"; do
    case "$arg" in
        --no-color) NO_COLOR=true ;;
        --verbose) VERBOSE=true ;;
        --json) JSON_OUTPUT=true ;;
        --help|-h)
            echo "Usage: $0 [--no-color] [--verbose] [--json]"
            echo "  --verbose   Show per-file metrics"
            echo "  --json      Output JSON instead of table"
            exit 0
            ;;
    esac
done

if [ "$NO_COLOR" = true ]; then
    BOLD="" DIM="" GREEN="" YELLOW="" RED="" CYAN="" RESET=""
else
    BOLD=$'\e[1m' DIM=$'\e[2m' GREEN=$'\e[32m' YELLOW=$'\e[33m'
    RED=$'\e[31m' CYAN=$'\e[36m' RESET=$'\e[0m'
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_DIR"

# Ensure built with AIMS
echo "${BOLD}Building...${RESET}"
cargo build 2>&1 | tail -1
ORI="target/debug/ori"

# --- Aggregation functions ---
declare -A TOTALS

reset_totals() {
    TOTALS[total_rc]=0
    TOTALS[reuse]=0
    TOTALS[cross_dim_evidence]=0
    TOTALS[cow_upgrades]=0
    TOTALS[total_cow]=0
    TOTALS[cross_dim_reuse]=0
    TOTALS[natural_fip]=0
    TOTALS[canonicalize_cross_fires]=0
    TOTALS[files]=0
    TOTALS[functions]=0
    TOTALS[errors]=0
}

# Strip ANSI escape codes from input
strip_ansi() {
    sed 's/\x1b\[[0-9;]*m//g'
}

# Parse a single tracing line and add to totals
parse_metrics_line() {
    local line
    line=$(echo "$1" | strip_ansi)
    # Extract fields from structured tracing output
    local total_rc reuse cross_dim_ev cow_upgrades total_cow cross_dim_reuse natural_fip canon_fires

    total_rc=$(echo "$line" | grep -oP 'total_rc=\K[0-9]+')
    reuse=$(echo "$line" | grep -oP 'reuse=\K[0-9]+' | head -1)
    cross_dim_ev=$(echo "$line" | grep -oP 'cross_dim_evidence=\K[0-9]+')
    cow_upgrades=$(echo "$line" | grep -oP 'cow_upgrades=\K[0-9]+')
    total_cow=$(echo "$line" | grep -oP 'total_cow=\K[0-9]+')
    cross_dim_reuse=$(echo "$line" | grep -oP 'cross_dim_reuse=\K[0-9]+')
    natural_fip=$(echo "$line" | grep -oP 'natural_fip=\K[0-9]+')
    canon_fires=$(echo "$line" | grep -oP 'canonicalize_cross_fires=\K[0-9]+')

    TOTALS[total_rc]=$(( ${TOTALS[total_rc]} + ${total_rc:-0} ))
    TOTALS[reuse]=$(( ${TOTALS[reuse]} + ${reuse:-0} ))
    TOTALS[cross_dim_evidence]=$(( ${TOTALS[cross_dim_evidence]} + ${cross_dim_ev:-0} ))
    TOTALS[cow_upgrades]=$(( ${TOTALS[cow_upgrades]} + ${cow_upgrades:-0} ))
    TOTALS[total_cow]=$(( ${TOTALS[total_cow]} + ${total_cow:-0} ))
    TOTALS[cross_dim_reuse]=$(( ${TOTALS[cross_dim_reuse]} + ${cross_dim_reuse:-0} ))
    TOTALS[natural_fip]=$(( ${TOTALS[natural_fip]} + ${natural_fip:-0} ))
    TOTALS[canonicalize_cross_fires]=$(( ${TOTALS[canonicalize_cross_fires]} + ${canon_fires:-0} ))
    TOTALS[functions]=$(( ${TOTALS[functions]} + 1 ))
}

# Build a file and collect metrics; returns 0 on success, 1 on build failure
collect_file_metrics() {
    local file="$1"
    local output
    output=$(ORI_LOG=ori_arc::aims::realize::metrics=info "$ORI" build "$file" 2>&1) || {
        TOTALS[errors]=$(( ${TOTALS[errors]} + 1 ))
        if [ "$VERBOSE" = true ]; then
            echo "  ${RED}FAIL${RESET} $file"
        fi
        return 0
    }

    local metrics_lines
    metrics_lines=$(echo "$output" | grep "AIMS synergy metrics" || true)

    if [ -z "$metrics_lines" ]; then
        # No metrics emitted (no functions with RC/COW decisions)
        TOTALS[files]=$(( ${TOTALS[files]} + 1 ))
        if [ "$VERBOSE" = true ]; then
            echo "  ${DIM}skip${RESET} $file (no RC/COW decisions)"
        fi
        return 0
    fi

    TOTALS[files]=$(( ${TOTALS[files]} + 1 ))

    while IFS= read -r line; do
        parse_metrics_line "$line"
    done <<< "$metrics_lines"

    if [ "$VERBOSE" = true ]; then
        local file_fns
        file_fns=$(echo "$metrics_lines" | wc -l)
        echo "  ${GREEN}ok${RESET}   $file ($file_fns functions)"
    fi
}

# Print category results
print_category() {
    local name="$1"
    local reuse_pct="0.0"
    if [ "${TOTALS[total_rc]}" -gt 0 ]; then
        reuse_pct=$(awk "BEGIN { printf \"%.1f\", (${TOTALS[reuse]} / ${TOTALS[total_rc]}) * 100 }")
    fi

    if [ "$JSON_OUTPUT" = true ]; then
        cat <<EOF
    "$name": {
      "files": ${TOTALS[files]},
      "functions": ${TOTALS[functions]},
      "errors": ${TOTALS[errors]},
      "total_rc_decisions": ${TOTALS[total_rc]},
      "reuse_decisions": ${TOTALS[reuse]},
      "reuse_percent": $reuse_pct,
      "cross_dim_evidence": ${TOTALS[cross_dim_evidence]},
      "cow_upgrades": ${TOTALS[cow_upgrades]},
      "total_cow_decisions": ${TOTALS[total_cow]},
      "cross_dim_reuse": ${TOTALS[cross_dim_reuse]},
      "natural_fip": ${TOTALS[natural_fip]},
      "canonicalize_cross_fires": ${TOTALS[canonicalize_cross_fires]}
    }
EOF
    else
        printf "  %-28s %s\n" "Files:" "${TOTALS[files]} (${TOTALS[errors]} errors)"
        printf "  %-28s %s\n" "Functions analyzed:" "${TOTALS[functions]}"
        printf "  %-28s %s\n" "Reuse %:" "${reuse_pct}% (${TOTALS[reuse]}/${TOTALS[total_rc]})"
        printf "  %-28s %s\n" "Cross-dim evidence:" "${TOTALS[cross_dim_evidence]}"
        printf "  %-28s %s\n" "COW upgrades:" "${TOTALS[cow_upgrades]} (of ${TOTALS[total_cow]} total)"
        printf "  %-28s %s\n" "Cross-dim reuse:" "${TOTALS[cross_dim_reuse]}"
        printf "  %-28s %s\n" "Natural FIP:" "${TOTALS[natural_fip]}"
        printf "  %-28s %s\n" "Canonicalize cross-fires:" "${TOTALS[canonicalize_cross_fires]}"
    fi
}

# --- Category 1: Golden Corpus ---
echo ""
echo "${BOLD}${CYAN}=== Golden Corpus (tests/aims/) ===${RESET}"
reset_totals

for f in tests/aims/*.ori tests/aims/synergy/*.ori; do
    [ -f "$f" ] || continue
    grep -q "@main" "$f" || continue
    collect_file_metrics "$f"
done

echo ""
print_category "golden_corpus"
# Save for JSON
GC_FILES=${TOTALS[files]} GC_FUNCS=${TOTALS[functions]} GC_ERRORS=${TOTALS[errors]}
GC_TOTAL_RC=${TOTALS[total_rc]} GC_REUSE_DEC=${TOTALS[reuse]} GC_CROSS_DIM=${TOTALS[cross_dim_evidence]}
GC_COW_UP=${TOTALS[cow_upgrades]} GC_TOTAL_COW=${TOTALS[total_cow]}
GC_REUSE=${TOTALS[cross_dim_reuse]} GC_FIP=${TOTALS[natural_fip]}
GC_CANON=${TOTALS[canonicalize_cross_fires]}

# --- Category 2: Full Spec Suite ---
echo ""
echo "${BOLD}${CYAN}=== Full Spec Suite (tests/spec/ @main programs) ===${RESET}"
reset_totals

while IFS= read -r f; do
    collect_file_metrics "$f"
done < <(grep -rl "@main" tests/spec/ 2>/dev/null || true)

echo ""
print_category "full_spec"
FS_FILES=${TOTALS[files]} FS_FUNCS=${TOTALS[functions]} FS_ERRORS=${TOTALS[errors]}
FS_TOTAL_RC=${TOTALS[total_rc]} FS_REUSE_DEC=${TOTALS[reuse]} FS_CROSS_DIM=${TOTALS[cross_dim_evidence]}
FS_COW_UP=${TOTALS[cow_upgrades]} FS_TOTAL_COW=${TOTALS[total_cow]}
FS_REUSE=${TOTALS[cross_dim_reuse]} FS_FIP=${TOTALS[natural_fip]}
FS_CANON=${TOTALS[canonicalize_cross_fires]}

# --- Category 3: Benchmarks ---
echo ""
echo "${BOLD}${CYAN}=== Benchmarks (tests/benchmarks/) ===${RESET}"
reset_totals

while IFS= read -r f; do
    collect_file_metrics "$f"
done < <(grep -rl "@main" tests/benchmarks/ 2>/dev/null || true)

echo ""
print_category "benchmarks"

# --- Summary Table ---
if [ "$JSON_OUTPUT" != true ]; then
    echo ""
    echo "${BOLD}${CYAN}=== Summary Table ===${RESET}"
    echo ""
    printf "%-26s  %-16s  %-16s  %-16s\n" "Metric" "Golden Corpus" "Full Spec" "Benchmarks"
    printf "%-26s  %-16s  %-16s  %-16s\n" "--------------------------" "----------------" "----------------" "----------------"

    gc_rpct="0.0"; [ "$GC_TOTAL_RC" -gt 0 ] && gc_rpct=$(awk "BEGIN { printf \"%.1f\", ($GC_REUSE_DEC / $GC_TOTAL_RC) * 100 }")
    fs_rpct="0.0"; [ "$FS_TOTAL_RC" -gt 0 ] && fs_rpct=$(awk "BEGIN { printf \"%.1f\", ($FS_REUSE_DEC / $FS_TOTAL_RC) * 100 }")
    bm_rpct="0.0"; [ "${TOTALS[total_rc]}" -gt 0 ] && bm_rpct=$(awk "BEGIN { printf \"%.1f\", (${TOTALS[reuse]} / ${TOTALS[total_rc]}) * 100 }")

    printf "%-26s  %-16s  %-16s  %-16s\n" "Reuse %" "${gc_rpct}%" "${fs_rpct}%" "${bm_rpct}%"
    printf "%-26s  %-16s  %-16s  %-16s\n" "Cross-dim evidence" "$GC_CROSS_DIM" "$FS_CROSS_DIM" "${TOTALS[cross_dim_evidence]}"
    printf "%-26s  %-16s  %-16s  %-16s\n" "COW upgrades" "$GC_COW_UP" "$FS_COW_UP" "${TOTALS[cow_upgrades]}"
    printf "%-26s  %-16s  %-16s  %-16s\n" "Cross-dim reuse" "$GC_REUSE" "$FS_REUSE" "${TOTALS[cross_dim_reuse]}"
    printf "%-26s  %-16s  %-16s  %-16s\n" "Natural FIP" "$GC_FIP" "$FS_FIP" "${TOTALS[natural_fip]}"
    printf "%-26s  %-16s  %-16s  %-16s\n" "Canon. cross-fires" "$GC_CANON" "$FS_CANON" "${TOTALS[canonicalize_cross_fires]}"
    echo ""
    echo "${DIM}Date: $(date -u +%Y-%m-%dT%H:%M:%SZ) | Commit: $(git rev-parse --short HEAD)${RESET}"
fi

# Clean up build artifacts
rm -f tests/aims/*.bin tests/aims/synergy/*.bin tests/spec/**/*.bin tests/benchmarks/**/*.bin 2>/dev/null || true
find tests/ -name "*.bin" -delete 2>/dev/null || true
