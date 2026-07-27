#!/bin/bash
# Negative filters protecting live code from a terminology-matched sweep.
#
# Usage:
#   diagnostics/sweep-negative-filter.sh [--check] [--compiler-dir DIR]
#   diagnostics/sweep-negative-filter.sh --is-protected PATH
#   diagnostics/sweep-negative-filter.sh --list
#   diagnostics/sweep-negative-filter.sh --self-test
#
# Options:
#   --check             Assert every protected surface is still live (default)
#   --is-protected PATH  Exit 0 if PATH requires manual disposition, 1 if not
#   --list              Print the protected file set
#   --compiler-dir DIR  Tree to scan (default: <repo>/compiler)
#   --self-test         Run the falsifier matrix and exit
#   -h, --help          Show this help
#
# A sweep that enumerates by predicate-stack TERMINOLOGY matches two live
# surfaces that are not legacy. Neither may be deleted, renamed, or gutted on a
# terminology match; a hit inside either requires manual disposition.
#
#   burden_lowering     the live unified lowering surface
#   ssa_alias_classes   the live intraprocedural alias substrate
#
# The guard is a floor, not an equality: a surface GROWING is fine, SHRINKING is
# the sweep eating live code. Counts are minimums, re-measured on every run.
#
# Exit codes:
#   0 = PASS  (--check: every surface live; --is-protected: PATH is protected)
#   1 = FAIL  (--check: a surface shrank or vanished; --is-protected: not protected)
#   2 = ERROR (usage, missing tree, or missing rg — never reported as PASS)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/_common.sh"

# Protected surface: symbol => minimum live file count.
PROTECTED_SYMBOLS=(burden_lowering ssa_alias_classes)
PROTECTED_FLOORS=(1 7)

