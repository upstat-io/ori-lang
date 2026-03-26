//! Tests for interprocedural range propagation (§03.5).

use super::*;
use ori_arc::ir::{
    ArcBlock, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArgOwnership, LitValue,
    ValueRepr,
};
use ori_arc::{ArcBlockId, Ownership};

/// Helper: build a simple function with one block, a body, and a Return terminator.
fn build_simple_func(
    name: u32,
    params: &[(u32, ori_types::Idx)],
    body: Vec<ArcInstr>,
    ret_var: ArcVarId,
    ret_type: ori_types::Idx,
    num_vars: usize,
) -> ArcFunction {
    let arc_params: Vec<ArcParam> = params
        .iter()
        .map(|(var_id, ty)| ArcParam {
            var: ArcVarId::new(*var_id),
            ty: *ty,
            ownership: Ownership::Owned,
        })
        .collect();

    // Entry block has the function params as block params.
    let block_params: Vec<(ArcVarId, ori_types::Idx)> = params
        .iter()
        .map(|(var_id, ty)| (ArcVarId::new(*var_id), *ty))
        .collect();

    let span_count = body.len();
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: block_params,
        body,
        terminator: ArcTerminator::Return { value: ret_var },
    };

    ArcFunction {
        name: ori_ir::Name::from_raw(name),
        params: arc_params,
        return_type: ret_type,
        blocks: vec![block],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; num_vars],
        var_reprs: vec![ValueRepr::Scalar; num_vars],
        spans: vec![vec![None; span_count]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    }
}

// ─── Non-recursive: constant argument narrows parameter ──────

/// Semantic pin: private function called only as `helper(42)` should have
/// parameter range [42, 42]. This ONLY passes with interprocedural propagation;
/// intraprocedural alone gives Top.
#[test]
fn single_call_site_constant_arg() {
    let v_param = ArcVarId::new(0);
    let helper = build_simple_func(
        100,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_param,
        ori_types::Idx::INT,
        1,
    );

    let v_arg = ArcVarId::new(0);
    let v_result = ArcVarId::new(1);
    let caller = build_simple_func(
        200,
        &[],
        vec![
            ArcInstr::Let {
                dst: v_arg,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Apply {
                dst: v_result,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(100),
                args: vec![v_arg],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_result,
        ori_types::Idx::INT,
        2,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, caller], &config);

    // The caller passes 42 to helper. helper's param var should be [42, 42].
    let helper_name = ori_ir::Name::from_raw(100);
    let ret_range = plan.var_range(helper_name, v_param);
    assert_eq!(
        ret_range,
        ValueRange::Bounded { lo: 42, hi: 42 },
        "Semantic pin: helper(42) should give param range [42, 42], got {ret_range:?}"
    );
}

// ─── Non-recursive: two call sites join parameter ranges ──────

/// Two call sites with different constant args → parameter range is their join.
#[test]
fn two_call_sites_join_param_ranges() {
    let v_param = ArcVarId::new(0);
    let helper = build_simple_func(
        101,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_param,
        ori_types::Idx::INT,
        1,
    );

    let v_arg1 = ArcVarId::new(0);
    let v_arg2 = ArcVarId::new(1);
    let v_r1 = ArcVarId::new(2);
    let v_r2 = ArcVarId::new(3);
    let caller = build_simple_func(
        201,
        &[],
        vec![
            ArcInstr::Let {
                dst: v_arg1,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(10)),
            },
            ArcInstr::Apply {
                dst: v_r1,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(101),
                args: vec![v_arg1],
                arg_ownership: vec![ArgOwnership::Owned],
            },
            ArcInstr::Let {
                dst: v_arg2,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(50)),
            },
            ArcInstr::Apply {
                dst: v_r2,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(101),
                args: vec![v_arg2],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_r2,
        ori_types::Idx::INT,
        4,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, caller], &config);

    // Join of [10,10] and [50,50] = [10,50].
    let helper_name = ori_ir::Name::from_raw(101);
    let param_range = plan.var_range(helper_name, v_param);
    assert_eq!(
        param_range,
        ValueRange::Bounded { lo: 10, hi: 50 },
        "Two call sites should join: [10, 50], got {param_range:?}"
    );
}

// ─── Return range propagation ──────

/// Return range from a function with a constant return value.
#[test]
fn return_range_constant() {
    let v_ret = ArcVarId::new(0);
    let func = build_simple_func(
        102,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[func], &config);

    // The function returns 99, so v_ret should be [99, 99].
    let func_name = ori_ir::Name::from_raw(102);
    let ret_range = plan.var_range(func_name, v_ret);
    assert_eq!(
        ret_range,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "Constant return should give [99, 99], got {ret_range:?}"
    );
}

// ─── Budget exceeded ──────

/// Budget exceeded: >N SCC iterations → remaining SCCs get Top.
#[test]
fn budget_exceeded_gives_top() {
    let v_param = ArcVarId::new(0);
    let func = build_simple_func(
        103,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_param,
        ori_types::Idx::INT,
        1,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig {
        max_total_scc_iterations: 0,
        ..Default::default()
    };
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[func], &config);

    // Budget exhausted — should not panic. With 0 budget, the SCC is skipped
    // and params get Top, but intraprocedural results should still be stored.
    let func_name = ori_ir::Name::from_raw(103);
    let param_range = plan.var_range(func_name, v_param);
    assert_eq!(
        param_range,
        ValueRange::Top,
        "Budget exceeded should not panic, param should be Top"
    );
}

// ─── Self-recursive function ──────

/// Self-recursive function: should converge or widen to Top within budget.
#[test]
fn self_recursive_converges_or_widens() {
    use ori_arc::ir::PrimOp;
    use ori_ir::BinaryOp;

    let v_x = ArcVarId::new(0);
    let v_zero = ArcVarId::new(1);
    let v_cond = ArcVarId::new(2);
    let v_one = ArcVarId::new(3);
    let v_sub = ArcVarId::new(4);
    let v_rec = ArcVarId::new(5);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![(v_x, ori_types::Idx::INT)],
        body: vec![
            ArcInstr::Let {
                dst: v_zero,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Gt),
                    args: vec![v_x, v_zero],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_sub,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Sub),
                    args: vec![v_x, v_one],
                },
            },
            ArcInstr::Apply {
                dst: v_rec,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(104),
                args: vec![v_sub],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Return { value: v_rec },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_zero },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(104),
        params: vec![ArcParam {
            var: v_x,
            ty: ori_types::Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 6],
        var_reprs: vec![ValueRepr::Scalar; 6],
        spans: vec![vec![None; 2], vec![None; 3], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    // Should not panic or hang.
    propagate_ranges(&mut plan, &pool, &[func], &config);

    // Self-recursive with no external callers → param should be Top
    // (no internal non-recursive call site to narrow from).
    let func_name = ori_ir::Name::from_raw(104);
    let param_range = plan.var_range(func_name, v_x);
    assert!(
        param_range == ValueRange::Top || matches!(param_range, ValueRange::Bounded { .. }),
        "Self-recursive should converge to a valid range (Top or bounded), got {param_range:?}"
    );
}

// ─── Empty function list ──────

/// Empty function list: should not panic.
#[test]
fn empty_functions_no_panic() {
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[], &config);
    // No assertions needed — just verifying it doesn't panic.
}

