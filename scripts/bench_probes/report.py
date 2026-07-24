#!/usr/bin/env python3
"""Corpus comparator report over the ONE generic bench harness.

Performs no measurement: every reading is delegated to `measure.sh` (the single
harness) for one registry subject, one mode, one metric. Subjects and their
comparators are registry data rows.

Emits:
  - per-program readings, ratios, and raw-observation distributions,
  - the corpus geometric mean and the worst (slowest / largest) ratio, reported
    together so an aggregate cannot hide a per-program floor miss,
  - the environment and artifact identities needed to reproduce the comparison,
  - an output-equality check AND a work-count-equality check per comparator
    triple before any timing is admitted. Equal output does not prove equal
    work: a divergent loop bound, an early exit, or a closed-form shortcut can
    reach the same answer with different work, so both gates must pass.
  - the per-workload language-independent dynamic characterization (normalized
    operation mix, allocation rate, call-polymorphism degree) recorded beside
    that program's timing and memory readings, plus a corpus DIVERSITY verdict
    over those profiles. The verdict is what turns "the breadth matrix spans
    several operation regions" from an assertion into a falsifiable check: a
    corpus whose workloads share one dominant operation category, or whose
    profiles sit closer together than the registry's `min_profile_distance`,
    reads `missed`.

Usage:
    report.py [--subject NAME ...] [--out PATH] [--repo-root DIR] [--json]
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

#: The provenance layer owns environment capture, artifact identity, and the
#: AOT lane's invalid-run vocabulary and rendering; this report only classifies
#: and aggregates what the harness reports.
from provenance import (  # noqa: F401
    INVALID_RUN_EXIT,
    environment,
    executable_of,
    invalid_run_reason,
    render_aot,
    resolve_executable,
)

#: The characterization layer owns the verdict vocabulary, the diversity
#: verdict, and their rendering; the comparator verdicts below read the same
#: three values. `dominant_category` and `mix_distance` are re-exported for the
#: harness self-test.
from characterize import (  # noqa: F401
    DEFAULT_MIN_PROFILE_DISTANCE,
    VERDICT_INCONCLUSIVE,
    VERDICT_MET,
    VERDICT_MISSED,
    diversity,
    dominant_category,
    mix_distance,
    profile_of,
    render_diversity,
    render_profile,
)

HARNESS_REL = "scripts/bench_probes/measure.sh"
REGISTRY_REL = "scripts/bench_probes/registry.json"
PROGRAM_WALLCLOCK = "program-wallclock"

#: Ratio-bearing metrics. `work` is deliberately absent: work counts gate
#: admission by EQUALITY (BM-V03), they are never divided into a ratio.
METRICS = ("time", "rss")
WORK_METRIC = "work"

#: Characterization metric. Like `work` it is never divided into a ratio; its
#: readings describe a workload rather than compare two of them.
PROFILE_METRIC = "profile"


def run_probe(
    harness: Path, subject: str, *, metric: str, label: str | None
) -> dict:
    """Invoke the harness once; return its samples record plus invocation status.

    `label` selects a comparator via `--baseline-ref=<label>`; None measures the
    subject itself.
    """
    argv = [str(harness), subject, f"--metric={metric}"]
    if label is not None:
        argv.append(f"--baseline-ref={label}")
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as handle:
        samples_path = Path(handle.name)
    argv.append(f"--samples-json={samples_path}")
    try:
        completed = subprocess.run(argv, capture_output=True, text=True, check=False)
        record: dict = {"argv": argv, "returncode": completed.returncode}
        if completed.returncode != 0:
            record["error"] = completed.stderr.strip().splitlines()[-1:] or ["probe failed"]
            if completed.returncode == INVALID_RUN_EXIT:
                record["invalid_run"] = record["error"][0]
            return record
        try:
            record.update(json.loads(samples_path.read_text(encoding="utf-8")))
        except (OSError, json.JSONDecodeError) as exc:
            record["error"] = [f"unreadable samples json: {exc}"]
        return record
    finally:
        samples_path.unlink(missing_ok=True)


def distribution(observations: list[float]) -> dict:
    """Robust summary of raw observations; the median is the point estimate."""
    if not observations:
        return {"n": 0}
    ordered = sorted(observations)
    return {
        "n": len(ordered),
        "median": statistics.median(ordered),
        "min": ordered[0],
        "max": ordered[-1],
        "stdev": statistics.stdev(ordered) if len(ordered) > 1 else 0.0,
    }


def geometric_mean(values: list[float]) -> float | None:
    """Geometric mean; the only meaning-preserving average of normalized ratios."""
    positive = [v for v in values if v > 0]
    if not positive or len(positive) != len(values):
        return None
    return math.exp(sum(math.log(v) for v in positive) / len(positive))


def declares_program(spec: object) -> bool:
    """A comparator is measurable when it declares a command or an aot block."""
    return isinstance(spec, dict) and bool(spec.get("command") or spec.get("aot"))


def comparator_rows(row: dict) -> dict[str, dict]:
    plural = row.get("external_baselines")
    if isinstance(plural, dict) and plural:
        return {k: v for k, v in plural.items() if declares_program(v)}
    single = row.get("external_baseline")
    if declares_program(single):
        return {single.get("label", "external_baseline"): single}
    return {}


def measure_subject(harness: Path, subject: str) -> dict:
    """Every ratio metric plus the work-count and characterization probes.

    The counter probes run in their own untimed harness invocations, so neither
    the work pin nor the characterization perturbs the timing or RSS readings.
    """
    return {
        metric: run_probe(harness, subject, metric=metric, label=None)
        for metric in (*METRICS, WORK_METRIC, PROFILE_METRIC)
    }


def measure_comparator(harness: Path, subject: str, label: str, spec: dict) -> dict:
    """Every metric for one comparator, or an availability record when absent."""
    command = spec.get("command")
    if not command:
        # An aot comparator has no PATH executable: the harness builds and links it.
        resolved = "<aot>"
    else:
        resolved = resolve_executable(command)
    if resolved is None:
        return {
            "available": False,
            "reason": f"executable {executable_of(command)!r} not on PATH",
            "command": command,
        }
    return {
        "available": True,
        "executable_path": resolved,
        "command": command,
        "probes": {
            metric: run_probe(harness, subject, metric=metric, label=label)
            for metric in (*METRICS, WORK_METRIC)
        },
    }


def work_counts_of(probe: dict) -> dict[str, int] | None:
    """The probe's `{key: count}` map, or None when the pin is missing/failed."""
    counts = probe.get("work_counts")
    return counts if isinstance(counts, dict) and counts else None


