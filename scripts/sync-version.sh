#!/usr/bin/env bash
# Synchronize version across all project manifests
#
# Single source of truth: BUILD_NUMBER file at the repo root
#
# Conversion functions:
#   calver_to_cargo: 2026.02.28.1-alpha -> 2026.2.28-alpha.1  (valid SemVer for Cargo)
#   calver_to_npm:   2026.02.28.1-alpha -> 2026.2.28           (npm-compatible)
#
# Usage:
#   ./scripts/sync-version.sh         # Update all version files
#   ./scripts/sync-version.sh --check # Check if versions are in sync (for CI)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_FILE="$ROOT_DIR/BUILD_NUMBER"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Parse arguments
CHECK_MODE=false
if [[ "${1:-}" == "--check" ]]; then
    CHECK_MODE=true
fi

# Read BUILD_NUMBER (the single source of truth)
get_build_number() {
    if [[ ! -f "$BUILD_FILE" ]]; then
        echo -e "${RED}ERROR${NC}: BUILD_NUMBER file not found at $BUILD_FILE" >&2
        echo "Run ./scripts/bump-build.sh to create it." >&2
        exit 1
    fi
    tr -d '[:space:]' < "$BUILD_FILE"
}

# Convert CalVer to Cargo-compatible SemVer
# 2026.02.28.1-alpha -> 2026.2.28-alpha.1
# 2026.02.28.3       -> 2026.2.28-3        (stable: build counter in pre-release)
# SemVer forbids leading zeros, so 02->2, 06->6
calver_to_cargo() {
    local calver="$1"
    local year month day rest stage counter

    # Split on dots: YYYY.MM.DD.N[-STAGE]
    IFS='.' read -r year month day rest <<< "$calver"

    # Strip leading zeros for SemVer compliance
    month=$((10#$month))
    day=$((10#$day))

    # rest is "N-STAGE" or just "N"
    if [[ "$rest" == *-* ]]; then
        counter="${rest%%-*}"
        stage="${rest#*-}"
        echo "${year}.${month}.${day}-${stage}.${counter}"
    else
        counter="$rest"
        # Stable: no stage suffix, but Cargo needs pre-release for build counter > 0
        # For stable release, counter is informational only — use just YYYY.M.D
        if [[ "$counter" -eq 1 ]]; then
            echo "${year}.${month}.${day}"
        else
            echo "${year}.${month}.${day}-${counter}"
        fi
    fi
}

# Convert CalVer to NPM-compatible version (base semver only)
# 2026.02.28.1-alpha -> 2026.2.28
calver_to_npm() {
    local calver="$1"
    local year month day _rest

    IFS='.' read -r year month day _rest <<< "$calver"
    month=$((10#$month))
    day=$((10#$day))
    echo "${year}.${month}.${day}"
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
            sed -i "s/^version\s*=\s*\"[^\"]*\"/version = \"$version\"/" "$file"
            echo -e "${GREEN}UPDATED${NC}: $file -> $version"
        fi
    else
        echo -e "${GREEN}OK${NC}: $file ($version)"
    fi
}

# Every standalone (non-workspace-member) manifest. Read by both the update loop
# and the coverage check.
STANDALONE_MANIFESTS=(
    "compiler/ori_llvm/Cargo.toml"
    "compiler/ori_rt/Cargo.toml"
    "tools/ori-lsp/Cargo.toml"
    "tools/ori-rc-remarks/Cargo.toml"
)

# Fail when an on-disk standalone manifest is absent from STANDALONE_MANIFESTS.
check_standalone_coverage() {
    local missing=0 found rel h seen
    local -a handled=("${STANDALONE_MANIFESTS[@]}")
    # NUL-delimited so a whitespace-bearing path is enumerated, not dropped.
    while IFS= read -r -d '' found; do
        grep -q '^\[package\]' "$found" 2>/dev/null || continue
        grep -q 'version.workspace *= *true' "$found" 2>/dev/null && continue
        rel="${found#"$ROOT_DIR"/}"
        seen=0
        for h in "${handled[@]}"; do
            [[ "$rel" == "$h" ]] && { seen=1; break; }
        done
        if [[ $seen -eq 0 ]]; then
            echo "  MISSING: $rel is a standalone manifest not covered by sync-version.sh"
            missing=1
        fi
    done < <(find "$ROOT_DIR/compiler" "$ROOT_DIR/tools" -name Cargo.toml -type f -print0 2>/dev/null)
    if [[ $missing -eq 1 ]]; then
        echo "  Add each MISSING manifest to the standalone block above."
        return 1
    fi
    echo "  standalone coverage: all on-disk standalone manifests are handled"
    return 0
}

check_workspace_version() {
    local version="$1"
    local file="$ROOT_DIR/Cargo.toml"

    local current
    current=$(grep -E '^version\s*=' "$file" | head -1 | sed 's/.*=\s*"\([^"]*\)".*/\1/' || true)

    if [[ "$current" != "$version" ]]; then
        if $CHECK_MODE; then
            echo -e "${RED}MISMATCH${NC}: $file has version '$current', expected '$version'"
            return 1
        else
            sed -i "s/^version = \"[^\"]*\"/version = \"$version\"/" "$file"
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
            sed -i "s/\"version\"\s*:\s*\"[^\"]*\"/\"version\": \"$version\"/" "$file"
            echo -e "${GREEN}UPDATED${NC}: $file -> $version"
        fi
    else
        echo -e "${GREEN}OK${NC}: $file ($version)"
    fi
}

main() {
    local build_number
    build_number=$(get_build_number)

    local cargo_version
    cargo_version=$(calver_to_cargo "$build_number")

    local npm_version
    npm_version=$(calver_to_npm "$build_number")

    echo "BUILD_NUMBER:  $build_number"
    echo "Cargo version: $cargo_version"
    echo "NPM version:   $npm_version"
    echo ""

    local failed=false

    # === Workspace root ===
    echo "=== Workspace Cargo.toml ==="
    check_workspace_version "$cargo_version" || failed=true

    echo ""
    echo "=== Excluded/standalone Cargo.toml files ==="

    local manifest
    for manifest in "${STANDALONE_MANIFESTS[@]}"; do
        update_cargo_version "$ROOT_DIR/$manifest" "$cargo_version" || failed=true
    done

    # Fail on any on-disk standalone manifest absent from STANDALONE_MANIFESTS.
    check_standalone_coverage || failed=true

    echo ""
    echo "=== package.json files ==="

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
