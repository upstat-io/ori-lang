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
        assert m["attr_applicable"] == 10
        assert m["attr_correct"] == 9
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


class TestV2Fields:
    """Tests for V2 integration fields (effect summaries, ownership, attributes)."""

    def test_module_balanced_field(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert "arc_module_balanced" in m
        assert m["arc_module_balanced"] is True

    def test_ownership_transfers_field(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        assert "arc_ownership_transfers" in m
        assert m["arc_ownership_transfers"] == 0

    def test_per_function_attribute_detail(self):
        m = extract_metrics(_load_journey1_ir(), eval_exit=33, aot_exit=33, expected=33)
        add = m["per_function"]["@_ori_add"]
        assert "attribute" in add
        assert "checks" in add["attribute"]
        assert add["attribute"]["checks"] > 0

    def test_ownership_transfer_annotation(self):
        """Ownership transfer detection: producer/consumer pair."""
        ir = """
define fastcc void @_ori_producer() #0 {
entry:
  call void @ori_rc_inc(ptr null)
  ret void
}
define fastcc void @_ori_consumer() #0 {
entry:
  call void @ori_rc_dec(ptr null, ptr null)
  ret void
}
declare void @ori_rc_inc(ptr)
declare void @ori_rc_dec(ptr, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = extract_metrics(ir, eval_exit=0, aot_exit=0, expected=0)
        # Module is balanced (1 inc, 1 dec across functions)
        assert m["arc_module_balanced"] is True
        # Individual functions are unbalanced → ownership transfers
        assert m["arc_ownership_transfers"] > 0

    def test_effect_summary_balances_allocation(self):
        """ori_str_from_raw counted as +1 balances buffer_rc_dec -1."""
        ir = """
define fastcc void @_ori_f(ptr %0) #0 {
entry:
  call void @ori_str_from_raw(ptr %sret, ptr %src, i64 5)
  call void @ori_buffer_rc_dec(ptr %data, i64 5, i64 8, i64 1, ptr null)
  ret void
}
declare void @ori_str_from_raw(ptr, ptr, i64)
declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = extract_metrics(ir, eval_exit=0, aot_exit=0, expected=0)
        assert m["arc_violations"] == 0
        assert m["arc_has_unbalanced"] is False


class TestReproducibility:
    def test_same_output_10_times(self):
        """Determinism: same input → identical output."""
        ir = _load_journey1_ir()
        results = [
            json.dumps(extract_metrics(ir, 33, 33, 33), sort_keys=True)
            for _ in range(10)
        ]
        assert len(set(results)) == 1
