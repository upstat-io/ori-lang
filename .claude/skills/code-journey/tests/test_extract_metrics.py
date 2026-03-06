"""Tests for extract-metrics.py — end-to-end integration."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

# Import the function directly (not the script's main).
# extract-metrics.py has a hyphen so we use importlib to load it as a module.
import importlib.util
_spec = importlib.util.spec_from_file_location(
    "extract_metrics",
    str(Path(__file__).resolve().parent.parent / "extract-metrics.py"),
)
_em = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_em)
extract_metrics = _em.extract_metrics

GOLDEN_DIR = Path(__file__).resolve().parent / "golden"


def _load_journey1_ir() -> str:
    return (GOLDEN_DIR / "journey1_ir.txt").read_text()


class TestJourney1EndToEnd:
    def test_instruction_ratio(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["instruction_ratio"] == 1.0

    def test_instruction_ratio_max(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["instruction_ratio_max"] == 1.0

    def test_arc_violations(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["arc_violations"] == 0
        assert m["arc_has_unbalanced"] is False
        assert m["arc_has_scalar_rc"] is False

    def test_attribute_compliance(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["attr_applicable"] == 13
        assert m["attr_correct"] == 12
        assert m["attr_has_wrong"] is False

    def test_control_flow(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["cf_defects"] == 0
        assert m["cf_incorrect"] is False

    def test_ir_quality(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["ir_unjustified"] == 0
        assert m["ir_incorrect"] is False

    def test_binary_quality(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["bin_defects"] == 0
        assert m["bin_hard_fail"] is False

    def test_other_findings_are_null(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["other_critical"] is None
        assert m["other_high"] is None
        assert m["other_low"] is None

    def test_no_parse_errors(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert m["parse_errors"] is None

    def test_per_function_detail(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert "@_ori_add" in m["per_function"]
        assert "@_ori_main" in m["per_function"]
        add = m["per_function"]["@_ori_add"]
        assert add["instruction"]["actual"] == 7
        assert add["arc"]["rc_inc"] == 0

    def test_json_serializable(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        # Must not raise
        json.dumps(m)


class TestEmptyIR:
    def test_empty_ir_returns_failure_defaults(self):
        m = extract_metrics("", eval_exit=33, aot_exit=33, expected=33)
        assert m["ir_incorrect"] is True
        assert m["instruction_ratio"] == 999.0
        assert m["parse_errors"] is not None

    def test_empty_ir_still_computes_binary(self):
        m = extract_metrics("", eval_exit=33, aot_exit=33, expected=33)
        assert m["bin_defects"] == 0
        assert m["bin_hard_fail"] is False


class TestReproducibility:
    def test_same_output_10_times(self):
        """Determinism: same input → identical output."""
        ir = _load_journey1_ir()
        results = [
            json.dumps(extract_metrics(ir, 33, 33, 33), sort_keys=True)
            for _ in range(10)
        ]
        assert len(set(results)) == 1
