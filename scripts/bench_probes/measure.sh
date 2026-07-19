#!/usr/bin/env bash
# Generic benchmark probe: ONE harness for ALL subjects;
# subjects are rows in registry.json (data), never per-subject scripts.
#
# Usage: measure.sh <subject> [--baseline-ref]
# A row declares a probe `kind`:
#   "criterion-throughput" (default): cargo bench target + criterion case; parse
#       the noise-robust `thrpt:` median; print ONE reading (MiB/s, higher-is-better).
#   "program-wallclock": run the row's `command` best-of-N (optional one-shot
#       `build` first, one warmup); print the MIN wall-clock seconds
#       (lower-is-better). `--baseline-ref` measures the row's
#       `external_baseline.command` instead (the reference the grind beats).
# Prints EXACTLY ONE numeric reading to stdout. Diagnostics go to stderr.
# Exit: 1 real bench failure; 2 IO/parse error; 3 unregistered subject / bad mode.
set -euo pipefail

SUBJECT=""
BASELINE_REF=0
for arg in "$@"; do
    case "$arg" in
        --baseline-ref) BASELINE_REF=1 ;;
        --*) echo "measure.sh: unknown flag $arg" >&2; exit 2 ;;
        *) [ -z "$SUBJECT" ] && SUBJECT="$arg" || { echo "measure.sh: extra arg $arg" >&2; exit 2; } ;;
    esac
done
[ -n "$SUBJECT" ] || { echo "usage: measure.sh <subject> [--baseline-ref]" >&2; exit 2; }

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"   # compiler_repo
REGISTRY="$HERE/registry.json"
[ -f "$REGISTRY" ] || { echo "measure.sh: missing $REGISTRY" >&2; exit 2; }

ROW_JSON="$(mktemp)"
trap 'rm -f "$ROW_JSON"' EXIT

# Resolve the row + mode. Prints KIND on line 1; for criterion-throughput also
# "<bench> <case>" on line 2; for program-wallclock writes {build,command,samples}
# to $ROW_JSON. Exit 3 on unregistered subject or an invalid --baseline-ref mode.
KIND="$(python3 - "$REGISTRY" "$SUBJECT" "$BASELINE_REF" "$ROW_JSON" <<'PY'
import json, sys
registry, subject, baseline_ref, row_json = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4]
reg = json.load(open(registry))
row = (reg.get("subjects") or {}).get(subject)
if not isinstance(row, dict):
    sys.stderr.write(f"measure.sh: subject {subject!r} not in registry\n"); sys.exit(3)
kind = row.get("kind", "criterion-throughput")

if baseline_ref:
    if kind != "program-wallclock":
        sys.stderr.write("measure.sh: --baseline-ref requires a program-wallclock row\n"); sys.exit(3)
    ext = row.get("external_baseline")
    if not isinstance(ext, dict) or not ext.get("command"):
        sys.stderr.write("measure.sh: row has no external_baseline.command\n"); sys.exit(3)
    json.dump({"build": ext.get("build", ""), "command": ext["command"],
               "samples": int(row.get("samples", 3) or 3)}, open(row_json, "w"))
    print("program-wallclock"); sys.exit(0)

if kind == "criterion-throughput":
    if not (row.get("bench") and row.get("case")):
        sys.stderr.write("measure.sh: criterion row needs bench + case\n"); sys.exit(3)
    print(kind); print(f"{row['bench']} {row['case']}"); sys.exit(0)

if kind == "program-wallclock":
    if not row.get("command"):
        sys.stderr.write("measure.sh: program-wallclock row needs command\n"); sys.exit(3)
    json.dump({"build": row.get("build", ""), "command": row["command"],
               "samples": int(row.get("samples", 3) or 3)}, open(row_json, "w"))
    print(kind); sys.exit(0)

sys.stderr.write(f"measure.sh: unknown probe kind {kind!r}\n"); sys.exit(3)
PY
)"
# `KIND` holds line 1; line 2 (criterion bench/case) is re-read below when needed.
KIND="$(printf '%s\n' "$KIND" | head -n1)"
cd "$REPO_ROOT"

if [ "$KIND" = "criterion-throughput" ]; then
    read -r BENCH CASE < <(python3 - "$REGISTRY" "$SUBJECT" <<'PY'
import json, sys
row = json.load(open(sys.argv[1]))["subjects"][sys.argv[2]]
print(row["bench"], row["case"])
PY
)
    echo "measure.sh: subject=$SUBJECT kind=criterion-throughput bench=$BENCH case=$CASE" >&2
    if ! OUT="$(cargo bench -p oric --bench "$BENCH" -- "$CASE" --noplot 2>&1)"; then
        printf '%s\n' "$OUT" >&2; echo "measure.sh: cargo bench failed" >&2; exit 1
    fi
    printf '%s\n' "$OUT" >&2
    OUT_FILE="$(mktemp)"; trap 'rm -f "$ROW_JSON" "$OUT_FILE"' EXIT
    printf '%s\n' "$OUT" > "$OUT_FILE"
    python3 - "$OUT_FILE" <<'PY'
import re, sys
UNIT_TO_MIB = {"GiB/s": 1024.0, "MiB/s": 1.0, "KiB/s": 1.0 / 1024.0,
               "GB/s": 1000.0 ** 3 / 1048576.0, "MB/s": 1000.0 ** 2 / 1048576.0,
               "KB/s": 1000.0 / 1048576.0}
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
    exit 0
fi

# program-wallclock: optional one-shot build, one warmup, best-of-N min seconds.
echo "measure.sh: subject=$SUBJECT kind=program-wallclock baseline_ref=$BASELINE_REF" >&2
python3 - "$ROW_JSON" <<'PY'
import json, subprocess, sys, time
spec = json.load(open(sys.argv[1]))
build, command, samples = spec["build"], spec["command"], spec["samples"]

def run(cmd, *, capture):
    return subprocess.run(["bash", "-lc", cmd],
                          stdout=(subprocess.DEVNULL if capture else None),
                          stderr=subprocess.DEVNULL if capture else sys.stderr.buffer)

if build:
    sys.stderr.write(f"measure.sh: build: {build}\n")
    if run(build, capture=False).returncode != 0:
        sys.stderr.write("measure.sh: build failed\n"); sys.exit(1)

if run(command, capture=True).returncode != 0:      # warmup (discarded)
    sys.stderr.write("measure.sh: warmup run failed\n"); sys.exit(1)

best = None
for i in range(max(1, samples)):
    t0 = time.perf_counter()
    rc = run(command, capture=True).returncode
    dt = time.perf_counter() - t0
    if rc != 0:
        sys.stderr.write(f"measure.sh: timed run {i + 1} failed\n"); sys.exit(1)
    sys.stderr.write(f"measure.sh: run {i + 1}/{samples} = {dt:.6g}s\n")
    best = dt if best is None else min(best, dt)
print(f"{best:.6g}")
PY
