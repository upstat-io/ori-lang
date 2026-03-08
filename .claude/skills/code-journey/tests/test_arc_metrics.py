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


class TestEffectAwareBalance:
    """Tests for effect-summary-aware RC balance counting."""

    def test_str_from_raw_counts_as_allocation(self):
        """ori_str_from_raw +1 balances with buffer_rc_dec -1."""
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
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        f = am.per_function[0]
        assert f.rc_inc == 1  # ori_str_from_raw +1
        assert f.rc_dec == 1  # ori_buffer_rc_dec -1
        assert f.balanced is True
        assert am.total_violations == 0

    def test_list_alloc_data_counts_as_allocation(self):
        """ori_list_alloc_data +1 balances with buffer_rc_dec -1."""
        ir = """
define fastcc void @_ori_f() #0 {
entry:
  %data = call ptr @ori_list_alloc_data(i64 10, i64 8)
  call void @ori_buffer_rc_dec(ptr %data, i64 0, i64 10, i64 8, ptr null)
  ret void
}
declare ptr @ori_list_alloc_data(i64, i64)
declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        f = am.per_function[0]
        assert f.rc_inc == 1
        assert f.rc_dec == 1
        assert f.balanced is True

    def test_map_buffer_rc_dec_now_counted(self):
        """ori_map_buffer_rc_dec was missing from the old regex."""
        ir = """
define fastcc void @_ori_f() #0 {
entry:
  %data = call ptr @ori_map_literal_alloc(i64 2, i64 8, i64 8, ptr %cap)
  call void @ori_map_buffer_rc_dec(ptr %data, i64 4, i64 2, i64 8, i64 8, ptr null, ptr null)
  ret void
}
declare ptr @ori_map_literal_alloc(i64, i64, i64, ptr)
declare void @ori_map_buffer_rc_dec(ptr, i64, i64, i64, i64, ptr, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        f = am.per_function[0]
        assert f.rc_inc == 1  # map_literal_alloc +1
        assert f.rc_dec == 1  # map_buffer_rc_dec -1
        assert f.balanced is True

    def test_set_buffer_rc_dec_now_counted(self):
        """ori_set_buffer_rc_dec was missing from the old regex."""
        ir = """
define fastcc void @_ori_f() #0 {
entry:
  %data = call ptr @ori_set_literal_alloc(i64 2, i64 8, ptr %cap)
  call void @ori_set_buffer_rc_dec(ptr %data, i64 4, i64 2, i64 8, ptr null, ptr null)
  ret void
}
declare ptr @ori_set_literal_alloc(i64, i64, ptr)
declare void @ori_set_buffer_rc_dec(ptr, i64, i64, i64, ptr, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        f = am.per_function[0]
        assert f.rc_inc == 1  # set_literal_alloc +1
        assert f.rc_dec == 1  # set_buffer_rc_dec -1
        assert f.balanced is True

    def test_invoke_rc_ops_counted(self):
        """invoke (not just call) should be counted for RC ops."""
        ir = """
define fastcc void @_ori_f(ptr %0) #0 {
entry:
  invoke void @ori_rc_inc(ptr %0) to label %cont unwind label %cleanup
cont:
  invoke void @ori_rc_dec(ptr %0, ptr null) to label %ret unwind label %cleanup
ret:
  ret void
cleanup:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}
declare void @ori_rc_inc(ptr)
declare void @ori_rc_dec(ptr, ptr)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        f = am.per_function[0]
        assert f.rc_inc == 1
        assert f.rc_dec == 1
        assert f.balanced is True


class TestLandingpadExclusion:
    """Landingpad blocks are exception cleanup paths, not cumulative with normal flow."""

    def test_landingpad_rc_dec_excluded(self):
        """RC ops in landingpad blocks must not inflate normal-path counts.

        Pattern from J10 _ori_check_length: invoke creates a landing pad
        with a cleanup ori_buffer_rc_dec. This dec is an alternative to the
        normal-path dec (they are mutually exclusive execution paths).
        """
        ir = """
define fastcc i64 @_ori_f() #0 personality ptr @ori_eh_personality {
entry:
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %data = call ptr @ori_list_alloc_data(i64 3, i64 8)
  %list = insertvalue { i64, i64, ptr } undef, ptr %data, 2
  store { i64, i64, ptr } %list, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_callee(ptr %ref_arg)
          to label %cont unwind label %cleanup
cont:
  %rc.data = extractvalue { i64, i64, ptr } %list, 2
  call void @ori_buffer_rc_dec(ptr %rc.data, i64 3, i64 3, i64 8, ptr null)
  ret i64 %call
cleanup:
  %lp = landingpad { ptr, i32 } cleanup
  %rc.data2 = extractvalue { i64, i64, ptr } %list, 2
  call void @ori_buffer_rc_dec(ptr %rc.data2, i64 3, i64 3, i64 8, ptr null)
  resume { ptr, i32 } %lp
}
declare fastcc i64 @_ori_callee(ptr)
declare ptr @ori_list_alloc_data(i64, i64)
declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr)
declare ptr @ori_eh_personality(...)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        am = compute_arc_metrics(m)
        f = am.per_function[0]
        # 1 inc from ori_list_alloc_data, 1 dec from normal-path ori_buffer_rc_dec
        # The landingpad dec must NOT be counted (it's an alternative path)
        assert f.rc_inc == 1
        assert f.rc_dec == 1
        assert f.balanced is True
        assert am.has_unbalanced is False
        assert am.total_violations == 0

    def test_without_landingpad_exclusion_would_be_unbalanced(self):
        """Verify the landingpad block actually contains an RC op.

        Same IR as above — if we counted ALL blocks, we'd get 1 inc / 2 dec.
        This test documents the false positive that would occur without the fix.
        """
        ir = """
define fastcc i64 @_ori_f() #0 personality ptr @ori_eh_personality {
entry:
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %data = call ptr @ori_list_alloc_data(i64 3, i64 8)
  %list = insertvalue { i64, i64, ptr } undef, ptr %data, 2
  store { i64, i64, ptr } %list, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_callee(ptr %ref_arg)
          to label %cont unwind label %cleanup
cont:
  %rc.data = extractvalue { i64, i64, ptr } %list, 2
  call void @ori_buffer_rc_dec(ptr %rc.data, i64 3, i64 3, i64 8, ptr null)
  ret i64 %call
cleanup:
  %lp = landingpad { ptr, i32 } cleanup
  %rc.data2 = extractvalue { i64, i64, ptr } %list, 2
  call void @ori_buffer_rc_dec(ptr %rc.data2, i64 3, i64 3, i64 8, ptr null)
  resume { ptr, i32 } %lp
}
declare fastcc i64 @_ori_callee(ptr)
declare ptr @ori_list_alloc_data(i64, i64)
declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr)
declare ptr @ori_eh_personality(...)
attributes #0 = { nounwind uwtable }
"""
        m = parse_module(ir)
        # Verify the landingpad block exists and has instructions
        func = m.user_functions()[0]
        cleanup_block = [b for b in func.blocks if b.label == "cleanup"][0]
        assert cleanup_block.instructions[0].opcode == "landingpad"
        # The block contains an ori_buffer_rc_dec call
        has_rc_dec = any("ori_buffer_rc_dec" in i.text for i in cleanup_block.instructions)
        assert has_rc_dec is True