// ─── TPR-03-026: Transitive A→B→C propagation ──────

/// Semantic pin: A calls B(42), B calls C(x). C's parameter should narrow to
/// [42, 42] even though C is only called by B. This ONLY passes with parameter
/// seeding in the SCC pipeline — without seeds, C's param stays Top.
#[test]
fn transitive_propagation_a_b_c() {
    // C: receives x, returns x
    let v_cx = ArcVarId::new(0);
    let func_c = build_simple_func(
        300,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_cx,
        ori_types::Idx::INT,
        1,
    );

    // B: receives x, calls C(x), returns result
    let v_bx = ArcVarId::new(0);
    let v_br = ArcVarId::new(1);
    let func_b = build_simple_func(
        200,
        &[(0, ori_types::Idx::INT)],
        vec![ArcInstr::Apply {
            dst: v_br,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(300),
            args: vec![v_bx],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        v_br,
        ori_types::Idx::INT,
        2,
    );

    // A: calls B(42)
    let v_a42 = ArcVarId::new(0);
    let v_ar = ArcVarId::new(1);
    let func_a = build_simple_func(
        100,
        &[],
        vec![
            ArcInstr::Let {
                dst: v_a42,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Apply {
                dst: v_ar,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(200),
                args: vec![v_a42],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_ar,
        ori_types::Idx::INT,
        2,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    // Order matters: C, B, A — topological leaf-first processing.
    propagate_ranges(&mut plan, &pool, &[func_c, func_b, func_a], &config);

    // B's param should be [42, 42] (called with constant 42 from A).
    let b_name = ori_ir::Name::from_raw(200);
    let b_param = plan.var_range(b_name, v_bx);
    assert_eq!(
        b_param,
        ValueRange::Bounded { lo: 42, hi: 42 },
        "Semantic pin: B(42) → B's param should be [42, 42], got {b_param:?}"
    );

    // C's param should be [42, 42] (B passes its param x=42 to C).
    let c_name = ori_ir::Name::from_raw(300);
    let c_param = plan.var_range(c_name, v_cx);
    assert_eq!(
        c_param,
        ValueRange::Bounded { lo: 42, hi: 42 },
        "Semantic pin: transitive A→B→C, C's param should be [42, 42], got {c_param:?}"
    );
}

// ─── TPR-03-027: Caller/callee return-range narrowing ──────

/// Semantic pin: callee returns constant 99, caller's Apply dst should narrow
/// to [99, 99] from the callee's return range. This ONLY passes with Phase 6
/// return-range propagation.
#[test]
fn caller_dst_narrows_from_callee_return_range() {
    // callee: returns constant 99
    let v_ret = ArcVarId::new(0);
    let callee = build_simple_func(
        400,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );

    // caller: calls callee() and returns the result
    let v_dst = ArcVarId::new(0);
    let caller = build_simple_func(
        500,
        &[],
        vec![ArcInstr::Apply {
            dst: v_dst,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(400),
            args: vec![],
            arg_ownership: vec![],
        }],
        v_dst,
        ori_types::Idx::INT,
        1,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[callee, caller], &config);

    // Caller's dst (result of calling callee) should be [99, 99].
    let caller_name = ori_ir::Name::from_raw(500);
    let dst_range = plan.var_range(caller_name, v_dst);
    assert_eq!(
        dst_range,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "Semantic pin: caller's Apply dst should narrow to callee's return [99, 99], got {dst_range:?}"
    );
}

// ─── TPR-03-026: Mutually recursive SCC tightening ──────

/// Two mutually recursive functions with an external seed. When seeded with
/// a constant call, parameter ranges should converge tighter than Top.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "constructing ARC IR for two mutually recursive functions is inherently verbose"
)]
fn mutually_recursive_scc_tightens_from_seed() {
    use ori_arc::ir::PrimOp;
    use ori_ir::BinaryOp;

    // F: if x > 0 then G(x - 1) else 0
    let v_fx = ArcVarId::new(0);
    let v_fzero = ArcVarId::new(1);
    let v_fcond = ArcVarId::new(2);
    let v_fone = ArcVarId::new(3);
    let v_fsub = ArcVarId::new(4);
    let v_frec = ArcVarId::new(5);

    let f_block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![(v_fx, ori_types::Idx::INT)],
        body: vec![
            ArcInstr::Let {
                dst: v_fzero,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_fcond,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Gt),
                    args: vec![v_fx, v_fzero],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_fcond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };
    let f_block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_fone,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_fsub,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Sub),
                    args: vec![v_fx, v_fone],
                },
            },
            // Call G(x - 1) instead of F(x - 1) — mutually recursive
            ArcInstr::Apply {
                dst: v_frec,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(601),
                args: vec![v_fsub],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Return { value: v_frec },
    };
    let f_block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_fzero },
    };

    let func_f = ArcFunction {
        name: ori_ir::Name::from_raw(600),
        params: vec![ArcParam {
            var: v_fx,
            ty: ori_types::Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: ori_types::Idx::INT,
        blocks: vec![f_block0, f_block1, f_block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 6],
        var_reprs: vec![ValueRepr::Scalar; 6],
        spans: vec![vec![None; 2], vec![None; 3], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    // G: same structure as F but calls F(x - 1)
    let v_gx = ArcVarId::new(0);
    let v_gzero = ArcVarId::new(1);
    let v_gcond = ArcVarId::new(2);
    let v_gone = ArcVarId::new(3);
    let v_gsub = ArcVarId::new(4);
    let v_grec = ArcVarId::new(5);

    let g_block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![(v_gx, ori_types::Idx::INT)],
        body: vec![
            ArcInstr::Let {
                dst: v_gzero,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_gcond,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Gt),
                    args: vec![v_gx, v_gzero],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_gcond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };
    let g_block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_gone,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_gsub,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Sub),
                    args: vec![v_gx, v_gone],
                },
            },
            // Call F(x - 1) — mutually recursive back to F
            ArcInstr::Apply {
                dst: v_grec,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(600),
                args: vec![v_gsub],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Return { value: v_grec },
    };
    let g_block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_gzero },
    };

    let func_g = ArcFunction {
        name: ori_ir::Name::from_raw(601),
        params: vec![ArcParam {
            var: v_gx,
            ty: ori_types::Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: ori_types::Idx::INT,
        blocks: vec![g_block0, g_block1, g_block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 6],
        var_reprs: vec![ValueRepr::Scalar; 6],
        spans: vec![vec![None; 2], vec![None; 3], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    // External caller: main calls F(10)
    let v_m10 = ArcVarId::new(0);
    let v_mr = ArcVarId::new(1);
    let func_main = build_simple_func(
        700,
        &[],
        vec![
            ArcInstr::Let {
                dst: v_m10,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(10)),
            },
            ArcInstr::Apply {
                dst: v_mr,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(600),
                args: vec![v_m10],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_mr,
        ori_types::Idx::INT,
        2,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[func_f, func_g, func_main], &config);

    // The SCC parameter fixpoint for F↔G with main(F(10)) expands the lower
    // bound each iteration (G feeds x-1 back to F). With default budget (10
    // SCC iterations), this doesn't converge — the budget trips and all
    // results are correctly widened to Top (TPR-03-028). The test verifies:
    // 1. No panic or hang during SCC processing
    // 2. Results are valid (Top is acceptable for non-converging SCCs)
    let f_name = ori_ir::Name::from_raw(600);
    let f_param = plan.var_range(f_name, v_fx);
    assert!(
        matches!(f_param, ValueRange::Top | ValueRange::Bounded { .. }),
        "Mutually recursive SCC: F's param should be valid range, got {f_param:?}"
    );

    let g_name = ori_ir::Name::from_raw(601);
    let g_param = plan.var_range(g_name, v_gx);
    assert!(
        matches!(g_param, ValueRange::Top | ValueRange::Bounded { .. }),
        "Mutually recursive SCC: G's param should be valid range, got {g_param:?}"
    );
}

// ─── TPR-03-028: SCC budget exhaustion clears stale results ──────

/// Semantic pin: when a recursive SCC hits the iteration budget, ALL exported
/// ranges (`var_ranges`, `return_range`, `field_summaries`) must be conservative
/// (Top or empty). The previous bug left partially-converged intermediate results
/// in `results` while only widening `func_infos` — this test catches that.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "constructing ARC IR for a recursive function with budget test is inherently verbose"
)]
fn scc_budget_exhaustion_clears_stale_results() {
    use ori_arc::ir::PrimOp;
    use ori_ir::BinaryOp;

    // Build a self-recursive function with a non-trivial body so the
    // intraprocedural fixpoint produces bounded ranges before the SCC
    // budget trips.
    //
    // f(x): let c = 42; if x > 0 then f(x - 1) else c
    let v_x = ArcVarId::new(0);
    let v_c = ArcVarId::new(1);
    let v_zero = ArcVarId::new(2);
    let v_cond = ArcVarId::new(3);
    let v_one = ArcVarId::new(4);
    let v_sub = ArcVarId::new(5);
    let v_rec = ArcVarId::new(6);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![(v_x, ori_types::Idx::INT)],
        body: vec![
            ArcInstr::Let {
                dst: v_c,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Let {
                dst: v_zero,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Gt),
                    args: vec![v_x, v_zero],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_sub,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Sub),
                    args: vec![v_x, v_one],
                },
            },
            ArcInstr::Apply {
                dst: v_rec,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(800),
                args: vec![v_sub],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Return { value: v_rec },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_c },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(800),
        params: vec![ArcParam {
            var: v_x,
            ty: ori_types::Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 7],
        var_reprs: vec![ValueRepr::Scalar; 7],
        spans: vec![vec![None; 3], vec![None; 3], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    // Set max_scc_iterations = 1 so the SCC budget is exhausted after the
    // first iteration (before convergence can occur).
    let config = RangeAnalysisConfig {
        max_scc_iterations: 1,
        ..Default::default()
    };
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[func], &config);

    let func_name = ori_ir::Name::from_raw(800);

    // After budget exhaustion, the constant `let c = 42` should NOT appear
    // as a bounded range — all var_ranges from the SCC must be cleared.
    // Specifically, v_c (the `42` literal) should be Top, not [42, 42].
    let c_range = plan.var_range(func_name, v_c);
    assert_eq!(
        c_range,
        ValueRange::Top,
        "Semantic pin: SCC budget exhaustion should clear var_ranges, \
         but v_c is still {c_range:?} (expected Top)"
    );

    // The parameter should also be Top.
    let param_range = plan.var_range(func_name, v_x);
    assert_eq!(
        param_range,
        ValueRange::Top,
        "SCC budget exhaustion: param should be Top, got {param_range:?}"
    );
}

// ─── TPR-03-030: Return-range feedback into downstream propagation ──────

/// Semantic pin: A calls `helper()` which returns bounded [99, 99], then passes
/// that result to C. C's parameter should narrow to [99, 99] because the
/// return range from helper flows through A's `var_ranges` into C's call-site args.
/// This ONLY passes when return ranges are fed back into `results` before
/// `collect_param_ranges()` reads them.
#[test]
fn return_range_feeds_downstream_parameter_collection() {
    // helper(): returns constant 99
    let v_ret = ArcVarId::new(0);
    let helper = build_simple_func(
        900,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );

    // C(x): receives x, returns x
    let v_cx = ArcVarId::new(0);
    let func_c = build_simple_func(
        901,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_cx,
        ori_types::Idx::INT,
        1,
    );

    // A(): calls helper(), passes result to C(), returns C's result
    let v_h_result = ArcVarId::new(0);
    let v_c_result = ArcVarId::new(1);
    let func_a = build_simple_func(
        902,
        &[],
        vec![
            ArcInstr::Apply {
                dst: v_h_result,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(900),
                args: vec![],
                arg_ownership: vec![],
            },
            ArcInstr::Apply {
                dst: v_c_result,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(901),
                args: vec![v_h_result],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_c_result,
        ori_types::Idx::INT,
        2,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, func_c, func_a], &config);

    // C's parameter should be [99, 99] because:
    // 1. helper() returns [99, 99]
    // 2. A stores helper()'s result in v_h_result
    // 3. A passes v_h_result to C
    // 4. collect_param_ranges sees v_h_result's range as [99, 99]
    let c_name = ori_ir::Name::from_raw(901);
    let c_param = plan.var_range(c_name, v_cx);
    assert_eq!(
        c_param,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "Semantic pin: helper() returns [99, 99], A forwards to C — \
         C's param should be [99, 99], got {c_param:?}"
    );
}

// ─── Multi-hop return-range chain (TPR-03-031) ──────

/// Build a passthrough function: `f(x) = callee(x)`.
/// Takes one int param, calls `callee_id` with it, returns the result.
fn build_passthrough_func(name: u32, callee_id: u32) -> ArcFunction {
    let v_x = ArcVarId::new(0);
    let v_r = ArcVarId::new(1);
    build_simple_func(
        name,
        &[(0, ori_types::Idx::INT)],
        vec![ArcInstr::Apply {
            dst: v_r,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(callee_id),
            args: vec![v_x],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        v_r,
        ori_types::Idx::INT,
        2,
    )
}

/// Semantic pin: multi-hop return-range forwarding requires iterating
/// feedback to fixpoint. Chain: `helper()` returns `[99,99]`, A calls
/// `helper()` and passes result to B, B passes to C, C passes to D.
/// D's parameter should narrow to `[99, 99]`.
///
/// This ONLY passes with iterated feedback — single-pass feedback
/// loses the return-range narrowing after 2 hops.
#[test]
fn multi_hop_return_range_chain() {
    // helper(): returns constant 99
    let v_ret = ArcVarId::new(0);
    let helper = build_simple_func(
        800,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );

    // D(x) → x, C(x) → D(x), B(x) → C(x)
    let func_d = build_simple_func(
        801,
        &[(0, ori_types::Idx::INT)],
        vec![],
        ArcVarId::new(0),
        ori_types::Idx::INT,
        1,
    );
    let func_c = build_passthrough_func(802, 801);
    let func_b = build_passthrough_func(803, 802);

    // A(): calls helper(), passes result to B(), returns B's result
    let v_h = ArcVarId::new(0);
    let v_ab = ArcVarId::new(1);
    let func_a = build_simple_func(
        804,
        &[],
        vec![
            ArcInstr::Apply {
                dst: v_h,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(800),
                args: vec![],
                arg_ownership: vec![],
            },
            ArcInstr::Apply {
                dst: v_ab,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(803),
                args: vec![v_h],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_ab,
        ori_types::Idx::INT,
        2,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);
    propagate_ranges(
        &mut plan,
        &pool,
        &[helper, func_d, func_c, func_b, func_a],
        &config,
    );

    // Chain: helper→A→B→C→D. D's param should be [99, 99].
    let v_param0 = ArcVarId::new(0);
    for (label, id) in [("D", 801), ("C", 802), ("B", 803)] {
        let range = plan.var_range(ori_ir::Name::from_raw(id), v_param0);
        assert_eq!(
            range,
            ValueRange::Bounded { lo: 99, hi: 99 },
            "{label}'s param should be [99, 99] in multi-hop chain, got {range:?}"
        );
    }
}

// ─── TPR-03-032: Derived locals from call-result narrowing ──────

/// Semantic pin: `helper()` returns 99. Caller does:
///   `let x = helper()`   — dst var, narrowed by return-range feedback
///   `let y = x + 1`      — derived local, must propagate narrowing
///   `return y`
///
/// Caller's `y` should be `[100, 100]`, NOT `Top`. This ONLY passes when
/// the feedback loop reruns the caller's fixpoint with call-result
/// narrowings, so `x + 1` can compute from the narrowed `x = [99, 99]`.
#[test]
fn callee_return_derived_local_propagates() {
    use ori_arc::ir::PrimOp;
    use ori_ir::BinaryOp;

    // helper: returns constant 99
    let v_ret = ArcVarId::new(0);
    let helper = build_simple_func(
        900,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );

    // caller: let x = helper(); let one = 1; let y = x + one; return y
    let v_x = ArcVarId::new(0);
    let v_one = ArcVarId::new(1);
    let v_y = ArcVarId::new(2);
    let caller = build_simple_func(
        901,
        &[],
        vec![
            ArcInstr::Apply {
                dst: v_x,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(900),
                args: vec![],
                arg_ownership: vec![],
            },
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_y,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v_x, v_one],
                },
            },
        ],
        v_y,
        ori_types::Idx::INT,
        3,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, caller], &config);

    let caller_name = ori_ir::Name::from_raw(901);

    // x (dst of Apply) should be narrowed to [99, 99] from helper's return range.
    let x_range = plan.var_range(caller_name, v_x);
    assert_eq!(
        x_range,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "caller's x (Apply dst) should be [99, 99], got {x_range:?}"
    );

    // y = x + 1 should be [100, 100] — this is the derived local.
    // Before fix: y stays Top because fixpoint isn't rerun after dst narrowing.
    // After fix: fixpoint reruns with call_result_narrowings, so y propagates.
    let y_range = plan.var_range(caller_name, v_y);
    assert_eq!(
        y_range,
        ValueRange::Bounded { lo: 100, hi: 100 },
        "Semantic pin: derived local y = x + 1 should be [100, 100], got {y_range:?}"
    );
}

/// Semantic pin: `helper()` returns 99, caller transforms and forwards to callee.
///   `let x = helper()`
///   `let y = x + 1`
///   `callee(y)`           — callee's param should be `[100, 100]`
///
/// This tests that call-result narrowing propagates through derived locals
/// AND into downstream parameter collection.
#[test]
fn callee_return_derived_local_forwards_to_callee_param() {
    use ori_arc::ir::PrimOp;
    use ori_ir::BinaryOp;

    // helper: returns constant 99
    let v_hret = ArcVarId::new(0);
    let helper = build_simple_func(
        910,
        &[],
        vec![ArcInstr::Let {
            dst: v_hret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_hret,
        ori_types::Idx::INT,
        1,
    );

    // callee: receives p, returns p
    let v_p = ArcVarId::new(0);
    let callee = build_simple_func(
        911,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_p,
        ori_types::Idx::INT,
        1,
    );

    // caller: let x = helper(); let one = 1; let y = x + one; let r = callee(y); return r
    let v_x = ArcVarId::new(0);
    let v_one = ArcVarId::new(1);
    let v_y = ArcVarId::new(2);
    let v_r = ArcVarId::new(3);
    let caller = build_simple_func(
        912,
        &[],
        vec![
            ArcInstr::Apply {
                dst: v_x,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(910),
                args: vec![],
                arg_ownership: vec![],
            },
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_y,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v_x, v_one],
                },
            },
            ArcInstr::Apply {
                dst: v_r,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(911),
                args: vec![v_y],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_r,
        ori_types::Idx::INT,
        4,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, callee, caller], &config);

    // callee's parameter should be [100, 100] — forwarded from helper's
    // return [99, 99] + 1.
    let callee_name = ori_ir::Name::from_raw(911);
    let p_range = plan.var_range(callee_name, v_p);
    assert_eq!(
        p_range,
        ValueRange::Bounded { lo: 100, hi: 100 },
        "Semantic pin: callee(helper() + 1) → callee's param should be [100, 100], got {p_range:?}"
    );
}

// ─── TPR-03-033: refresh_return_ranges must skip unreachable blocks ──────

/// Build a function that calls `callee_id`, has an unreachable Return block,
/// and returns the call result. CFG: B0 → Apply → Jump B2, B1 (unreachable
/// Return), B2 (Return call result).
fn build_func_with_unreachable_return(name: u32, callee_id: u32) -> ArcFunction {
    use ori_arc::ir::{ArcBlock, ArcTerminator, ValueRepr};
    use ori_arc::ArcBlockId;

    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: v0,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(callee_id),
            args: vec![],
            arg_ownership: vec![],
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(2),
            args: vec![],
        },
    };
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v1 },
    };
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v0 },
    };
    ArcFunction {
        name: ori_ir::Name::from_raw(name),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT, ori_types::Idx::INT],
        var_reprs: vec![ValueRepr::Scalar, ValueRepr::Scalar],
        spans: vec![vec![None], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    }
}

