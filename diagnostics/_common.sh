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
        echo "Rebuild with: cargo bl (debug) or cargo blr (release)" >&2
        exit 2
    fi

    # Try candidates in order: debug first (cargo bl builds debug with LLVM),
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
    echo "Rebuild with: cargo bl (debug) or cargo blr (release)" >&2
    exit 2
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
