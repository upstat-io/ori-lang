#!/usr/bin/env bash
# Derive the build number from git history and write to BUILD_NUMBER.
#
# Format: YYYY.MM.DD.N-STAGE (UTC date + daily merge count + release stage)
#
# The counter N is the number of commits merged to master on the current
# UTC date (via --first-parent to count only merge/direct commits).
# No persistent state needed — the git log IS the counter.
#
# The release stage (alpha, beta, rc, or empty for stable) is extracted
# from the current BUILD_NUMBER content.
#
# Usage:
#   ./scripts/bump-build.sh                   # Write BUILD_NUMBER
#   ./scripts/bump-build.sh --check           # Dry-run: show what it would write
#   ./scripts/bump-build.sh --set-stage beta  # Change release stage

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_FILE="$ROOT_DIR/BUILD_NUMBER"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

CHECK_MODE=false
NEW_STAGE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_MODE=true
            shift
            ;;
        --set-stage)
            if [[ -z "${2:-}" ]]; then
                echo -e "${RED}ERROR${NC}: --set-stage requires an argument (alpha, beta, rc, stable)"
                exit 1
            fi
            NEW_STAGE="$2"
            if [[ "$NEW_STAGE" != "alpha" && "$NEW_STAGE" != "beta" && "$NEW_STAGE" != "rc" && "$NEW_STAGE" != "stable" ]]; then
                echo -e "${RED}ERROR${NC}: Invalid stage '$NEW_STAGE'. Must be: alpha, beta, rc, stable"
                exit 1
            fi
            shift 2
            ;;
        *)
            echo -e "${RED}ERROR${NC}: Unknown option: $1"
            echo "Usage: $0 [--check] [--set-stage STAGE]"
            exit 1
            ;;
    esac
done

# Extract current stage from BUILD_NUMBER (e.g., "2026.02.28.1-alpha" -> "alpha")
# Defaults to "alpha" if BUILD_NUMBER doesn't exist or has no stage.
extract_stage() {
    if [[ -f "$BUILD_FILE" ]]; then
        local content
        content=$(tr -d '[:space:]' < "$BUILD_FILE")
        if [[ "$content" == *-* ]]; then
            echo "${content#*-}"
            return
        fi
    fi
    echo "alpha"
}

# Determine the stage to use
if [[ -n "$NEW_STAGE" ]]; then
    STAGE="$NEW_STAGE"
    # "stable" means no suffix
    if [[ "$STAGE" == "stable" ]]; then
        STAGE=""
    fi
else
    STAGE=$(extract_stage)
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

# Build the version: YYYY.MM.DD.N or YYYY.MM.DD.N-stage
if [[ -n "$STAGE" ]]; then
    NEXT="${TODAY}.${COUNT}-${STAGE}"
else
    NEXT="${TODAY}.${COUNT}"
fi

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