def work_divergence(subject: dict | None, comparator: dict | None) -> str | None:
    """None when the two counter maps are identical; else why the pin failed."""
    if subject is None or comparator is None:
        missing = [
            side
            for side, counts in (("subject", subject), ("comparator", comparator))
            if counts is None
        ]
        return f"work-count pin missing on {' and '.join(missing)}"
    if subject == comparator:
        return None
    keys = sorted(set(subject) | set(comparator))
    diffs = [
        f"{key} subject={subject.get(key, 'absent')} comparator={comparator.get(key, 'absent')}"
        for key in keys
        if subject.get(key) != comparator.get(key)
    ]
    return "work-count check failed: " + "; ".join(diffs)


def ratio_for(subject_probe: dict, comparator_probe: dict) -> float | None:
    """`subject / comparator` for a lower-is-better metric (BM-F01 orientation)."""
    left, right = subject_probe.get("reading"), comparator_probe.get("reading")
    if not isinstance(left, (int, float)) or not isinstance(right, (int, float)) or right <= 0:
        return None
    return left / right


def evaluate_subject(harness: Path, subject: str, row: dict) -> dict:
    subject_probes = measure_subject(harness, subject)
    subject_invalid = invalid_run_reason(subject_probes)
    result: dict = {
        "subject": subject,
        "command": row.get("command"),
        # Compiler + native-runtime archive identity for an AOT row; None otherwise.
        "aot": subject_probes["time"].get("aot"),
        "invalid_run": subject_invalid,
        "readings": {
            metric: {
                "reading": probe.get("reading"),
                "unit": probe.get("unit"),
                "distribution": distribution(probe.get("observations") or []),
                "error": probe.get("error"),
            }
            for metric, probe in subject_probes.items()
        },
        "output_sha256": subject_probes["time"].get("output_sha256"),
        "work_counts": work_counts_of(subject_probes[WORK_METRIC]),
        # Recorded beside the timing/RSS readings above, from the same run identity.
        "characterization": profile_of(subject_probes[PROFILE_METRIC]),
        "comparators": {},
    }

    for label, spec in sorted(comparator_rows(row).items()):
        measured = measure_comparator(harness, subject, label, spec)
        entry: dict = {
            "role": spec.get("role"),
            "max_ratio": spec.get("max_ratio"),
            "command": measured.get("command"),
            "available": measured["available"],
        }
        if subject_invalid:
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["invalid_run"] = subject_invalid
            entry["reason"] = f"invalid run: {subject_invalid}"
            result["comparators"][label] = entry
            continue
        if not measured["available"]:
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["reason"] = measured["reason"]
            result["comparators"][label] = entry
            continue

        entry["executable_path"] = measured["executable_path"]
        probes = measured["probes"]
        comparator_invalid = invalid_run_reason(probes)
        if comparator_invalid:
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["invalid_run"] = comparator_invalid
            entry["reason"] = f"invalid run: {comparator_invalid}"
            result["comparators"][label] = entry
            continue
        entry["aot"] = probes["time"].get("aot")
        entry["output_sha256"] = probes["time"].get("output_sha256")
        entry["output_equal"] = (
            entry["output_sha256"] is not None
            and entry["output_sha256"] == result["output_sha256"]
        )
        entry["readings"] = {
            metric: {
                "reading": probe.get("reading"),
                "unit": probe.get("unit"),
                "distribution": distribution(probe.get("observations") or []),
                "error": probe.get("error"),
            }
            for metric, probe in probes.items()
        }
        if not entry["output_equal"]:
            # A mismatched-result triple invalidates every sample from it (BM-02).
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["reason"] = "output check failed: comparator output differs from subject"
            result["comparators"][label] = entry
            continue

        # Equal output is not equal work (BM-V03): the counter maps must match too.
        entry["work_counts"] = work_counts_of(probes[WORK_METRIC])
        divergence = work_divergence(result["work_counts"], entry["work_counts"])
        entry["work_equal"] = divergence is None
        if divergence is not None:
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["reason"] = divergence
            result["comparators"][label] = entry
            continue

        entry["ratios"] = {
            metric: ratio_for(subject_probes[metric], probes[metric]) for metric in METRICS
        }
        cap = spec.get("max_ratio")
        admissible = [r for r in entry["ratios"].values() if isinstance(r, (int, float))]
        if not admissible or len(admissible) != len(METRICS):
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["reason"] = "one or more metric readings unavailable"
        elif isinstance(cap, (int, float)):
            entry["verdict"] = (
                VERDICT_MET if max(admissible) <= cap else VERDICT_MISSED
            )
        else:
            entry["verdict"] = VERDICT_INCONCLUSIVE
            entry["reason"] = "row declares no max_ratio"
        result["comparators"][label] = entry

    return result


