#!/bin/bash
# Shared helpers for diagnostic scripts.
#
# Source this file at the top of each diagnostic script:
#   SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
#   source "$SCRIPT_DIR/_common.sh"
#
# Provides:
#   find_ori_bin      — locate an LLVM-enabled ori binary, or exit with error
#   find_any_ori_bin  — locate any ori binary (no LLVM check), or exit with error
#   ORI               — set by find_ori_bin to the path of the chosen LLVM binary
#   ORI_INTERP        — set by find_any_ori_bin to the path of any ori binary

# Test whether a binary has LLVM support.
# Returns 0 if LLVM is available, 1 otherwise.
# Note: uses variable capture instead of pipeline to avoid pipefail interaction.
_has_llvm() {
    local output
    output=$("$1" build /dev/null 2>&1 || true)
    [[ "$output" != *"without LLVM"* ]]
}

# Locate an LLVM-enabled ori binary.
# Tries: $ORI_BIN (env), target/debug/ori, target/release/ori, `ori` (PATH).
# Sets ORI to the first candidate with LLVM support.
# Exits with code 2 if no suitable binary is found.
find_ori_bin() {
    local root_dir
    root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"

    if [[ -n "${ORI_BIN:-}" ]]; then
        if _has_llvm "$ORI_BIN"; then
            ORI="$ORI_BIN"
            return
        fi
        echo "Error: ORI_BIN='$ORI_BIN' does not have LLVM support" >&2
        echo "Rebuild with: cargo b (debug) or cargo b --release (release)" >&2
        exit 2
    fi

    # Try candidates in order: debug first (cargo b builds debug with LLVM),
    # then release, then PATH.
    local candidates=(
        "$root_dir/target/debug/ori"
        "$root_dir/target/release/ori"
        "ori"
    )

    for bin in "${candidates[@]}"; do
        if [[ -x "$bin" ]] && _has_llvm "$bin"; then
            ORI="$bin"
            return
        fi
    done

    echo "Error: no LLVM-enabled ori binary found" >&2
    echo "Tried: ${candidates[*]}" >&2
    echo "Rebuild with: cargo b (debug) or cargo b --release (release)" >&2
    exit 2
}

# Locate an LLVM-enabled ori binary for a specific build profile.
# Usage: find_ori_bin_profile <profile>
#   where <profile> is "debug" or "release"
# Sets ORI_PROFILE to the path if found.
# Exits with code 2 if the binary doesn't exist or lacks LLVM support.
find_ori_bin_profile() {
    local profile="$1"
    local root_dir
    root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"
    local bin="$root_dir/target/$profile/ori"

    if [[ ! -x "$bin" ]]; then
        echo "Error: $profile binary not found at $bin" >&2
        if [[ "$profile" == "release" ]]; then
            echo "Build with: cargo b --release" >&2
        else
            echo "Build with: cargo b" >&2
        fi
        exit 2
    fi

    if ! _has_llvm "$bin"; then
        echo "Error: $profile binary at $bin does not have LLVM support" >&2
        if [[ "$profile" == "release" ]]; then
            echo "Rebuild with: cargo b --release" >&2
        else
            echo "Rebuild with: cargo b" >&2
        fi
        exit 2
    fi

    ORI_PROFILE="$bin"
}

# Validate that both debug and release LLVM-enabled binaries exist.
# Sets ORI_DEBUG and ORI_RELEASE to the respective paths.
# Exits with code 2 if either is missing or lacks LLVM support.
require_both_builds() {
    find_ori_bin_profile debug
    ORI_DEBUG="$ORI_PROFILE"

    find_ori_bin_profile release
    ORI_RELEASE="$ORI_PROFILE"
}

# Locate any ori binary (no LLVM requirement).
# Tries: $ORI_BIN (env), target/debug/ori, target/release/ori, `ori` (PATH).
# Sets ORI_INTERP to the first working candidate.
# Exits with code 2 if no binary is found.
find_any_ori_bin() {
    local root_dir
    root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"

    if [[ -n "${ORI_BIN:-}" ]] && [[ -x "$ORI_BIN" ]]; then
        ORI_INTERP="$ORI_BIN"
        return
    fi

    local candidates=(
        "$root_dir/target/debug/ori"
        "$root_dir/target/release/ori"
        "ori"
    )

    for bin in "${candidates[@]}"; do
        if [[ -x "$bin" ]]; then
            ORI_INTERP="$bin"
            return
        fi
    done

    echo "Error: no ori binary found" >&2
    echo "Tried: ${candidates[*]}" >&2
    echo "Rebuild with: cargo build" >&2
    exit 2
}

# Print a captured command stream without truncation.
# Usage: render_captured_stream <label> <path>
render_captured_stream() {
    local label="$1"
    local path="$2"

    echo "  ${label}:"
    if [[ -s "$path" ]]; then
        sed 's/^/  │ /' "$path"
    else
        echo "  │ (empty)"
    fi
}
