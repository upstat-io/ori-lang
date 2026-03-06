"""Tests for instruction_metrics.py — instruction efficiency scoring."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from ir_parser import parse_module
from instruction_metrics import compute_instruction_metrics, compute_unjustified

GOLDEN_DIR = Path(__file__).resolve().parent / "golden"


def _load_journey1_ir() -> str:
    return (GOLDEN_DIR / "journey1_ir.txt").read_text()


class TestJourney1:
    def test_avg_ratio_is_optimal(self):
        m = parse_module(_load_journey1_ir())
        im = compute_instruction_metrics(m)
        assert im.avg_ratio == 1.0

    def test_max_ratio_is_optimal(self):
        m = parse_module(_load_journey1_ir())
        im = compute_instruction_metrics(m)
        assert im.max_ratio == 1.0

    def test_zero_unjustified(self):
        m = parse_module(_load_journey1_ir())
        im = compute_instruction_metrics(m)
        assert im.total_unjustified == 0

    def test_two_user_functions(self):
        m = parse_module(_load_journey1_ir())
        im = compute_instruction_metrics(m)
        assert len(im.per_function) == 2

    def test_ori_add_counts(self):
        m = parse_module(_load_journey1_ir())
        im = compute_instruction_metrics(m)
        add = [f for f in im.per_function if f.name == "@_ori_add"][0]
        assert add.actual == 7
        assert add.ideal == 7
        assert add.unjustified == 0
        assert add.verdict == "OPTIMAL"

    def test_ori_main_counts(self):
        m = parse_module(_load_journey1_ir())
        im = compute_instruction_metrics(m)
        main = [f for f in im.per_function if f.name == "@_ori_main"][0]
        assert main.actual == 14
        assert main.ideal == 14
        assert main.unjustified == 0
        assert main.verdict == "OPTIMAL"


class TestUnjustifiedDetection:
    def test_redundant_branch_detected(self):
        """Inject a redundant unconditional branch."""
        ir = """
define fastcc i64 @_ori_f() #0 {
entry:
  br label %next
next:
  ret i64 42
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        assert compute_unjustified(f) == 1

    def test_trivial_phi_detected(self):
        """Inject a trivial phi with single incoming value."""
        ir = """
define fastcc i64 @_ori_f() #0 {
entry:
  br label %next
next:
  %x = phi i64 [ 42, %entry ]
  ret i64 %x
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        # 1 redundant branch + 1 trivial phi
        assert compute_unjustified(f) == 2

    def test_ret_only_function(self):
        """Empty function (only ret) → 0 unjustified."""
        ir = """
define fastcc void @_ori_noop() #0 {
entry:
  ret void
}
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        im = compute_instruction_metrics(m)
        assert im.per_function[0].actual == 1
        assert im.per_function[0].ideal == 1
        assert im.per_function[0].ratio == 1.0

    def test_call_and_ret(self):
        """Pure call function → 0 unjustified."""
        ir = """
define fastcc i64 @_ori_f() #0 {
entry:
  %r = call fastcc i64 @_ori_g()
  ret i64 %r
}
declare fastcc i64 @_ori_g()
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        im = compute_instruction_metrics(m)
        f = im.per_function[0]
        assert f.actual == 2
        assert f.ideal == 2
        assert f.ratio == 1.0


class TestVerdicts:
    def test_verdict_thresholds(self):
        from instruction_metrics import _verdict
        assert _verdict(1.0) == "OPTIMAL"
        assert _verdict(1.25) == "NEAR-OPTIMAL"
        assert _verdict(1.50) == "NEAR-OPTIMAL"
        assert _verdict(2.0) == "ACCEPTABLE"
        assert _verdict(3.0) == "BLOATED"
        assert _verdict(6.0) == "WASTEFUL"
