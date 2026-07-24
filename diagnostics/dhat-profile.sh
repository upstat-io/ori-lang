#!/bin/bash
# Profile an Ori program's heap-allocation sites with DHAT (valgrind --tool=dhat).
#
# Usage:
#   diagnostics/dhat-profile.sh [options] <file.ori | binary>
#
# Options:
#   --top N            Show the top N allocation sites (default: 15)
#   --json             Emit structured JSON instead of the human table
#   --self-test        Run the embedded parser against a fixture and exit
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Given a .ori file, builds a DEBUG binary (DWARF symbols) so allocation sites
# attribute to Ori functions; given an existing binary, profiles it as-is.
# Allocations are ranked by count and attributed past allocator plumbing
# (malloc / Rust std alloc / RawVec growth) to the owning Ori frame.
#
# Requires: ori compiler (LLVM), valgrind with the dhat tool (>=3.14), python3.
#
# Exit codes:
#   0 = profiled clean
#   2 = usage error or missing dependency

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

TOP=15
AS_JSON=0
SELF_TEST=0
INPUT=""

# --- The SSOT DHAT-JSON parser (Ori-frame attribution). Embedded so this
# diagnostic is self-contained like the rest of the family; --self-test pins it.
read -r -d '' DHAT_PARSER <<'PYEOF' || true
import json, sys
data = json.load(open(sys.argv[1]))
top = int(sys.argv[2])
as_json = sys.argv[3] == "json"
ftbl = data.get("ftbl", [])
# Frames that are allocator plumbing, not the Ori construct that allocated.
PLUMBING = ("malloc", "vgpreload", "???", "alloc::alloc", "alloc.rs",
            "raw_vec", "RawVecInner", "try_allocate_in", "__rust", "core::")
def location(frames):
    for f in frames:
        if ".ori:" in f:
            return f
    for f in frames:
        if "_ori_" in f:
            return f
    for f in frames:
        if not any(p in f for p in PLUMBING):
            return f
    return frames[0] if frames else "?"
sites = []
for pp in data.get("pps", []):
    frames = [ftbl[i] for i in pp.get("fs", []) if i < len(ftbl)]
    sites.append({"allocs": pp.get("tbk", 0), "bytes": pp.get("tb", 0),
                  "location": location(frames)})
sites.sort(key=lambda s: s["allocs"], reverse=True)
out = {"total_allocs": sum(s["allocs"] for s in sites),
       "total_bytes": sum(s["bytes"] for s in sites),
       "sites": sites[:top]}
if as_json:
    print(json.dumps(out, indent=2))
else:
    print(f"DHAT: total allocs {out['total_allocs']:,}  total bytes {out['total_bytes']:,}")
    print("top allocation sites (by count):")
    for s in out["sites"]:
        print(f"  allocs={s['allocs']:>12,}  bytes={s['bytes']:>14,}  {s['location']}")
PYEOF

_run_self_test() {
    local td
    td=$(mktemp -d)
    cat > "$td/fixture.json" <<'JSON'
{"ftbl": ["[root]", "0x484: malloc (in vgpreload_dhat)", "0x1: alloc::alloc::alloc (alloc.rs:95)", "0x2: _ori_simulate (in bin)", "0x3: doors.ori:3"],
 "pps": [{"tbk": 5, "tb": 408, "fs": [1, 2, 4]}, {"tbk": 2, "tb": 256, "fs": [1, 2, 3]}]}
JSON
    local out
    out=$(python3 -c "$DHAT_PARSER" "$td/fixture.json" 15 json)
    rm -rf "$td"
    # Site 0 must attribute to the .ori source line, not malloc/alloc.rs.
    if echo "$out" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['total_allocs']==7, d; assert 'doors.ori:3' in d['sites'][0]['location'], d['sites'][0]; assert '_ori_simulate' in d['sites'][1]['location'], d['sites'][1]"; then
        echo "dhat-profile self-test: OK"
        return 0
    fi
    echo "dhat-profile self-test: FAIL" >&2
    return 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --top) TOP="$2"; shift 2 ;;
        --json) AS_JSON=1; shift ;;
        --self-test) SELF_TEST=1; shift ;;
        --no-color) USE_COLOR=never; shift ;;
        --color) USE_COLOR=always; shift ;;
        -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        -*) echo "Unknown option: $1" >&2; exit 2 ;;
        *) INPUT="$1"; shift ;;
    esac
done

if [[ $SELF_TEST -eq 1 ]]; then
    _run_self_test
    exit $?
fi

if [[ -z "$INPUT" ]]; then
    echo "Usage: diagnostics/dhat-profile.sh [options] <file.ori | binary>" >&2
    exit 2
fi
if ! command -v valgrind >/dev/null 2>&1; then
    echo "Error: valgrind not found (needs the dhat tool, valgrind >=3.14)" >&2
    exit 2
fi

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

BINARY="$INPUT"
if [[ "$INPUT" == *.ori ]]; then
    find_ori_bin  # sets ORI (debug preferred → DWARF symbols)
    BINARY="$tmpdir/dhat_target"
    if ! "$ORI" build "$INPUT" -o "$BINARY" 2>"$tmpdir/build.err"; then
        echo "Error: failed to build $INPUT for profiling" >&2
        cat "$tmpdir/build.err" >&2
        exit 2
    fi
fi

valgrind --tool=dhat --dhat-out-file="$tmpdir/dhat.json" "$BINARY" \
    >/dev/null 2>"$tmpdir/vg.err"
if [[ ! -f "$tmpdir/dhat.json" ]]; then
    echo "Error: DHAT produced no output (is the dhat tool installed?)" >&2
    cat "$tmpdir/vg.err" >&2
    exit 2
fi

json_arg=$([[ $AS_JSON -eq 1 ]] && echo json || echo human)
python3 -c "$DHAT_PARSER" "$tmpdir/dhat.json" "$TOP" "$json_arg"
