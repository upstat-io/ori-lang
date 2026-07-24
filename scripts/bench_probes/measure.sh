#!/usr/bin/env bash
# Generic benchmark probe: ONE harness for ALL subjects;
# subjects are rows in registry.json (data), never per-subject scripts.
#
# Usage: measure.sh <subject> [--baseline-ref[=LABEL]] [--metric time|rss]
#                             [--samples-json PATH]
# A row declares a probe `kind`:
#   "criterion-throughput" (default): cargo bench target + criterion case; parse
#       the noise-robust `thrpt:` median; print ONE reading (MiB/s, higher-is-better).
#   "program-wallclock": run the row's `command` best-of-N (optional one-shot
#       `build` first, one warmup); print the MIN wall-clock seconds
#       (lower-is-better). `--baseline-ref` measures a comparator command instead
#       (the reference the grind beats): the bare form selects the row's singular
#       `external_baseline`, `=LABEL` selects `external_baselines[LABEL]`.
#   `--metric rss` prints the peak resident-set-size reading (KiB,
#       lower-is-better) for the same row and mode, sampled per fresh process.
#   `--metric work` runs the row's (or comparator's) `work_count.command` ONCE,
#       untimed and outside every timed measurement, and prints the SUM of the
#       counters it emitted. The program's stdout is a stream of
#       `work <key> <count>` lines and nothing else; the per-key map is the
#       comparator work-equivalence pin and is recorded via `--samples-json`.
#   `--metric profile` runs the row's (or comparator's) `characterization.command`
#       ONCE, untimed and outside every timed measurement, over the same
#       `work <key> <count>` stream, and prints the SUM of the operation-mix
#       counters. Its keys are a FIXED language-independent vocabulary (see
#       PROFILE_MIX_CATEGORIES below) describing source-level dynamic events any
#       imperative language exhibits, never one runtime's opcode names, so the
#       same profile is countable in Ori, Python, and Lua. `--samples-json`
#       records the raw counters plus the derived `profile` block (normalized
#       operation mix, allocation rate, call-polymorphism degree).
#   A program-wallclock row (subject or comparator) may replace its `command`
#       with an `aot` block ({profile, source, args?, cache_dir?}); the harness
#       then builds the runtime static archive and the compiler binary
#       explicitly, records their canonical paths and content hashes, links the
#       program into a hash-keyed cache (so a changed runtime identity forces a
#       relink), re-verifies the identity after the runs, and reports a missing,
#       drifted, or cache-mismatched identity as an INVALID RUN (exit 4) rather
#       than as a measurement or a backend failure. See provenance.py.
#   `--samples-json PATH` additionally writes every raw observation plus the
#       warmup run's stdout digest (and, under `--metric work`/`--metric
#       profile`, the counter map). It never changes the stdout reading.
# Prints EXACTLY ONE numeric reading to stdout. Diagnostics go to stderr.
# Exit: 1 real bench failure; 2 IO/parse error; 3 unregistered subject / bad mode;
#       4 invalid run (compiler/runtime/artifact identity missing, drifted, or
#         stale relative to a cached link).
set -euo pipefail

SUBJECT=""
BASELINE_REF=0
BASELINE_LABEL=""
METRIC="time"
SAMPLES_JSON=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --baseline-ref) BASELINE_REF=1 ;;
        --baseline-ref=*) BASELINE_REF=1; BASELINE_LABEL="${1#*=}" ;;
        --metric) shift; METRIC="${1:-}" ;;
        --metric=*) METRIC="${1#*=}" ;;
        --samples-json) shift; SAMPLES_JSON="${1:-}" ;;
        --samples-json=*) SAMPLES_JSON="${1#*=}" ;;
        --*) echo "measure.sh: unknown flag $1" >&2; exit 2 ;;
        *)
            if [ -z "$SUBJECT" ]; then
                SUBJECT="$1"
            else
                echo "measure.sh: extra arg $1" >&2; exit 2
            fi
            ;;
    esac
    shift
done
[ -n "$SUBJECT" ] || { echo "usage: measure.sh <subject> [--baseline-ref[=LABEL]]" >&2; exit 2; }
case "$METRIC" in
    time|rss|work|profile) ;;
    *) echo "measure.sh: unknown --metric $METRIC (want time|rss|work|profile)" >&2; exit 2 ;;
