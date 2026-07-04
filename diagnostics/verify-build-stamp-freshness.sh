#!/bin/bash
# Verify oric's build.rs re-executes and refreshes its git-identity stamp
# on an ordinary local rebuild.
#
# Usage:
#   diagnostics/verify-build-stamp-freshness.sh [--no-color]
#
# Options:
#   --no-color   Disable color output
#   -h, --help   Show this help
#
# Method:
#   1. `cargo check -p oric -vv` once to warm the build-script cache.
#   2. `touch` a tracked source file inside compiler/oric/src/ WITHOUT
#      staging it (no `git add`) — Cargo fingerprints build-script inputs
#      from the staged/tracked git tree, so an untracked mtime-only edit
#      is the regression-prone path for a stale rebuild.
#   3. `cargo check -p oric -vv` again; assert TWO things:
#      (a) the log shows the build script actually re-executed (a
#          `Running \`.../build-script-build\`` line), not merely a
#          stale cached rerun;
#      (b) the stamped `output` file's `ORI_GIT_DIRTY` value matches an
#          independently-computed `git status --porcelain` reading taken
#          at the same moment — never a fixed expected value, since an
#          actively-developed checkout may already be dirty from
#          unrelated pending work (a fixed "must be 1" assertion would
#          pass trivially on a dirty checkout regardless of whether the
#          stamp is actually fresh).
#
#   Assertion (a) is the primary, environment-independent regression
#   signal (a bare mtime touch never changes `git status --porcelain`,
#   so it only demonstrates whether Cargo's own fingerprinting reran the
#   script). Assertion (b) pins the stamp to CURRENT reality, not merely
#   re-execution. Never assert on `ORI_BUILD_DATE` — it is HEAD's commit
#   date and does not change on an uncommitted edit.
#
# Exit codes:
#   0 = build script reran AND the stamped ORI_GIT_DIRTY matches reality (fix present)
#   1 = regression: no rerun, or the stamped ORI_GIT_DIRTY diverges from reality
#   2 = environment error (compiler_repo not found, cargo missing, etc.)

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

NO_COLOR=false
for arg in "$@"; do
    case "$arg" in
        --no-color) NO_COLOR=true ;;
        -h|--help)
            sed -n '2,38p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 2
            ;;
    esac
done

if [[ -t 1 ]] && [[ "$NO_COLOR" == "false" ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
else
    RED=''; GREEN=''; NC=''
fi

cd "$ROOT_DIR" || { echo "Error: compiler_repo not found at $ROOT_DIR" >&2; exit 2; }

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found on PATH" >&2
    exit 2
fi

TOUCH_TARGET="compiler/oric/src/main.rs"
if [[ ! -f "$TOUCH_TARGET" ]]; then
    echo "Error: expected tracked file not found: $TOUCH_TARGET" >&2
    exit 2
fi

# Locate the stamped build-script output file for the oric binary crate
# (there are two `oric-*` build dirs — the lib and the bin target; the
# bin target's output carries the full ORI_GIT_* stamp).
find_stamp_output() {
    grep -l '^cargo:rustc-env=ORI_GIT_SHA=' target/debug/build/oric-*/output 2>/dev/null | head -n1
}

# Read the current whole-repo dirty state directly from git, independent
# of any cached build stamp.
current_git_dirty() {
    if [[ -n "$(git status --porcelain)" ]]; then
        echo "1"
    else
        echo "0"
    fi
}

echo "Warming build-script cache (cargo check -p oric -vv)..."
if ! cargo check -p oric -vv >/dev/null 2>&1; then
    echo -e "${RED}Error${NC}: baseline 'cargo check -p oric' failed to build" >&2
    exit 2
fi

echo "Touching $TOUCH_TARGET (untracked edit — no git add)..."
touch "$TOUCH_TARGET"

echo "Rebuilding (cargo check -p oric -vv) to observe rerun behavior..."
SECOND_LOG=$(cargo check -p oric -vv 2>&1)
SECOND_STATUS=$?

if [[ $SECOND_STATUS -ne 0 ]]; then
    echo -e "${RED}Error${NC}: second 'cargo check -p oric' failed to build" >&2
    echo "$SECOND_LOG" >&2
    exit 2
fi

RERAN=false
if echo "$SECOND_LOG" | grep -q 'Running `.*build-script-build'; then
    RERAN=true
fi

STAMP_FILE=$(find_stamp_output)
if [[ -z "$STAMP_FILE" ]]; then
    echo -e "${RED}Error${NC}: no stamped build-script output file found under target/debug/build/oric-*/output" >&2
    exit 2
fi

STAMPED_DIRTY=$(grep '^cargo:rustc-env=ORI_GIT_DIRTY=' "$STAMP_FILE" | tail -n1 | cut -d= -f3)
ACTUAL_DIRTY=$(current_git_dirty)

if [[ "$RERAN" == "true" ]] && [[ "$STAMPED_DIRTY" == "$ACTUAL_DIRTY" ]]; then
    echo -e "${GREEN}PASS${NC}: build script reran after an untracked source edit, and the stamped ORI_GIT_DIRTY=$STAMPED_DIRTY matches the current git state."
    exit 0
else
    echo -e "${RED}FAIL${NC}: freshness regression detected."
    if [[ "$RERAN" == "false" ]]; then
        echo "  - build script did NOT rerun after touching $TOUCH_TARGET without staging." >&2
    fi
    if [[ "$STAMPED_DIRTY" != "$ACTUAL_DIRTY" ]]; then
        echo "  - stamped ORI_GIT_DIRTY=$STAMPED_DIRTY diverges from actual git dirty state ($ACTUAL_DIRTY)." >&2
    fi
    echo "ORI_GIT_SHA/ORI_GIT_DIRTY/ORI_BUILD_DATE froze at the first build's values instead of refreshing." >&2
    exit 1
fi
