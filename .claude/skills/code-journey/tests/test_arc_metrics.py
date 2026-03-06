"""Tests for arc_metrics.py — ARC violation scoring."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from ir_parser import parse_module
from arc_metrics import compute_arc_metrics

GOLDEN_DIR = Path(__file__).resolve().parent / "golden"


def _load_journey1_ir() -> str:
    return (GOLDEN_DIR / "journey1_ir.txt").read_text()


class TestJourney1:
    def test_zero_violations(self):
        m = parse_module(_load_journey1_ir())
        am = compute_arc_metrics(m)
        assert am.total_violations == 0

    def test_no_unbalanced(self):
        m = parse_module(_load_journey1_ir())
        am = compute_arc_metrics(m)
        assert am.has_unbalanced is False

    def test_no_scalar_rc(self):
        m = parse_module(_load_journey1_ir())
        am = compute_arc_metrics(m)
        assert am.has_scalar_rc is False

    def test_both_functions_balanced(self):
        m = parse_module(_load_journey1_ir())
        am = compute_arc_metrics(m)
        for f in am.per_function:
            assert f.rc_inc == 0
            assert f.rc_dec == 0
            assert f.balanced is True


class TestWithRcOps:
    def test_balanced_rc(self):
        """Balanced inc/dec separated by usage (not wasted)."""
        ir = """
define fastcc void @_ori_f(ptr %0) #0 {
entry:
  call void @ori_rc_inc(ptr %0)
  %x = load i64, ptr %0
  call void @ori_rc_dec(ptr %0, ptr null)
  ret void
}
declare void @ori_rc_inc(ptr)
declare void @ori_rc_dec(ptr, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        assert am.per_function[0].rc_inc == 1
        assert am.per_function[0].rc_dec == 1
        assert am.per_function[0].balanced is True
        assert am.per_function[0].wasted_pairs == 0
        assert am.total_violations == 0

    def test_unbalanced_rc(self):
        ir = """
define fastcc void @_ori_f(ptr %0) #0 {
entry:
  call void @ori_rc_inc(ptr %0)
  ret void
}
declare void @ori_rc_inc(ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        assert am.per_function[0].rc_inc == 1
        assert am.per_function[0].rc_dec == 0
        assert am.per_function[0].balanced is False
        assert am.has_unbalanced is True
        # Unbalanced by 1, weight 3
        assert am.total_violations == 3

    def test_wasted_pair_detected(self):
        ir = """
define fastcc void @_ori_f(ptr %0) #0 {
entry:
  call void @ori_rc_inc(ptr %0)
  call void @ori_rc_dec(ptr %0, ptr null)
  ret void
}
declare void @ori_rc_inc(ptr)
declare void @ori_rc_dec(ptr, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        # Consecutive inc+dec on same ptr = wasted pair
        assert am.per_function[0].wasted_pairs == 1
        assert am.total_violations == 1  # weight 1 per wasted pair
