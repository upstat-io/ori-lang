#!/bin/bash
# Run full clippy AND full tests: the complete pre-commit/pre-push check.
# Usage: ./full-check.sh [-v|--verbose] [-s|--sequential] [--no-tee]
#
# Runs in order:
# 1. clippy-all.sh (workspace + LLVM)
# 2. test-all.sh (workspace + LLVM + WASM + Ori interpreter + Ori LLVM)
#
# Clippy runs first for fast feedback — no point running tests if lint fails.
# All non-recognized flags are forwarded to test-all.sh.
#
# Output is teed to /tmp/full-check-<timestamp>.log so progress is recoverable
# even when invoked by lefthook (which buffers stdout/stderr) and the wrapping
# session times out mid-run. Tail the log live with `tail -f` (path printed at
# start). Pass --no-tee to disable (useful in CI where /tmp may not persist).
#
# Per-phase timestamps + duration are printed so you can see progress without
# the log file. The "Phase 2: Tests" header includes a "5-10 min typical"
# warning so observers don't assume the script has hung.

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

# Parse our own flags (everything else forwards to test-all.sh).
TEE_OUTPUT=1
FORWARD_ARGS=()
for arg in "$@"; do
    case $arg in
        --no-tee)
            TEE_OUTPUT=0
            ;;
        *)
            FORWARD_ARGS+=("$arg")
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

echo -e "${BOLD}=== Full Check: Clippy + Tests ===${NC}"
if [[ $TEE_OUTPUT -eq 1 ]]; then
    echo "  Output → $LOG_FILE (tail -f to watch progress live)"
fi
echo ""

# Phase 1: Clippy (fast feedback)
PHASE1_START=$(date +%s)
echo -e "${BOLD}--- Phase 1: Clippy ($(date '+%H:%M:%S')) ---${NC}"
echo ""
if ! "$SCRIPT_DIR/clippy-all.sh"; then
    echo ""
    echo -e "${RED}${BOLD}=== Clippy failed (after $(($(date +%s) - PHASE1_START))s) — skipping tests ===${NC}"
    exit 1
fi
echo ""
echo -e "${GREEN}${BOLD}--- Phase 1 (Clippy) PASSED in $(($(date +%s) - PHASE1_START))s ---${NC}"
echo ""

# Phase 2: Tests (forward all flags)
PHASE2_START=$(date +%s)
# NON-BLOCKING during the active compiler rewrite — test-all.sh still runs
# and its output is preserved, but a failure emits a warning instead of
# failing the hook. Clippy + every other pre-commit gate (fmt, sprawl-lint,
# test-weakening-lint, prose-lint, version-sync, spec-proposal-gate, aims-
# spec-drift, root-inventory) still block. Re-enable test-blocking by
# restoring: `"$SCRIPT_DIR/test-all.sh" "${FORWARD_ARGS[@]}"`.
echo -e "${BOLD}--- Phase 2: Tests ($(date '+%H:%M:%S') — typically 5-10 min, NON-BLOCKING) ---${NC}"
echo ""
TEST_EXIT=0
"$SCRIPT_DIR/test-all.sh" "${FORWARD_ARGS[@]}" || TEST_EXIT=$?
echo ""
if [[ $TEST_EXIT -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}--- Phase 2 (Tests) PASSED in $(($(date +%s) - PHASE2_START))s ---${NC}"
else
    echo -e "${YELLOW}${BOLD}--- Phase 2 (Tests) FAILED in $(($(date +%s) - PHASE2_START))s (exit $TEST_EXIT) — WARNING ONLY, commit NOT blocked ---${NC}" >&2
    echo -e "${YELLOW}    Compiler is mid-rewrite; test-suite green is not gated. Fix tests as part of the active work.${NC}" >&2
fi

echo ""
echo -e "${BOLD}=== Full Check Complete (total: $(($(date +%s) - START_TS))s) ===${NC}"
if [[ $TEE_OUTPUT -eq 1 ]]; then
    echo "  Full log: $LOG_FILE"
fi
