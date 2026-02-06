#!/usr/bin/env bash
# Derive the build number from git history and write to BUILD_NUMBER.
#
# Format: YYYY.MM.DD.N (UTC date + daily merge count)
#
# The counter N is the number of commits merged to master on the current
# UTC date (via --first-parent to count only merge/direct commits).
# No persistent state needed — the git log IS the counter.
#
# Usage:
#   ./scripts/bump-build.sh          # Write BUILD_NUMBER
#   ./scripts/bump-build.sh --check  # Dry-run: show what it would write

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_FILE="$ROOT_DIR/BUILD_NUMBER"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

CHECK_MODE=false
if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

# Today's date in UTC
TODAY=$(date -u +"%Y.%m.%d")
MIDNIGHT=$(date -u +"%Y-%m-%dT00:00:00Z")

# Count commits to master today (first-parent = merge commits + direct pushes only).
# Use origin/master if available (CI), fall back to master (local).
BRANCH="master"
if git rev-parse --verify origin/master &>/dev/null; then
    BRANCH="origin/master"
fi

COUNT=$(git log --first-parent --oneline --since="$MIDNIGHT" "$BRANCH" 2>/dev/null | wc -l)
COUNT=$(( COUNT )) # trim whitespace from wc -l on macOS

# Build number: at least 1 (the current merge may not be in the log yet during CI)
if [[ "$COUNT" -eq 0 ]]; then
    COUNT=1
fi

NEXT="${TODAY}.${COUNT}"

# Read current for display
CURRENT="(none)"
if [[ -f "$BUILD_FILE" ]]; then
    CURRENT=$(tr -d '[:space:]' < "$BUILD_FILE")
fi

if $CHECK_MODE; then
    echo -e "${YELLOW}Current${NC}: $CURRENT"
    echo -e "${GREEN}Derived${NC}: $NEXT"
else
    echo "$NEXT" > "$BUILD_FILE"
    echo -e "${GREEN}Build number${NC}: $CURRENT -> $NEXT"
fi
