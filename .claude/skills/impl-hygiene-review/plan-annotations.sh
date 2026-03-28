#!/usr/bin/env bash
# plan-annotations.sh — Find stale plan annotations in source code for cleanup.
#
# Plan annotations (TPR-04-005, CROSS-04-014, §04.3, Phase A, etc.) are
# allowed as temporary scaffolding during active development, but MUST be
# removed when the plan completes. This script finds cleanup candidates.
#
# By default, it excludes:
#   - Active plan annotations (from plans with status != resolved/complete)
#   - Architecture-internal section numbering (AIMS, eval_v2)
#   - Spec references (Spec: Clause N.M)
#
# Usage:
#   plan-annotations.sh                # stale annotations only (cleanup candidates)
#   plan-annotations.sh --all-raw      # ALL annotations (no filtering)
#   plan-annotations.sh --count        # show counts per file
#   plan-annotations.sh --plan 04      # filter to plan 04 only
#   plan-annotations.sh --help

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

# ─────────────────────────────────────────────────────────────
# The master regex — all ephemeral plan annotation patterns.
#
# Categories:
#   1. Task/finding IDs:   TPR-NN-NNN, CROSS-NN-NNN, BUG-NN-NN
#   2. Section refs:       §NN.N, Section NN.N, section-NN-name
#   3. Phase refs:         Phase A/B/C (letter), Phase 0b/0c (sub-phase)
#   4. Plan file paths:    plans/.../section-...
#
# NOT matched (permanent/legitimate):
#   - "Spec: Clause N.M" — spec references
#   - "Phase Dumps" / "phase dumps" — feature name
#   - "define_phase" — code identifier
#   - Pipeline phase numbers without § (e.g., "Phase 1:" alone)
# ─────────────────────────────────────────────────────────────

# Sub-patterns (PCRE syntax for grep -P)
PLAN_ID='(TPR|CROSS|BUG|FIND|TASK|ISSUE)-\d+-\d+\w*'
SECTION_SYMBOL='§\d+[\d.]*'
SECTION_SPELLED='\bSection\s+\d+[\d.]+'
SECTION_FILE='\bsection-\d+-[a-z]'
PHASE_LETTER='\bPhase\s+[A-C]\b'
PHASE_SUB='\bPhase\s+\d+[a-z]\b'
PLAN_PATH='plans/[a-z_-]+/section-'

MASTER_PATTERN="${PLAN_ID}|${SECTION_SYMBOL}|${SECTION_SPELLED}|${SECTION_FILE}|${PHASE_LETTER}|${PHASE_SUB}|${PLAN_PATH}"

# Directories that use section numbering for INTERNAL architecture docs
# (not plan references). These are excluded from "Section XX" matches.
ARCH_DOC_DIRS=(
    compiler/ori_arc/src/aims       # AIMS pipeline uses Section 01-13 internally
    compiler/ori_canon              # eval_v2 uses Section 02-07 internally
)

# Defaults
INCLUDE_ORI=false
COUNT_MODE=false
PLAN_FILTER=""
RAW_MODE=false

usage() {
    cat <<'EOF'
Usage: plan-annotations.sh [OPTIONS]

Find stale plan annotations in source code for cleanup.

By default, shows only cleanup candidates — excludes active plans and
architecture-internal section numbering.

Options:
  --all-raw     Show ALL annotations (no smart filtering)
  --all         Scan .rs and .ori files (default: .rs only)
  --count       Show match counts per file instead of lines
  --plan NN     Filter to a specific plan number (e.g., 04)
  --pattern     Print the master regex and exit
  --help        Show this help

The master regex catches:
  TPR-04-005    Task/finding IDs (TPR, CROSS, BUG, FIND, TASK, ISSUE)
  §04.3         Section symbol references
  Section 12.13 Spelled-out section references
  section-04-*  Section file references in paths
  Phase A       Letter-phase references (A, B, C)
  Phase 0b      Sub-phase references (0a, 0b, 0c)
  plans/.../    Plan file path references

Excluded by default:
  Spec: Clause 14.3   Spec references (permanent)
  Phase Dumps          Feature/code names, not plan phases
  AIMS Section XX      Architecture-internal (ori_arc/aims/)
  eval_v2 Section XX   Architecture-internal (ori_canon/)
  Active plan refs     From plans with status != resolved/complete
EOF
    exit 0
}

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --all-raw) RAW_MODE=true; shift ;;
        --all)     INCLUDE_ORI=true; shift ;;
        --count)   COUNT_MODE=true; shift ;;
        --plan)
            PLAN_FILTER="$2"
            MASTER_PATTERN="(TPR|CROSS|BUG|FIND|TASK|ISSUE)-${PLAN_FILTER}-\d+\w*|§${PLAN_FILTER}[\d.]*|\bSection\s+${PLAN_FILTER}[\d.]*|\bsection-${PLAN_FILTER}-[a-z]|\bPhase\s+[A-C]\b|\bPhase\s+\d+[a-z]\b|plans/[a-z_-]+/section-${PLAN_FILTER}"
            shift 2
            ;;
        --pattern)
            echo "$MASTER_PATTERN"
            exit 0
            ;;
        --help|-h) usage ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            ;;
    esac
