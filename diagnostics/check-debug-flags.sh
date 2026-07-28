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
# Checks (all derived from debug_flags/ + compiler/ — self-contained to this repo):
#   1. Every ORI_* flag defined under debug_flags/ is used somewhere in the codebase
#   2. Every raw std::env::var("ORI_*") or std::env::var_os("ORI_*") check
#      references a flag defined under debug_flags/
#      (excludes runtime-only flags in ori_rt, non-diagnostic flags, and test guards)
#   3. Every `Consumed in `<module>`` doc claim resolves to a real module that
#      contains at least one of the flag's actual read sites
#
# Reports: stale flags (defined but unused), orphan checks (used but undefined),
#          diverged consumer claims (doc names a module that does not read it).
#
# Options (continued):
#   --self-test    Run the consumer-claim check against synthetic fixtures
#
# Exit codes:
#   0 = all checks pass
#   1 = one or more issues found
#   2 = usage error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DEBUG_FLAGS_DIR="$ROOT_DIR/compiler/oric/src/debug_flags"
COMPILER_DIR="$ROOT_DIR/compiler"

# --- Defaults ---
USE_COLOR=auto
SELF_TEST=no

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        --self-test) SELF_TEST=yes; shift ;;
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

# --- Consumer-claim cross-check (shared by the real run and --self-test) ---
#
# A doc comment may claim `Consumed in `crate::a::b``. The claim is a promise a
# reader relies on when bisecting, and nothing enforces it: the read site can
# move, or the toggle can stop being read entirely, while the claim still reads
# as live. This resolves each claim to a module path and requires at least one
# of the flag's real read sites to live inside it.

# Emit `FLAG<TAB>CLAIM` for every `Consumed in` doc claim under a debug_flags dir.
# Claims accumulate across the doc block and bind to the identifier that closes it.
parse_consumer_claims() {
    local flags_dir="$1" f line claim
    for f in "$flags_dir"/*.rs; do
        [[ -f "$f" ]] || continue
        local -a claims=()
        while IFS= read -r line; do
            if [[ "$line" =~ Consumed[[:space:]]in[[:space:]]\`([^\`]+)\` ]]; then
                claims+=("${BASH_REMATCH[1]}")
            elif [[ "$line" =~ ^[[:space:]]*(ORI_[A-Z0-9_]+)[[:space:]]*,?[[:space:]]*$ ]]; then
                for claim in ${claims+"${claims[@]}"}; do
                    printf '%s\t%s\n' "${BASH_REMATCH[1]}" "$claim"
                done
                claims=()
            elif [[ ! "$line" =~ ^[[:space:]]*/// ]]; then
                claims=()
            fi
        done < "$f"
    done
}