COMPILER_DIR="$ROOT_DIR/compiler"
MODE=check
TARGET_PATH=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) MODE=check; shift ;;
        --list)  MODE=list; shift ;;
        --is-protected)
            if [[ $# -lt 2 || "$2" == -* ]]; then
                echo "Error: --is-protected requires a path argument" >&2
                echo "Usage: $(basename "$0") --is-protected PATH" >&2
                exit 2
            fi
            MODE=is_protected; TARGET_PATH="$2"; shift 2 ;;
        --compiler-dir)
            if [[ $# -lt 2 || "$2" == -* ]]; then
                echo "Error: --compiler-dir requires a directory argument" >&2
                echo "Usage: $(basename "$0") [--compiler-dir DIR]" >&2
                exit 2
            fi
            COMPILER_DIR="$2"; shift 2 ;;
        --self-test) MODE=self_test; shift ;;
        -h|--help)   sed -n '2,34p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)           echo "Error: unknown argument '$1'" >&2; exit 2 ;;
    esac
done

# Preconditions must be checked by the CALLER, not inside a command
# substitution: an `exit 2` from a $(...) subshell only kills that subshell and
# degrades to an unclassified 1 in the parent.
require_scannable() {
    local dir="$1"
    if ! command -v rg >/dev/null 2>&1; then
        echo "Error: rg not found; cannot run the query" >&2
        return 2
    fi
    if [[ ! -d "$dir" ]]; then
        echo "Error: compiler dir not found: $dir" >&2
        return 2
    fi
    return 0
}

# Sets PROTECTED_FILE_LIST; returns 2 on a real query failure.
collect_protected() {
    local dir="$1"
    PROTECTED_FILE_LIST=""
    list_symbol_files "$dir" "${PROTECTED_SYMBOLS[@]}" || return 2
    PROTECTED_FILE_LIST="$LIST_SYMBOL_FILES_RESULT"
    return 0
}

run_check() {
    local dir="$1" i symbol floor count failures=0

    require_scannable "$dir" || return 2

    for i in "${!PROTECTED_SYMBOLS[@]}"; do
        symbol="${PROTECTED_SYMBOLS[$i]}"
        floor="${PROTECTED_FLOORS[$i]}"
        count_symbol_files "$symbol" "$dir" || return 2
        count="$COUNT_SYMBOL_FILES_RESULT"
        if [[ "$count" -ge "$floor" ]]; then
            echo "PASS protected  $symbol: $count files (floor $floor)"
        else
            echo "FAIL protected  $symbol: $count files, below floor $floor"
            echo "  A terminology sweep must not delete, rename, or gut this surface."
            failures=$((failures + 1))
        fi
    done

    if [[ "$failures" -gt 0 ]]; then
        echo "VERDICT: FAIL — $failures protected surface(s) shrank"
        return 1
    fi
    echo "VERDICT: PASS — every protected surface is live"
    return 0
}

run_is_protected() {
    local dir="$1" target="$2" f

    require_scannable "$dir" || return 2
    collect_protected "$dir" || return 2
    # Compare by basename-anchored suffix so callers may pass absolute or
    # repo-relative paths.
    while IFS= read -r f; do
        [[ -z "$f" ]] && continue
        if [[ "$f" == *"$target" || "$target" == *"$f" ]]; then
            echo "PROTECTED $target"
            echo "  Manual disposition required; automatic deletion is banned."
            return 0
        fi
    done <<< "$PROTECTED_FILE_LIST"
    echo "not-protected $target"
    return 1
}

self_test() {
    local tmp rc status=0 self
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN
    self="$SCRIPT_DIR/$(basename "$0")"

    # Intact tree: both surfaces at or above floor. MUST PASS.
    mkdir -p "$tmp/ok"
    echo "mod burden_lowering;" > "$tmp/ok/emit_unified.rs"
    local n
    for n in 1 2 3 4 5 6 7; do echo "use ssa_alias_classes;" > "$tmp/ok/alias$n.rs"; done
    set +e; "$self" --check --compiler-dir "$tmp/ok" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 0 ]]; then echo "PASS self-test A: intact surfaces pass"
    else echo "FAIL self-test A: returned $rc, expected 0"; status=1; fi

    # Swept tree: burden_lowering deleted. MUST FAIL — this is the sweep eating live code.
    mkdir -p "$tmp/swept"
    for n in 1 2 3 4 5 6 7; do echo "use ssa_alias_classes;" > "$tmp/swept/alias$n.rs"; done
    set +e; "$self" --check --compiler-dir "$tmp/swept" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 1 ]]; then echo "PASS self-test B: deleted burden_lowering fails the guard"
    else echo "FAIL self-test B: returned $rc, expected 1"; status=1; fi

    # Partially swept: alias substrate gutted below its floor. MUST FAIL.
    mkdir -p "$tmp/gutted"
    echo "mod burden_lowering;" > "$tmp/gutted/emit_unified.rs"
    echo "use ssa_alias_classes;" > "$tmp/gutted/alias1.rs"
    set +e; "$self" --check --compiler-dir "$tmp/gutted" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 1 ]]; then echo "PASS self-test C: gutted alias substrate fails the guard"
    else echo "FAIL self-test C: returned $rc, expected 1"; status=1; fi

    # Growth is not a failure: a floor, not an equality.
    mkdir -p "$tmp/grown"
    echo "mod burden_lowering;" > "$tmp/grown/emit_unified.rs"
    echo "mod burden_lowering;" > "$tmp/grown/emit_unified2.rs"
    for n in 1 2 3 4 5 6 7 8; do echo "use ssa_alias_classes;" > "$tmp/grown/alias$n.rs"; done
    set +e; "$self" --check --compiler-dir "$tmp/grown" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 0 ]]; then echo "PASS self-test D: a grown surface passes (floor, not equality)"
    else echo "FAIL self-test D: returned $rc, expected 0"; status=1; fi

    # The consumable predicate: a protected file, and an unrelated one.
    set +e; "$self" --is-protected emit_unified.rs --compiler-dir "$tmp/ok" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 0 ]]; then echo "PASS self-test E: --is-protected identifies a protected file"
    else echo "FAIL self-test E: returned $rc, expected 0"; status=1; fi

    echo "fn unrelated() {}" > "$tmp/ok/unrelated.rs"
    set +e; "$self" --is-protected unrelated.rs --compiler-dir "$tmp/ok" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 1 ]]; then echo "PASS self-test F: --is-protected rejects an unrelated file"
    else echo "FAIL self-test F: returned $rc, expected 1"; status=1; fi

    # Usage boundaries are ERROR 2 with a diagnostic, never a bare 1.
    local out
    set +e; out="$("$self" --is-protected 2>&1)"; rc=$?; set -e
    if [[ $rc -eq 2 && "$out" == *"requires a path argument"* ]]; then
        echo "PASS self-test G: missing --is-protected value is ERROR 2 with a diagnostic"
    else echo "FAIL self-test G: returned $rc out='$out', expected 2"; status=1; fi

    set +e; out="$("$self" --compiler-dir 2>&1)"; rc=$?; set -e
    if [[ $rc -eq 2 && "$out" == *"requires a directory argument"* ]]; then
        echo "PASS self-test H: missing --compiler-dir value is ERROR 2 with a diagnostic"
    else echo "FAIL self-test H: returned $rc out='$out', expected 2"; status=1; fi

    set +e; "$self" --check --compiler-dir "$tmp/does-not-exist" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 2 ]]; then echo "PASS self-test I: missing tree is ERROR, not PASS"
    else echo "FAIL self-test I: returned $rc, expected 2"; status=1; fi


    # Runtime query failure: rg EXISTS but exits >1. Distinct from "rg missing".
    # Must be ERROR 2 on every CLI path; a 1 here would mean a real scanner
    # failure is being reported as a substantive verdict.
    mkdir -p "$tmp/shim"
    printf '#!/bin/bash\nexit 2\n' > "$tmp/shim/rg"
    chmod +x "$tmp/shim/rg"

    set +e; PATH="$tmp/shim:$PATH" "$self" --check --compiler-dir "$tmp/ok" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 2 ]]; then echo "PASS self-test J: --check with rg exiting >1 is ERROR 2"
    else echo "FAIL self-test J: returned $rc, expected 2"; status=1; fi

    set +e; PATH="$tmp/shim:$PATH" "$self" --is-protected emit_unified.rs --compiler-dir "$tmp/ok" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 2 ]]; then echo "PASS self-test K: --is-protected with rg exiting >1 is ERROR 2, never not-protected"
    else echo "FAIL self-test K: returned $rc, expected 2"; status=1; fi

    set +e; PATH="$tmp/shim:$PATH" "$self" --list --compiler-dir "$tmp/ok" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 2 ]]; then echo "PASS self-test L: --list with rg exiting >1 is ERROR 2"
    else echo "FAIL self-test L: returned $rc, expected 2"; status=1; fi

    if [[ $status -eq 0 ]]; then echo "SELF-TEST: PASS"; else echo "SELF-TEST: FAIL"; fi
    return $status
}

case "$MODE" in
    self_test)    set +e; self_test; rc=$?; set -e; exit $rc ;;
    list)         collect_protected "$COMPILER_DIR" || exit 2; printf '%s\n' "$PROTECTED_FILE_LIST"; exit 0 ;;
    is_protected) set +e; run_is_protected "$COMPILER_DIR" "$TARGET_PATH"; rc=$?; set -e; exit $rc ;;
    check)        set +e; run_check "$COMPILER_DIR"; rc=$?; set -e; exit $rc ;;
esac