esac

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"   # compiler_repo
REGISTRY="$HERE/registry.json"
[ -f "$REGISTRY" ] || { echo "measure.sh: missing $REGISTRY" >&2; exit 2; }

ROW_JSON="$(mktemp)"
trap 'rm -f "$ROW_JSON"' EXIT

# Resolve the row + mode. Prints KIND on line 1; for criterion-throughput also
# "<bench> <case>" on line 2; for program-wallclock writes {build,command,samples,
# label} to $ROW_JSON. Exit 3 on unregistered subject or an invalid mode/label.
KIND="$(python3 - "$REGISTRY" "$SUBJECT" "$BASELINE_REF" "$ROW_JSON" "$BASELINE_LABEL" "$METRIC" <<'PY'
import json, sys
registry, subject, baseline_ref, row_json = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4]
label_sel, metric = sys.argv[5], sys.argv[6]
reg = json.load(open(registry))
row = (reg.get("subjects") or {}).get(subject)
if not isinstance(row, dict):
    sys.stderr.write(f"measure.sh: subject {subject!r} not in registry\n"); sys.exit(3)
kind = row.get("kind", "criterion-throughput")

if metric in ("rss", "work", "profile") and kind != "program-wallclock":
    sys.stderr.write(f"measure.sh: --metric {metric} requires a program-wallclock row\n"); sys.exit(3)

#: Counter-stream metric -> the row field declaring its instrumented program.
COUNTER_FIELD = {"work": "work_count", "profile": "characterization"}

def aot_block(spec, what):
    """The spec's `aot` block, or None. A row declares `aot` or `command`, never both."""
    aot = spec.get("aot")
    if aot is None:
        return None
    if not isinstance(aot, dict) or not aot.get("source"):
        sys.stderr.write(f"measure.sh: {what} declares an aot block with no source\n"); sys.exit(3)
    if spec.get("command") or spec.get("build"):
        sys.stderr.write(
            f"measure.sh: {what} declares aot alongside command/build; the harness owns "
            "the AOT build and link\n")
        sys.exit(3)
    return aot

def executed_command(spec, what):
    """The command for the requested metric; counter metrics use their own field."""
    field = COUNTER_FIELD.get(metric)
    if field is None:
        # An aot row has no literal command; the harness links one from `aot`.
        return "" if aot_block(spec, what) else spec.get("command", "")
    declared = spec.get(field)
    if not isinstance(declared, dict) or not declared.get("command"):
        sys.stderr.write(f"measure.sh: {what} declares no {field}.command\n"); sys.exit(3)
    return declared["command"]

def comparator(row):
    """Resolve the requested comparator to {label, command, build, aot}."""
    plural = row.get("external_baselines")
    plural = plural if isinstance(plural, dict) else {}
    single = row.get("external_baseline")
    single = single if isinstance(single, dict) else None

    def resolved(label, ext):
        what = f"comparator {label!r}"
        return {"label": label, "command": executed_command(ext, what),
                "build": ext.get("build", ""), "aot": aot_block(ext, what)}

    def declares_program(ext):
        return isinstance(ext, dict) and bool(ext.get("command") or ext.get("aot"))

    if label_sel:
        ext = plural.get(label_sel)
        if not declares_program(ext):
            sys.stderr.write(f"measure.sh: no external_baselines[{label_sel!r}] with a command\n")
            sys.exit(3)
        return resolved(label_sel, ext)
    if declares_program(single):
        return resolved(single.get("label", "external_baseline"), single)
    for label in sorted(plural):
        if declares_program(plural[label]):
            return resolved(label, plural[label])
    sys.stderr.write("measure.sh: row declares no external baseline command\n"); sys.exit(3)

if baseline_ref:
    if kind != "program-wallclock":
        sys.stderr.write("measure.sh: --baseline-ref requires a program-wallclock row\n"); sys.exit(3)
    ext = comparator(row)
    json.dump({"build": ext["build"], "command": ext["command"], "aot": ext["aot"],
               "samples": int(row.get("samples", 3) or 3), "label": ext["label"]},
              open(row_json, "w"))
    print("program-wallclock"); sys.exit(0)