# Resolve `crate::a::b[::fn]` to every source path prefix its items live under,
# one per line. Under the 2018 module layout a module owns BOTH `X.rs` and the
# `X/` directory holding its children, so a claim naming that module is satisfied
# by a read site in either; returning only the first would report a false
# divergence for every child-module read. A trailing segment naming a function has
# no file, so drop segments until one resolves; print nothing when the claim names
# no reachable module.
resolve_claim_scope() {
    local claim="$1" compiler_dir="$2"
    local crate="${claim%%::*}" rest="${claim#*::}"
    [[ "$crate" == "$claim" ]] && return 0
    local base="$compiler_dir/$crate/src"
    local rel="${rest//:://}"
    while [[ -n "$rel" ]]; do
        local found=0
        if [[ -f "$base/$rel.rs" ]]; then printf '%s\n' "$base/$rel.rs"; found=1; fi
        if [[ -d "$base/$rel" ]]; then printf '%s\n' "$base/$rel/"; found=1; fi
        [[ $found -eq 1 ]] && return 0
        [[ "$rel" == */* ]] || break
        rel="${rel%/*}"
    done
    return 0
}

# Print one line per divergence; the caller counts them.
check_consumer_claims() {
    local flags_dir="$1" compiler_dir="$2"
    local flag claim scopes scope sites site matched
    while IFS=$'\t' read -r flag claim; do
        [[ -n "$flag" ]] || continue
        scopes="$(resolve_claim_scope "$claim" "$compiler_dir")"
        if [[ -z "$scopes" ]]; then
            printf 'UNRESOLVED\t%s\t%s\t(claimed module has no source file)\n' "$flag" "$claim"
            continue
        fi
        sites=$(grep -rlP "std::env::var(?:_os)?\(\"$flag\"" "$compiler_dir" \
            --include='*.rs' 2>/dev/null | grep -v '/target/' | grep -v '/debug_flags/' || true)
        if [[ -z "$sites" ]]; then
            printf 'UNREAD\t%s\t%s\t(no std::env::var read site anywhere)\n' "$flag" "$claim"
            continue
        fi
        matched=0
        while IFS= read -r site; do
            [[ -n "$site" ]] || continue
            while IFS= read -r scope; do
                [[ -n "$scope" ]] || continue
                if [[ "$site" == "$scope"* ]]; then matched=1; break 2; fi
            done <<< "$scopes"
        done <<< "$sites"
        if [[ $matched -eq 0 ]]; then
            printf 'DIVERGED\t%s\t%s\t%s\n' "$flag" "$claim" "$(echo "$sites" | tr '\n' ' ')"
        fi
    done < <(parse_consumer_claims "$flags_dir")
}

# --- Self-test: the check must catch a diverged claim, not just pass a good one ---
if [[ "$SELF_TEST" == "yes" ]]; then
    fixture="$(mktemp -d)"
    trap 'rm -rf "$fixture"' EXIT
    mkdir -p "$fixture/flags" "$fixture/compiler/ori_probe/src/good" \
        "$fixture/compiler/ori_probe/src/other" "$fixture/compiler/ori_probe/src/split"
    cat > "$fixture/flags/probe.rs" <<'FIXTURE'
    /// Consumed in `ori_probe::good`.
    ORI_PROBE_MATCHING

    /// Consumed in `ori_probe::good`.
    ORI_PROBE_DIVERGED

    /// Consumed in `ori_probe::good`.
    ORI_PROBE_UNREAD

    /// Consumed in `ori_probe::gone`.
    ORI_PROBE_UNRESOLVED

    /// Consumed in `ori_probe::split`.
    ORI_PROBE_SPLIT_MODULE

    /// Consumed in `ori_probe::good::some_function`.
    ORI_PROBE_FUNCTION_SEGMENT
FIXTURE
    echo 'let a = std::env::var("ORI_PROBE_MATCHING");' > "$fixture/compiler/ori_probe/src/good/mod.rs"
    echo 'let b = std::env::var("ORI_PROBE_DIVERGED");' > "$fixture/compiler/ori_probe/src/other/mod.rs"
    echo 'let c = std::env::var("ORI_PROBE_FUNCTION_SEGMENT");' >> "$fixture/compiler/ori_probe/src/good/mod.rs"
    # 2018 layout: the module root file AND its child directory are one module.
    echo 'mod child;' > "$fixture/compiler/ori_probe/src/split.rs"
    echo 'let d = std::env::var("ORI_PROBE_SPLIT_MODULE");' > "$fixture/compiler/ori_probe/src/split/child.rs"
    st_out="$(check_consumer_claims "$fixture/flags" "$fixture/compiler")"
    st_fail=0
    assert_st() {
        if [[ "$2" == "present" ]]; then
            grep -q "$1" <<< "$st_out" || { echo "  FAIL: expected $1"; st_fail=1; }
        else
            grep -q "$1" <<< "$st_out" && { echo "  FAIL: unexpected $1"; st_fail=1; }
        fi
        return 0
    }
    assert_st "ORI_PROBE_MATCHING" absent
    assert_st "DIVERGED	ORI_PROBE_DIVERGED" present
    assert_st "UNREAD	ORI_PROBE_UNREAD" present
    assert_st "UNRESOLVED	ORI_PROBE_UNRESOLVED" present
    assert_st "ORI_PROBE_SPLIT_MODULE" absent
    assert_st "ORI_PROBE_FUNCTION_SEGMENT" absent
    if [[ $st_fail -eq 0 ]]; then
        printf "${C_GREEN}${C_BOLD}self-test passed${C_NC} (6 fixtures: matching, diverged, unread, unresolved, split-module, function-segment)\n"
        exit 0
    fi
    printf "${C_RED}${C_BOLD}self-test FAILED${C_NC}\n%s\n" "$st_out"
    exit 1
fi

# --- Verify required files exist ---
if [[ ! -d "$DEBUG_FLAGS_DIR" ]]; then
    echo "Error: debug_flags/ not found at $DEBUG_FLAGS_DIR" >&2
    exit 2
fi

# --- Step 1: Parse defined flags from debug_flags/*.rs ---
# Flags are defined as: ORI_FLAG_NAME (preceded by doc comments)
mapfile -t DEFINED_FLAGS < <(grep -rohP '^\s+ORI_\w+' "$DEBUG_FLAGS_DIR" | tr -d ' ' | sort -u)

if [[ ${#DEFINED_FLAGS[@]} -eq 0 ]]; then
    echo "Error: no ORI_* flags found under debug_flags/" >&2
    exit 2
fi

printf "${C_BOLD}Checking %d defined flags in debug_flags/${C_NC}\n" "${#DEFINED_FLAGS[@]}"

# --- Known exceptions ---
# Flags checked in ori_rt via raw env var (ori_rt can't depend on oric):
#   ORI_TRACE_RC, ORI_RT_DEBUG, ORI_CHECK_LEAKS
# These are documented in debug_flags/ for consistency but used in ori_rt directly.
RUNTIME_FLAGS=("ORI_TRACE_RC" "ORI_RT_DEBUG" "ORI_CHECK_LEAKS")

# Non-diagnostic env vars (not debug flags, but ORI_* prefixed):
#   ORI_STDLIB — stdlib path override (development)
#   ORI_WORKSPACE_DIR — workspace root for runtime discovery
#   ORI_SYSROOT — system library root override
#   ORI_LOG — tracing filter (handled by tracing crate, not debug_flags)
#   ORI_LOG_TREE — tracing tree mode
# These should NOT be in debug_flags/.
NON_DIAGNOSTIC=(
    "ORI_STDLIB"
    "ORI_WORKSPACE_DIR"
    "ORI_SYSROOT"
    "ORI_LOG"
    "ORI_LOG_TREE"
)

# Test-only env vars (guard patterns in test files, not production flags):
TEST_ONLY=(
    "ORI_RC_OVERFLOW_TEST"
    "ORI_RC_TRACE_TEST"
    "ORI_LEAK_ATTRIB_TEST"
    "ORI_RC_UNDERFLOW_TEST"
    "ORI_UNCAUGHT_PANIC_BOUNDARY_TEST"
    "ORI_PUSH_TOGGLE_TRACE_CHILD"
    "ORI_RL31_TOGGLE_TRACE_CHILD"
)

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

    # Search for usage in compiler/ (excluding debug_flags/ itself and plans/).
    # A zero-match grep exits 1; under pipefail + set -e the || true keeps a
    # stale flag reportable instead of killing the script at its first hit.
    usage_count=$(grep -r --include='*.rs' "$flag" "$ROOT_DIR/compiler/" \
        | grep -v "/debug_flags/" \
        | grep -v "/target/" \
        | wc -l || true)

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
        | wc -l || true)

    if [[ "$usage_count" -eq 0 ]]; then
        printf "  ${C_RED}STALE${C_NC}: %s — defined as runtime flag but not used in ori_rt\n" "$flag"
        issues=$((issues + 1))
    else
        printf "  ${C_GREEN}OK${C_NC}: %s (%d references)\n" "$flag" "$usage_count"
    fi
done

# --- Step 4: Check for orphan env var checks ---
printf "\n${C_BOLD}3. Orphan checks (raw env var, not in debug_flags/):${C_NC}\n"
orphan_count=0

# Find all raw ORI_* env var checks in compiler source.
# Matches both std::env::var("ORI_*") and std::env::var_os("ORI_*") — a
# var_os-accessed flag is the same SSOT-registration obligation as a
# var-accessed one and must not slip past orphan detection.
mapfile -t RAW_CHECKS < <(
    grep -rnoP 'std::env::var(?:_os)?\("(ORI_\w+)"' "$ROOT_DIR/compiler/" \
        --include='*.rs' \
        | grep -v "/target/" \
        | grep -v "/debug_flags/" \
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

    # Check if it's defined under debug_flags/
    if ! grep -rq "^\s*$check\$" "$DEBUG_FLAGS_DIR"; then
        printf "  ${C_YELLOW}ORPHAN${C_NC}: %s — used in source but not defined in debug_flags/\n" "$check"
        # Show where it's used (both var and var_os access forms)
        grep -rnP "std::env::var(?:_os)?\(\"$check\"" "$ROOT_DIR/compiler/" --include='*.rs' \
            | grep -v "/target/" \
            | grep -v "/debug_flags/" \
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

# --- Step 5: Consumer claims match actual read sites ---
printf "\n${C_BOLD}4. Diverged consumer claims (\`Consumed in\` vs read site):${C_NC}\n"
diverged_count=0
claim_report="$(check_consumer_claims "$DEBUG_FLAGS_DIR" "$COMPILER_DIR")"

if [[ -n "$claim_report" ]]; then
    while IFS=$'\t' read -r kind flag claim actual; do
        [[ -n "$kind" ]] || continue
        printf "  ${C_RED}%s${C_NC}: %s — doc claims \`%s\`\n" "$kind" "$flag" "$claim"
        printf "    actual: %s\n" "${actual# }"
        diverged_count=$((diverged_count + 1))
        issues=$((issues + 1))
    done <<< "$claim_report"
fi

if [[ $diverged_count -eq 0 ]]; then
    printf "  ${C_GREEN}OK${C_NC} — every consumer claim matches a real read site\n"
fi

# --- Summary ---
printf "\n${C_BOLD}Summary:${C_NC}\n"
printf "  Defined flags: %d\n" "${#DEFINED_FLAGS[@]}"
printf "  Stale: %d | Orphan: %d | Diverged claims: %d\n" \
    "$stale_count" "$orphan_count" "$diverged_count"

if [[ $issues -eq 0 ]]; then
    printf "\n${C_GREEN}${C_BOLD}All checks passed.${C_NC}\n"
    exit 0
else
    printf "\n${C_RED}${C_BOLD}%d issue(s) found.${C_NC}\n" "$issues"
    exit 1
fi
