#!/usr/bin/env python3
"""Extract all journey metrics deterministically from compiler artifacts.

Reads LLVM IR and binary analysis output, produces a JSON metrics file
that feeds directly into score.py. No AI judgment in the numbers.

Requires Python 3.10+.

Usage:
    python3 extract-metrics.py \
      --ir-file /tmp/journey_N/llvm_ir.txt \
      --eval-exit 33 \
      --aot-exit 33 \
      --expected 33 \
      [--sections-file /tmp/journey_N/sections.txt] \
      [--json]

Output: JSON with all metrics ready for score.py input.
Also prints the score.py command line to stderr for easy copy-paste.

Exit codes:
    0 — success (even with failure metrics for bad input)
    1 — invalid arguments or missing required files
    2 — internal error (unexpected exception)
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Add script directory to path for sibling imports
sys.path.insert(0, str(Path(__file__).resolve().parent))

from ir_parser import parse_module
from instruction_metrics import compute_instruction_metrics
from arc_metrics import compute_arc_metrics
from attribute_metrics import compute_attribute_metrics
from control_flow_metrics import compute_control_flow_metrics
from binary_metrics import compute_binary_metrics


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Extract journey metrics deterministically from compiler artifacts.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
    # Basic usage with IR and exit codes:
    python3 extract-metrics.py --ir-file ir.txt --eval-exit 33 --aot-exit 33 --expected 33

    # With section sizes:
    python3 extract-metrics.py --ir-file ir.txt --eval-exit 33 --aot-exit 33 --expected 33 \\
        --sections-file sections.txt

Note: other_critical, other_high, other_low are output as null — they must be
supplied by the background agent (AI-determined dimension).
""",
    )
    p.add_argument("--ir-file", type=Path, required=True,
                    help="Path to LLVM IR dump (from ORI_DUMP_AFTER_LLVM=1)")
    p.add_argument("--eval-exit", type=int, required=True,
                    help="Interpreter exit code")
    p.add_argument("--aot-exit", type=int, default=None,
                    help="AOT binary exit code (omit if compilation failed)")
    p.add_argument("--expected", type=int, required=True,
                    help="Expected exit code")
    p.add_argument("--sections-file", type=Path, default=None,
                    help="Path to `size -A binary` output (optional)")
    p.add_argument("--json", action="store_true", default=True,
                    help="Output JSON (default)")
    return p


def _failure_metrics(expected: int, eval_exit: int, aot_exit: int | None) -> dict:
    """Produce worst-case metrics when IR is empty/missing."""
    return {
        "instruction_ratio": 999.0,
        "instruction_ratio_max": 999.0,
        "arc_violations": 0,
        "arc_has_unbalanced": False,
        "arc_has_scalar_rc": False,
        "attr_applicable": 0,
        "attr_correct": 0,
        "attr_has_wrong": False,
        "cf_defects": 0,
        "cf_incorrect": False,
        "ir_unjustified": 0,
        "ir_incorrect": True,
        "bin_defects": 0,
        "bin_hard_fail": False,
        "other_critical": None,
        "other_high": None,
        "other_low": None,
        "parse_errors": ["Empty IR input"],
        "per_function": {},
    }


