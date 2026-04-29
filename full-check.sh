#!/bin/bash
# Run full clippy AND full tests: the complete pre-commit/pre-push check.
# Usage: ./full-check.sh [-v|--verbose] [-s|--sequential]
#
# Runs in order:
# 1. clippy-all.sh (workspace + LLVM)
# 2. test-all.sh (workspace + LLVM + WASM + Ori interpreter + Ori LLVM)
#
# Clippy runs first for fast feedback — no point running tests if lint fails.
# All flags are forwarded to test-all.sh.

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

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
        echo "" >&2
    fi
}
trap print_hook_failure_directive EXIT

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo -e "${BOLD}=== Full Check: Clippy + Tests ===${NC}"
echo ""

# Phase 1: Clippy (fast feedback)
echo -e "${BOLD}--- Phase 1: Clippy ---${NC}"
echo ""
if ! "$SCRIPT_DIR/clippy-all.sh"; then
    echo ""
    echo -e "${RED}${BOLD}=== Clippy failed — skipping tests ===${NC}"
    exit 1
fi

echo ""

# Phase 2: Tests (forward all flags)
echo -e "${BOLD}--- Phase 2: Tests ---${NC}"
echo ""
"$SCRIPT_DIR/test-all.sh" "$@"
