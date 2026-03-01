#!/usr/bin/env bash
# Prepare a new release by bumping the build number and syncing all manifests.
#
# This is a thin wrapper around bump-build.sh and sync-version.sh.
# BUILD_NUMBER is the single source of truth for all versions.
#
# Usage:
#   ./scripts/release.sh                    # Bump build number, sync manifests
#   ./scripts/release.sh --set-stage beta   # Change stage to beta, bump, sync
#   ./scripts/release.sh --check            # Dry-run: show what would change
#   ./scripts/release.sh --yes              # Skip interactive confirmation (CI)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --set-stage STAGE  Change release stage (alpha, beta, rc, stable)"
    echo "  --check            Dry-run: show what would change without writing"
    echo "  -y, --yes          Skip interactive confirmation (for CI)"
    echo "  -h, --help         Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                       # Bump build number (same stage)"
    echo "  $0 --set-stage beta      # Transition to beta, bump build"
    echo "  $0 --set-stage stable    # Transition to stable release"
    echo "  $0 --yes                 # Non-interactive (CI)"
    exit 1
}

main() {
    local auto_confirm=false
    local check_mode=false
    local bump_args=()

    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help) usage ;;
            -y|--yes) auto_confirm=true; shift ;;
            --check) check_mode=true; bump_args+=(--check); shift ;;
            --set-stage)
                if [[ -z "${2:-}" ]]; then
                    echo -e "${RED}ERROR${NC}: --set-stage requires an argument"
                    exit 1
                fi
                bump_args+=(--set-stage "$2")
                shift 2
                ;;
            *) echo -e "${RED}ERROR${NC}: Unknown option: $1"; usage ;;
        esac
    done

    # Ensure we're on master branch
    local current_branch
    current_branch=$(git branch --show-current)
    if [[ "$current_branch" != "master" && "$current_branch" != "main" ]]; then
        echo -e "${RED}ERROR${NC}: Releases must be created from the master branch."
        echo ""
        echo "Current branch: $current_branch"
        echo ""
        echo "To release, first merge your changes to master:"
        echo "  git checkout master"
        echo "  git merge $current_branch"
        echo "  ./scripts/release.sh"
        exit 1
    fi

    # Show current state
    local current_build="(none)"
    if [[ -f "$ROOT_DIR/BUILD_NUMBER" ]]; then
        current_build=$(tr -d '[:space:]' < "$ROOT_DIR/BUILD_NUMBER")
    fi
    echo -e "${BOLD}Current build:${NC} $current_build"
    echo ""

    # Step 1: Bump build number
    echo "=== Bumping build number ==="
    "$SCRIPT_DIR/bump-build.sh" "${bump_args[@]}"

    if $check_mode; then
        echo ""
        echo "=== Checking version sync ==="
        "$SCRIPT_DIR/sync-version.sh" --check
        return
    fi

    # Confirm before syncing (skip with --yes)
    local new_build
    new_build=$(tr -d '[:space:]' < "$ROOT_DIR/BUILD_NUMBER")

    if ! $auto_confirm; then
        echo ""
        read -p "Sync all manifests to $new_build? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Aborted. BUILD_NUMBER was updated but manifests were not synced."
            echo "Run './scripts/sync-version.sh' to sync, or restore BUILD_NUMBER."
            exit 1
        fi
    fi

    # Step 2: Sync all manifests
    echo ""
    echo "=== Syncing all manifests ==="
    "$SCRIPT_DIR/sync-version.sh"

    # Show next steps only in interactive mode
    if ! $auto_confirm; then
        echo ""
        echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
        echo -e "${CYAN}                      NEXT STEPS                           ${NC}"
        echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
        echo ""
        echo "1. Review changes:"
        echo "   git diff"
        echo ""
        echo "2. Run full test suite:"
        echo "   ./test-all.sh"
        echo ""
        echo "3. Commit and tag:"
        echo -e "   ${YELLOW}git add -A && git commit -m \"chore: release v$new_build\"${NC}"
        echo -e "   ${YELLOW}git tag v$new_build${NC}"
        echo -e "   ${YELLOW}git push origin master --tags${NC}"
        echo ""
        echo "The release workflow will automatically:"
        echo "  - Build binaries for Linux, macOS, Windows"
        echo "  - Create GitHub release (marked as pre-release if alpha/beta/rc)"
        echo "  - Generate checksums and release notes"
    fi

    echo ""
    echo -e "${GREEN}Release preparation complete!${NC}"
}

main "$@"
