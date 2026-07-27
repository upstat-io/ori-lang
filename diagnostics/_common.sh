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

# Print a monotonic nanosecond timestamp for elapsed-time measurements.
monotonic_now_ns() {
    python3 -c 'import time; print(time.monotonic_ns())'
}

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

# Enumerate the scan corpus under one or more paths, NUL-separated on stdout.
# Usage: enumerate_corpus <path>...
# Returns 0 on success (an empty corpus is a valid success), 2 when no
# enumerator can run (no git work tree AND no usable `find`).
#
# Corpus identity never comes from the SCANNER's directory walk. A scanner's
# ignore-walker is host- and version-dependent: ripgrep 14.1.0 drops a tracked
# file re-included by a `!path` negation under an ignored parent when the scan
# root is a subdirectory, while git and ripgrep 15.2.0 keep it. Callers pass
# EXPLICIT paths to the scanner so it performs no walk and cannot alter the set.
#
# Inside a work tree git is the enumerator, and it enumerates the UNION of
# tracked files and relevant untracked files (`--others --exclude-standard`).
# The union is required, not an either/or: an absence proof that enumerated
# only tracked files would report a clean sweep while an untracked file beside
# them carries the very symbol being proven absent. Ignored build artifacts stay
# out, and git -- not the scanner -- decides what "ignored" means.
#
# Outside a work tree -- a fixture or temp tree -- `find` is the enumerator: it
# has no ignore logic at all, so it is equally host-independent. A git
# enumeration that yields nothing while files exist on disk falls back to
# `find`, so an untracked or ignored fixture tree is never silently reported as
# an empty corpus.
#
# The result is de-duplicated: overlapping caller paths (`dir` and `dir/sub`)
# would otherwise list one file twice and inflate every count.
enumerate_corpus() {
    local rc tmp used_git=0
    tmp="$(mktemp)" || { echo "Error: mktemp failed" >&2; return 2; }
    local had_e=0
    [[ $- == *e* ]] && had_e=1
    set +e
    if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        git ls-files -z -- "$@" >> "$tmp" 2>/dev/null
        git ls-files -z --others --exclude-standard -- "$@" >> "$tmp" 2>/dev/null
        [[ -s "$tmp" ]] && used_git=1
    fi
    if [[ $used_git -eq 0 ]]; then
        : > "$tmp"
        if ! command -v find >/dev/null 2>&1; then
            [[ $had_e -eq 1 ]] && set -e
            rm -f "$tmp"
            echo "Error: no usable corpus enumerator (no git work tree, no find)" >&2
            return 2
        fi
        find "$@" -type f -print0 > "$tmp" 2>/dev/null
        rc=$?
        if [[ $rc -ne 0 ]]; then
            [[ $had_e -eq 1 ]] && set -e
            rm -f "$tmp"
            echo "Error: find failed (exit $rc) enumerating the corpus" >&2
            return 2
        fi
    fi
    [[ $had_e -eq 1 ]] && set -e
    # Dedup with a read-loop, never `mapfile`: this function must stay usable
    # from any shell that can source the file. A missing builtin here would
    # print to stderr, emit an EMPTY corpus, and still return 0 -- a clean
    # absence verdict for a corpus that was never scanned.
    # The corpus is the set of files present in the WORKING TREE. `git ls-files`
    # reports the INDEX, so a tracked path deleted or renamed without staging is
    # listed but absent on disk. Emitting it hands a scanner a path it cannot
    # open: `rg` exits 2, and a chunked caller discards its whole batch. Filtering
    # here makes that state unrepresentable rather than detected downstream.
    local -A __seen=()
    local __f
    while IFS= read -r -d '' __f; do
        [[ -n "$__f" ]] || continue
        [[ -e "$__f" ]] || continue
        if [[ -z "${__seen[$__f]:-}" ]]; then
            __seen["$__f"]=1
            printf '%s\0' "$__f"
        fi
    done < "$tmp"
    rm -f "$tmp"
    return 0
}

