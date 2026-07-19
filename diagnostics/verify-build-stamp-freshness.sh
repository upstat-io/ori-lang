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
# Method — TWO checks, one gating, one informational:
#
#   PRIMARY (gating): static source check. `compiler/oric/build.rs` must
#   emit NO `cargo:rerun-if-changed=` / `cargo:rerun-if-env-changed=` line.
#   Per Cargo's documented rerun-if semantics, emitting ANY such line
#   disables Cargo's default "rerun if any file in the package changed"
#   fallback; the ABSENCE of any such line IS the fix. This check is fully
#   deterministic and environment-independent.
#
#   A prior live-subprocess-only design was replaced by this static check
#   because the live probe produced a false PASS on the pre-fix build.rs:
#   build.rs's own `git status --porcelain` call can rewrite `.git/index`'s
#   mtime as a stat-cache-refresh side effect, and a narrowly-gated
#   `rerun-if-changed=<.git/index>` line then treats that self-inflicted
#   change as a genuine rerun trigger, independent of the unstaged source
#   edit the probe means to test. The static check has no such self-trigger
#   path, so it is the PRIMARY gate; the live probe below stays informational.
#
#   Coverage boundary: the static check catches re-introduction of a
#   narrowing `rerun-if-changed` / `rerun-if-env-changed` line. It does NOT
#   catch a future regression in `build.rs`'s OWN logic (e.g. the `git()`
#   helper or the `ORI_GIT_DIRTY` computation silently breaking) that
#   leaves the stamp stale without ever emitting such a line — that class
#   of defect needs its own dedicated check if it materializes.
#
#   INFORMATIONAL (non-gating): a single warm build + an unstaged mtime
#   touch on a tracked file + rebuild, reporting whether the build script
#   visibly reran and whether the stamped `ORI_GIT_DIRTY` matches a live
#   `git status --porcelain`
#   reading. Builds in an isolated `CARGO_TARGET_DIR` (never the shared
#   `target/`, avoiding interference from a background `rust-analyzer`).
#   Runs `git status` with `GIT_OPTIONAL_LOCKS=0` (matching build.rs's own
#   `git()` helper) so this comparison never rewrites `.git/index`'s mtime.
#   Never asserts on `ORI_BUILD_DATE` — it is HEAD's commit date and does
#   not change on an uncommitted edit. A miss here does NOT fail the
#   PRIMARY verdict (exit 0 stands when the static check passed) but is
#   surfaced via a distinct exit code so it stays observable rather than
#   silently swallowed. Stays non-gating even with the mtime fix above:
#   background build-lock contention (a concurrent `rust-analyzer` racing
#   the isolated `CARGO_TARGET_DIR`) remains a residual source of a live
#   rebuild not visibly rerunning.
#
# Exit codes:
#   0 = static check passed AND the informational demonstration also succeeded (or was skipped)
#   1 = regression: a narrowing rerun-if-changed / rerun-if-env-changed line is present
#   2 = environment error (compiler_repo not found, cargo missing, etc.)
#   3 = static check passed but the informational demonstration observed a miss — not gating, but worth a look

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

NO_COLOR=false
for arg in "$@"; do
    case "$arg" in
        --no-color) NO_COLOR=true ;;
        -h|--help)
            awk 'NR>1 && /^#/{sub(/^# ?/,""); print; next} NR>1{exit}' "$0"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 2
            ;;
    esac
done

if [[ -t 1 ]] && [[ "$NO_COLOR" == "false" ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; NC='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; NC=''
fi

cd "$ROOT_DIR" || { echo "Error: compiler_repo not found at $ROOT_DIR" >&2; exit 2; }

BUILD_RS="compiler/oric/build.rs"
if [[ ! -f "$BUILD_RS" ]]; then
    echo "Error: $BUILD_RS not found" >&2
    exit 2
fi

echo "Checking $BUILD_RS for rerun-if-changed / rerun-if-env-changed emissions..."
if grep -qE 'cargo:rerun-if-(changed|env-changed)=' "$BUILD_RS"; then
    echo -e "${RED}FAIL${NC}: $BUILD_RS emits a rerun-if-changed/rerun-if-env-changed line, narrowing Cargo's rebuild trigger below the whole package."
    echo "This reproduces a stale-stamp regression: an untracked, unstaged edit inside compiler/oric/src/ will not trigger a rebuild, so ORI_GIT_SHA/ORI_GIT_DIRTY/ORI_BUILD_DATE can freeze at a stale build's values." >&2
    exit 1
fi
echo -e "${GREEN}PASS${NC}: $BUILD_RS emits no rerun-if-changed / rerun-if-env-changed line — Cargo's default whole-package rebuild fallback governs."

echo
echo "Informational: live single-shot rebuild demonstration (not gating; see script header)..."

if ! command -v cargo >/dev/null 2>&1; then
    echo -e "${YELLOW}Skipped${NC}: cargo not found on PATH"
    exit 0
fi

TOUCH_TARGET="compiler/oric/src/main.rs"
if [[ ! -f "$TOUCH_TARGET" ]]; then
    echo -e "${YELLOW}Skipped${NC}: expected tracked file not found: $TOUCH_TARGET"
    exit 0
fi

ISOLATED_TARGET_DIR=$(mktemp -d -t verify-build-stamp-freshness-XXXXXX)
cleanup() { rm -rf "$ISOLATED_TARGET_DIR"; }
trap cleanup EXIT
export CARGO_TARGET_DIR="$ISOLATED_TARGET_DIR"

find_stamp_output() {
    grep -l '^cargo:rustc-env=ORI_GIT_SHA=' "$ISOLATED_TARGET_DIR"/debug/build/oric-*/output 2>/dev/null | head -n1
}

current_git_dirty() {
    if [[ -n "$(GIT_OPTIONAL_LOCKS=0 git status --porcelain)" ]]; then
        echo "1"
    else
        echo "0"
    fi
}

if ! cargo check -p oric -vv >/dev/null 2>&1; then
    echo -e "${YELLOW}Skipped${NC}: baseline 'cargo check -p oric' failed to build"
    exit 0
fi

touch -d "@$(($(date +%s) + 2))" "$TOUCH_TARGET"

SECOND_LOG=$(cargo check -p oric -vv 2>&1)
if [[ $? -ne 0 ]]; then
    echo -e "${YELLOW}Skipped${NC}: second 'cargo check -p oric' failed to build"
    exit 0
fi

RERAN=false
if echo "$SECOND_LOG" | grep -q 'Running `.*build-script-build'; then
    RERAN=true
fi

STAMP_FILE=$(find_stamp_output)
if [[ -z "$STAMP_FILE" ]]; then
    echo -e "${YELLOW}Note${NC}: no stamped output file found; skipping ORI_GIT_DIRTY cross-check"
    echo "rerun observed: $RERAN"
    exit 0
fi

STAMPED_DIRTY=$(grep '^cargo:rustc-env=ORI_GIT_DIRTY=' "$STAMP_FILE" | tail -n1 | cut -d= -f3)
ACTUAL_DIRTY=$(current_git_dirty)

echo "rerun observed: $RERAN | stamped ORI_GIT_DIRTY=$STAMPED_DIRTY | live git-dirty=$ACTUAL_DIRTY"
if [[ "$RERAN" == "true" ]] && [[ "$STAMPED_DIRTY" == "$ACTUAL_DIRTY" ]]; then
    exit 0
else
    echo -e "${YELLOW}Note${NC}: informational demonstration missed (non-gating) — this can be a transient environmental race (see decision doc); re-run to confirm before investigating further."
    exit 3
fi