def extract_metrics(
    ir_text: str,
    eval_exit: int,
    aot_exit: int | None,
    expected: int,
    sections_text: str | None = None,
) -> dict:
    """Extract all metrics from IR text and exit codes.

    Returns a dict ready for JSON serialization. All 6 algorithmic dimensions
    are computed; other_critical/high/low are set to null (AI-determined).
    """
    # Handle empty IR
    if not ir_text or not ir_text.strip():
        print("ERROR: IR file is empty (compilation likely failed). "
              "IR-derived metrics set to failure defaults.", file=sys.stderr)
        result = _failure_metrics(expected, eval_exit, aot_exit)
        # Still compute binary metrics
        bm = compute_binary_metrics(eval_exit, aot_exit, expected, sections_text)
        result["bin_defects"] = bm.defects
        result["bin_hard_fail"] = bm.hard_fail
        result["binary_info"] = {
            "eval_exit": bm.eval_exit,
            "aot_exit": bm.aot_exit,
            "binary_size": bm.binary_size,
            "text_size": bm.text_size,
            "rodata_size": bm.rodata_size,
        }
        return result

    # Parse IR
    module = parse_module(ir_text)

    # Compute all metrics (5 independent extractors)
    im = compute_instruction_metrics(module)
    am = compute_arc_metrics(module)
    atm = compute_attribute_metrics(module)
    cfm = compute_control_flow_metrics(module)
    bm = compute_binary_metrics(eval_exit, aot_exit, expected, sections_text)

    # Build per-function detail
    per_function: dict[str, dict] = {}
    for fm in im.per_function:
        per_function.setdefault(fm.name, {})["instruction"] = {
            "actual": fm.actual, "unjustified": fm.unjustified,
            "ideal": fm.ideal, "ratio": fm.ratio, "verdict": fm.verdict,
        }
    for fm in am.per_function:
        per_function.setdefault(fm.name, {})["arc"] = {
            "rc_inc": fm.rc_inc, "rc_dec": fm.rc_dec,
            "balanced": fm.balanced, "has_scalar_rc": fm.has_scalar_rc,
        }
    for fm in cfm.per_function:
        per_function.setdefault(fm.name, {})["control_flow"] = {
            "blocks": fm.block_count, "empty": fm.empty_blocks,
            "redundant_br": fm.redundant_branches, "trivial_phi": fm.trivial_phis,
        }

    return {
        "instruction_ratio": im.avg_ratio,
        "instruction_ratio_max": im.max_ratio,
        "arc_violations": am.total_violations,
        "arc_has_unbalanced": am.has_unbalanced,
        "arc_has_scalar_rc": am.has_scalar_rc,
        "attr_applicable": atm.total_applicable,
        "attr_correct": atm.total_correct,
        "attr_has_wrong": atm.has_wrong,
        "cf_defects": cfm.total_defects,
        "cf_incorrect": cfm.is_incorrect,
        "ir_unjustified": im.total_unjustified,
        "ir_incorrect": cfm.is_incorrect,
        "bin_defects": bm.defects,
        "bin_hard_fail": bm.hard_fail,
        "other_critical": None,
        "other_high": None,
        "other_low": None,
        "parse_errors": module.parse_errors if module.parse_errors else None,
        "per_function": per_function,
        "binary_info": {
            "eval_exit": bm.eval_exit,
            "aot_exit": bm.aot_exit,
            "binary_size": bm.binary_size,
            "text_size": bm.text_size,
            "rodata_size": bm.rodata_size,
        },
    }


def _score_py_command(metrics: dict) -> str:
    """Build the score.py command line from extracted metrics."""
    def _bool(v: bool) -> str:
        return "true" if v else "false"

    parts = [
        "python3 score.py",
        f"--instruction-ratio {metrics['instruction_ratio']:.2f}",
        f"--instruction-ratio-max {metrics['instruction_ratio_max']:.2f}",
        f"--arc-violations {metrics['arc_violations']}",
        f"--arc-has-unbalanced {_bool(metrics['arc_has_unbalanced'])}",
        f"--arc-has-scalar-rc {_bool(metrics['arc_has_scalar_rc'])}",
        f"--attr-applicable {metrics['attr_applicable']}",
        f"--attr-correct {metrics['attr_correct']}",
        f"--attr-has-wrong {_bool(metrics['attr_has_wrong'])}",
        f"--cf-defects {metrics['cf_defects']}",
        f"--cf-incorrect {_bool(metrics['cf_incorrect'])}",
        f"--ir-unjustified {metrics['ir_unjustified']}",
        f"--ir-incorrect {_bool(metrics['ir_incorrect'])}",
        f"--bin-defects {metrics['bin_defects']}",
        f"--bin-hard-fail {_bool(metrics['bin_hard_fail'])}",
    ]
    # other_* are null — agent must supply them
    return " \\\n    ".join(parts)


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    # Validate IR file
    if not args.ir_file.exists():
        print(f"ERROR: IR file not found: {args.ir_file}", file=sys.stderr)
        return 1

    try:
        ir_text = args.ir_file.read_text()
    except OSError as e:
        print(f"ERROR: Cannot read IR file: {e}", file=sys.stderr)
        return 1

    sections_text = None
    if args.sections_file:
        try:
            sections_text = args.sections_file.read_text()
        except OSError:
            pass  # Optional — missing is fine

    try:
        metrics = extract_metrics(
            ir_text=ir_text,
            eval_exit=args.eval_exit,
            aot_exit=args.aot_exit,
            expected=args.expected,
            sections_text=sections_text,
        )
    except Exception as e:
        print(f"INTERNAL ERROR: {e}", file=sys.stderr)
        return 2

    # Output score.py command to stderr
    print("\n--- score.py command ---", file=sys.stderr)
    print(_score_py_command(metrics), file=sys.stderr)
    print(file=sys.stderr)

    # Output JSON to stdout
    print(json.dumps(metrics, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