# Read the enumerated corpus into a caller-named bash array, NUL-safe.
# Usage: corpus_files_into <array-name> <path>...
# Returns 0 on success (an empty corpus is a valid success), 2 on failure.
#
# The enumeration goes through a temp file, never `$(...)`: command
# substitution silently DROPS NUL bytes, which would corrupt a NUL-separated
# path list and reunite the corpus with the whitespace-splitting bugs the NUL
# separator exists to prevent.
corpus_files_into() {
    local __arr="$1"; shift
    local tmp rc
    # Fail closed when the array builtin is absent: without this guard the
    # caller receives an EMPTY array and a success return, which every consumer
    # reads as "no matches" rather than "the corpus was never read".
    if ! type mapfile >/dev/null 2>&1; then
        echo "Error: 'mapfile' unavailable; run this helper under bash 4.4+" >&2
        return 2
    fi
    tmp="$(mktemp)" || return 2
    enumerate_corpus "$@" > "$tmp"
    rc=$?
    if [[ $rc -ne 0 ]]; then
        rm -f "$tmp"
        return 2
    fi
    mapfile -d '' -t "$__arr" < "$tmp"
    rm -f "$tmp"
    return 0
}

# Scan EXPLICIT files for a fixed-string symbol; matched paths into a named array.
# Usage: scan_symbol_into <array-name> <symbol> <file>...
# Returns 0 on success (no match is a valid success), 2 on a real query failure.
#
# The record separator stays NUL from ripgrep to the array. `rg -l` alone emits
# NEWLINE-delimited paths, so a filename containing a newline is read as several
# records: one physical file then inflates a count and corrupts a de-duplication.
# That direction is admitting -- a single crafted or accidental name could
# satisfy a protected floor on its own -- so `-0` and NUL-aware reading are
# load-bearing, not stylistic. Output goes through a temp file, never `$(...)`,
# because command substitution silently DROPS NUL bytes.
scan_symbol_into() {
    local __arr="$1" symbol="$2"; shift 2
    local tmp rc had_e=0
    if ! type mapfile >/dev/null 2>&1; then
        echo "Error: 'mapfile' unavailable; run this helper under bash 4.4+" >&2
        return 2
    fi
    tmp="$(mktemp)" || { echo "Error: mktemp failed" >&2; return 2; }
    [[ $- == *e* ]] && had_e=1
    set +e
    rg -0 -l --fixed-strings "$symbol" "$@" > "$tmp" 2>/dev/null
    rc=$?
    [[ $had_e -eq 1 ]] && set -e
    if [[ $rc -gt 1 ]]; then
        rm -f "$tmp"
        echo "Error: rg failed (exit $rc) scanning for '$symbol'" >&2
        return 2
    fi
    mapfile -d '' -t "$__arr" < "$tmp"
    rm -f "$tmp"
    return 0
}

# Scan EXPLICIT files for a case-insensitive REGEX; matched paths into a named array.
# Usage: scan_pattern_into <array-name> <regex> <file>...
# Returns 0 on success (no match is a valid success), 2 on a real query failure.
#
# Regex sibling of scan_symbol_into. The NUL record separator, the temp file, and
# the `rc > 1` failure split are load-bearing for the same reasons stated there;
# the ONLY difference is that ripgrep interprets the pattern instead of matching
# it literally. A caller needing a literal match uses scan_symbol_into.
scan_pattern_into() {
    local __arr="$1" pattern="$2"; shift 2
    local tmp rc had_e=0
    if ! type mapfile >/dev/null 2>&1; then
        echo "Error: 'mapfile' unavailable; run this helper under bash 4.4+" >&2
        return 2
    fi
    tmp="$(mktemp)" || { echo "Error: mktemp failed" >&2; return 2; }
    [[ $- == *e* ]] && had_e=1
    set +e
    rg -0 -l -i "$pattern" "$@" > "$tmp" 2>/dev/null
    rc=$?
    [[ $had_e -eq 1 ]] && set -e
    if [[ $rc -gt 1 ]]; then
        rm -f "$tmp"
        echo "Error: rg failed (exit $rc) scanning for pattern '$pattern'" >&2
        return 2
    fi
    mapfile -d '' -t "$__arr" < "$tmp"
    rm -f "$tmp"
    return 0
}

