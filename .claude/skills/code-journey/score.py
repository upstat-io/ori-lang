#!/usr/bin/env python3
"""Code Journey scoring script — deterministic score computation from countable metrics.

The AI's job is to COUNT things from the IR.
This script's job is to MAP counts to scores.

Usage:
    python3 score.py \
      --instruction-ratio 1.07 \
      --instruction-ratio-max 1.07 \
      --arc-violations 0 \
      --arc-has-unbalanced false \
      --arc-has-scalar-rc false \
      --attr-applicable 12 \
      --attr-correct 10 \
      --attr-has-wrong false \
      --cf-defects 1 \
      --cf-incorrect false \
      --ir-unjustified 1 \
      --ir-incorrect false \
      --bin-defects 0 \
      --bin-hard-fail false

Output: JSON with scores + human-readable summary to stderr.
Exit code: 0 on success, 1 on invalid input.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field


# ---------------------------------------------------------------------------
# Threshold tables — each returns (score, tier_name)
# ---------------------------------------------------------------------------

def score_instruction_efficiency(ratio: float) -> tuple[int, str]:
    """Map weighted average instruction ratio to score."""
    if ratio == 1.0:
        return 10, "OPTIMAL"
    if ratio <= 1.10:
        return 9, "NEAR-OPTIMAL"
    if ratio <= 1.25:
        return 8, "NEAR-OPTIMAL"
    if ratio <= 1.50:
        return 7, "NEAR-OPTIMAL"
    if ratio <= 1.75:
        return 6, "ACCEPTABLE"
    if ratio <= 2.00:
        return 5, "ACCEPTABLE"
    if ratio <= 2.50:
        return 4, "ACCEPTABLE"
    if ratio <= 3.50:
        return 3, "BLOATED"
    if ratio <= 5.00:
        return 2, "BLOATED"
    return 1, "WASTEFUL"


def score_arc_correctness(violations: int) -> tuple[int, str]:
    """Map total ARC violation count to score."""
    if violations == 0:
        return 10, "PERFECT"
    if violations == 1:
        return 9, "NEAR-PERFECT"
    if violations == 2:
        return 8, "MINOR"
    if violations <= 4:
        return 7, "MINOR"
    if violations <= 6:
        return 6, "MODERATE"
    if violations <= 9:
        return 5, "MODERATE"
    if violations <= 14:
        return 4, "SIGNIFICANT"
    if violations <= 19:
        return 3, "SIGNIFICANT"
    if violations <= 29:
        return 2, "SEVERE"
    return 1, "CRITICAL"


def score_attributes_safety(compliance_pct: float) -> tuple[int, str]:
    """Map attribute compliance percentage to score."""
    if compliance_pct == 100.0:
        return 10, "COMPLETE"
    if compliance_pct >= 95.0:
        return 9, "NEAR-COMPLETE"
    if compliance_pct >= 90.0:
        return 8, "GOOD"
    if compliance_pct >= 80.0:
        return 7, "GOOD"
    if compliance_pct >= 70.0:
        return 6, "FAIR"
    if compliance_pct >= 60.0:
        return 5, "FAIR"
    if compliance_pct >= 50.0:
        return 4, "WEAK"
    if compliance_pct >= 40.0:
        return 3, "WEAK"
    if compliance_pct >= 25.0:
        return 2, "POOR"
    if compliance_pct >= 1.0:
        return 1, "POOR"
    return 0, "NONE"


def score_control_flow(defects: int) -> tuple[int, str]:
    """Map total control flow defect count to score."""
    if defects == 0:
        return 10, "PERFECT"
    if defects == 1:
        return 9, "NEAR-PERFECT"
    if defects <= 3:
        return 8, "MINOR"
    if defects <= 5:
        return 7, "MINOR"
    if defects <= 8:
        return 6, "MODERATE"
    if defects <= 12:
        return 5, "MODERATE"
    if defects <= 17:
        return 4, "SIGNIFICANT"
    if defects <= 24:
        return 3, "SIGNIFICANT"
    if defects <= 34:
        return 2, "SEVERE"
    return 1, "CRITICAL"


def score_ir_quality(unjustified: int) -> tuple[int, str]:
    """Map total unjustified instruction count to score."""
    if unjustified == 0:
        return 10, "PERFECT"
    if unjustified <= 2:
        return 9, "NEAR-PERFECT"
    if unjustified <= 5:
        return 8, "MINOR"
    if unjustified <= 9:
        return 7, "MINOR"
    if unjustified <= 14:
        return 6, "MODERATE"
    if unjustified <= 20:
        return 5, "MODERATE"
    if unjustified <= 30:
        return 4, "SIGNIFICANT"
    if unjustified <= 45:
        return 3, "SIGNIFICANT"
    if unjustified <= 70:
        return 2, "SEVERE"
    return 1, "CRITICAL"


def score_binary_quality(defects: int) -> tuple[int, str]:
    """Map binary quality defect count to score."""
    if defects == 0:
        return 10, "PERFECT"
    if defects == 1:
        return 9, "NEAR-PERFECT"
    if defects == 2:
        return 8, "MINOR"
    if defects <= 4:
        return 7, "MINOR"
    if defects <= 6:
        return 6, "MODERATE"
    if defects <= 9:
        return 5, "MODERATE"
    if defects <= 14:
        return 3, "SIGNIFICANT"
    return 1, "CRITICAL"


def score_other_findings(critical: int, high: int, low: int) -> tuple[int, str]:
    """Score for findings outside the 6 defined categories.

    Starts at 10, deducts by severity: critical=3, high=2, low=1.
    Clamped to [0, 10]. If no other findings, score is 10 (nothing wrong).
    """
    penalty = critical * 3 + high * 2 + low * 1
    score = max(0, 10 - penalty)
    if score == 10:
        return 10, "CLEAN"
    if score >= 8:
        return score, "MINOR"
    if score >= 5:
        return score, "MODERATE"
    if score >= 1:
        return score, "SIGNIFICANT"
    return 0, "SEVERE"


# ---------------------------------------------------------------------------
# Gate conditions
# ---------------------------------------------------------------------------

@dataclass
class GateResult:
    """Result of applying gate conditions to a dimension."""
    score: int
    tier: str
    gates_applied: list[str] = field(default_factory=list)


def apply_instruction_gates(
    base_score: int, base_tier: str, ratio_max: float
) -> GateResult:
    """Gate: if ANY function has ratio > 5.00x, cap at 5."""
    gates = []
    score = base_score
    if ratio_max > 5.0 and score > 5:
        score = 5
        gates.append(f"instruction_max_ratio_gate: function ratio {ratio_max:.2f}x > 5.00x, capped at 5")
    return GateResult(score, base_tier, gates)


def apply_arc_gates(
    base_score: int, base_tier: str,
    has_unbalanced: bool, has_scalar_rc: bool
) -> GateResult:
    """Gates: unbalanced RC caps at 3; scalar RC forces 0."""
    gates = []
    score = base_score
    if has_scalar_rc:
        score = 0
        gates.append("arc_scalar_rc_gate: RC operation on scalar type, score forced to 0")
        return GateResult(score, "FAILED", gates)
    if has_unbalanced and score > 3:
        score = 3
        gates.append("arc_unbalanced_gate: unbalanced RC pair (leak/double-free), capped at 3")
    return GateResult(score, base_tier, gates)


def apply_attribute_gates(
    base_score: int, base_tier: str, has_wrong: bool
) -> GateResult:
    """Gate: wrong attribute applied caps at 2."""
    gates = []
    score = base_score
    if has_wrong and score > 2:
        score = 2
        gates.append("attr_wrong_gate: wrong attribute applied (e.g., nounwind on unwinding fn), capped at 2")
    return GateResult(score, base_tier, gates)


def apply_control_flow_gates(
    base_score: int, base_tier: str, is_incorrect: bool
) -> GateResult:
    """Gate: semantically incorrect control flow caps at 1."""
    gates = []
    score = base_score
    if is_incorrect and score > 1:
        score = 1
        gates.append("cf_incorrect_gate: semantically incorrect control flow, capped at 1")
    return GateResult(score, base_tier, gates)


def apply_ir_gates(
    base_score: int, base_tier: str, is_incorrect: bool
) -> GateResult:
    """Gate: semantically incorrect IR caps at 1."""
    gates = []
    score = base_score
    if is_incorrect and score > 1:
        score = 1
        gates.append("ir_incorrect_gate: semantically incorrect IR, capped at 1")
    return GateResult(score, base_tier, gates)


def apply_binary_gates(
    base_score: int, base_tier: str, hard_fail: bool
) -> GateResult:
    """Gate: wrong output / crash / mismatch forces 0."""
    gates = []
    score = base_score
    if hard_fail:
        score = 0
        gates.append("bin_hard_fail_gate: wrong output, crash, or eval/AOT mismatch, score forced to 0")
    return GateResult(score, "FAILED" if hard_fail else base_tier, gates)


# ---------------------------------------------------------------------------
# Input parsing & validation
# ---------------------------------------------------------------------------

def parse_bool(s: str) -> bool:
    """Parse 'true'/'false' string to bool."""
    if s.lower() in ("true", "1", "yes"):
        return True
    if s.lower() in ("false", "0", "no"):
        return False
    raise argparse.ArgumentTypeError(f"Invalid boolean value: {s!r} (expected true/false)")


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Compute deterministic Code Journey scores from countable metrics."
    )

    # Instruction Efficiency
    p.add_argument("--instruction-ratio", type=float, required=True,
                    help="Weighted average instruction ratio (actual/ideal)")
    p.add_argument("--instruction-ratio-max", type=float, required=True,
                    help="Maximum per-function instruction ratio")

    # ARC Correctness
    p.add_argument("--arc-violations", type=int, required=True,
                    help="Total weighted ARC violation count")
    p.add_argument("--arc-has-unbalanced", type=parse_bool, required=True,
                    help="Whether any unbalanced RC pair exists")
    p.add_argument("--arc-has-scalar-rc", type=parse_bool, required=True,
                    help="Whether any RC operation on scalar type exists")

    # Attributes & Safety
    p.add_argument("--attr-applicable", type=int, required=True,
                    help="Total number of applicable attributes across all functions")
    p.add_argument("--attr-correct", type=int, required=True,
                    help="Total number of correctly applied attributes")
    p.add_argument("--attr-has-wrong", type=parse_bool, required=True,
                    help="Whether any wrong attribute is applied")

    # Control Flow
    p.add_argument("--cf-defects", type=int, required=True,
                    help="Total control flow defect count")
    p.add_argument("--cf-incorrect", type=parse_bool, required=True,
                    help="Whether control flow is semantically incorrect")

    # IR Quality
    p.add_argument("--ir-unjustified", type=int, required=True,
                    help="Total unjustified instruction count")
    p.add_argument("--ir-incorrect", type=parse_bool, required=True,
                    help="Whether IR is semantically incorrect")

    # Binary Quality
    p.add_argument("--bin-defects", type=int, required=True,
                    help="Total binary quality defect count")
    p.add_argument("--bin-hard-fail", type=parse_bool, required=True,
                    help="Whether binary has wrong output, crash, or eval/AOT mismatch")

    # Other Findings (optional — defaults to 0/0/0 if omitted)
    p.add_argument("--other-critical", type=int, default=0,
                    help="Count of CRITICAL findings outside the 6 defined categories")
    p.add_argument("--other-high", type=int, default=0,
                    help="Count of HIGH findings outside the 6 defined categories")
    p.add_argument("--other-low", type=int, default=0,
                    help="Count of LOW/MEDIUM findings outside the 6 defined categories")

    return p


def validate(args: argparse.Namespace) -> list[str]:
    """Return list of validation errors (empty = valid)."""
    errors = []
    if args.instruction_ratio < 1.0:
        errors.append(f"--instruction-ratio must be >= 1.0, got {args.instruction_ratio}")
    if args.instruction_ratio_max < 1.0:
        errors.append(f"--instruction-ratio-max must be >= 1.0, got {args.instruction_ratio_max}")
    if args.instruction_ratio_max < args.instruction_ratio:
        errors.append(
            f"--instruction-ratio-max ({args.instruction_ratio_max}) "
            f"must be >= --instruction-ratio ({args.instruction_ratio})"
        )
    if args.arc_violations < 0:
        errors.append(f"--arc-violations must be >= 0, got {args.arc_violations}")
    if args.attr_applicable < 0:
        errors.append(f"--attr-applicable must be >= 0, got {args.attr_applicable}")
    if args.attr_correct < 0:
        errors.append(f"--attr-correct must be >= 0, got {args.attr_correct}")
    if args.attr_correct > args.attr_applicable:
        errors.append(
            f"--attr-correct ({args.attr_correct}) "
            f"must be <= --attr-applicable ({args.attr_applicable})"
        )
    if args.cf_defects < 0:
        errors.append(f"--cf-defects must be >= 0, got {args.cf_defects}")
    if args.ir_unjustified < 0:
        errors.append(f"--ir-unjustified must be >= 0, got {args.ir_unjustified}")
    if args.bin_defects < 0:
        errors.append(f"--bin-defects must be >= 0, got {args.bin_defects}")
    if args.other_critical < 0:
        errors.append(f"--other-critical must be >= 0, got {args.other_critical}")
    if args.other_high < 0:
        errors.append(f"--other-high must be >= 0, got {args.other_high}")
    if args.other_low < 0:
        errors.append(f"--other-low must be >= 0, got {args.other_low}")
    return errors


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

# Weights (must sum to 1.0)
WEIGHTS = {
    "instruction_efficiency": 0.15,
    "arc_correctness": 0.20,
    "attributes_safety": 0.10,
    "control_flow": 0.10,
    "ir_quality": 0.20,
    "binary_quality": 0.10,
    "other_findings": 0.15,
}


def compute_scores(args: argparse.Namespace) -> dict:
    """Compute all scores from validated inputs. Returns JSON-serializable dict."""

    all_gates: list[str] = []

    # --- Instruction Efficiency ---
    ie_base_score, ie_tier = score_instruction_efficiency(args.instruction_ratio)
    ie_result = apply_instruction_gates(ie_base_score, ie_tier, args.instruction_ratio_max)
    all_gates.extend(ie_result.gates_applied)

    # --- ARC Correctness ---
    arc_base_score, arc_tier = score_arc_correctness(args.arc_violations)
    arc_result = apply_arc_gates(arc_base_score, arc_tier,
                                 args.arc_has_unbalanced, args.arc_has_scalar_rc)
    all_gates.extend(arc_result.gates_applied)

    # --- Attributes & Safety ---
    if args.attr_applicable == 0:
        attr_compliance_pct = 100.0  # no attributes to check = perfect compliance
    else:
        attr_compliance_pct = (args.attr_correct / args.attr_applicable) * 100.0
    attr_base_score, attr_tier = score_attributes_safety(attr_compliance_pct)
    attr_result = apply_attribute_gates(attr_base_score, attr_tier, args.attr_has_wrong)
    all_gates.extend(attr_result.gates_applied)

    # --- Control Flow ---
    cf_base_score, cf_tier = score_control_flow(args.cf_defects)
    cf_result = apply_control_flow_gates(cf_base_score, cf_tier, args.cf_incorrect)
    all_gates.extend(cf_result.gates_applied)

    # --- IR Quality ---
    ir_base_score, ir_tier = score_ir_quality(args.ir_unjustified)
    ir_result = apply_ir_gates(ir_base_score, ir_tier, args.ir_incorrect)
    all_gates.extend(ir_result.gates_applied)

    # --- Binary Quality ---
    bin_base_score, bin_tier = score_binary_quality(args.bin_defects)
    bin_result = apply_binary_gates(bin_base_score, bin_tier, args.bin_hard_fail)
    all_gates.extend(bin_result.gates_applied)

    # --- Other Findings ---
    other_score, other_tier = score_other_findings(
        args.other_critical, args.other_high, args.other_low
    )

    # --- Overall ---
    scores = {
        "instruction_efficiency": ie_result.score,
        "arc_correctness": arc_result.score,
        "attributes_safety": attr_result.score,
        "control_flow": cf_result.score,
        "ir_quality": ir_result.score,
        "binary_quality": bin_result.score,
        "other_findings": other_score,
    }

    overall = sum(scores[k] * WEIGHTS[k] for k in WEIGHTS)

    # Global gate: binary_quality == 0 caps overall at 3.0
    if bin_result.score == 0 and overall > 3.0:
        overall = 3.0
        all_gates.append("global_gate: binary_quality == 0, overall capped at 3.0")

    overall = round(overall, 1)

    return {
        "instruction_efficiency": ie_result.score,
        "arc_correctness": arc_result.score,
        "attributes_safety": attr_result.score,
        "control_flow": cf_result.score,
        "ir_quality": ir_result.score,
        "binary_quality": bin_result.score,
        "other_findings": other_score,
        "overall": overall,
        "gates_applied": all_gates,
        "metrics": {
            "instruction_ratio": args.instruction_ratio,
            "instruction_ratio_max": args.instruction_ratio_max,
            "arc_violation_count": args.arc_violations,
            "attribute_compliance_pct": round(attr_compliance_pct, 1),
            "control_flow_defects": args.cf_defects,
            "ir_unjustified_count": args.ir_unjustified,
            "binary_defect_count": args.bin_defects,
            "other_critical": args.other_critical,
            "other_high": args.other_high,
            "other_low": args.other_low,
        },
    }


def build_notes(result: dict) -> dict[str, str]:
    """Build Notes column text from metrics — no AI judgment needed."""
    m = result["metrics"]
    notes = {}

    # Instruction Efficiency
    ratio = m["instruction_ratio"]
    ratio_max = m["instruction_ratio_max"]
    if ratio == 1.0:
        notes["instruction_efficiency"] = "1.00x — OPTIMAL"
    elif ratio == ratio_max:
        notes["instruction_efficiency"] = f"{ratio:.2f}x avg ratio"
    else:
        notes["instruction_efficiency"] = f"{ratio:.2f}x avg ratio (max {ratio_max:.2f}x)"

    # ARC Correctness
    v = m["arc_violation_count"]
    notes["arc_correctness"] = f"{v} violation{'s' if v != 1 else ''}" if v else "0 violations"

    # Attributes & Safety
    pct = m["attribute_compliance_pct"]
    notes["attributes_safety"] = f"{pct:.1f}% compliance"

    # Control Flow
    d = m["control_flow_defects"]
    notes["control_flow"] = f"{d} defect{'s' if d != 1 else ''}" if d else "0 defects"

    # IR Quality
    u = m["ir_unjustified_count"]
    notes["ir_quality"] = (
        f"{u} unjustified instruction{'s' if u != 1 else ''}" if u
        else "0 unjustified instructions"
    )

    # Binary Quality
    b = m["binary_defect_count"]
    notes["binary_quality"] = f"{b} defect{'s' if b != 1 else ''}" if b else "0 defects"

    # Other Findings
    oc, oh, ol = m["other_critical"], m["other_high"], m["other_low"]
    total = oc + oh + ol
    if total == 0:
        notes["other_findings"] = "No uncategorized findings"
    else:
        parts = []
        if oc:
            parts.append(f"{oc} critical")
        if oh:
            parts.append(f"{oh} high")
        if ol:
            parts.append(f"{ol} low")
        notes["other_findings"] = ", ".join(parts)

    return notes


def build_markdown(result: dict) -> str:
    """Build the ready-to-paste Codegen Quality Score markdown section."""
    notes = build_notes(result)
    dims = [
        ("Instruction Efficiency", "instruction_efficiency", "15%"),
        ("ARC Correctness",       "arc_correctness",        "20%"),
        ("Attributes & Safety",   "attributes_safety",      "10%"),
        ("Control Flow",          "control_flow",           "10%"),
        ("IR Quality",            "ir_quality",             "20%"),
        ("Binary Quality",        "binary_quality",         "10%"),
        ("Other Findings",        "other_findings",         "15%"),
    ]

    lines = [
        "## Codegen Quality Score",
        "",
        "| Category | Weight | Score | Notes |",
        "|----------|--------|-------|-------|",
    ]
    for label, key, weight in dims:
        score = result[key]
        note = notes[key]
        lines.append(f"| {label} | {weight} | {score}/10 | {note} |")

    lines.append("")
    lines.append(f"**Overall: {result['overall']} / 10**")

    if result["gates_applied"]:
        lines.append("")
        lines.append("Gates applied:")
        for g in result["gates_applied"]:
            lines.append(f"- {g}")

    return "\n".join(lines)


def print_summary(result: dict) -> None:
    """Print human-readable summary to stderr."""
    print("\n--- Code Journey Score ---", file=sys.stderr)
    dims = [
        ("Instruction Efficiency", "instruction_efficiency", "15%"),
        ("ARC Correctness",       "arc_correctness",        "20%"),
        ("Attributes & Safety",   "attributes_safety",      "10%"),
        ("Control Flow",          "control_flow",           "10%"),
        ("IR Quality",            "ir_quality",             "20%"),
        ("Binary Quality",        "binary_quality",         "10%"),
        ("Other Findings",        "other_findings",         "15%"),
    ]
    for label, key, weight in dims:
        print(f"  {label:25s} ({weight}): {result[key]:>2}/10", file=sys.stderr)
    print(f"  {'Overall':25s}      : {result['overall']}/10", file=sys.stderr)

    if result["gates_applied"]:
        print("\n  Gates applied:", file=sys.stderr)
        for g in result["gates_applied"]:
            print(f"    - {g}", file=sys.stderr)
    print(file=sys.stderr)


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    errors = validate(args)
    if errors:
        for e in errors:
            print(f"error: {e}", file=sys.stderr)
        return 1

    result = compute_scores(args)
    result["markdown"] = build_markdown(result)
    print_summary(result)
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
