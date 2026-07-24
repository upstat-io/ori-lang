#!/usr/bin/env bash
# Pins the generic bench harness's additive surface (multi-comparator selection,
# the rss metric, the work-count pin, the language-independent characterization
# metric, and the raw-observation samples file) and the corpus comparator
# report's admission gates: output-equality, work-count equality, the
# geomean-plus-worst-ratio aggregate, and the corpus diversity verdict.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

mkdir -p "$WORK/scripts/bench_probes"
cp "$ROOT/scripts/bench_probes/measure.sh" "$WORK/scripts/bench_probes/"
cp "$ROOT/scripts/bench_probes/report.py" "$WORK/scripts/bench_probes/"
cp "$ROOT/scripts/bench_probes/characterize.py" "$WORK/scripts/bench_probes/"
cp "$ROOT/scripts/bench_probes/provenance.py" "$WORK/scripts/bench_probes/"
MEASURE="$WORK/scripts/bench_probes/measure.sh"
REGISTRY="$WORK/scripts/bench_probes/registry.json"

# Stub comparators on PATH. Output stubs: `agree` prints the subject's output,
# `differ` does not. Work stubs: `work_same` emits the subject's counters,
# `work_skew` emits the SAME output-producing answer via one fewer toggle --
# the equal-output/unequal-work case the work-count pin exists to catch.
mkdir -p "$WORK/bin"
printf '#!/bin/sh\nprintf "540000\\n"\n' > "$WORK/bin/agree"
printf '#!/bin/sh\nprintf "999\\n"\n' > "$WORK/bin/differ"
printf '#!/bin/sh\nprintf "work calls 500\\nwork toggles 241500\\n"\n' > "$WORK/bin/work_same"
printf '#!/bin/sh\nprintf "work calls 500\\nwork toggles 241499\\n"\n' > "$WORK/bin/work_skew"
printf '#!/bin/sh\nprintf "not a work line\\n"\n' > "$WORK/bin/work_junk"

# Characterization stubs. `prof_index` is index/loop dominated, `prof_string` is
# string dominated: two genuinely different operation regions. `prof_badkey`,
# `prof_nosites`, and `prof_badsites` are the vocabulary/consistency rejections.
printf '#!/bin/sh\nprintf "work alloc 10\\nwork arith 20\\nwork branch 5\\nwork call 5\\nwork field 0\\nwork index 500\\nwork loop_iter 400\\nwork string_op 0\\nwork call_sites 2\\nwork call_targets 3\\n"\n' > "$WORK/bin/prof_index"
printf '#!/bin/sh\nprintf "work alloc 300\\nwork arith 20\\nwork branch 5\\nwork call 5\\nwork field 0\\nwork index 10\\nwork loop_iter 100\\nwork string_op 600\\nwork call_sites 2\\nwork call_targets 2\\n"\n' > "$WORK/bin/prof_string"
printf '#!/bin/sh\nprintf "work opcode_copy 5\\nwork call_sites 1\\nwork call_targets 1\\n"\n' > "$WORK/bin/prof_badkey"
printf '#!/bin/sh\nprintf "work index 5\\nwork call_targets 1\\n"\n' > "$WORK/bin/prof_nosites"
printf '#!/bin/sh\nprintf "work index 5\\nwork call_sites 5\\nwork call_targets 2\\n"\n' > "$WORK/bin/prof_badsites"

chmod +x "$WORK/bin/agree" "$WORK/bin/differ" \
    "$WORK/bin/work_same" "$WORK/bin/work_skew" "$WORK/bin/work_junk" \
    "$WORK/bin/prof_index" "$WORK/bin/prof_string" "$WORK/bin/prof_badkey" \
    "$WORK/bin/prof_nosites" "$WORK/bin/prof_badsites"
export PATH="$WORK/bin:$PATH"

cat > "$REGISTRY" <<'EOF'
{
  "subjects": {
    "prog": {
      "kind": "program-wallclock",
      "command": "printf '540000\\n'",
      "direction": "lower-is-better",
      "unit": "s",
      "samples": 2,
      "work_count": { "command": "work_same" },
      "characterization": { "command": "prof_index" },
      "external_baseline": { "label": "legacy", "command": "agree",
                             "work_count": { "command": "work_same" } },
      "external_baselines": {
        "agreeing": { "role": "floor", "max_ratio": 1.0, "command": "agree",
                      "work_count": { "command": "work_same" } },
        "skewed": { "role": "floor", "max_ratio": 1.0, "command": "agree",
                    "work_count": { "command": "work_skew" } },
        "unpinned": { "role": "floor", "max_ratio": 1.0, "command": "agree" },
        "differing": { "role": "north-star", "max_ratio": 1.0, "command": "differ",
                       "work_count": { "command": "work_same" } },
        "absent": { "role": "north-star", "max_ratio": 1.0, "command": "no_such_comparator_exe" }
      }
    },
    "prog_nochar": {
      "kind": "program-wallclock",
      "command": "printf '540000\\n'",
      "samples": 1,
      "work_count": { "command": "work_same" },
      "external_baselines": {
        "nochar_floor": { "role": "floor", "max_ratio": 1.0, "command": "agree",
                          "work_count": { "command": "work_same" } }
      }
    }
  },
  "noise_threshold_pct": 5.0,
  "min_profile_distance": 0.2
}
EOF

