"""Tests for control_flow_metrics.py — control flow defect scoring."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from ir_parser import parse_module
from control_flow_metrics import compute_control_flow_metrics

GOLDEN_DIR = Path(__file__).resolve().parent / "golden"


def _load_journey1_ir() -> str:
    return (GOLDEN_DIR / "journey1_ir.txt").read_text()


class TestJourney1:
    def test_zero_defects(self):
        m = parse_module(_load_journey1_ir())
        cfm = compute_control_flow_metrics(m)
        assert cfm.total_defects == 0

    def test_not_incorrect(self):
        m = parse_module(_load_journey1_ir())
        cfm = compute_control_flow_metrics(m)
        assert cfm.is_incorrect is False

    def test_two_user_functions(self):
        m = parse_module(_load_journey1_ir())
        cfm = compute_control_flow_metrics(m)
        assert len(cfm.per_function) == 2


class TestDefectDetection:
    def test_empty_block(self):
        ir = """
define fastcc i64 @_ori_f() #0 {
entry:
  %x = add i64 1, 2
  br label %empty
empty:
  br label %real
real:
  ret i64 %x
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        cfm = compute_control_flow_metrics(m)
        # `empty` is the only empty block (entry has add+br, real has ret)
        assert cfm.per_function[0].empty_blocks == 1
        assert cfm.total_defects >= 1

    def test_redundant_branch(self):
        """Non-entry empty block with unconditional br to next block is redundant."""
        ir = """
define fastcc i64 @_ori_f(i1 %cond) #0 {
entry:
  br i1 %cond, label %then, label %empty
then:
  ret i64 1
empty:
  br label %merge
merge:
  ret i64 0
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        cfm = compute_control_flow_metrics(m)
        assert cfm.per_function[0].redundant_branches == 1

    def test_same_target_conditional_is_incorrect(self):
        ir = """
define fastcc i64 @_ori_f(i1 %cond) #0 {
entry:
  br i1 %cond, label %same, label %same
same:
  ret i64 42
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        cfm = compute_control_flow_metrics(m)
        assert cfm.is_incorrect is True


class TestCleanIR:
    def test_conditional_branch_not_redundant(self):
        """Conditional branches are never redundant."""
        ir = """
define fastcc i64 @_ori_f(i1 %cond) #0 {
entry:
  br i1 %cond, label %then, label %else
then:
  ret i64 1
else:
  ret i64 0
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        cfm = compute_control_flow_metrics(m)
        assert cfm.total_defects == 0
        assert cfm.is_incorrect is False