# Count files under a tree containing a fixed-string symbol.
# Usage: count_symbol_files <symbol> <dir>
# Sets COUNT_SYMBOL_FILES_RESULT to the count of DISTINCT matching files.
# Returns 2 on a real query failure (missing rg/git, missing dir, or rg exiting >1).
#
# The result travels in a GLOBAL, never on stdout: a caller writing
# `n="$(count_symbol_files ...)"` runs this in a subshell, where a non-zero exit
# is discarded and an empty value silently becomes a zero count. A zero is
# evidence in every consumer, so a swallowed query error must never produce one.
count_symbol_files() {
    local symbol="$1" dir="$2"
    COUNT_SYMBOL_FILES_RESULT=0
    if ! command -v rg >/dev/null 2>&1; then
        echo "Error: rg not found; cannot run the query" >&2
        return 2
    fi
    if [[ ! -d "$dir" ]]; then
        echo "Error: directory not found: $dir" >&2
        return 2
    fi
    local -a files=()
    corpus_files_into files "$dir" || return 2
    if [[ ${#files[@]} -eq 0 ]]; then
        return 0
    fi
    local -a matched=()
    scan_symbol_into matched "$symbol" "${files[@]}" || return 2
    local -A __seen=()
    local __m __n=0
    for __m in "${matched[@]}"; do
        [[ -n "$__m" ]] || continue
        [[ -n "${__seen[$__m]:-}" ]] && continue
        __seen["$__m"]=1
        __n=$((__n + 1))
    done
    COUNT_SYMBOL_FILES_RESULT="$__n"
    return 0
}

# List files under a tree containing any of the given symbols.
# Usage: list_symbol_files <dir> <symbol>...
# Sets LIST_SYMBOL_FILES_RESULT to a newline-separated sorted-unique list for
# display, and LIST_SYMBOL_FILES_COUNT to the count of DISTINCT matching files.
# De-duplication is NUL-based, so the COUNT is correct even for a name the
# newline-joined display list cannot faithfully render.
# Returns 2 on a real query failure. Never masks an rg error with `|| true`:
# an empty list must mean "no matches", never "the scan failed".
list_symbol_files() {
    local dir="$1" symbol
    shift
    LIST_SYMBOL_FILES_RESULT=""
    LIST_SYMBOL_FILES_COUNT=0
    if ! command -v rg >/dev/null 2>&1; then
        echo "Error: rg not found; cannot run the query" >&2
        return 2
    fi
    if [[ ! -d "$dir" ]]; then
        echo "Error: directory not found: $dir" >&2
        return 2
    fi
    local -a files=()
    corpus_files_into files "$dir" || return 2
    if [[ ${#files[@]} -eq 0 ]]; then
        return 0
    fi
    local -A __seen=()
    local -a __unique=()
    local -a matched=()
    local __m
    for symbol in "$@"; do
        matched=()
        scan_symbol_into matched "$symbol" "${files[@]}" || return 2
        for __m in "${matched[@]}"; do
            [[ -n "$__m" ]] || continue
            [[ -n "${__seen[$__m]:-}" ]] && continue
            __seen["$__m"]=1
            __unique+=("$__m")
        done
    done
    LIST_SYMBOL_FILES_COUNT="${#__unique[@]}"
    if [[ ${#__unique[@]} -gt 0 ]]; then
        LIST_SYMBOL_FILES_RESULT="$(printf '%s\n' "${__unique[@]}" | LC_ALL=C sort)"
    fi
    return 0
}
