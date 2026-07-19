#!/usr/bin/env bash
# Auto-refresh BUILD_NUMBER when its date has rolled past today (UTC).
#
# BUILD_NUMBER is otherwise a manually-bumped release marker
# (./scripts/bump-build.sh + ./scripts/sync-version.sh) with no freshness
# check of its own — /commit-push's sync-version.sh --check gate only
# catches BUILD_NUMBER disagreeing with its own downstream mirrors
# (Cargo.toml etc.), never BUILD_NUMBER's date itself going stale. This
# script closes that gap at a day-boundary: a commit's date differs from
# BUILD_NUMBER's stored date -> bump + sync + stage, so the calendar
# version can never be more than zero days stale at commit time. Same-day
# commits are left untouched, preserving bump-build.sh's existing
# same-day counter/stage semantics.
#
# Usage:
#   ./scripts/auto-bump-if-stale.sh          # bump + sync + stage if stale
#   ./scripts/auto-bump-if-stale.sh --check  # report drift only, exit 1 if stale

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_FILE="$ROOT_DIR/BUILD_NUMBER"

GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

CHECK_MODE=false
if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

TODAY=$(date -u +"%Y.%m.%d")

CURRENT_DATE="(none)"
if [[ -f "$BUILD_FILE" ]]; then
    CURRENT_DATE=$(tr -d '[:space:]' < "$BUILD_FILE" | cut -d. -f1-3)
fi

if [[ "$CURRENT_DATE" == "$TODAY" ]]; then
    echo -e "${GREEN}OK${NC}: BUILD_NUMBER date is current ($CURRENT_DATE)"
    exit 0
fi

echo -e "${YELLOW}STALE${NC}: BUILD_NUMBER date ($CURRENT_DATE) != today ($TODAY)"

if $CHECK_MODE; then
    exit 1
fi

"$SCRIPT_DIR/bump-build.sh"
"$SCRIPT_DIR/sync-version.sh"

for f in "$BUILD_FILE" \
         "$ROOT_DIR/Cargo.toml" \
         "$ROOT_DIR/compiler/ori_llvm/Cargo.toml" \
         "$ROOT_DIR/compiler/ori_rt/Cargo.toml" \
         "$ROOT_DIR/tools/ori-lsp/Cargo.toml" \
         "$ROOT_DIR/editors/vscode-ori/package.json"; do
    if [[ -f "$f" ]]; then
        git -C "$ROOT_DIR" add "$f"
    fi
done