done

cd "$REPO_ROOT"

# Build include patterns
INCLUDE_ARGS=(--include='*.rs')
if $INCLUDE_ORI; then
    INCLUDE_ARGS+=(--include='*.ori')
fi

# Exclude dirs that are never cleanup candidates
EXCLUDE_ARGS=(
    --exclude-dir=plans
    --exclude-dir=docs
    --exclude-dir=.claude
    --exclude-dir=target
    --exclude-dir=.git
)

# ─────────────────────────────────────────────────────────────
# Smart filtering: detect active plans and exclude their annotations
# ─────────────────────────────────────────────────────────────

build_active_plan_excludes() {
    # Find active (non-completed) plans by checking for plan dirs NOT in completed/
    local active_plans=()
    for plan_dir in plans/*/; do
        [[ "$plan_dir" == "plans/completed/" ]] && continue
        [[ "$plan_dir" == "plans/code-journeys/" ]] && continue
        local plan_name
        plan_name=$(basename "$plan_dir")
        active_plans+=("$plan_name")
    done

    # For each active plan, find its section numbers and build exclude patterns
    # Active plan annotations should NOT appear in cleanup results
    local section_nums=()
    for plan in "${active_plans[@]}"; do
        for section_file in "plans/${plan}/section-"*.md; do
            [[ -f "$section_file" ]] || continue
            # Extract section number from filename: section-04-foo.md → 04
            local num
            num=$(basename "$section_file" | sed -n 's/^section-\([0-9]\+\).*/\1/p')
            [[ -n "$num" ]] && section_nums+=("$num")
        done
    done

    # Deduplicate section numbers
    local unique_nums
    unique_nums=$(printf '%s\n' "${section_nums[@]}" | sort -u)

    echo "$unique_nums"
}

run_grep() {
    local pattern="$1"
    local mode="$2"  # "count" or "lines"

    if [[ "$mode" == "count" ]]; then
        grep -rPc "${INCLUDE_ARGS[@]}" "${EXCLUDE_ARGS[@]}" \
            "$pattern" . 2>/dev/null \
            | grep -v ':0$' \
            | sed 's|^\./||' \
            | sort -t: -k2 -rn
    else
        grep -rPn "${INCLUDE_ARGS[@]}" "${EXCLUDE_ARGS[@]}" \
            --color=always \
            "$pattern" . 2>/dev/null \
            | sed 's|^\./||' \
            || true
    fi
}

get_total() {
    local pattern="$1"
    grep -rPc "${INCLUDE_ARGS[@]}" "${EXCLUDE_ARGS[@]}" \
        "$pattern" . 2>/dev/null \
        | awk -F: '{s+=$NF} END {print s+0}'
}

if $RAW_MODE; then
    # Raw mode: show everything, no filtering
    if $COUNT_MODE; then
        run_grep "$MASTER_PATTERN" "count"
    else
        run_grep "$MASTER_PATTERN" "lines"
    fi

    MATCH_COUNT=$(get_total "$MASTER_PATTERN")
    echo ""
    echo "─────────────────────────────────────────────────"
    echo "Total plan annotations (raw): $MATCH_COUNT"
    if [[ -n "$PLAN_FILTER" ]]; then
        echo "Filtered to plan: $PLAN_FILTER"
    fi
    echo "─────────────────────────────────────────────────"