# 1. Subject reading: exactly one numeric line on stdout (the harness contract).
subject_out="$("$MEASURE" prog 2>/dev/null)"
[ "$(printf '%s\n' "$subject_out" | wc -l)" = "1" ] \
    || fail "subject probe printed more than one line: $subject_out"
case "$subject_out" in
    ''|*[!0-9.e+-]*) fail "subject reading is not numeric: $subject_out" ;;
esac

# 2. Back-compat: bare --baseline-ref still resolves the singular external_baseline.
"$MEASURE" prog --baseline-ref >/dev/null 2>&1 \
    || fail "bare --baseline-ref regressed (singular external_baseline path)"

# 3. Labelled comparator selection reaches external_baselines[LABEL].
"$MEASURE" prog --baseline-ref=agreeing >/dev/null 2>&1 \
    || fail "--baseline-ref=agreeing did not resolve"
if "$MEASURE" prog --baseline-ref=nope >/dev/null 2>&1; then
    fail "--baseline-ref=nope should exit non-zero for an unregistered label"
fi

# 4. The rss metric prints one numeric reading in KiB.
rss_out="$("$MEASURE" prog --metric rss 2>/dev/null)"
[ "$(printf '%s\n' "$rss_out" | wc -l)" = "1" ] || fail "rss probe printed more than one line"
python3 -c "import sys; v=float(sys.argv[1]); sys.exit(0 if v > 0 else 1)" "$rss_out" \
    || fail "rss reading not a positive number: $rss_out"

# 5. A criterion-throughput row rejects both program-wallclock-only modes.
python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
reg["subjects"]["bench_only"] = {"bench": "b", "case": "c", "direction": "higher-is-better"}
json.dump(reg, open(sys.argv[1], "w"))
PY
if "$MEASURE" bench_only --metric rss >/dev/null 2>&1; then
    fail "--metric rss must reject a criterion-throughput row"
fi
if "$MEASURE" bench_only --baseline-ref >/dev/null 2>&1; then
    fail "--baseline-ref must reject a criterion-throughput row"
fi
if "$MEASURE" bench_only --metric work >/dev/null 2>&1; then
    fail "--metric work must reject a criterion-throughput row"
fi
if "$MEASURE" bench_only --metric profile >/dev/null 2>&1; then
    fail "--metric profile must reject a criterion-throughput row"
fi

# 5b. --metric work prints the counter SUM as its single reading, records the
#     per-key map, rejects a non-work stdout stream, and rejects an unpinned row.
work_out="$("$MEASURE" prog --metric work 2>/dev/null)"
[ "$work_out" = "242000" ] || fail "work reading should be the counter sum 242000, got: $work_out"
"$MEASURE" prog --metric work --samples-json "$WORK/work.json" >/dev/null 2>&1 \
    || fail "--metric work --samples-json invocation failed"
python3 - "$WORK/work.json" <<'PY' || fail "work samples json missing required fields"
import json, sys
rec = json.load(open(sys.argv[1]))
assert rec["metric"] == "work" and rec["unit"] == "ops", rec
assert rec["direction"] == "equality", rec
assert rec["work_counts"] == {"calls": 500, "toggles": 241500}, rec
assert rec["reading"] == 242000, rec
PY
if "$MEASURE" prog --metric work --baseline-ref=unpinned >/dev/null 2>&1; then
    fail "--metric work must exit non-zero for a comparator with no work_count"
fi
python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
reg["subjects"]["junk_work"] = {
    "kind": "program-wallclock", "command": "agree", "samples": 1,
    "work_count": {"command": "work_junk"},
}
json.dump(reg, open(sys.argv[1], "w"))
PY
if "$MEASURE" junk_work --metric work >/dev/null 2>&1; then
    fail "--metric work must reject stdout that is not a 'work <key> <count>' stream"
fi

