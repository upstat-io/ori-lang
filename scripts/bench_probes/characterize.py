#!/usr/bin/env python3
"""Language-independent workload characterization and corpus diversity verdict.

Reads the derived characterization block `measure.sh --metric profile` writes
into its samples file and turns a set of those profiles into a falsifiable
diversity verdict over the workload corpus.

The harness owns the operation-mix vocabulary and its derivation; this module
only compares the profiles it is handed, so a new category needs no change here.

Owns the bench-probe verdict vocabulary shared with `report.py`.
"""

from __future__ import annotations

import statistics

#: Verdict vocabulary shared by every bench-probe verdict surface.
#: `inconclusive` is never a pass and never blocks a surface that did produce a
#: usable reading.
VERDICT_MET = "met"
VERDICT_MISSED = "missed"
VERDICT_INCONCLUSIVE = "inconclusive"

#: Fallback when the registry declares no `min_profile_distance`. The unit is
#: total-variation distance between two normalized operation-mix vectors, so
#: 0.2 means the two workloads disagree about where a fifth of their counted
#: operations go.
DEFAULT_MIN_PROFILE_DISTANCE = 0.2


def profile_of(probe: dict) -> dict | None:
    """The probe's derived characterization block, or None when it is absent.

    The harness owns the category vocabulary and the derivation; this reader
    never reconstructs either.
    """
    profile = probe.get("profile")
    if not isinstance(profile, dict):
        return None
    mix = profile.get("operation_mix")
    return profile if isinstance(mix, dict) and mix else None


def mix_distance(left: dict, right: dict) -> float:
    """Total-variation distance between two normalized operation-mix vectors.

    0.0 = identical mixes; 1.0 = disjoint. Half the L1 distance, so the value
    reads directly as "the fraction of counted operations the two workloads
    place in different categories".
    """
    return 0.5 * sum(
        abs(float(left.get(key, 0.0)) - float(right.get(key, 0.0)))
        for key in set(left) | set(right)
    )


def dominant_category(mix: dict) -> str:
    """The highest-share category; ties break on category name for determinism."""
    return min(sorted(mix), key=lambda key: (-float(mix[key]), key))


def diversity(subject_results: list[dict], *, min_distance: float) -> dict:
    """Corpus diversity verdict over the per-workload operation mixes.

    A corpus is `met` only when its characterized workloads land in more than
    one dominant operation category AND their mean pairwise mix distance clears
    `min_distance`. A corpus of near-identical workloads therefore reads
    `missed`, and a corpus too small to compare reads `inconclusive` -- never
    `met` by default.
    """
    characterized = [
        (result["subject"], result["characterization"])
        for result in subject_results
        if result.get("characterization")
    ]
    entry: dict = {
        "min_distance": min_distance,
        "characterized_programs": [name for name, _ in characterized],
        "uncharacterized_programs": [
            result["subject"] for result in subject_results if not result.get("characterization")
        ],
    }
    if len(characterized) < 2:
        entry["verdict"] = VERDICT_INCONCLUSIVE
        entry["reason"] = (
            f"corpus has {len(characterized)} characterized workload(s); "
            "a diversity verdict needs at least 2"
        )
        return entry

    dominants = {name: dominant_category(p["operation_mix"]) for name, p in characterized}
    pairs = [
        {
            "programs": [characterized[i][0], characterized[j][0]],
            "distance": mix_distance(
                characterized[i][1]["operation_mix"], characterized[j][1]["operation_mix"]
            ),
        }
        for i in range(len(characterized))
        for j in range(i + 1, len(characterized))
    ]
    distances = [pair["distance"] for pair in pairs]
    entry.update(
        {
            "dominant_categories": dominants,
            "distinct_dominant_categories": len(set(dominants.values())),
            "allocation_rates": {name: p.get("allocation_rate") for name, p in characterized},
            "call_polymorphism_degrees": {
                name: p.get("call_polymorphism_degree") for name, p in characterized
            },
            "pairwise_distances": pairs,
            "mean_pairwise_distance": statistics.fmean(distances),
            "min_pairwise_distance": min(distances),
            "max_pairwise_distance": max(distances),
        }
    )

    failures = []
    if entry["distinct_dominant_categories"] < 2:
        failures.append(
            "every characterized workload is dominated by the same operation category "
            f"({next(iter(set(dominants.values())))})"
        )
    if entry["mean_pairwise_distance"] < min_distance:
        failures.append(
            f"mean pairwise operation-mix distance {entry['mean_pairwise_distance']:.6g} "
            f"is below the required {min_distance:.6g}"
        )
    entry["verdict"] = VERDICT_MISSED if failures else VERDICT_MET
    if failures:
        entry["reason"] = "; ".join(failures)
    return entry


def render_profile(profile: dict | None) -> list[str]:
    """The subject's characterization lines, rendered beside its timing lines."""
    if not profile:
        return []
    mix = " ".join(
        f"{key}={share:.4f}" for key, share in sorted(profile["operation_mix"].items())
    )
    return [
        f"  profile total_ops={profile['total_ops']} "
        f"alloc_rate={profile['allocation_rate']:.6g} "
        f"call_polymorphism={profile['call_polymorphism_degree']:.6g}",
        f"  mix   {mix}",
    ]


def render_diversity(div: dict) -> list[str]:
    """The corpus diversity verdict block."""
    lines = [
        "",
        f"CORPUS DIVERSITY verdict={div['verdict']} min_distance={div['min_distance']}",
    ]
    if div.get("reason"):
        lines.append(f"  reason: {div['reason']}")
    if "mean_pairwise_distance" in div:
        lines.append(
            f"  mix distance mean={div['mean_pairwise_distance']:.6g} "
            f"min={div['min_pairwise_distance']:.6g} max={div['max_pairwise_distance']:.6g}"
        )
        lines.append(
            f"  distinct dominant categories={div['distinct_dominant_categories']} "
            f"({', '.join(f'{k}:{v}' for k, v in sorted(div['dominant_categories'].items()))})"
        )
    if div["uncharacterized_programs"]:
        lines.append(f"  uncharacterized: {', '.join(div['uncharacterized_programs'])}")
    return lines