if kind == "criterion-throughput":
    if not (row.get("bench") and row.get("case")):
        sys.stderr.write("measure.sh: criterion row needs bench + case\n"); sys.exit(3)
    print(kind); print(f"{row['bench']} {row['case']}"); sys.exit(0)

if kind == "program-wallclock":
    what = f"subject {subject!r}"
    if not row.get("command") and not aot_block(row, what):
        sys.stderr.write("measure.sh: program-wallclock row needs command or aot\n"); sys.exit(3)
    json.dump({"build": row.get("build", ""),
               "command": executed_command(row, what), "aot": aot_block(row, what),
               "samples": int(row.get("samples", 3) or 3), "label": "subject"},
              open(row_json, "w"))
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

# program-wallclock: optional one-shot build, one warmup, best-of-N.
# --metric time prints MIN seconds; --metric rss prints the MAX per-process peak KiB;
# --metric work prints the SUM of one untimed counting run's `work` counters;
# --metric profile prints the SUM of one untimed characterization run's mix counters.
echo "measure.sh: subject=$SUBJECT kind=program-wallclock baseline_ref=$BASELINE_REF metric=$METRIC" >&2
python3 - "$ROW_JSON" "$METRIC" "$SAMPLES_JSON" "$SUBJECT" "$HERE" "$REPO_ROOT" <<'PY'
import hashlib, json, os, shlex, subprocess, sys, tempfile, time
spec = json.load(open(sys.argv[1]))
metric, samples_json, subject = sys.argv[2], sys.argv[3], sys.argv[4]
sys.path.insert(0, sys.argv[5])
repo_root = sys.argv[6]
build, command, samples = spec["build"], spec["command"], spec["samples"]
aot_spec = spec.get("aot")

#: `ru_maxrss` is KiB on Linux and bytes on Darwin.
RSS_DIVISOR = 1024 if sys.platform == "darwin" else 1

def run(cmd, *, stdout):
    """Run `cmd`, returning (returncode, peak_rss_kib) for that child alone."""
    proc = subprocess.Popen(["bash", "-lc", cmd], stdout=stdout,
                            stderr=subprocess.DEVNULL if stdout is not None else sys.stderr.buffer)
    if hasattr(os, "wait4"):
        _, status, usage = os.wait4(proc.pid, 0)
        proc.returncode = os.waitstatus_to_exitcode(status)
        return proc.returncode, usage.ru_maxrss / RSS_DIVISOR
    proc.wait()
    return proc.returncode, None

def capture(cmd, what):
    """Run `cmd` once, returning its stdout bytes; exit 1 when it fails."""
    with tempfile.NamedTemporaryFile(delete=False) as sink:
        sink_path = sink.name
    try:
        with open(sink_path, "wb") as handle:
            rc, _ = run(cmd, stdout=handle)
        if rc != 0:
            sys.stderr.write(f"measure.sh: {what} failed\n"); sys.exit(1)
        return open(sink_path, "rb").read()
    finally:
        os.unlink(sink_path)

def parse_work_counters(payload, what="work-count"):
    """Parse a `work <key> <count>` stream into an ordered {key: count} map."""
    counts = {}
    for raw in payload.decode("utf-8", "replace").splitlines():
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) != 3 or parts[0] != "work" or not parts[2].lstrip("-").isdigit():
            sys.stderr.write(f"measure.sh: unparsable {what} line: {line!r}\n"); sys.exit(2)
        if parts[1] in counts:
            sys.stderr.write(f"measure.sh: duplicate {what} key {parts[1]!r}\n"); sys.exit(2)
        counts[parts[1]] = int(parts[2])
    if not counts:
        sys.stderr.write(f"measure.sh: {what} run emitted no `work <key> <count>` line\n")
        sys.exit(2)
    return counts

#: The FIXED language-independent operation-mix vocabulary. Each key names a
#: source-level dynamic event every imperative language exhibits, so an Ori, a
#: Python, and a Lua implementation of one workload count the same categories.
#: Runtime-internal opcode names are NOT admissible here.
PROFILE_MIX_CATEGORIES = (
    "alloc",      # aggregate / collection / object constructions performed
    "arith",      # arithmetic and comparison primitive evaluations
    "branch",     # conditional branch decisions evaluated
    "call",       # user function / method invocations entered
    "field",      # aggregate field or tuple-component reads and writes
    "index",      # indexed collection element reads and writes
    "loop_iter",  # loop iterations entered
    "string_op",  # string constructions and operations
)
#: Call-polymorphism counters; excluded from the mix vector.
PROFILE_CALL_SITES, PROFILE_CALL_TARGETS = "call_sites", "call_targets"