def aggregate(subject_results: list[dict]) -> dict:
    """Corpus aggregate per comparator and metric: geomean plus the worst ratio."""
    per_label: dict[str, dict] = {}
    for result in subject_results:
        for label, entry in result["comparators"].items():
            bucket = per_label.setdefault(
                label,
                {"role": entry.get("role"), "max_ratio": entry.get("max_ratio"), "metrics": {},
                 "inconclusive_programs": [], "blocking_programs": []},
            )
            if entry["verdict"] == VERDICT_INCONCLUSIVE:
                bucket["inconclusive_programs"].append(result["subject"])
                continue
            if entry["verdict"] == VERDICT_MISSED:
                bucket["blocking_programs"].append(result["subject"])
            for metric, ratio in (entry.get("ratios") or {}).items():
                if isinstance(ratio, (int, float)):
                    bucket["metrics"].setdefault(metric, []).append((result["subject"], ratio))

    aggregated: dict[str, dict] = {}
    for label, bucket in sorted(per_label.items()):
        metrics: dict[str, dict] = {}
        for metric, pairs in sorted(bucket["metrics"].items()):
            ratios = [r for _, r in pairs]
            worst_subject, worst_ratio = max(pairs, key=lambda pair: pair[1])
            metrics[metric] = {
                "per_program": {subject: ratio for subject, ratio in pairs},
                "geometric_mean": geometric_mean(ratios),
                "worst_ratio": worst_ratio,
                "worst_program": worst_subject,
                "programs": len(ratios),
            }
        cap = bucket["max_ratio"]
        if bucket["blocking_programs"]:
            verdict = VERDICT_MISSED
        elif not metrics or bucket["inconclusive_programs"]:
            verdict = VERDICT_INCONCLUSIVE
        elif isinstance(cap, (int, float)) and any(
            m["geometric_mean"] is None or m["geometric_mean"] > cap for m in metrics.values()
        ):
            verdict = VERDICT_MISSED
        else:
            verdict = VERDICT_MET
        aggregated[label] = {
            "role": bucket["role"],
            "max_ratio": cap,
            "verdict": verdict,
            "metrics": metrics,
            "inconclusive_programs": sorted(set(bucket["inconclusive_programs"])),
            "blocking_programs": sorted(set(bucket["blocking_programs"])),
        }
    return aggregated


