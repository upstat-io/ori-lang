#!/bin/bash
# Count RC and COW operations in LLVM IR per function.
#
# Usage:
#   diagnostics/rc-stats.sh [options] <file.ori>
#
# Options:
#   --block-level      Show per-block RC operation breakdown
#   --optimized        Analyze optimized IR (post-optimization histogram)
#   --compare-awk      Compare JSON totals with legacy awk parser (migration verification)
#   --rc-remarks       Consume the --emit-rc-remarks survivor stream
#                      (per-function surviving-RC-op summary; the eventual-supersede
#                      of the ORI_AUDIT histogram)
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Output:
#   A table summarizing RC operations (alloc, inc, dec, free) and COW
#   operations per function, with a balance column. Imbalanced functions
#   are flagged with a warning.
#
#   Balance = (alloc + inc) - (dec + free)
#     Positive → potential leak (more retains than releases)
#     Negative → potential over-release (more releases than retains)
#     Zero → balanced (not guaranteed correct, but a good sign)
#
# Data source:
#   All modes consume compiler JSON emitted via ORI_AUDIT_CODEGEN=1.
#   RC operation classification is defined in rc_histogram.rs (RcOpKind).
#   This is the SSOT — no awk parsing of LLVM IR text.
#
# Environment:
#   ORI_BIN    Override path to ori binary (default: cargo run -p oric --bin ori --)
#
# Requires: ori compiler with LLVM support, python3
#
# Exit codes:
#   0 = success (all functions balanced)
#   1 = imbalanced functions detected
#   2 = usage error or compilation failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Defaults ---
OPTIMIZED=0
BLOCK_LEVEL=0
COMPARE_AWK=0
RC_REMARKS=0
USE_COLOR=auto
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --block-level) BLOCK_LEVEL=1; shift ;;
        --optimized) OPTIMIZED=1; shift ;;
        --compare-awk) COMPARE_AWK=1; shift ;;
        --rc-remarks) RC_REMARKS=1; shift ;;
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
    echo "Usage: diagnostics/rc-stats.sh [options] <file.ori>" >&2
    exit 2
fi

if [[ ! -f "$FILE" ]]; then
    echo "Error: file not found: $FILE" >&2
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

# --- Color codes ---
if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_YELLOW="" C_BOLD="" C_DIM="" C_NC=""
fi

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# --- Compile with audit and extract JSON ---
echo "Analyzing $(basename "$FILE")..." >&2

ORI_BIN="${ORI_BIN:-cargo run -p oric --bin ori --}"

# --- RC-remarks mode: consume the --emit-rc-remarks survivor stream, the
# eventual-supersede of the ORI_AUDIT histogram below. The stream reports which
# RC ops survived; whether that survivor set is CORRECT is decided by the
# verifier (ORI_VERIFY_ARC / ORI_VERIFY_EACH) plus a leak check, not by this
# summary. The class-ledger path is unconditional.
if [[ "$RC_REMARKS" -eq 1 ]]; then
    remarks="$tmpdir/rc-remarks.jsonl"
    # shellcheck disable=SC2086
    $ORI_BIN build "$FILE" --emit-rc-remarks "$remarks" -o "$tmpdir/test_bin" \
        2>"$tmpdir/stderr.txt" || true
    if [[ ! -s "$remarks" ]]; then
        echo "Error: no RC-remark stream produced for $FILE" >&2
        sed -n '1,20p' "$tmpdir/stderr.txt" >&2
        exit 2
    fi
    python3 - "$remarks" <<'PYEOF'
import json, sys
from collections import Counter, defaultdict
survivors = defaultdict(int)
causes = defaultdict(Counter)
schema_version = None
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    o = json.loads(line)
    if o.get("record") == "header":
        schema_version = o.get("schema_version")
        continue
    fn = o.get("function") or "<unknown>"
    survivors[fn] += 1
    c = o.get("cause") or {}
    causes[fn][f"{c.get('lattice_dim', '-')}/{c.get('proof_failure', '-')}"] += 1
print(f"RC survivors (stream schema v{schema_version}):")
print(f"{'survivors':>10}  function  [top cause]")
for fn, n in sorted(survivors.items(), key=lambda kv: (-kv[1], kv[0])):
    top = causes[fn].most_common(1)[0][0] if causes[fn] else "-"
    print(f"{n:>10}  {fn}  [{top}]")