# 5c. --metric profile prints the operation-mix SUM, derives the
#     language-independent profile into --samples-json, and rejects a counter
#     stream outside the fixed vocabulary or inconsistent with itself.
prof_out="$("$MEASURE" prog --metric profile 2>/dev/null)"
[ "$prof_out" = "940" ] || fail "profile reading should be the mix sum 940, got: $prof_out"
"$MEASURE" prog --metric profile --samples-json "$WORK/profile.json" >/dev/null 2>&1 \
    || fail "--metric profile --samples-json invocation failed"
python3 - "$WORK/profile.json" <<'PY' || fail "profile samples json missing required fields"
import json, sys
rec = json.load(open(sys.argv[1]))
assert rec["metric"] == "profile" and rec["unit"] == "ops", rec
assert rec["direction"] == "characterization", rec
p = rec["profile"]
assert p["total_ops"] == 940, p
# Fixed vocabulary: exactly the eight mix categories, normalized to a simplex.
assert set(p["operation_mix"]) == {
    "alloc", "arith", "branch", "call", "field", "index", "loop_iter", "string_op"}, p
assert abs(sum(p["operation_mix"].values()) - 1.0) < 1e-12, p
assert abs(p["operation_mix"]["index"] - 500 / 940) < 1e-12, p
assert abs(p["allocation_rate"] - 10 / 940) < 1e-12, p
# Polymorphism degree = distinct dynamic targets per executed call site.
assert p["call_sites"] == 2 and p["call_targets"] == 3, p
assert abs(p["call_polymorphism_degree"] - 1.5) < 1e-12, p
PY
python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
for name, cmd in (("prof_bad", "prof_badkey"), ("prof_missing", "prof_nosites"),
                  ("prof_skewed_sites", "prof_badsites")):
    reg["subjects"][name] = {"kind": "program-wallclock", "command": "agree", "samples": 1,
                             "characterization": {"command": cmd}}
reg["subjects"]["prof_undeclared"] = {"kind": "program-wallclock", "command": "agree",
                                      "samples": 1}
json.dump(reg, open(sys.argv[1], "w"))
PY
if "$MEASURE" prof_bad --metric profile >/dev/null 2>&1; then
    fail "--metric profile must reject a key outside the fixed vocabulary"
fi
if "$MEASURE" prof_missing --metric profile >/dev/null 2>&1; then
    fail "--metric profile must reject a stream omitting call_sites"
fi
if "$MEASURE" prof_skewed_sites --metric profile >/dev/null 2>&1; then
    fail "--metric profile must reject call_targets < call_sites"
fi
if "$MEASURE" prof_undeclared --metric profile >/dev/null 2>&1; then
    fail "--metric profile must reject a row declaring no characterization.command"
fi
python3 - "$REGISTRY" <<'PY'
import json, sys
reg = json.load(open(sys.argv[1]))
for name in ("prof_bad", "prof_missing", "prof_skewed_sites", "prof_undeclared",
             "junk_work", "bench_only"):
    reg["subjects"].pop(name, None)
json.dump(reg, open(sys.argv[1], "w"))
PY

# 6. --samples-json records every raw observation plus the output digest.
"$MEASURE" prog --samples-json "$WORK/samples.json" >/dev/null 2>&1 \
    || fail "--samples-json invocation failed"
python3 - "$WORK/samples.json" <<'PY' || fail "samples json missing required fields"
import hashlib, json, sys
rec = json.load(open(sys.argv[1]))
assert rec["metric"] == "time" and rec["unit"] == "s", rec
assert len(rec["observations"]) == 2, rec["observations"]
assert rec["reading"] == min(rec["observations"]), rec
assert rec["output_sha256"] == hashlib.sha256(b"540000\n").hexdigest(), rec
assert rec["output_bytes"] == 7, rec
PY

# 7. Report: agreeing comparator admitted (POSITIVE work-count case); skewed
#    rejected on the work-count pin despite identical output (NEGATIVE case);
#    unpinned rejected for declaring no work-count pin at all; differing rejected
#    on the output check; absent comparator recorded unavailable without
#    blocking the others.
python3 "$WORK/scripts/bench_probes/report.py" --repo-root "$WORK" --json \
    > "$WORK/report.json" 2>"$WORK/report.err" \
    || { cat "$WORK/report.err" >&2; fail "report.py exited non-zero"; }
python3 - "$WORK/report.json" <<'PY' || fail "report admission gates did not hold"
import json, sys
report = json.load(open(sys.argv[1]))
subject = report["subjects"][0]
comparators = subject["comparators"]

