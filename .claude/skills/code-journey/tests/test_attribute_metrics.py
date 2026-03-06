"""Tests for attribute_metrics.py — attribute compliance scoring."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from ir_parser import parse_module
from attribute_metrics import compute_attribute_metrics

GOLDEN_DIR = Path(__file__).resolve().parent / "golden"


def _load_journey1_ir() -> str:
    return (GOLDEN_DIR / "journey1_ir.txt").read_text()


class TestJourney1:
    def test_compliance_pct(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        # 12/13 = 92.3% (main wrapper missing noundef on return)
        assert am.compliance_pct == 92.3

    def test_no_wrong_attributes(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        assert am.has_wrong is False

    def test_applicable_count(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        assert am.total_applicable == 13

    def test_correct_count(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        assert am.total_correct == 12


class TestOriAdd:
    def test_fastcc_present(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        add_metrics = [f for f in am.per_function if f.name == "@_ori_add"][0]
        fastcc = [c for c in add_metrics.checks if c.attr == "fastcc"][0]
        assert fastcc.applicable is True
        assert fastcc.present is True

    def test_noundef_return_present(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        add_metrics = [f for f in am.per_function if f.name == "@_ori_add"][0]
        noundef = [c for c in add_metrics.checks if c.attr == "noundef"][0]
        assert noundef.applicable is True
        assert noundef.present is True


class TestOriPanicCstr:
    def test_noreturn_present(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        panic = [f for f in am.per_function if f.name == "@ori_panic_cstr"][0]
        noreturn = [c for c in panic.checks if c.attr == "noreturn"][0]
        assert noreturn.applicable is True
        assert noreturn.present is True

    def test_cold_present(self):
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        panic = [f for f in am.per_function if f.name == "@ori_panic_cstr"][0]
        cold = [c for c in panic.checks if c.attr == "cold"][0]
        assert cold.applicable is True
        assert cold.present is True

    def test_nounwind_not_applicable(self):
        """Declarations don't have nounwind checked."""
        m = parse_module(_load_journey1_ir())
        am = compute_attribute_metrics(m)
        panic = [f for f in am.per_function if f.name == "@ori_panic_cstr"][0]
        nounwind = [c for c in panic.checks if c.attr == "nounwind"][0]
        assert nounwind.applicable is False


class TestWrongAttributeDetection:
    def test_nounwind_on_panic_is_wrong(self):
        ir = """
declare void @ori_panic_cstr(ptr) #0
attributes #0 = { cold noreturn nounwind }
"""
        m = parse_module(ir)
        am = compute_attribute_metrics(m)
        assert am.has_wrong is True

    def test_noreturn_on_returning_function(self):
        ir = """
define fastcc i64 @_ori_f() #0 {
entry:
  ret i64 42
}
attributes #0 = { nounwind uwtable noreturn }
"""
        m = parse_module(ir)
        am = compute_attribute_metrics(m)
        assert am.has_wrong is True


class TestEdgeCases:
    def test_no_applicable_attributes(self):
        """An LLVM intrinsic has no applicable attribute rules."""
        ir = """
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
"""
        m = parse_module(ir)
        am = compute_attribute_metrics(m)
        assert am.total_applicable == 0
        assert am.compliance_pct == 100.0  # 0/0 = 100%
