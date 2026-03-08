"""Tests for rc_state.py -- CFG-aware RC balance checking."""

import sys
import textwrap
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from ir_parser import parse_module
from rc_state import (
    RcState,
    ValueState,
    analyze_function_rc,
    build_cfg,
    build_predecessor_map,
    compute_rpo,
    merge_value_states,
)


class TestValueState:
    def test_initial_state(self):
        vs = ValueState()
        assert vs.state == RcState.UNKNOWN
        assert vs.balanced

    def test_alloc_then_dec(self):
        vs = ValueState()
        vs.apply_alloc()
        assert vs.state == RcState.LIVE
        assert vs.inc_count == 1
        vs.apply_dec()
        assert vs.inc_count == 1
        assert vs.dec_count == 1
        assert vs.balanced

    def test_inc_then_dec(self):
        vs = ValueState()
        vs.apply_inc()
        assert vs.state == RcState.INCREMENTED
        vs.apply_dec()
        assert vs.state == RcState.LIVE
        assert vs.balanced


class TestMerge:
    def test_single_state(self):
        vs = ValueState(state=RcState.LIVE, inc_count=1)
        merged = merge_value_states([vs])
        assert merged.state == RcState.LIVE
        assert merged.inc_count == 1

    def test_divergent_dec_counts_are_conditional(self):
        a = ValueState(state=RcState.LIVE, inc_count=1, dec_count=1)
        b = ValueState(state=RcState.LIVE, inc_count=1, dec_count=0)
        merged = merge_value_states([a, b])
        assert merged.is_conditional

    def test_same_counts_not_conditional(self):
        a = ValueState(state=RcState.LIVE, inc_count=1, dec_count=1)
        b = ValueState(state=RcState.LIVE, inc_count=1, dec_count=1)
        merged = merge_value_states([a, b])
        assert not merged.is_conditional


class TestCfgConstruction:
    def test_simple_branch(self):
        ir = textwrap.dedent("""\
            define fastcc void @_ori_f(i64 %0) #0 {
            entry:
              br i1 %0, label %then, label %else

            then:
              br label %merge

            else:
              br label %merge

            merge:
              ret void
            }
            attributes #0 = { nounwind }
        """)
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        cfg = build_cfg(f)
        assert set(cfg["entry"]) == {"then", "else"}
        assert cfg["then"] == ["merge"]
        assert cfg["else"] == ["merge"]
        assert cfg["merge"] == []

    def test_predecessor_map(self):
        ir = textwrap.dedent("""\
            define fastcc void @_ori_f(i64 %0) #0 {
            entry:
              br i1 %0, label %then, label %else

            then:
              br label %merge

            else:
              br label %merge

            merge:
              ret void
            }
            attributes #0 = { nounwind }
        """)
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        cfg = build_cfg(f)
        preds = build_predecessor_map(cfg)
        assert set(preds["merge"]) == {"then", "else"}
        assert preds["entry"] == []

    def test_rpo_ordering(self):
        ir = textwrap.dedent("""\
            define fastcc void @_ori_f(i64 %0) #0 {
            entry:
              br label %body

            body:
              br label %exit

            exit:
              ret void
            }
            attributes #0 = { nounwind }
        """)
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        cfg = build_cfg(f)
        rpo = compute_rpo(cfg, "entry")
        assert rpo == ["entry", "body", "exit"]


class TestFunctionAnalysis:
    def test_balanced_straight_line(self):
        ir = textwrap.dedent("""\
            define fastcc void @_ori_f(ptr %0) #0 {
            entry:
              call void @ori_rc_inc(ptr %0)
              call void @ori_rc_dec(ptr %0, ptr null)
              ret void
            }
            declare void @ori_rc_inc(ptr)
            declare void @ori_rc_dec(ptr, ptr)
            attributes #0 = { nounwind }
        """)
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        result = analyze_function_rc(f)
        assert result.balanced
        assert result.conditional_ops == 0

    def test_conditional_dec_detected(self):
        """SSO gate pattern: dec only on one branch."""
        ir = textwrap.dedent("""\
            define fastcc void @_ori_f(ptr %data) #0 {
            entry:
              %is_heap = icmp ne ptr %data, null
              br i1 %is_heap, label %heap, label %sso

            heap:
              call void @ori_rc_dec(ptr %data, ptr null)
              br label %merge

            sso:
              br label %merge

            merge:
              ret void
            }
            declare void @ori_rc_dec(ptr, ptr)
            attributes #0 = { nounwind }
        """)
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        result = analyze_function_rc(f)
        # Dec is conditional (only on heap branch)
        assert result.conditional_ops > 0

    def test_no_rc_ops(self):
        ir = textwrap.dedent("""\
            define fastcc i64 @_ori_f(i64 %0) #0 {
            entry:
              ret i64 %0
            }
            attributes #0 = { nounwind }
        """)
        m = parse_module(ir)
        f = m.functions["@_ori_f"]
        result = analyze_function_rc(f)
        assert result.balanced
        assert result.total_inc == 0
        assert result.total_dec == 0