def derive_profile(counts):
    """Validate a characterization counter map and derive its language-independent profile."""
    allowed = set(PROFILE_MIX_CATEGORIES) | {PROFILE_CALL_SITES, PROFILE_CALL_TARGETS}
    unknown = sorted(set(counts) - allowed)
    if unknown:
        sys.stderr.write(
            f"measure.sh: characterization emitted key(s) outside the fixed vocabulary: "
            f"{', '.join(unknown)} (allowed: {', '.join(sorted(allowed))})\n")
        sys.exit(2)
    missing = [k for k in (PROFILE_CALL_SITES, PROFILE_CALL_TARGETS) if k not in counts]
    if missing:
        sys.stderr.write(f"measure.sh: characterization omits {', '.join(missing)}\n"); sys.exit(2)
    if any(v < 0 for v in counts.values()):
        sys.stderr.write("measure.sh: characterization emitted a negative counter\n"); sys.exit(2)
    sites, targets = counts[PROFILE_CALL_SITES], counts[PROFILE_CALL_TARGETS]
    if sites <= 0:
        sys.stderr.write("measure.sh: characterization needs a positive call_sites\n"); sys.exit(2)
    if targets < sites:
        sys.stderr.write(
            f"measure.sh: call_targets ({targets}) < call_sites ({sites}); every executed call "
            "site has at least one target\n")
        sys.exit(2)
    mix = {k: counts.get(k, 0) for k in PROFILE_MIX_CATEGORIES}
    total = sum(mix.values())
    if total <= 0:
        sys.stderr.write("measure.sh: characterization counted zero operations\n"); sys.exit(2)
    return {
        "total_ops": total,
        "counts": mix,
        "operation_mix": {k: v / total for k, v in mix.items()},
        "allocation_rate": mix["alloc"] / total,
        "call_sites": sites,
        "call_targets": targets,
        "call_polymorphism_degree": targets / sites,
    }

if build:
    sys.stderr.write(f"measure.sh: build: {build}\n")
    if run(build, stdout=None)[0] != 0:
        sys.stderr.write("measure.sh: build failed\n"); sys.exit(1)


def invalid_run(reason):
    """Terminate as an INVALID RUN: neither a measurement nor a backend failure."""
    sys.stderr.write(f"measure.sh: invalid run: {reason}\n")
    sys.exit(provenance.INVALID_RUN_EXIT)


aot_record = None
if aot_spec:
    # Imported only on the AOT lane; a plain program-wallclock row needs no sibling.
    import provenance
    for cmd in provenance.build_commands(provenance.profile_of(aot_spec)):
        sys.stderr.write(f"measure.sh: aot build: {cmd}\n")
        if run(cmd, stdout=None)[0] != 0:
            sys.stderr.write(f"measure.sh: aot build failed: {cmd}\n"); sys.exit(1)
    aot_before = provenance.capture(repo_root, aot_spec)
    reason = provenance.missing_reason(aot_before)
    if reason:
        invalid_run(reason)
    key = provenance.cache_key(aot_before)
    link_dir = provenance.link_dir(repo_root, aot_spec, key)
    executable = provenance.executable_path(repo_root, aot_spec, key)
    recorded = provenance.read_recorded_identity(link_dir)
    if recorded is None:
        # No cache entry for this identity: link one. A changed compiler,
        # runtime archive, or source yields a different key, so a stale
        # executable is never reachable by reuse.
        compiler = aot_before["compiler"]["path"]
        source = aot_before["source"]["path"]
        link_dir.mkdir(parents=True, exist_ok=True)
        link_cmd = (f"{shlex.quote(compiler)} build {shlex.quote(source)} "
                    f"-o {shlex.quote(str(executable))}")
        sys.stderr.write(f"measure.sh: aot link: {link_cmd}\n")
        if run(link_cmd, stdout=None)[0] != 0:
            sys.stderr.write("measure.sh: aot link failed\n"); sys.exit(1)
        if not executable.exists():
            invalid_run(f"aot link produced no executable at {executable}")
        provenance.write_recorded_identity(link_dir, aot_before)
        relinked = True
    else:
        stale = provenance.stale_cache_reason(recorded, aot_before, executable)
        if stale:
            invalid_run(stale)
        relinked = False
    aot_record = {"identity": aot_before, "cache_key": key, "relinked": relinked,
                  "executable": str(executable), "verdict": "valid"}
    if not command:
        args = aot_spec.get("args", "")
        command = f"{shlex.quote(str(executable))}{' ' + args if args else ''}"


