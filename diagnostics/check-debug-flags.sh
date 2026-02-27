#!/bin/bash
# Validate consistency of diagnostic debug flags.
#
# Usage:
#   diagnostics/check-debug-flags.sh [options]
#
# Options:
#   --color        Force color output (default: auto-detect terminal)
#   --no-color     Disable color output
#   -h, --help     Show this help
#
# Checks:
#   1. Every ORI_* flag defined in debug_flags.rs is used somewhere in the codebase
#   2. Every raw std::env::var("ORI_*") check references a flag in debug_flags.rs
#      (excludes runtime-only flags in ori_rt, non-diagnostic flags, and test guards)
#   3. CLAUDE.md documents all diagnostic env vars
#
# Reports: stale flags (defined but unused), orphan checks (used but undefined),
# undocumented flags (defined but not in CLAUDE.md).
#
# Exit codes:
#   0 = all checks pass
#   1 = one or more issues found
#   2 = usage error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DEBUG_FLAGS="$ROOT_DIR/compiler/oric/src/debug_flags.rs"
CLAUDE_MD="$ROOT_DIR/CLAUDE.md"

# --- Defaults ---
USE_COLOR=auto

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        *)
            echo "Error: unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
    esac
done

# --- Resolve color mode ---
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then USE_COLOR=yes; else USE_COLOR=no; fi
fi

if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_BOLD='\033[1m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_YELLOW="" C_BOLD="" C_NC=""
fi

# --- Verify required files exist ---
if [[ ! -f "$DEBUG_FLAGS" ]]; then
    echo "Error: debug_flags.rs not found at $DEBUG_FLAGS" >&2
    exit 2
fi

if [[ ! -f "$CLAUDE_MD" ]]; then
    echo "Error: CLAUDE.md not found at $CLAUDE_MD" >&2
    exit 2
fi

# --- Step 1: Parse defined flags from debug_flags.rs ---
# Flags are defined as: ORI_FLAG_NAME (preceded by doc comments)
mapfile -t DEFINED_FLAGS < <(grep -oP '^\s+ORI_\w+' "$DEBUG_FLAGS" | tr -d ' ')

