#!/usr/bin/env bash
# Prepare a new release by updating version across all manifests
#
# Usage:
#   ./scripts/release.sh           # Auto-increment alpha (0.1.0-alpha.1 → 0.1.0-alpha.2)
#   ./scripts/release.sh 0.2.0     # Explicit version (for major/minor bumps)
#
# This script:
# 1. Determines the new version (auto-increment or explicit)
# 2. Updates the workspace Cargo.toml version
# 3. Runs sync-version.sh to propagate to all manifests
# 4. Prints next steps (review, test, commit, tag, push)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

usage() {
    echo "Usage: $0 [version]"
    echo ""
    echo "If no version specified, auto-increments the pre-release number:"
    echo "  0.1.0-alpha.1  →  0.1.0-alpha.2"
    echo "  0.1.0-alpha.9  →  0.1.0-alpha.10"
    echo "  0.1.0-beta.3   →  0.1.0-beta.4"
    echo ""
    echo "Examples:"
    echo "  $0              # Auto-increment nightly"
    echo "  $0 0.1.0-beta.1 # Start beta phase"
    echo "  $0 0.2.0        # Stable release"
    echo ""
    echo "Version format: MAJOR.MINOR.PATCH[-PRERELEASE]"
    exit 1
}

# Get current version from workspace Cargo.toml
get_current_version() {
    grep -E '^version\s*=' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*=\s*"\([^"]*\)".*/\1/'
}

# Auto-increment pre-release number
# 0.1.0-alpha.1 → 0.1.0-alpha.2
# 0.1.0-beta.9 → 0.1.0-beta.10
auto_increment_version() {
    local current="$1"

    # Check if it has a pre-release suffix with a number
    if [[ "$current" =~ ^([0-9]+\.[0-9]+\.[0-9]+)-([a-zA-Z]+)\.([0-9]+)$ ]]; then
        local base="${BASH_REMATCH[1]}"
        local prerelease="${BASH_REMATCH[2]}"
        local num="${BASH_REMATCH[3]}"
        local new_num=$((num + 1))
        echo "${base}-${prerelease}.${new_num}"
    else
        echo -e "${RED}ERROR${NC}: Cannot auto-increment version '$current'" >&2
        echo "Auto-increment only works with versions like: 0.1.0-alpha.1, 0.1.0-beta.2" >&2
        echo "Use an explicit version instead: $0 <version>" >&2
        exit 1
    fi
}

# Validate semver format (basic check)
validate_version() {
    local version="$1"
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
        echo -e "${RED}ERROR${NC}: Invalid version format: $version"
        echo "Expected: MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-PRERELEASE"
        echo "Examples: 0.1.0, 0.1.0-alpha.1, 1.0.0-beta.2"
        exit 1
    fi
}

# Update version in workspace Cargo.toml
update_workspace_version() {
    local version="$1"
    local cargo_toml="$ROOT_DIR/Cargo.toml"

    # Update the version line in [workspace.package] section
    sed -i "s/^version = \"[^\"]*\"/version = \"$version\"/" "$cargo_toml"

    echo -e "${GREEN}UPDATED${NC}: workspace Cargo.toml -> $version"
}

main() {
    # Handle help flag
    if [[ "${1:-}" == "-h" ]] || [[ "${1:-}" == "--help" ]]; then
        usage
    fi

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

    # Get current version
    local current_version
    current_version=$(get_current_version)

    # Determine new version
    local new_version
    if [[ $# -eq 0 ]]; then
        # Auto-increment
        new_version=$(auto_increment_version "$current_version")
        echo -e "${CYAN}Auto-incrementing version...${NC}"
    else
        new_version="$1"
    fi

    # Validate
    validate_version "$new_version"

    echo ""
    echo -e "${BOLD}Current version:${NC} $current_version"
    echo -e "${BOLD}New version:${NC}     $new_version"
    echo ""

    # Confirm
    read -p "Proceed with version bump? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 1
    fi

    echo ""
    echo "=== Updating workspace version ==="
    update_workspace_version "$new_version"

    echo ""
    echo "=== Syncing all manifests ==="
    "$SCRIPT_DIR/sync-version.sh"

    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}                      NEXT STEPS                           ${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "1. Review changes:"
    echo "   git diff"
    echo ""
    echo "2. Run full test suite:"
    echo "   ./test-all"
    echo ""
    echo "3. Commit and tag:"
    echo -e "   ${YELLOW}git add -A && git commit -m \"chore: release v$new_version\"${NC}"
    echo -e "   ${YELLOW}git tag v$new_version${NC}"
    echo -e "   ${YELLOW}git push origin master --tags${NC}"
    echo ""
    echo "The release workflow will automatically:"
    echo "  • Build binaries for Linux, macOS, Windows"
    echo "  • Create GitHub release (marked as nightly/pre-release)"
    echo "  • Generate checksums and release notes"
    echo ""
    echo -e "${GREEN}Version bump complete!${NC}"
}

main "$@"