else
    # Smart mode: exclude active plans and architecture-internal docs
    # Strategy: run grep, then post-filter out active plan areas

    # Add architecture-internal dirs to exclude list
    for dir in "${ARCH_DOC_DIRS[@]}"; do
        EXCLUDE_ARGS+=(--exclude-dir="$(basename "$dir")")
    done
    # The above only excludes by basename which is too broad for nested dirs.
    # Instead, we'll post-filter.
    # Reset — we'll do post-filtering via grep -v
    EXCLUDE_ARGS=(
        --exclude-dir=plans
        --exclude-dir=docs
        --exclude-dir=.claude
        --exclude-dir=target
        --exclude-dir=.git
    )

    # Build post-filter to exclude architecture-internal section refs
    # These dirs use "Section XX" for their own architecture docs, not plan refs
    ARCH_FILTER=""
    for dir in "${ARCH_DOC_DIRS[@]}"; do
        if [[ -n "$ARCH_FILTER" ]]; then
            ARCH_FILTER="${ARCH_FILTER}|${dir}"
        else
            ARCH_FILTER="${dir}"
        fi
    done

    # Build post-filter to exclude active plan section numbers
    if [[ -z "$PLAN_FILTER" ]]; then
        ACTIVE_SECTIONS=$(build_active_plan_excludes)
        ACTIVE_FILTER_PARTS=()
        while IFS= read -r num; do
            [[ -z "$num" ]] && continue
            # Exclude TPR-NN-, CROSS-NN-, BUG-NN-, §NN, Section NN from active plans
            ACTIVE_FILTER_PARTS+=("(TPR|CROSS|BUG)-${num}-" "§${num}" "Section ${num}" "section-${num}-" "Phase ")
        done <<< "$ACTIVE_SECTIONS"
    fi

    # Run grep and post-filter
    RAW_OUTPUT=$(grep -rPn "${INCLUDE_ARGS[@]}" "${EXCLUDE_ARGS[@]}" \
        "$MASTER_PATTERN" . 2>/dev/null \
        | sed 's|^\./||' \
        || true)

    if [[ -z "$RAW_OUTPUT" ]]; then
        echo "No plan annotations found."
        echo ""
        echo "─────────────────────────────────────────────────"
        echo "Total stale annotations: 0"
        echo "─────────────────────────────────────────────────"
        exit 0
    fi

    # Post-filter 1: remove spec references (permanent — never cleanup candidates)
    # Lines containing "spec" or "Spec" near a § are spec citations, not plan refs
    FILTERED="$RAW_OUTPUT"
    FILTERED=$(echo "$FILTERED" | grep -Piv '[Ss]pec.*§|§.*[Ss]pec' || true)
    # Also exclude §NN where NN >= 20 (always spec sections — no plan has 20+ sections)
    FILTERED=$(echo "$FILTERED" | grep -Pv '§[2-9]\d[\d.]*' || true)

    # Post-filter 2: remove architecture-internal section refs
    if [[ -n "$ARCH_FILTER" ]]; then
        FILTERED=$(echo "$FILTERED" | grep -Pv "(${ARCH_FILTER}).*\bSection\s+\d+" || true)
    fi

    # Post-filter 3: remove active plan annotations (when not using --plan filter)
    if [[ -z "$PLAN_FILTER" ]] && [[ ${#ACTIVE_FILTER_PARTS[@]} -gt 0 ]]; then
        ACTIVE_NUMS_PATTERN=""
        while IFS= read -r num; do
            [[ -z "$num" ]] && continue
            if [[ -n "$ACTIVE_NUMS_PATTERN" ]]; then
                ACTIVE_NUMS_PATTERN="${ACTIVE_NUMS_PATTERN}|"
            fi
            # Strip leading zeros: "03" → also match "3"
            stripped=$(echo "$num" | sed 's/^0*//')
            [[ -z "$stripped" ]] && stripped="0"
            # Match plan-ID refs and § refs for this section number (with/without leading zeros)
            ACTIVE_NUMS_PATTERN="${ACTIVE_NUMS_PATTERN}(TPR|CROSS|BUG|FIND|TASK|ISSUE)-0*${stripped}-|§0*${stripped}[\d.]*\b|\bSection\s+0*${stripped}[\d.]*\b|\bsection-0*${stripped}-|\bPhase\s+[A-C]\b|\bPhase\s+\d+[a-z]\b"
        done <<< "$ACTIVE_SECTIONS"

        if [[ -n "$ACTIVE_NUMS_PATTERN" ]]; then
            FILTERED=$(echo "$FILTERED" | grep -Pv "$ACTIVE_NUMS_PATTERN" || true)
        fi
    fi

    if [[ -z "$FILTERED" ]]; then
        echo "No stale plan annotations found. (Active plan annotations excluded.)"
        echo ""
        echo "─────────────────────────────────────────────────"
        echo "Total stale annotations: 0"
        echo "─────────────────────────────────────────────────"
        exit 0
    fi

    if $COUNT_MODE; then
        # Recount from filtered output
        echo "$FILTERED" | sed 's/:[0-9]*:.*//' | sort | uniq -c | sort -rn | awk '{print $2 ":" $1}'
    else
        # Add color to filtered output
        echo "$FILTERED" | grep -P --color=always "$MASTER_PATTERN" 2>/dev/null || echo "$FILTERED"
    fi

    MATCH_COUNT=$(echo "$FILTERED" | wc -l)
    echo ""
    echo "─────────────────────────────────────────────────"
    echo "Stale plan annotations: $MATCH_COUNT"
    if [[ -n "$PLAN_FILTER" ]]; then
        echo "Filtered to plan: $PLAN_FILTER"
    fi
    echo "(Use --all-raw to see all annotations including active plans)"
    echo "─────────────────────────────────────────────────"
fi