total = sum(survivors.values())
print(f"\ntotal: {total} surviving RC op(s) across {len(survivors)} function(s)")
PYEOF
    exit 0
fi

# shellcheck disable=SC2086
ORI_AUDIT_CODEGEN=1 $ORI_BIN build "$FILE" -o "$tmpdir/test_bin" 2>"$tmpdir/stderr.txt" || true

# Select the JSON line matching the requested optimized flag.
if [[ "$OPTIMIZED" -eq 1 ]]; then
    want_optimized='"optimized":true'
else
    want_optimized='"optimized":false'
fi

json_line=$(grep "^codegen stats: json:" "$tmpdir/stderr.txt" | grep "$want_optimized" | head -1 || true)

if [[ -z "$json_line" ]]; then
    # Show compiler errors (excluding codegen audit lines).
    grep -v "^codegen " "$tmpdir/stderr.txt" >&2 || true
    echo "Error: compilation failed before stats pass" >&2
    exit 2
fi

# Strip the prefix to get raw JSON.
json="${json_line#codegen stats: json: }"

# --- Compare with awk (migration verification, Phase B) ---
if [[ "$COMPARE_AWK" -eq 1 ]]; then
    echo -e "\n${C_BOLD}=== Migration comparison: awk vs JSON ===${C_NC}" >&2

    # Run awk parser on IR text for comparison.
    dump_args=(--raw)
    if [[ "$OPTIMIZED" -eq 1 ]]; then
        dump_args+=(--optimized)
    fi
    # shellcheck disable=SC2086
    "$SCRIPT_DIR/ir-dump.sh" "${dump_args[@]}" "$FILE" > "$tmpdir/ir.ll" 2>/dev/null || true

    python3 -c "
import sys, json, re

json_data = json.loads(sys.argv[1])
ir_file = sys.argv[2]
red, green, yellow, bold, nc = sys.argv[3:8]

# Parse awk-style counts from IR text.
awk_counts = {}
current_fn = None
with open(ir_file) as f:
    for line in f:
        m = re.match(r'^define .*@\"?([^(\"]+)\"?\(', line)
        if m:
            current_fn = m.group(1)
            awk_counts[current_fn] = {'alloc':0, 'inc':0, 'dec':0, 'free':0, 'cow':0}
        if current_fn and 'call' in line:
            if 'ori_rc_alloc' in line: awk_counts[current_fn]['alloc'] += 1
            if 'ori_rc_inc' in line: awk_counts[current_fn]['inc'] += 1
            if 'ori_rc_dec' in line: awk_counts[current_fn]['dec'] += 1
            if 'ori_rc_free' in line: awk_counts[current_fn]['free'] += 1
            # COW: ori_<type>_<op>_cow
            if re.search(r'ori_(list|str|map|set)_[a-z_]*_cow', line):
                awk_counts[current_fn]['cow'] += 1

# Compare.
json_funcs = {fn['name']: fn['totals'] for fn in json_data['functions']}
all_ok = True
for fn_name, jt in json_funcs.items():
    # Find matching awk entry (awk uses raw names, JSON uses demangled).
    at = None
    for awk_name, awk_t in awk_counts.items():
        if awk_name == fn_name or awk_name.replace('_ori_', '@').replace('\$', '.') == fn_name:
            at = awk_t
            break
    if at is None:
        continue

    for op in ['alloc', 'inc', 'dec', 'free', 'cow']:
        j_val = jt[op]
        a_val = at[op]
        if j_val != a_val:
            diff = j_val - a_val
            if diff > 0:
                # JSON higher — expected for typed RC ops.
                print(f'  {yellow}EXPECTED{nc} {fn_name}.{op}: awk={a_val} json={j_val} (+{diff} typed RC ops)')
            else:
                print(f'  {red}MISMATCH{nc} {fn_name}.{op}: awk={a_val} json={j_val} ({diff})')
                all_ok = False

if all_ok:
    print(f'  {green}All base-5 RC ops match (JSON may be higher for typed ops){nc}')
" "$json" "$tmpdir/ir.ll" "$C_RED" "$C_GREEN" "$C_YELLOW" "$C_BOLD" "$C_NC" >&2
fi

# --- Render table from JSON ---
if [[ "$BLOCK_LEVEL" -eq 1 ]]; then
    # Per-block view.
    python3 -c "