assert subject["work_counts"] == {"calls": 500, "toggles": 241500}, subject

# POSITIVE: matching work counts admit the reading.
agreeing = comparators["agreeing"]
assert agreeing["available"] and agreeing["output_equal"], agreeing
assert agreeing["work_equal"] is True, agreeing
assert agreeing["work_counts"] == subject["work_counts"], agreeing
assert agreeing["verdict"] in {"met", "missed"}, agreeing
assert set(agreeing["ratios"]) == {"time", "rss"}, agreeing

# NEGATIVE: identical OUTPUT, divergent WORK -> detected, reading withheld.
skewed = comparators["skewed"]
assert skewed["available"] and skewed["output_equal"] is True, skewed
assert skewed["work_equal"] is False, skewed
assert skewed["verdict"] == "inconclusive", skewed
assert "work-count check failed" in skewed["reason"], skewed
assert "toggles subject=241500 comparator=241499" in skewed["reason"], skewed
assert "ratios" not in skewed, "unequal-work triple must yield no admitted ratio"

# NEGATIVE: a triple that declares no work-count pin is never admitted.
unpinned = comparators["unpinned"]
assert unpinned["output_equal"] is True, unpinned
assert unpinned["work_equal"] is False, unpinned
assert unpinned["verdict"] == "inconclusive", unpinned
assert "work-count pin missing on comparator" in unpinned["reason"], unpinned
assert "ratios" not in unpinned, "unpinned triple must yield no admitted ratio"

differing = comparators["differing"]
assert differing["available"], differing
assert differing["output_equal"] is False, differing
assert differing["verdict"] == "inconclusive", differing
assert "ratios" not in differing, "unequal-work triple must yield no admitted ratio"

absent = comparators["absent"]
assert absent["available"] is False and absent["verdict"] == "inconclusive", absent
assert "not on PATH" in absent["reason"], absent

# A missing comparator blocks only its own reading.
assert report["aggregate"]["agreeing"]["metrics"], "agreeing lost its aggregate"

agg = report["aggregate"]["agreeing"]
for metric, values in agg["metrics"].items():
    assert values["geometric_mean"] is not None, (metric, values)
    assert values["worst_ratio"] >= values["geometric_mean"] - 1e-12, (metric, values)
    assert values["worst_program"] == "prog", values

for metric in ("time", "rss"):
    reading = subject["readings"][metric]
    assert reading["distribution"]["n"] == 2, reading
    assert reading["distribution"]["median"] is not None, reading
assert subject["readings"]["work"]["reading"] == 242000, subject["readings"]["work"]

env = report["environment"]
for key in ("platform", "cpu_count", "harness_sha256", "registry_sha256", "head_sha"):
    assert key in env, key

# Characterization is recorded BESIDE the timing/rss readings for the same subject.
prof = subject["characterization"]
assert prof["total_ops"] == 940, prof
assert abs(prof["allocation_rate"] - 10 / 940) < 1e-12, prof
assert abs(prof["call_polymorphism_degree"] - 1.5) < 1e-12, prof
assert subject["readings"]["profile"]["reading"] == 940, subject["readings"]["profile"]

# A one-workload corpus can never read `met`: it is INCONCLUSIVE by construction.
div = report["diversity"]
assert div["verdict"] == "inconclusive", div
assert div["characterized_programs"] == ["prog"], div
assert "at least 2" in div["reason"], div
assert div["min_distance"] == 0.2, div

# A subject declaring no characterization degrades gracefully: it still produces
# its comparator readings and is REPORTED as uncharacterized, never dropped.
nochar = next(s for s in report["subjects"] if s["subject"] == "prog_nochar")
assert nochar["characterization"] is None, nochar
assert nochar["comparators"]["nochar_floor"]["verdict"] in {"met", "missed"}, nochar
assert div["uncharacterized_programs"] == ["prog_nochar"], div
PY

# 8. A per-program miss is never masked by a passing aggregate.
python3 - "$WORK/scripts/bench_probes" <<'PY' || fail "aggregate masked a per-program miss"
import sys
sys.path.insert(0, sys.argv[1])
import report as r

subjects = [
    {"subject": "fast", "comparators": {"c": {"role": "floor", "max_ratio": 0.5,
        "verdict": "met", "ratios": {"time": 0.05, "rss": 0.05}}}},
    {"subject": "slow", "comparators": {"c": {"role": "floor", "max_ratio": 0.5,
        "verdict": "missed", "ratios": {"time": 1.0, "rss": 0.05}}}},
]
agg = r.aggregate(subjects)["c"]
assert agg["verdict"] == "missed", agg
assert agg["blocking_programs"] == ["slow"], agg
assert agg["metrics"]["time"]["worst_program"] == "slow", agg["metrics"]["time"]
# geomean(0.05, 1.0) = 0.2236 reads under the 0.5 cap; the worst ratio blocks.
assert agg["metrics"]["time"]["geometric_mean"] < 0.5 < agg["metrics"]["time"]["worst_ratio"]
PY

