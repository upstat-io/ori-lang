#!/bin/bash
# Compile an Ori source file and disassemble with Ori-aware demangling.
#
# Usage:
#   diagnostics/disasm-ori.sh [options] <file.ori>
#
# Options:
#   --all              Include runtime functions (ori_rc_*, ori_list_*, etc.)
#   --function <name>  Show only the named function (Ori name, e.g. @main)
#   --symbols          List function symbols only (no disassembly)
#   --color            Force color output (default: auto-detect terminal)
#   --no-color         Disable color output
#   -h, --help         Show this help
#
# Demangling:
#   _ori_main         → @main
#   _ori_math$add     → math.add       (module separator)
#   _ori_int$$Eq$eq   → int impl Eq.eq (trait impl)
#   _ori_drop$97      → drop<97>       (generated drop)
#   ori_rc_alloc      → [rt] ori_rc_alloc  (with --all)
#
# Environment:
#   ORI_BIN    Override path to ori binary (default: auto-detect LLVM-enabled build)
#
# Requires: ori compiler with LLVM support, objdump
#
# Exit codes:
#   0 = success
#   1 = compilation failed
#   2 = usage error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Locate ori binary ---
if [[ -n "${ORI_BIN:-}" ]]; then
    ORI="$ORI_BIN"
elif [[ -x "$ROOT_DIR/target/release/ori" ]]; then
    ORI="$ROOT_DIR/target/release/ori"
elif [[ -x "$ROOT_DIR/target/debug/ori" ]]; then
    ORI="$ROOT_DIR/target/debug/ori"
else
    ORI="ori"
fi

# --- Defaults ---
SHOW_ALL=0
USE_COLOR=auto
FILTER_FN=""
SYMBOLS_ONLY=0
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --all) SHOW_ALL=1; shift ;;
        --symbols) SYMBOLS_ONLY=1; shift ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        --function)
            if [[ $# -lt 2 ]]; then
                echo "Error: --function requires an argument" >&2
                exit 2
            fi
            FILTER_FN="$2"; shift 2
            ;;
        --function=*) FILTER_FN="${1#--function=}"; shift ;;
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
            if [[ -n "$FILE" ]]; then
                echo "Error: multiple files specified" >&2
                exit 2
            fi
            FILE="$1"; shift
            ;;
    esac
done

if [[ -z "$FILE" ]]; then
    echo "Error: no input file specified" >&2
    echo "Usage: diagnostics/disasm-ori.sh [options] <file.ori>" >&2
    exit 2
fi

if [[ ! -f "$FILE" ]]; then
    echo "Error: file not found: $FILE" >&2
    exit 2
fi

if ! command -v objdump &>/dev/null; then
    echo "Error: objdump not found (install binutils)" >&2
    exit 2
fi

# --- Resolve color mode ---
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then
        USE_COLOR=yes
    else
        USE_COLOR=no
    fi
fi

# --- Compile to native binary ---
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

echo "Compiling $(basename "$FILE")..." >&2
if ! "$ORI" build "$FILE" -o "$tmpdir/binary" 2>"$tmpdir/build_err.txt"; then
    cat "$tmpdir/build_err.txt" >&2
    echo "Error: compilation failed" >&2
    exit 1
fi

# --- Convert Ori function name to mangled symbol for filtering ---
# @main → _ori_main, math.add → _ori_math$add
ori_name_to_symbol() {
    local name="$1"
    # Strip @ prefix
    name="${name#@}"
    # Convert . to $ (module separator)
    name="${name//\./$}"
    echo "_ori_${name}"
}

# --- Demangle function: mangled symbol → Ori name ---
demangle_ori() {
    sed -E \
        -e 's/<_ori_drop\$([0-9]+)>/<drop<\1>>/g' \
        -e 's/<_ori_([a-zA-Z0-9_]+)\$\$([A-Za-z0-9_]+)\$([a-zA-Z0-9_]+)>/<\1 impl \2.\3>/g' \
        -e 's/<_ori_([a-zA-Z0-9_]+)\$([a-zA-Z0-9_]+)>/<\1.\2>/g' \
        -e 's/<_ori_([a-zA-Z0-9_]+)>/<@\1>/g'
}

# --- Color functions ---
if [[ "$USE_COLOR" == "yes" ]]; then
    C_BOLD='\033[1m'
    C_CYAN='\033[0;36m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_BOLD="" C_CYAN="" C_DIM="" C_NC=""
fi

# --- Disassemble ---
if [[ "$SYMBOLS_ONLY" -eq 1 ]]; then
    # List symbols only
    echo -e "${C_BOLD}Ori Functions:${C_NC}"
    objdump -d "$tmpdir/binary" | grep "^[0-9a-f]* <_ori_" | \
        demangle_ori | \
        sed -E 's/^[0-9a-f]+ //' | \
        sed 's/:$//' | \
        while IFS= read -r line; do
            echo -e "  ${C_CYAN}${line}${C_NC}"
        done

    if [[ "$SHOW_ALL" -eq 1 ]]; then
        echo ""
        echo -e "${C_BOLD}Runtime Functions:${C_NC}"
        objdump -d "$tmpdir/binary" | grep "^[0-9a-f]* <ori_" | grep -v "^[0-9a-f]* <_ori_" | \
            sed -E 's/^[0-9a-f]+ //' | \
            sed 's/:$//' | \
            while IFS= read -r line; do
                echo -e "  ${C_DIM}[rt]${C_NC} ${line}"
            done
    fi
    exit 0
fi

# --- Full disassembly ---
# Build the filter pattern
if [[ -n "$FILTER_FN" ]]; then
    # Convert Ori name to mangled symbol
    symbol=$(ori_name_to_symbol "$FILTER_FN")
    # Extract the specific function's disassembly
    objdump -d "$tmpdir/binary" | awk -v sym="$symbol" '
        /^[0-9a-f]+ </ && index($0, "<" sym ">") { capture = 1 }
        capture { print }
        capture && /^$/ { capture = 0 }
    ' | demangle_ori
elif [[ "$SHOW_ALL" -eq 1 ]]; then
    # Show all ori_ and _ori_ functions
    objdump -d "$tmpdir/binary" | awk '
        /^[0-9a-f]+ <_?ori_/ { capture = 1 }
        /^[0-9a-f]+ <[^o]/ || /^[0-9a-f]+ <[^_r]/ && !/^[0-9a-f]+ <_?ori_/ { capture = 0 }
        /^[0-9a-f]+ <_ZN/ { capture = 0 }
        capture { print }
        capture && /^$/ { capture = 0 }
    ' | demangle_ori
else
    # Show only _ori_ functions (user code)
    objdump -d "$tmpdir/binary" | awk '
        /^[0-9a-f]+ <_ori_/ { capture = 1 }
        /^[0-9a-f]+ <[^_]/ { capture = 0 }
        /^[0-9a-f]+ <_[^o]/ { capture = 0 }
        /^[0-9a-f]+ <_ZN/ { capture = 0 }
        capture { print }
        capture && /^$/ { capture = 0 }
    ' | demangle_ori
fi