def render_text(report: dict) -> str:
    lines = [
        "COMPARATOR CORPUS REPORT",
        f"  head={report['environment']['head_sha']} dirty={report['environment']['tree_dirty']}",
        f"  platform={report['environment']['platform']} cpus={report['environment']['cpu_count']}",
        "",
    ]
    for result in report["subjects"]:
        lines.append(f"subject {result['subject']}")
        lines.extend(render_aot(result.get("aot"), result.get("invalid_run")))
        lines.extend(render_profile(result.get("characterization")))
        for metric, reading in result["readings"].items():
            dist = reading["distribution"]
            lines.append(
                f"  {metric:<5} {reading['reading']} {reading['unit'] or ''} "
                f"(n={dist.get('n', 0)} median={dist.get('median')} "
                f"min={dist.get('min')} max={dist.get('max')})"
            )
        for label, entry in result["comparators"].items():
            if not entry["available"] or entry["verdict"] == VERDICT_INCONCLUSIVE:
                lines.append(
                    f"  vs {label:<10} {entry['verdict']}: {entry.get('reason', 'n/a')}"
                )
                continue
            ratios = " ".join(
                f"{metric}={entry['ratios'][metric]:.6g}" for metric in METRICS
                if isinstance(entry["ratios"].get(metric), (int, float))
            )
            work = entry.get("work_counts") or {}
            lines.append(
                f"  vs {label:<10} {entry['verdict']} (max_ratio={entry['max_ratio']}) {ratios}"
                f" work_equal={entry.get('work_equal')} work={sum(work.values())}"
            )
        lines.append("")
    lines.append("CORPUS AGGREGATE")
    for label, agg in report["aggregate"].items():
        lines.append(f"  {label} [{agg['role']}] verdict={agg['verdict']} cap={agg['max_ratio']}")
        for metric, values in agg["metrics"].items():
            lines.append(
                f"    {metric:<5} geomean={values['geometric_mean']:.6g} "
                f"worst={values['worst_ratio']:.6g} ({values['worst_program']}) "
                f"programs={values['programs']}"
            )
        if agg["blocking_programs"]:
            lines.append(f"    blocking: {', '.join(agg['blocking_programs'])}")
        if agg["inconclusive_programs"]:
            lines.append(f"    inconclusive: {', '.join(agg['inconclusive_programs'])}")
    lines.extend(render_diversity(report["diversity"]))
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--subject", action="append", default=None)
    parser.add_argument("--repo-root", default=None)
    parser.add_argument("--out", default=None)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    repo_root = Path(args.repo_root).resolve() if args.repo_root else Path(__file__).resolve().parents[2]
    harness, registry = repo_root / HARNESS_REL, repo_root / REGISTRY_REL
    if not harness.is_file() or not registry.is_file():
        sys.stderr.write(f"report.py: missing harness or registry under {repo_root}\n")
        return 2
    try:
        registry_doc = json.loads(registry.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        sys.stderr.write(f"report.py: unreadable registry: {exc}\n")
        return 2
    rows = registry_doc.get("subjects") or {}
    declared_distance = registry_doc.get("min_profile_distance")
    min_profile_distance = (
        float(declared_distance)
        if isinstance(declared_distance, (int, float))
        else DEFAULT_MIN_PROFILE_DISTANCE
    )

    wanted = args.subject or sorted(
        name
        for name, row in rows.items()
        if isinstance(row, dict)
        and row.get("kind") == PROGRAM_WALLCLOCK
        and comparator_rows(row)
    )
    unknown = [name for name in wanted if name not in rows]
    if unknown:
        sys.stderr.write(f"report.py: unregistered subject(s): {', '.join(unknown)}\n")
        return 3
    if not wanted:
        sys.stderr.write("report.py: no program-wallclock subject declares a comparator\n")
        return 3

    subject_results = [evaluate_subject(harness, name, rows[name]) for name in wanted]
    report = {
        "environment": environment(repo_root, harness, registry),
        "subjects": subject_results,
        "aggregate": aggregate(subject_results),
        "diversity": diversity(subject_results, min_distance=min_profile_distance),
    }
    if args.out:
        Path(args.out).write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    sys.stdout.write(
        json.dumps(report, indent=2, sort_keys=True) + "\n" if args.json else render_text(report)
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