# 9. Corpus diversity: a genuinely diverse corpus is admitted; a corpus whose
#    workloads share one operation profile is DETECTED and withheld. Without the
#    negative case the check would be decoration.
python3 - "$WORK/scripts/bench_probes" <<'PY' || fail "diversity check did not hold"
import sys
sys.path.insert(0, sys.argv[1])
import report as r

def workload(name, mix, *, alloc_rate=0.1, poly=1.0):
    return {"subject": name, "characterization": {
        "operation_mix": mix, "allocation_rate": alloc_rate,
        "call_polymorphism_degree": poly, "total_ops": 1000}}

index_heavy = {"alloc": 0.01, "arith": 0.02, "branch": 0.01, "call": 0.01,
               "field": 0.0, "index": 0.55, "loop_iter": 0.40, "string_op": 0.0}
string_heavy = {"alloc": 0.30, "arith": 0.02, "branch": 0.01, "call": 0.01,
                "field": 0.0, "index": 0.01, "loop_iter": 0.05, "string_op": 0.60}
call_heavy = {"alloc": 0.05, "arith": 0.05, "branch": 0.05, "call": 0.60,
              "field": 0.05, "index": 0.10, "loop_iter": 0.10, "string_op": 0.0}

# POSITIVE: three distinct operation regions -> admitted.
diverse = r.diversity(
    [workload("doors", index_heavy), workload("parse", string_heavy),
     workload("calls", call_heavy, poly=2.5)],
    min_distance=0.2)
assert diverse["verdict"] == "met", diverse
assert diverse["distinct_dominant_categories"] == 3, diverse
assert diverse["dominant_categories"] == {
    "doors": "index", "parse": "string_op", "calls": "call"}, diverse
assert diverse["mean_pairwise_distance"] >= 0.2, diverse
assert len(diverse["pairwise_distances"]) == 3, diverse
assert diverse["call_polymorphism_degrees"]["calls"] == 2.5, diverse
assert "reason" not in diverse, diverse

# NEGATIVE: one profile cloned across the corpus -> detected, never `met`.
cloned = r.diversity(
    [workload("a", index_heavy), workload("b", index_heavy), workload("c", index_heavy)],
    min_distance=0.2)
assert cloned["verdict"] == "missed", cloned
assert cloned["distinct_dominant_categories"] == 1, cloned
assert cloned["max_pairwise_distance"] == 0.0, cloned
assert "dominated by the same operation category (index)" in cloned["reason"], cloned
assert "mean pairwise operation-mix distance" in cloned["reason"], cloned

# NEGATIVE: distinct-but-adjacent profiles under the threshold -> detected. Two
# workloads that merely nudge the same mix are not two operation regions.
nudged = dict(index_heavy)
nudged["index"], nudged["loop_iter"] = 0.50, 0.45
adjacent = r.diversity(
    [workload("a", index_heavy), workload("b", nudged)], min_distance=0.2)
assert adjacent["verdict"] == "missed", adjacent
assert adjacent["distinct_dominant_categories"] == 1, adjacent
assert 0.0 < adjacent["mean_pairwise_distance"] < 0.2, adjacent

# NEGATIVE: a corpus too small to compare is INCONCLUSIVE, never a default pass.
alone = r.diversity([workload("only", index_heavy)], min_distance=0.2)
assert alone["verdict"] == "inconclusive", alone
assert "at least 2" in alone["reason"], alone
assert r.diversity([], min_distance=0.2)["verdict"] == "inconclusive"

# An uncharacterized workload is reported, not silently dropped from the corpus.
mixed = r.diversity(
    [workload("a", index_heavy), workload("b", string_heavy),
     {"subject": "unprofiled", "characterization": None}],
    min_distance=0.2)
assert mixed["uncharacterized_programs"] == ["unprofiled"], mixed
assert mixed["characterized_programs"] == ["a", "b"], mixed

# The distance metric itself: identical mixes are 0, disjoint mixes are 1.
assert r.mix_distance(index_heavy, index_heavy) == 0.0
assert abs(r.mix_distance({"a": 1.0}, {"b": 1.0}) - 1.0) < 1e-12
PY


printf 'All checks passed.\n'