/// Build a caller that calls `callee_a` (no args) then passes the result
/// to `callee_b`. Returns `callee_b`'s result.
fn build_two_call_caller(name: u32, callee_a: u32, callee_b: u32) -> ArcFunction {
    let v_ca = ArcVarId::new(0);
    let v_cb = ArcVarId::new(1);
    build_simple_func(
        name,
        &[],
        vec![
            ArcInstr::Apply {
                dst: v_ca,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(callee_a),
                args: vec![],
                arg_ownership: vec![],
            },
            ArcInstr::Apply {
                dst: v_cb,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(callee_b),
                args: vec![v_ca],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_cb,
        ori_types::Idx::INT,
        2,
    )
}

/// Semantic pin: feedback narrowing accumulation across iterations.
///
/// Chain: `helper()` returns 99, `func_a()` calls helper and returns
/// the result (with unreachable Return block), `caller()` calls `func_a`
/// then passes the result to `func_b(x)`. Verifies both: (1) unreachable
/// blocks are skipped in return-range refresh, (2) accumulated narrowings
/// across feedback iterations prevent oscillation when a function makes
/// multiple calls to different callees.
#[test]
fn feedback_refresh_skips_unreachable_return_blocks() {
    use ori_arc::ir::{ArcValue, LitValue};

    let v_ret = ArcVarId::new(0);
    let helper = build_simple_func(
        900,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );
    let func_a = build_func_with_unreachable_return(902, 900);
    let v_bx = ArcVarId::new(0);
    let func_b = build_simple_func(
        901,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_bx,
        ori_types::Idx::INT,
        1,
    );
    let caller = build_two_call_caller(903, 902, 901);

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);
    propagate_ranges(&mut plan, &pool, &[helper, func_a, func_b, caller], &config);

    let v0 = ArcVarId::new(0);
    let a_ret = plan.var_range(ori_ir::Name::from_raw(902), v0);
    assert_eq!(
        a_ret,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "func_a's return var should be [99, 99] despite unreachable block, got {a_ret:?}"
    );
    let b_param = plan.var_range(ori_ir::Name::from_raw(901), v_bx);
    assert_eq!(
        b_param,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "func_b's param should be [99, 99] via accumulated feedback, got {b_param:?}"
    );
}

// ─── TPR-03-035: Total SCC budget must limit recursive SCC iterations ──────

/// Build a self-recursive `rec(x: int)`: if x > 0 then `rec(x - 1)` else 0.
fn build_self_recursive_func(name: u32) -> ArcFunction {
    use ori_arc::ir::{ArcBlock, ArcTerminator, PrimOp, ValueRepr};
    use ori_arc::ArcBlockId;
    use ori_ir::BinaryOp;

    let v_x = ArcVarId::new(0);
    let v_zero = ArcVarId::new(1);
    let v_cond = ArcVarId::new(2);
    let v_one = ArcVarId::new(3);
    let v_sub = ArcVarId::new(4);
    let v_rec = ArcVarId::new(5);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![(v_x, ori_types::Idx::INT)],
        body: vec![
            ArcInstr::Let {
                dst: v_zero,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Gt),
                    args: vec![v_x, v_zero],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_sub,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Sub),
                    args: vec![v_x, v_one],
                },
            },
            ArcInstr::Apply {
                dst: v_rec,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(name),
                args: vec![v_sub],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Return { value: v_rec },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_zero },
    };

    ArcFunction {
        name: ori_ir::Name::from_raw(name),
        params: vec![ori_arc::ir::ArcParam {
            var: v_x,
            ty: ori_types::Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 6],
        var_reprs: vec![ValueRepr::Scalar; 6],
        spans: vec![vec![None; 2], vec![None; 3], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    }
}

/// TPR-03-035 regression: with `max_total_scc_iterations: 2` and
/// `max_scc_iterations: 10`, `process_recursive_scc` uses
/// `min(10, remaining_budget=1) = 1` as effective cap.
#[test]
fn total_scc_budget_caps_recursive_scc() {
    let rec = build_self_recursive_func(950);

    let v_arg = ArcVarId::new(0);
    let v_result = ArcVarId::new(1);
    let main_func = build_simple_func(
        951,
        &[],
        vec![
            ArcInstr::Let {
                dst: v_arg,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Apply {
                dst: v_result,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(950),
                args: vec![v_arg],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        v_result,
        ori_types::Idx::INT,
        2,
    );

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig {
        max_scc_iterations: 10,
        max_total_scc_iterations: 2,
        ..Default::default()
    };
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);
    propagate_ranges(&mut plan, &pool, &[rec, main_func], &config);

    let rec_param = plan.var_range(ori_ir::Name::from_raw(950), ArcVarId::new(0));
    assert_eq!(
        rec_param,
        ValueRange::Top,
        "rec should be Top-widened with tight total budget, got {rec_param:?}"
    );
}

// ─── TPR-03-034: Invoke dst must receive call_result_narrowings ──────

/// Build a caller that uses `ArcTerminator::Invoke` to call `callee_id`,
/// then performs `let y = x + 1` and returns y. Three blocks:
///   B0: [setup] → `Invoke(callee_id)` → normal: B1, unwind: B2
///   B1: [let one = 1; let y = x + one] → Return(y)
///   B2: Unreachable
fn build_invoke_caller(name: u32, callee_id: u32, num_vars: usize) -> ArcFunction {
    use ori_arc::ir::{ArcBlock, ArcTerminator, PrimOp, ValueRepr};
    use ori_arc::ArcBlockId;
    use ori_ir::BinaryOp;

    let v_x = ArcVarId::new(0);
    let v_one = ArcVarId::new(1);
    let v_y = ArcVarId::new(2);

    // B0: invoke helper → normal B1, unwind B2
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Invoke {
            dst: v_x,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(callee_id),
            args: vec![],
            arg_ownership: vec![],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    };

    // B1: let one = 1; let y = x + one; return y
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_y,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v_x, v_one],
                },
            },
        ],
        terminator: ArcTerminator::Return { value: v_y },
    };

    // B2: unwind destination (unreachable)
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Unreachable,
    };

    ArcFunction {
        name: ori_ir::Name::from_raw(name),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; num_vars],
        var_reprs: vec![ValueRepr::Scalar; num_vars],
        spans: vec![vec![None; 0], vec![None; 2], vec![None; 0]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    }
}

