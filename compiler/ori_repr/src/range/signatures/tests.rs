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
