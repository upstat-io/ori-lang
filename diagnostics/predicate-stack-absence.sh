#!/bin/bash
# Absence proof for the deleted predicate-stack emitter symbols.
#
# Usage:
#   diagnostics/predicate-stack-absence.sh [options]
#
# Options:
#   --compiler-dir DIR  Tree to scan (default: <repo>/compiler)
#   --self-test         Run the falsifier matrix and exit
#   -h, --help          Show this help
#
# Asserts two things that must hold together:
#   1. ABSENCE  — class_payload_of, emit_burden_ops, eliminate_burden_ops
#      resolve to zero files. Spec: Annex E §AIMS; the class-ledger path is the
#      sole logical ownership-event placement authority.
#   2. CONTROL  — the SAME query finds ssa_alias_classes live. A zero here means
#      the query itself is broken, so the absence result proves nothing.
#
# The control is load-bearing: an absence check with no positive control passes
# identically whether the symbols are gone or the scan never ran. ssa_alias_classes
# is legacy-adjacent terminology but is live and load-bearing in the
# intraprocedural alias substrate, so it is the in-tree control and is NEVER part
# of the absence set.
#
# Exit codes:
#   0 = PASS  (absence holds and the control found its known sites)
#   1 = FAIL  (a deleted symbol survives, or the control found nothing)
#   2 = ERROR (usage, missing tree, or missing rg — never reported as PASS)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/_common.sh"

ABSENT_SYMBOLS=(class_payload_of emit_burden_ops eliminate_burden_ops)
CONTROL_SYMBOL=ssa_alias_classes

COMPILER_DIR="$ROOT_DIR/compiler"
SELF_TEST=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --compiler-dir)
            if [[ $# -lt 2 || "$2" == -* ]]; then
                echo "Error: --compiler-dir requires a directory argument" >&2
                echo "Usage: $(basename "$0") [--compiler-dir DIR] [--self-test]" >&2
                exit 2
            fi
            COMPILER_DIR="$2"; shift 2 ;;
        --self-test)    SELF_TEST=1; shift ;;
        -h|--help)      sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *)              echo "Error: unknown argument '$1'" >&2; exit 2 ;;
    esac
done

run_proof() {
    local dir="$1" failures=0 control_count symbol count

    if ! command -v rg >/dev/null 2>&1; then
        echo "Error: rg not found; cannot run the query, so absence is UNKNOWN" >&2
        return 2
    fi
    if [[ ! -d "$dir" ]]; then
        echo "Error: compiler dir not found: $dir" >&2
        return 2
    fi

    # Control first: a dead query makes every absence result meaningless.
    count_symbol_files "$CONTROL_SYMBOL" "$dir" || return 2
    control_count="$COUNT_SYMBOL_FILES_RESULT"
    if [[ "$control_count" -eq 0 ]]; then
        echo "FAIL control  $CONTROL_SYMBOL: 0 files"
        echo "  The query found none of its known live sites, so it cannot"
        echo "  distinguish 'symbol deleted' from 'scan never ran'."
        return 1
    fi
    echo "PASS control  $CONTROL_SYMBOL: $control_count files (query proven live)"

    for symbol in "${ABSENT_SYMBOLS[@]}"; do
        count_symbol_files "$symbol" "$dir" || return 2
        count="$COUNT_SYMBOL_FILES_RESULT"
        if [[ "$count" -eq 0 ]]; then
            echo "PASS absence  $symbol: 0 files"
        else
            echo "FAIL absence  $symbol: $count files"
            rg -l --fixed-strings "$symbol" "$dir" | sed 's/^/                /'
            failures=$((failures + 1))
        fi
    done

    if [[ "$failures" -gt 0 ]]; then
        echo "VERDICT: FAIL — $failures deleted emitter symbol(s) survive"
        return 1
    fi
    echo "VERDICT: PASS — absence holds and the control found its known sites"
    return 0
}