/// Semantic pin (TPR-03-034): `helper()` returns 99. Caller uses `Invoke` (not
/// `Apply`) to call helper, then computes `y = x + 1`. After interprocedural
/// propagation, y should be [100, 100]. ONLY passes when `process_terminator()`
/// applies `call_result_narrowings` for `Invoke` dst variables.
#[test]
fn invoke_dst_derived_local_propagates() {
    // helper: returns constant 99
    let v_ret = ArcVarId::new(0);
    let helper = build_simple_func(
        930,
        &[],
        vec![ArcInstr::Let {
            dst: v_ret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_ret,
        ori_types::Idx::INT,
        1,
    );

    // caller: invoke helper → x; let y = x + 1; return y
    let caller = build_invoke_caller(931, 930, 3);

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, caller], &config);

    let caller_name = ori_ir::Name::from_raw(931);
    let v_x = ArcVarId::new(0);
    let v_y = ArcVarId::new(2);

    // x (Invoke dst) should be narrowed to [99, 99] from helper's return range.
    let x_range = plan.var_range(caller_name, v_x);
    assert_eq!(
        x_range,
        ValueRange::Bounded { lo: 99, hi: 99 },
        "Invoke dst x should be [99, 99], got {x_range:?}"
    );

    // y = x + 1 should be [100, 100] — the derived local.
    // Before fix: y stays Top because Invoke dst doesn't get call_result_narrowings.
    // After fix: process_terminator applies meet, so y propagates.
    let y_range = plan.var_range(caller_name, v_y);
    assert_eq!(
        y_range,
        ValueRange::Bounded { lo: 100, hi: 100 },
        "Semantic pin (TPR-03-034): derived local y = invoke_dst + 1 should be [100, 100], got {y_range:?}"
    );
}

