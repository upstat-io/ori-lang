#!/usr/bin/env bash
# Generic benchmark probe: ONE harness for ALL subjects;
# subjects are rows in registry.json (data), never per-subject scripts.
#
# Usage: measure.sh <subject>
# Looks up the subject's cargo bench target + criterion case in registry.json,
# runs the bench, parses criterion's noise-robust `thrpt:` median, and prints
# EXACTLY ONE numeric reading (MiB/s) to stdout. Diagnostics go to stderr.
# Exit non-zero only on a real bench failure or an unparseable result.
set -euo pipefail

SUBJECT="${1:?usage: measure.sh <subject>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"   # compiler_repo
REGISTRY="$HERE/registry.json"

[ -f "$REGISTRY" ] || { echo "measure.sh: missing $REGISTRY" >&2; exit 2; }

# Resolve bench target + criterion case from the data registry.
read -r BENCH CASE < <(python3 - "$REGISTRY" "$SUBJECT" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
row = (reg.get("subjects") or {}).get(sys.argv[2])
if not row:
    sys.stderr.write(f"measure.sh: subject {sys.argv[2]!r} not in registry\n"); sys.exit(3)
print(row["bench"], row["case"])
PY
)
echo "measure.sh: subject=$SUBJECT bench=$BENCH case=$CASE" >&2

cd "$REPO_ROOT"
if ! OUT="$(cargo bench -p oric --bench "$BENCH" -- "$CASE" --noplot 2>&1)"; then
    printf '%s\n' "$OUT" >&2
    echo "measure.sh: cargo bench failed" >&2
    exit 1
fi
printf '%s\n' "$OUT" >&2

# Parse the criterion `thrpt:` line median (point estimate = middle of 3),
# normalize the unit to MiB/s, emit the LAST matching case's reading.
# OUT goes to a temp file (NOT a pipe) so the heredoc stays the python script's stdin.
OUT_FILE="$(mktemp)"
trap 'rm -f "$OUT_FILE"' EXIT
printf '%s\n' "$OUT" > "$OUT_FILE"
python3 - "$OUT_FILE" <<'PY'
import re, sys

UNIT_TO_MIB = {
    "GiB/s": 1024.0, "MiB/s": 1.0, "KiB/s": 1.0 / 1024.0,
    "GB/s": 1000.0 * 1000.0 * 1000.0 / 1048576.0,
    "MB/s": 1000.0 * 1000.0 / 1048576.0,
    "KB/s": 1000.0 / 1048576.0,
}
pair = re.compile(r"([0-9]+\.?[0-9]*)\s*([KMG]i?B/s)")
reading = None
for line in open(sys.argv[1], encoding="utf-8"):
    if "thrpt:" not in line:
        continue
    vals = pair.findall(line)
    if len(vals) >= 2:               # [low  median  high]; median is the point estimate
        num, unit = vals[1]
        reading = float(num) * UNIT_TO_MIB.get(unit, 1.0)
if reading is None:
    sys.stderr.write("measure.sh: could not parse a thrpt reading\n"); sys.exit(2)
print(f"{reading:.4f}")
PY