def finalize_aot():
    """Re-verify identity after the measured runs; returns the record to store."""
    if aot_record is None:
        return None
    reason = provenance.drift_reason(aot_record["identity"],
                                       provenance.capture(repo_root, aot_spec))
    if reason:
        invalid_run(reason)
    return aot_record


if metric == "profile":
    payload = capture(command, "characterization run")
    counts = parse_work_counters(payload, "characterization")
    profile = derive_profile(counts)
    reading = profile["total_ops"]
    sys.stderr.write(f"measure.sh: characterization counters = {counts}\n")
    aot = finalize_aot()
    if samples_json:
        record = {"subject": subject, "mode": spec["label"], "metric": "profile",
                  "unit": "ops", "direction": "characterization", "command": command,
                  "reading": reading, "observations": [reading], "work_counts": counts,
                  "profile": profile, "aot": aot,
                  "output_sha256": hashlib.sha256(payload).hexdigest(),
                  "output_bytes": len(payload)}
        with open(samples_json, "w", encoding="utf-8") as handle:
            json.dump(record, handle, indent=2, sort_keys=True)
            handle.write("\n")
    print(reading)
    sys.exit(0)

if metric == "work":
    payload = capture(command, "work-count run")
    work_counts = parse_work_counters(payload)
    reading = sum(work_counts.values())
    sys.stderr.write(f"measure.sh: work counters = {work_counts}\n")
    aot = finalize_aot()
    if samples_json:
        record = {"subject": subject, "mode": spec["label"], "metric": "work", "unit": "ops",
                  "direction": "equality", "command": command, "reading": reading,
                  "observations": [reading], "work_counts": work_counts, "aot": aot,
                  "output_sha256": hashlib.sha256(payload).hexdigest(),
                  "output_bytes": len(payload)}
        with open(samples_json, "w", encoding="utf-8") as handle:
            json.dump(record, handle, indent=2, sort_keys=True)
            handle.write("\n")
    print(reading)
    sys.exit(0)

# Warmup is discarded from the readings; its stdout is the row's output check.
payload = capture(command, "warmup run")
output_sha256, output_bytes = hashlib.sha256(payload).hexdigest(), len(payload)

seconds, peaks = [], []
for i in range(max(1, samples)):
    t0 = time.perf_counter()
    rc, peak = run(command, stdout=subprocess.DEVNULL)
    dt = time.perf_counter() - t0
    if rc != 0:
        sys.stderr.write(f"measure.sh: timed run {i + 1} failed\n"); sys.exit(1)
    sys.stderr.write(f"measure.sh: run {i + 1}/{samples} = {dt:.6g}s peak_rss={peak}\n")
    seconds.append(dt)
    if peak is not None:
        peaks.append(peak)

if metric == "rss":
    if not peaks:
        sys.stderr.write("measure.sh: peak RSS unavailable on this platform\n"); sys.exit(2)
    observations, reading, unit = peaks, max(peaks), "KiB"
else:
    observations, reading, unit = seconds, min(seconds), "s"

aot = finalize_aot()
if samples_json:
    record = {"subject": subject, "mode": spec["label"], "metric": metric, "unit": unit,
              "direction": "lower-is-better", "command": command, "reading": reading,
              "observations": observations, "seconds": seconds, "peak_rss_kib": peaks,
              "aot": aot,
              "output_sha256": output_sha256, "output_bytes": output_bytes}
    with open(samples_json, "w", encoding="utf-8") as handle:
        json.dump(record, handle, indent=2, sort_keys=True)
        handle.write("\n")

print(f"{reading:.6g}")
PY
