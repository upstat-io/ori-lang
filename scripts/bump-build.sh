#!/usr/bin/env bash
# Bump the build number in the BUILD file.
#
# Format: YYYY.MM.DD.N (date in UTC + daily counter)
#
# Usage:
#   ./scripts/bump-build.sh          # Bump and write
#   ./scripts/bump-build.sh --check  # Dry-run: show current → next

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_FILE="$ROOT_DIR/BUILD"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

CHECK_MODE=false
if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

# Read current build number
current=$(tr -d '[:space:]' < "$BUILD_FILE")

# Parse current value: YYYY.MM.DD.N
IFS='.' read -r cur_year cur_month cur_day cur_counter <<< "$current"

# Today's date in UTC
today_year=$(date -u +%Y)
today_month=$(date -u +%m)
today_day=$(date -u +%d)

# Strip leading zeros for comparison (bash arithmetic treats 08/09 as invalid octal)
cur_month_n=$((10#${cur_month:-0}))
cur_day_n=$((10#${cur_day:-0}))
today_month_n=$((10#$today_month))
today_day_n=$((10#$today_day))

# Determine next build number
if [[ "$cur_year" == "$today_year" ]] && \
   [[ "$cur_month_n" == "$today_month_n" ]] && \
   [[ "$cur_day_n" == "$today_day_n" ]]; then
    # Same day: increment counter
    next_counter=$(( ${cur_counter:-0} + 1 ))
else
    # New day: reset counter
    next_counter=1
fi

next="${today_year}.${today_month}.${today_day}.${next_counter}"

if $CHECK_MODE; then
    echo -e "${YELLOW}Current${NC}: $current"
    echo -e "${GREEN}Next${NC}:    $next"
else
    echo "$next" > "$BUILD_FILE"
    echo -e "${GREEN}Build number${NC}: $current → $next"
fi