if [[ ${#DEFINED_FLAGS[@]} -eq 0 ]]; then
    echo "Error: no ORI_* flags found in debug_flags.rs" >&2
    exit 2
fi

printf "${C_BOLD}Checking %d defined flags in debug_flags.rs${C_NC}\n" "${#DEFINED_FLAGS[@]}"

# --- Known exceptions ---
# Flags checked in ori_rt via raw env var (ori_rt can't depend on oric):
#   ORI_TRACE_RC, ORI_RT_DEBUG, ORI_CHECK_LEAKS
# These are documented in debug_flags.rs for consistency but used in ori_rt directly.
RUNTIME_FLAGS=("ORI_TRACE_RC" "ORI_RT_DEBUG" "ORI_CHECK_LEAKS")

# Non-diagnostic env vars (not debug flags, but ORI_* prefixed):
#   ORI_STDLIB — stdlib path override (development)
#   ORI_WORKSPACE_DIR — workspace root for runtime discovery
#   ORI_SYSROOT — system library root override
#   ORI_LOG — tracing filter (handled by tracing crate, not debug_flags)
#   ORI_LOG_TREE — tracing tree mode
# These should NOT be in debug_flags.rs.
NON_DIAGNOSTIC=("ORI_STDLIB" "ORI_WORKSPACE_DIR" "ORI_SYSROOT" "ORI_LOG" "ORI_LOG_TREE")

# Test-only env vars (guard patterns in test files, not production flags):
TEST_ONLY=("ORI_RC_OVERFLOW_TEST" "ORI_RC_TRACE_TEST" "ORI_LEAK_ATTRIB_TEST" "ORI_RC_UNDERFLOW_TEST")

issues=0

# --- Step 2: Check each defined flag is used ---
printf "\n${C_BOLD}1. Stale flags (defined but unused):${C_NC}\n"
stale_count=0

for flag in "${DEFINED_FLAGS[@]}"; do
    # Skip runtime flags (they're used in ori_rt, checked separately)
    skip=0
    for rt in "${RUNTIME_FLAGS[@]}"; do
        if [[ "$flag" == "$rt" ]]; then skip=1; break; fi
    done
    if [[ $skip -eq 1 ]]; then continue; fi

    # Search for usage in compiler/ (excluding debug_flags.rs itself and plans/)
    usage_count=$(grep -r --include='*.rs' "$flag" "$ROOT_DIR/compiler/" \
        | grep -v "debug_flags.rs" \
        | grep -v "/target/" \
        | wc -l)

    if [[ "$usage_count" -eq 0 ]]; then
        printf "  ${C_RED}STALE${C_NC}: %s — defined but never referenced\n" "$flag"
        stale_count=$((stale_count + 1))
        issues=$((issues + 1))
    fi
done

if [[ $stale_count -eq 0 ]]; then
    printf "  ${C_GREEN}OK${C_NC} — all flags are used\n"
fi

# --- Step 3: Check runtime flags are used in ori_rt ---
printf "\n${C_BOLD}2. Runtime flags (used in ori_rt):${C_NC}\n"
for flag in "${RUNTIME_FLAGS[@]}"; do
    usage_count=$(grep -r --include='*.rs' "$flag" "$ROOT_DIR/compiler/ori_rt/src/" \
        | grep -v "/target/" \
        | wc -l)

    if [[ "$usage_count" -eq 0 ]]; then
        printf "  ${C_RED}STALE${C_NC}: %s — defined as runtime flag but not used in ori_rt\n" "$flag"
        issues=$((issues + 1))
    else
        printf "  ${C_GREEN}OK${C_NC}: %s (%d references)\n" "$flag" "$usage_count"
    fi
done

# --- Step 4: Check for orphan env var checks ---
printf "\n${C_BOLD}3. Orphan checks (raw env var, not in debug_flags.rs):${C_NC}\n"
orphan_count=0

# Find all raw ORI_* env var checks in compiler source
mapfile -t RAW_CHECKS < <(
    grep -rnoP 'std::env::var\("(ORI_\w+)"' "$ROOT_DIR/compiler/" \
        --include='*.rs' \
        | grep -v "/target/" \
        | grep -v "debug_flags.rs" \
        | grep -oP 'ORI_\w+' \
        | sort -u
)

for check in "${RAW_CHECKS[@]}"; do
    # Skip non-diagnostic vars
    skip=0
    for nd in "${NON_DIAGNOSTIC[@]}"; do
        if [[ "$check" == "$nd" ]]; then skip=1; break; fi
    done
    if [[ $skip -eq 1 ]]; then continue; fi

    # Skip test-only vars
    for to in "${TEST_ONLY[@]}"; do
        if [[ "$check" == "$to" ]]; then skip=1; break; fi
    done
    if [[ $skip -eq 1 ]]; then continue; fi

    # Check if it's defined in debug_flags.rs
    if ! grep -q "^\s*$check\$" "$DEBUG_FLAGS"; then
        printf "  ${C_YELLOW}ORPHAN${C_NC}: %s — used in source but not defined in debug_flags.rs\n" "$check"
        # Show where it's used
        grep -rn "std::env::var(\"$check\"" "$ROOT_DIR/compiler/" --include='*.rs' \
            | grep -v "/target/" \
            | grep -v "debug_flags.rs" \
            | sed 's|'"$ROOT_DIR/"'||' \
            | while read -r location; do
                printf "    %s\n" "$location"
            done
        orphan_count=$((orphan_count + 1))
        issues=$((issues + 1))
    fi
done

if [[ $orphan_count -eq 0 ]]; then
    printf "  ${C_GREEN}OK${C_NC} — no orphan env var checks\n"
fi

# --- Step 5: Check CLAUDE.md documentation ---
printf "\n${C_BOLD}4. Documentation (CLAUDE.md):${C_NC}\n"
undoc_count=0

for flag in "${DEFINED_FLAGS[@]}"; do
    if ! grep -q "$flag" "$CLAUDE_MD"; then
        printf "  ${C_YELLOW}UNDOC${C_NC}: %s — not mentioned in CLAUDE.md\n" "$flag"
        undoc_count=$((undoc_count + 1))
        issues=$((issues + 1))
    fi
done

if [[ $undoc_count -eq 0 ]]; then
    printf "  ${C_GREEN}OK${C_NC} — all flags documented\n"
fi

# --- Summary ---
printf "\n${C_BOLD}Summary:${C_NC}\n"
printf "  Defined flags: %d\n" "${#DEFINED_FLAGS[@]}"
printf "  Stale: %d | Orphan: %d | Undocumented: %d\n" "$stale_count" "$orphan_count" "$undoc_count"

if [[ $issues -eq 0 ]]; then
    printf "\n${C_GREEN}${C_BOLD}All checks passed.${C_NC}\n"
    exit 0
else
    printf "\n${C_RED}${C_BOLD}%d issue(s) found.${C_NC}\n" "$issues"
    exit 1
fi