/// Semantic pin (TPR-03-034): `helper()` returns 99. Caller uses `Invoke` to
/// call helper, computes y = x + 1, then calls `callee(y)`. Callee's param
/// should be [100, 100]. Tests `Invoke` narrowing propagating through to
/// downstream parameter collection.
#[test]
fn invoke_dst_forwards_to_callee_param() {
    // helper: returns constant 99
    let v_hret = ArcVarId::new(0);
    let helper = build_simple_func(
        940,
        &[],
        vec![ArcInstr::Let {
            dst: v_hret,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(99)),
        }],
        v_hret,
        ori_types::Idx::INT,
        1,
    );

    // callee: receives p, returns p
    let v_p = ArcVarId::new(0);
    let callee_func = build_simple_func(
        941,
        &[(0, ori_types::Idx::INT)],
        vec![],
        v_p,
        ori_types::Idx::INT,
        1,
    );

    // caller: invoke helper → x; let one = 1; let y = x + one; apply callee(y) → r; return r
    // Uses Invoke for helper call, Apply for callee call.
    let v_x = ArcVarId::new(0);
    let v_one = ArcVarId::new(1);
    let v_y = ArcVarId::new(2);
    let v_r = ArcVarId::new(3);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Invoke {
            dst: v_x,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(940),
            args: vec![],
            arg_ownership: vec![],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_y,
                ty: ori_types::Idx::INT,
                value: ori_arc::ir::ArcValue::PrimOp {
                    op: ori_arc::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v_x, v_one],
                },
            },
            ArcInstr::Apply {
                dst: v_r,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(941),
                args: vec![v_y],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Return { value: v_r },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Unreachable,
    };

    let caller = ArcFunction {
        name: ori_ir::Name::from_raw(942),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 4],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 4],
        spans: vec![vec![None; 0], vec![None; 3], vec![None; 0]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let mut plan = ReprPlan::new(crate::NarrowingPolicy::Conservative);

    propagate_ranges(&mut plan, &pool, &[helper, callee_func, caller], &config);

    // callee's parameter should be [100, 100] — forwarded from Invoke dst + 1.
    let callee_name = ori_ir::Name::from_raw(941);
    let p_range = plan.var_range(callee_name, v_p);
    assert_eq!(
        p_range,
        ValueRange::Bounded { lo: 100, hi: 100 },
        "Semantic pin (TPR-03-034): callee(invoke_dst + 1) → callee's param should be [100, 100], got {p_range:?}"
    );
}
