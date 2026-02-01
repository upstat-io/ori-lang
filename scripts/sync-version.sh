#!/usr/bin/env bash
# Synchronize version across all project manifests
#
# Single source of truth: workspace Cargo.toml [workspace.package] version
#
# Usage:
#   ./scripts/sync-version.sh         # Update all version files
#   ./scripts/sync-version.sh --check # Check if versions are in sync (for CI)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Parse arguments
CHECK_MODE=false
if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

# Extract version from workspace Cargo.toml
get_workspace_version() {
    grep -E '^version\s*=' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*=\s*"\([^"]*\)".*/\1/'
}

# Extract base semver (strip pre-release suffix for npm)
# e.g., "0.1.0-alpha.1" -> "0.1.0"
get_npm_version() {
    local version="$1"
    echo "$version" | sed 's/-.*//'
}

# Update version in a Cargo.toml file (non-workspace packages)
update_cargo_version() {
    local file="$1"
    local version="$2"

    if [[ ! -f "$file" ]]; then
        return
    fi

    local current
    current=$(grep -E '^version\s*=' "$file" | head -1 | sed 's/.*=\s*"\([^"]*\)".*/\1/' || true)

    if [[ "$current" != "$version" ]]; then
        if $CHECK_MODE; then
            echo -e "${RED}MISMATCH${NC}: $file has version '$current', expected '$version'"
            return 1
        else
            # Use sed to update the version line in [package] section
            sed -i "s/^version\s*=\s*\"[^\"]*\"/version = \"$version\"/" "$file"
            echo -e "${GREEN}UPDATED${NC}: $file -> $version"
        fi
    else
        echo -e "${GREEN}OK${NC}: $file ($version)"
    fi
}

# Update version in a package.json file
update_npm_version() {
    local file="$1"
    local version="$2"

    if [[ ! -f "$file" ]]; then
        return
    fi

    local current
    current=$(grep -E '"version"\s*:' "$file" | head -1 | sed 's/.*:\s*"\([^"]*\)".*/\1/' || true)

    if [[ "$current" != "$version" ]]; then
        if $CHECK_MODE; then
            echo -e "${RED}MISMATCH${NC}: $file has version '$current', expected '$version'"
            return 1
        else
            # Use sed to update the version field
            sed -i "s/\"version\"\s*:\s*\"[^\"]*\"/\"version\": \"$version\"/" "$file"
            echo -e "${GREEN}UPDATED${NC}: $file -> $version"
        fi
    else
        echo -e "${GREEN}OK${NC}: $file ($version)"
    fi
}

main() {
    local version
    version=$(get_workspace_version)
    local npm_version
    npm_version=$(get_npm_version "$version")

    echo "Workspace version: $version"
    echo "NPM version (base semver): $npm_version"
    echo ""

    local failed=false

    # Non-workspace Cargo.toml files that need manual sync
    echo "=== Cargo.toml files ==="

    # oric (workspace member but has its own Cargo.toml)
    update_cargo_version "$ROOT_DIR/compiler/oric/Cargo.toml" "$version" || failed=true

    # ori_macros (standalone)
    update_cargo_version "$ROOT_DIR/compiler/ori_macros/Cargo.toml" "$version" || failed=true

    # ori_llvm (excluded from workspace)
    update_cargo_version "$ROOT_DIR/compiler/ori_llvm/Cargo.toml" "$version" || failed=true

    # playground-wasm (standalone)
    update_cargo_version "$ROOT_DIR/website/playground-wasm/Cargo.toml" "$version" || failed=true

    echo ""
    echo "=== package.json files ==="

    # Website package.json files use base semver (without pre-release)
    update_npm_version "$ROOT_DIR/website/package.json" "$npm_version" || failed=true
    update_npm_version "$ROOT_DIR/website/src/wasm/package.json" "$npm_version" || failed=true
    update_npm_version "$ROOT_DIR/editors/vscode-ori/package.json" "$npm_version" || failed=true

    echo ""

    if $failed; then
        echo -e "${RED}Version sync check failed!${NC}"
        echo "Run './scripts/sync-version.sh' to fix."
        exit 1
    fi

    if $CHECK_MODE; then
        echo -e "${GREEN}All versions in sync!${NC}"
    else
        echo -e "${GREEN}Version sync complete!${NC}"
    fi
}

main
