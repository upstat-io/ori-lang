#!/bin/bash
# Dump the entry-point ownership seam for an Ori source file.
#
# Usage:
#   diagnostics/entry-ownership.sh [options] <file.ori>
#   diagnostics/entry-ownership.sh --compare <a.ori> <b.ori>
#
# Options:
#   --compare <a> <b>  Render both programs' seams side by side and report
#                      which columns differ
#   --raw              Emit the report verbatim, no header decoration
#   --color            Force color output (default: auto-detect terminal)
#   --no-color         Disable color output
#   -h, --help         Show this help
#
# Captures the seam via ORI_DUMP_ENTRY_OWNERSHIP=1 — the C main() wrapper's
# argv cleanup decision beside the AIMS facts that govern it. One block per
# entry-point parameter carries:
#   - semantic: ParamContract access / consumption / cardinality /
#     iter_consumes / transfers_through_return / borrowed_read_only /
#     borrowed_cow_consumed / escape / share / uniqueness / exact_transfer,
#     the RL-2 boundary verdict (callee_owner_demand), the realized
#     ArcParam ownership, and the borrowed-rooted flag
#   - physical: param_passing and the wrapper's wrapper_owns_on_normal flag
#   - the EMIT/SKIP verdict and ACTIVE/INACTIVE state of every
#     ori_args_cleanup site across both exception-handling legs
#   - seam: CONSISTENT when the physical decision agrees with the semantic
#     owner demand, DIVERGENT when it does not
#
# The dump is read-only: it changes no cleanup emission.
#
# --compare answers "do these two programs' seams differ, and where" in one
# command. Two programs with identical param_passing and identical
# wrapper_owns_on_normal can still differ in the semantic columns; that
# contrast is the seam this script exists to surface.
#
# For ARC IR use arc-dump.sh; for LLVM IR use ir-dump.sh.
#
# Environment:
#   ORI_BIN    Override path to ori binary (default: auto-detect build)
#
# Exit codes:
#   0 = success (in --compare mode: reports ran, regardless of difference)
#   1 = compilation failed, or no seam was captured (no @main, or no params)
#   2 = usage error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"
find_any_ori_bin
ORI="$ORI_INTERP"

# --- Defaults ---
RAW=0
USE_COLOR=auto
COMPARE=0
FILES=()

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --compare) COMPARE=1; shift ;;
        --raw) RAW=1; shift ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        -*)
            echo "Error: unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
        *)
            FILES+=("$1"); shift
            ;;
    esac
done

if [[ "$COMPARE" -eq 1 ]]; then
    if [[ ${#FILES[@]} -ne 2 ]]; then
        echo "Error: --compare requires exactly two files" >&2
        exit 2
    fi
else
    if [[ ${#FILES[@]} -ne 1 ]]; then
        echo "Error: exactly one input file is required" >&2
        echo "Usage: diagnostics/entry-ownership.sh [options] <file.ori>" >&2
        exit 2
    fi
fi

for f in "${FILES[@]}"; do
    if [[ ! -f "$f" ]]; then
        echo "Error: file not found: $f" >&2
        exit 2
    fi
done

# --- Resolve color mode ---
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then USE_COLOR=yes; else USE_COLOR=no; fi
fi

if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_CYAN='\033[0;36m'
    C_BOLD='\033[1m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_CYAN="" C_BOLD="" C_NC=""
fi

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# capture_seam <file> <out-path>
# Emits the seam block to <out-path>. Returns 1 when nothing was captured.
capture_seam() {
    local src="$1" out="$2" raw="$tmpdir/raw.$$.txt" build_exit=0
    ORI_DUMP_ENTRY_OWNERSHIP=1 "$ORI" build "$src" -o "$tmpdir/bin.$$" \
        2>"$raw" >/dev/null || build_exit=$?

    sed -n '/^=== entry-point ownership seam:/,/^    seam:/p' "$raw" > "$out"

    if [[ ! -s "$out" ]]; then
        if [[ "$build_exit" -ne 0 ]]; then
            grep -v '^=== entry-point ownership seam' "$raw" >&2 || true
            echo "Error: compilation of $src failed and no seam was captured" >&2
            return 1
        fi
        echo "Error: no entry-point ownership seam captured for $src" >&2
        echo "       (the program may have no @main, or an @main with no parameters)" >&2
        return 1
    fi
    return 0
}

annotate() {
    local file="$1"
    if [[ "$RAW" -eq 1 ]]; then
        cat "$file"
        return
    fi
    sed \
        -e "s/\(seam: CONSISTENT\)/$(printf '%b' "$C_GREEN")\1$(printf '%b' "$C_NC")/" \
        -e "s/\(seam: DIVERGENT\)/$(printf '%b' "$C_RED")\1$(printf '%b' "$C_NC")/" \
        -e "s/\(=== entry-point ownership seam:.*\)/$(printf '%b' "$C_BOLD")\1$(printf '%b' "$C_NC")/" \
        "$file"
}

if [[ "$COMPARE" -eq 0 ]]; then
    capture_seam "${FILES[0]}" "$tmpdir/seam.txt" || exit 1
    annotate "$tmpdir/seam.txt"
    exit 0
fi

# --- Compare mode ---
capture_seam "${FILES[0]}" "$tmpdir/a.txt" || exit 1
capture_seam "${FILES[1]}" "$tmpdir/b.txt" || exit 1

printf '%b=== A: %s ===%b\n' "$C_BOLD" "${FILES[0]}" "$C_NC"
annotate "$tmpdir/a.txt"
printf '\n%b=== B: %s ===%b\n' "$C_BOLD" "${FILES[1]}" "$C_NC"
annotate "$tmpdir/b.txt"

printf '\n%b=== differing fields ===%b\n' "$C_CYAN" "$C_NC"
# Compare per-field: a report line is `<label> = <value>` or a cleanup-site row.
# Emit one row per label whose values disagree.
if diff <(sed 's/[[:space:]]\+/ /g' "$tmpdir/a.txt") \
        <(sed 's/[[:space:]]\+/ /g' "$tmpdir/b.txt") >/dev/null; then
    echo "  (none — the two seams are identical)"
else
    paste -d'\t' \
        <(sed 's/[[:space:]]\+/ /g' "$tmpdir/a.txt") \
        <(sed 's/[[:space:]]\+/ /g' "$tmpdir/b.txt") \
        | awk -F'\t' '$1 != $2 { printf "  A: %s\n  B: %s\n", $1, $2 }'
fi