self_test() {
    local tmp rc status=0
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN

    # Case A — clean tree: control present, no deleted symbols. MUST PASS.
    mkdir -p "$tmp/a/aims"
    echo "fn $CONTROL_SYMBOL() {}" > "$tmp/a/aims/alias.rs"
    set +e; run_proof "$tmp/a" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 0 ]]; then echo "PASS self-test A: clean tree passes"
    else echo "FAIL self-test A: clean tree returned $rc, expected 0"; status=1; fi

    # Case B — reverted fix: a deleted symbol is back. MUST FAIL.
    mkdir -p "$tmp/b/aims"
    echo "fn $CONTROL_SYMBOL() {}" > "$tmp/b/aims/alias.rs"
    echo "fn class_payload_of() {}" > "$tmp/b/aims/legacy.rs"
    set +e; run_proof "$tmp/b" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 1 ]]; then echo "PASS self-test B: resurrected symbol fails the proof"
    else echo "FAIL self-test B: returned $rc, expected 1"; status=1; fi

    # Case C — dead query: nothing at all, including the control. MUST FAIL.
    # Without the control this tree is indistinguishable from Case A.
    mkdir -p "$tmp/c/aims"
    echo "fn unrelated() {}" > "$tmp/c/aims/other.rs"
    set +e; run_proof "$tmp/c" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 1 ]]; then echo "PASS self-test C: empty scan fails instead of reporting a clean sweep"
    else echo "FAIL self-test C: returned $rc, expected 1"; status=1; fi

    # Case D — missing tree fails closed as ERROR, never PASS.
    set +e; run_proof "$tmp/does-not-exist" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 2 ]]; then echo "PASS self-test D: missing tree is ERROR, not PASS"
    else echo "FAIL self-test D: returned $rc, expected 2"; status=1; fi

    local self
    self="$SCRIPT_DIR/$(basename "$0")"

    # Cases E-G drive the real CLI as a subprocess, so the argument parser and
    # --compiler-dir path selection are exercised, not just run_proof.

    # E — CLI selects a clean fixture via --compiler-dir. MUST exit 0.
    set +e; "$self" --compiler-dir "$tmp/a" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 0 ]]; then echo "PASS self-test E: --compiler-dir selects a clean tree (0)"
    else echo "FAIL self-test E: returned $rc, expected 0"; status=1; fi

    # F — CLI selects the resurrected-symbol fixture. MUST exit 1.
    # Proves the flag selects the REQUESTED tree, not a default.
    set +e; "$self" --compiler-dir "$tmp/b" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 1 ]]; then echo "PASS self-test F: --compiler-dir selects the resurrected tree (1)"
    else echo "FAIL self-test F: returned $rc, expected 1"; status=1; fi

    # G — missing option value is a usage ERROR with a diagnostic, never a bare 1.
    local out
    set +e; out="$("$self" --compiler-dir 2>&1)"; rc=$?; set -e
    if [[ $rc -eq 2 && "$out" == *"requires a directory argument"* ]]; then
        echo "PASS self-test G: missing --compiler-dir value is ERROR 2 with a diagnostic"
    else
        echo "FAIL self-test G: returned $rc out='$out', expected 2 with a usage diagnostic"; status=1
    fi


    # Runtime query failure: rg EXISTS but exits >1. Distinct from "rg missing".
    # Must be ERROR 2 on every CLI path; a 1 here would mean a real scanner
    # failure is being reported as a substantive verdict.
    mkdir -p "$tmp/shim"
    printf '#!/bin/bash\nexit 2\n' > "$tmp/shim/rg"
    chmod +x "$tmp/shim/rg"

    set +e; PATH="$tmp/shim:$PATH" "$self" --compiler-dir "$tmp/a" >/dev/null 2>&1; rc=$?; set -e
    if [[ $rc -eq 2 ]]; then echo "PASS self-test H: rg exiting >1 is ERROR 2, not a verdict"
    else echo "FAIL self-test H: returned $rc, expected 2"; status=1; fi

    # Case I — ignore-negation enumeration: an ignored PARENT with a re-included
    # TRACKED child, scanned with the child's ancestor as the scan root. This
    # case covers the TRACKED half of the corpus only; Case J covers the
    # untracked half. A scanner's own
    # ignore-walker may drop that file (ripgrep 14.1.0 does; git and 15.2.0 do
    # not), which would make every absence proof here silently incomplete.
    # The helper enumerates through git, so the file MUST be counted.
    mkdir -p "$tmp/i/repo/nested/pkg/src/build"
    (
        cd "$tmp/i/repo" || exit 1
        git init -q . >/dev/null 2>&1 || exit 1
        git config user.email selftest@local >/dev/null 2>&1
        git config user.name selftest >/dev/null 2>&1
        printf '**/build/\n!nested/pkg/src/build/\n' > .gitignore
        printf 'fn %s() {}\n' "$CONTROL_SYMBOL" > nested/pkg/src/build/mod.rs
        git add -A -f .gitignore nested/pkg/src/build/mod.rs >/dev/null 2>&1
        git commit -qm selftest >/dev/null 2>&1
    ) || { echo "FAIL self-test I: fixture setup failed"; status=1; }

    if [[ $status -eq 0 ]]; then
        set +e
        (
            cd "$tmp/i/repo" || exit 3
            count_symbol_files "$CONTROL_SYMBOL" nested
            [[ "$COUNT_SYMBOL_FILES_RESULT" -eq 1 ]]
        )
        rc=$?
        set -e
        if [[ $rc -eq 0 ]]; then
            echo "PASS self-test I: tracked child under an ignored parent is enumerated"
        else
            echo "FAIL self-test I: re-included tracked child was not counted (corpus came from the scanner walk, not git)"
            status=1
        fi
    fi

    # Case J — mixed corpus: one TRACKED file beside one UNTRACKED file, both
    # carrying the symbol. An enumerator that returns tracked files whenever any
    # exist would count 1 and report the untracked occurrence absent — a
    # false-clean absence verdict in the admitting direction. The corpus MUST be
    # the union, so both files are counted.
    mkdir -p "$tmp/j/repo/src"
    (
        cd "$tmp/j/repo" || exit 1
        git init -q . >/dev/null 2>&1 || exit 1
        git config user.email selftest@local >/dev/null 2>&1
        git config user.name selftest >/dev/null 2>&1
        printf 'fn %s() {}\n' "$CONTROL_SYMBOL" > src/tracked.rs
        git add -A src/tracked.rs >/dev/null 2>&1
        git commit -qm selftest >/dev/null 2>&1
        printf 'fn %s() {}\n' "$CONTROL_SYMBOL" > src/untracked.rs
    ) || { echo "FAIL self-test J: fixture setup failed"; status=1; }

    if [[ $status -eq 0 ]]; then
        set +e
        (
            cd "$tmp/j/repo" || exit 3
            count_symbol_files "$CONTROL_SYMBOL" src
            [[ "$COUNT_SYMBOL_FILES_RESULT" -eq 2 ]]
        )
        rc=$?
        set -e
        if [[ $rc -eq 0 ]]; then
            echo "PASS self-test J: tracked and untracked files are both enumerated"
        else
            echo "FAIL self-test J: mixed corpus miscounted (enumerator returned tracked-only, not the union)"
            status=1
        fi
    fi

    # Case K — record separator: ONE file whose NAME contains newlines. A scanner
    # emitting newline-delimited paths makes that single file read as several
    # records, so a count inflates and a de-duplication corrupts. The direction is
    # admitting: one crafted or accidental name could satisfy a protected floor by
    # itself. The helper keeps NUL as the separator end to end, so the physical
    # file MUST count exactly once.
    mkdir -p "$tmp/k"
    local weird="$tmp/k/one"$'\n'"file"$'\n'"with"$'\n'"embedded"$'\n'"newlines"$'\n'"here.rs"
    printf 'fn %s() {}\n' "$CONTROL_SYMBOL" > "$weird"
    local disk_count
    disk_count="$(find "$tmp/k" -type f -printf 'x' | wc -c | tr -d ' ')"
    set +e
    count_symbol_files "$CONTROL_SYMBOL" "$tmp/k"
    rc=$?
    set -e
    if [[ $rc -eq 0 && "$disk_count" -eq 1 && "$COUNT_SYMBOL_FILES_RESULT" -eq 1 ]]; then
        echo "PASS self-test K: a newline-bearing filename counts once"
    else
        echo "FAIL self-test K: disk=$disk_count counted=$COUNT_SYMBOL_FILES_RESULT rc=$rc (expected 1/1/0; newline in a filename was read as a record separator)"
        status=1
    fi

    if [[ $status -eq 0 ]]; then echo "SELF-TEST: PASS"; else echo "SELF-TEST: FAIL"; fi
    return $status
}

if [[ "$SELF_TEST" -eq 1 ]]; then
    self_test
    exit $?
fi

set +e
run_proof "$COMPILER_DIR"
rc=$?
set -e
exit $rc
