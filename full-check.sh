#!/bin/bash
# Fast pre-commit check: clippy + UB-coverage gate. Does NOT run test-all.
# Usage: ./full-check.sh [-v|--verbose] [-s|--sequential] [--no-tee]
#
# Runs in order:
# 1. clippy-all.sh (workspace + LLVM)
# 2. UB coverage-matrix regression gate
#
# test-all is NOT a per-commit gate. It runs at bug/plan completion (the
# /fix-bug + /continue-roadmap verdict surfaces) and in CI before merge to
# main (ci.yml runs ./test-all.sh directly). Run it locally on demand with
# `./test-all.sh`.
#
# Output is teed to /tmp/full-check-<timestamp>.log so progress is recoverable
# even when invoked by lefthook (which buffers stdout/stderr). Tail the log
# live with `tail -f` (path printed at start). Pass --no-tee to disable.

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

# Parse our own flags. Unrecognized flags (-v / -s / etc.) are ignored — they
# were forwarded to test-all.sh, which this gate no longer runs.
TEE_OUTPUT=1
for arg in "$@"; do
    case $arg in
        --no-tee)
            TEE_OUTPUT=0
            ;;
    esac
done

LOG_FILE="/tmp/full-check-$(date +%Y%m%d-%H%M%S)-$$.log"
if [[ $TEE_OUTPUT -eq 1 ]]; then
    # Tee both stdout and stderr to the log file. Use process substitution so
    # the log file gets the same combined stream the user sees on terminal.
    exec > >(tee -a "$LOG_FILE") 2>&1
fi

# When this hook rejects a commit, print a specific directive — not a wall
# of text. Test failures are bugs; weakening tests to make the hook pass is
# banned by project testing discipline.
print_hook_failure_directive() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        echo "" >&2
        echo -e "${YELLOW}${BOLD}── Hook-failure directive ──${NC}" >&2
        echo "Test failures are bugs. Fix the production code, not the tests." >&2
        echo "BANNED: editing tests to make the hook pass (see project testing discipline)." >&2
        if [[ $TEE_OUTPUT -eq 1 ]]; then
            echo "" >&2
            echo "  Full output captured to: $LOG_FILE" >&2
            echo "  Tail it: tail $LOG_FILE" >&2
        fi
        echo "" >&2
    fi
}
trap print_hook_failure_directive EXIT

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
START_TS=$(date +%s)

echo -e "${BOLD}=== Full Check: Clippy + UB gate (test-all is NOT run here) ===${NC}"
if [[ $TEE_OUTPUT -eq 1 ]]; then
    echo "  Output → $LOG_FILE (tail -f to watch progress live)"
fi
echo ""

# Phase 1: Clippy (fast feedback)
PHASE1_START=$(date +%s)
echo -e "${BOLD}--- Phase 1: Clippy ($(date '+%H:%M:%S')) ---${NC}"
echo ""

# Clippy scope. When the commit driver exports ORI_COMMIT_SCOPE=staged (an
# unrelated-work check-in), scope this blocking gate to the committing diff's
# own crates: a parallel session's breakage in unrelated crates cannot block an
# independent check-in, while the committing diff's OWN breakage in its crates
# still fails (clippy still runs on those crates). Unset = full-workspace clippy.
CLIPPY_SCOPE_ARGS=()
RUN_CLIPPY=1
if [[ "${ORI_COMMIT_SCOPE:-}" == "staged" ]]; then
    if STAGED_CRATES=$(python3 "$SCRIPT_DIR/scripts/staged-crates.py" 2>/dev/null); then
        if [[ -z "$STAGED_CRATES" ]]; then
            RUN_CLIPPY=0
            echo -e "${YELLOW}No Rust crates in the staged set — skipping clippy (unrelated check-in).${NC}"
        else
            echo -e "${YELLOW}Scoped clippy (unrelated check-in) — staged crates:${NC}"
            while IFS= read -r crate; do
                [[ -n "$crate" ]] || continue
                echo "    - $crate"
                CLIPPY_SCOPE_ARGS+=("-p" "$crate")
            done <<< "$STAGED_CRATES"
        fi
    else
        echo -e "${YELLOW}Could not enumerate staged crates — falling back to full-workspace clippy.${NC}"
    fi
fi

if [[ $RUN_CLIPPY -eq 1 ]]; then
    # Auto-fix machine-applicable clippy lints first, then enforce. Anything not
    # auto-fixable still fails the blocking check below (autofix-first, manual on
    # the remainder). Stage only files the fix actually changed, so unrelated
    # working-tree edits are not swept into the commit.
    if [[ ${#CLIPPY_SCOPE_ARGS[@]} -gt 0 ]]; then
        CLIPPY_FIX_TARGET=("${CLIPPY_SCOPE_ARGS[@]}")
    else
        CLIPPY_FIX_TARGET=("--workspace")
    fi
    CLIPPY_PRE_FIX="$(git -C "$SCRIPT_DIR" diff --name-only | sort)"
    cargo clippy --fix --allow-dirty --allow-staged --all-targets "${CLIPPY_FIX_TARGET[@]}" 2>/dev/null || true
    comm -13 <(printf '%s\n' "$CLIPPY_PRE_FIX") <(git -C "$SCRIPT_DIR" diff --name-only | sort) \
        | while IFS= read -r f; do
            [[ -n "$f" ]] && git -C "$SCRIPT_DIR" add -- "$f" 2>/dev/null || true
        done

    if ! "$SCRIPT_DIR/clippy-all.sh" "${CLIPPY_SCOPE_ARGS[@]}"; then
        echo ""
        echo -e "${RED}${BOLD}=== Clippy failed after auto-fix (after $(($(date +%s) - PHASE1_START))s) — remaining lints need a manual fix; skipping tests ===${NC}"
        exit 1
    fi
fi
echo ""
echo -e "${GREEN}${BOLD}--- Phase 1 (Clippy) PASSED in $(($(date +%s) - PHASE1_START))s ---${NC}"
echo ""

# Phase 1.5: UB coverage-matrix regression gate (ub-safety-threat-model).
# Fast + deterministic: --strict asserts every canonical miri UB class is
# dispositioned + pinned (39/39); --self-test asserts the checker's own
# fail-closed invariants. Blocking — a foreclosure losing its pin, an
# unresolvable pin, or a new undispositioned UB class fails the commit.
echo -e "${BOLD}--- Phase 1.5: UB coverage gate ($(date '+%H:%M:%S')) ---${NC}"
echo ""
if ! python3 "$SCRIPT_DIR/scripts/ub-coverage-check.py" --strict --self-test; then
    echo "" >&2
    echo -e "${RED}${BOLD}=== UB coverage gate FAILED — the safety frontier regressed ===${NC}" >&2
    echo "  A UB class lost its disposition/pin, a pin no longer resolves, or a" >&2
    echo "  new class is undispositioned. See scripts/ub-safety/README.md." >&2
    exit 1
fi
echo ""
echo -e "${GREEN}${BOLD}--- Phase 1.5 (UB coverage gate) PASSED ---${NC}"
echo ""

# test-all is intentionally NOT run here (it gates bug/plan completion + CI,
# not individual commits). See the header comment.
echo -e "${BOLD}=== Full Check Complete (total: $(($(date +%s) - START_TS))s) — test-all runs at bug/plan completion + in CI ===${NC}"
if [[ $TEE_OUTPUT -eq 1 ]]; then
    echo "  Full log: $LOG_FILE"
fi