import sys, json

data = json.loads(sys.argv[1])
red, green, yellow, bold, dim, nc = sys.argv[2:8]

print(f'{bold}{\"Function / Block\":<40s}  alloc  inc   dec   free  cow   balance{nc}')
print(f'{\"─\"*40}  -----  ----  ----  ----  ----  -------')

has_imbalance = False

for fn in data['functions']:
    t = fn['totals']
    a, i, d, f, c = t['alloc'], t['inc'], t['dec'], t['free'], t['cow']
    if a + i + d + f + c == 0:
        continue

    balance = (a + i) - (d + f)
    if balance > 0:
        bal = f'{yellow}+{balance}{nc}'
        has_imbalance = True
    elif balance < 0:
        bal = f'{red}{balance}{nc}'
        has_imbalance = True
    else:
        bal = f'{green}0{nc}'

    print(f'{bold}{fn[\"name\"]:<40s}  {a:5d}  {i:4d}  {d:4d}  {f:4d}  {c:4d}  {bal}{nc}')

    for block in fn['blocks']:
        bc = block['counts']
        ba, bi, bd, bf, bco = bc['alloc'], bc['inc'], bc['dec'], bc['free'], bc['cow']
        if ba + bi + bd + bf + bco == 0:
            continue
        block_bal = (ba + bi) - (bd + bf)
        if block_bal > 0:
            bbal = f'{dim}{yellow}+{block_bal}{nc}'
        elif block_bal < 0:
            bbal = f'{dim}{red}{block_bal}{nc}'
        else:
            bbal = f'{dim}{green}0{nc}'
        print(f'{dim}  {block[\"label\"]:<38s}  {ba:5d}  {bi:4d}  {bd:4d}  {bf:4d}  {bco:4d}  {bbal}{nc}')

sys.exit(1 if has_imbalance else 0)
" "$json" "$C_RED" "$C_GREEN" "$C_YELLOW" "$C_BOLD" "$C_DIM" "$C_NC"
else
    # Function-level view (default).
    python3 -c "
import sys, json

data = json.loads(sys.argv[1])
red, green, yellow, bold, dim, nc = sys.argv[2:8]

print(f'{bold}{\"Function\":<30s}  alloc  inc   dec   free  cow   balance{nc}')
print(f'{\"─\"*30}  -----  ----  ----  ----  ----  -------')

has_imbalance = False
total = {'alloc':0, 'inc':0, 'dec':0, 'free':0, 'cow':0}

for fn in data['functions']:
    t = fn['totals']
    a, i, d, f, c = t['alloc'], t['inc'], t['dec'], t['free'], t['cow']
    if a + i + d + f + c == 0:
        continue

    balance = (a + i) - (d + f)
    total['alloc'] += a; total['inc'] += i; total['dec'] += d; total['free'] += f; total['cow'] += c

    if balance > 0:
        bal = f'{yellow}+{balance}{nc}'
        flag = f' {yellow}\u26a0 leak?{nc}'
        has_imbalance = True
    elif balance < 0:
        bal = f'{red}{balance}{nc}'
        flag = f' {red}\u26a0 over-release?{nc}'
        has_imbalance = True
    else:
        bal = f'{green}0{nc}'
        flag = ''

    print(f'{fn[\"name\"]:<30s}  {a:5d}  {i:4d}  {d:4d}  {f:4d}  {c:4d}  {bal}{flag}')

total_bal = (total['alloc'] + total['inc']) - (total['dec'] + total['free'])
print(f'{\"─\"*30}  -----  ----  ----  ----  ----  -------')
if total_bal > 0:
    bal = f'{yellow}+{total_bal}{nc}'
elif total_bal < 0:
    bal = f'{red}{total_bal}{nc}'
else:
    bal = f'{green}0{nc}'
print(f'{bold}{\"TOTAL\":<30s}  {total[\"alloc\"]:5d}  {total[\"inc\"]:4d}  {total[\"dec\"]:4d}  {total[\"free\"]:4d}  {total[\"cow\"]:4d}  {bal}{nc}')

sys.exit(1 if has_imbalance else 0)
" "$json" "$C_RED" "$C_GREEN" "$C_YELLOW" "$C_BOLD" "$C_DIM" "$C_NC"
fi
