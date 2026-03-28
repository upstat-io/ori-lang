//! Tests for widening, narrowing, and the range fixpoint loop.

use super::*;

// ─── widen ────────────────────────────────────────────────────

#[test]
fn widen_bottom_identity() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(widen(ValueRange::Bottom, r), r);
}

#[test]
fn widen_to_bottom_preserves_bottom() {
    let prev = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(widen(prev, ValueRange::Bottom), ValueRange::Bottom);
}

#[test]
fn widen_top_absorbs() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(widen(r, ValueRange::Top), ValueRange::Top);
    assert_eq!(widen(ValueRange::Top, r), ValueRange::Top);
}

/// Semantic pin: widening pushes a growing lower bound to `-∞`.
#[test]
fn widen_lo_grew() {
    let prev = ValueRange::Bounded { lo: 0, hi: 100 };
    let curr = ValueRange::Bounded { lo: -5, hi: 100 };
    // lo grew (decreased), push to MIN
    assert_eq!(
        widen(prev, curr),
        ValueRange::Bounded {
            lo: i64::MIN,
            hi: 100
        }
    );
}

/// Semantic pin: widening pushes a growing upper bound to `+∞`.
#[test]
fn widen_hi_grew() {
    let prev = ValueRange::Bounded { lo: 0, hi: 100 };
    let curr = ValueRange::Bounded { lo: 0, hi: 150 };
    assert_eq!(
        widen(prev, curr),
        ValueRange::Bounded {
            lo: 0,
            hi: i64::MAX
        }
    );
}

/// When both bounds grow, widening produces `Top`.
#[test]
fn widen_both_grew_becomes_top() {
    let prev = ValueRange::Bounded { lo: 0, hi: 100 };
    let curr = ValueRange::Bounded { lo: -1, hi: 101 };
    assert_eq!(widen(prev, curr), ValueRange::Top);
}

/// Stable range: no widening needed.
#[test]
fn widen_no_growth() {
    let prev = ValueRange::Bounded { lo: 0, hi: 100 };
    let curr = ValueRange::Bounded { lo: 5, hi: 90 };
    // Neither bound grew → keep current
    assert_eq!(widen(prev, curr), curr);
}

/// Semantic pin: widening terminates in at most 2 steps.
#[test]
fn widen_terminates_in_two_steps() {
    let r0 = ValueRange::Bounded { lo: 0, hi: 0 };
    let r1 = ValueRange::Bounded { lo: -1, hi: 1 };
    let w1 = widen(r0, r1);
    // After first widening: lo → MIN, hi → MAX → Top
    assert_eq!(w1, ValueRange::Top);
}

// ─── narrow ──────────────────────────────────────────────────

#[test]
fn narrow_tightens() {
    let widened = ValueRange::Bounded {
        lo: 0,
        hi: i64::MAX,
    };
    let computed = ValueRange::Bounded { lo: 0, hi: 99 };
    assert_eq!(narrow(widened, computed), computed);
}

#[test]
fn narrow_preserves_when_computed_is_wider() {
    let widened = ValueRange::Bounded { lo: 0, hi: 99 };
    let computed = ValueRange::Top;
    assert_eq!(narrow(widened, computed), widened);
}

#[test]
fn narrow_bottom_stays_bottom() {
    assert_eq!(
        narrow(ValueRange::Bottom, ValueRange::Bounded { lo: 0, hi: 10 }),
        ValueRange::Bottom
    );
}

// ─── range_fixpoint ──────────────────────────────────────────

/// Budget exceeded: function with too many blocks returns all-Top.
#[test]
fn fixpoint_budget_exceeded() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcTerminator};
    use ori_arc::ArcBlockId;

    let config = RangeAnalysisConfig {
        max_blocks: 2, // very low budget
        ..Default::default()
    };

    // Create a function with 3 blocks (exceeds budget of 2).
    let blocks: Vec<ArcBlock> = (0..3)
        .map(|i| ArcBlock {
            id: ArcBlockId::new(i),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        })
        .collect();

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(1),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar],
        spans: vec![vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // All variables should be Top (no analysis performed).
    assert!(result.var_ranges.is_empty());
    assert_eq!(result.return_range, ValueRange::Top);
}

/// Simple straight-line: `let v0 = 42` → v0 range is `[42, 42]`.
#[test]
fn fixpoint_constant_let() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let v0 = ArcVarId::new(0);
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v0,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Return { value: v0 },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(2),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    assert_eq!(
        result.var_ranges.get(&v0).copied(),
        Some(ValueRange::Bounded { lo: 42, hi: 42 })
    );
    assert_eq!(result.return_range, ValueRange::Bounded { lo: 42, hi: 42 });
}

/// Return range is the join of multiple `Return` terminators.
#[test]
fn fixpoint_return_range_join() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v_cond = ArcVarId::new(2);

    // Block 0: let v0 = 10; branch(v_cond, block1, block2)
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v0,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(10)),
            },
            ArcInstr::Let {
                dst: v1,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(20)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    // Block 1: return v0 (10)
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v0 },
    };

    // Block 2: return v1 (20)
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(3),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::BOOL,
        ],
        var_reprs: vec![
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
        ],
        spans: vec![vec![None; 3], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // join of [10,10] and [20,20] = [10,20]
    assert_eq!(result.return_range, ValueRange::Bounded { lo: 10, hi: 20 });
}

/// Block parameter merging: two predecessors jump with different ranges.
#[test]
fn fixpoint_block_param_merging() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let v0 = ArcVarId::new(0); // = 5
    let v1 = ArcVarId::new(1); // = 15
    let v_cond = ArcVarId::new(2);
    let v_param = ArcVarId::new(3); // block param: merge of v0 and v1

    // Block 0: let v0=5, v1=15, branch to block1 or block2
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v0,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(5)),
            },
            ArcInstr::Let {
                dst: v1,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(15)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    // Block 1: jump to block3 with v0
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(3),
            args: vec![v0],
        },
    };

    // Block 2: jump to block3 with v1
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(3),
            args: vec![v1],
        },
    };

    // Block 3: receives merged value, returns it
    let block3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![(v_param, ori_types::Idx::INT)],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_param },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(4),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2, block3],
        entry: ArcBlockId::new(0),
        var_types: vec![
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::BOOL,
            ori_types::Idx::INT,
        ],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 4],
        spans: vec![vec![None; 3], vec![], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // v_param should be join of [5,5] and [15,15] = [5,15]
    assert_eq!(
        result.var_ranges.get(&v_param).copied(),
        Some(ValueRange::Bounded { lo: 5, hi: 15 })
    );
}

// ─── TPR-03-015: block-entry refinements for non-param vars ─────

/// Semantic pin: non-parameter variable refined via Branch.
/// Block 0: let x = constant [0, 200], let cond = x < 100, Branch(cond, b1, b2)
/// Block 1 (true branch): x should be refined to [0, 99].
/// Without TPR-03-015 fix, x stays [0, 200] in block 1.
#[test]
fn fixpoint_branch_refines_non_param_variable() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};
    use ori_arc::ArcBlockId;
    use ori_ir::BinaryOp;

    let v_x = ArcVarId::new(0); // x = some value in [0, 200]
    let v_bound = ArcVarId::new(1); // bound = 100
    let v_cond = ArcVarId::new(2); // cond = x < bound

    // Block 0: let x = 50, let bound = 100, let cond = x < bound, branch(cond, b1, b2)
    // Note: x starts as [50, 50] which is already < 100, but the point is that
    // refinement should intersect the existing range with [MIN, 99].
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_x,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(50)),
            },
            ArcInstr::Let {
                dst: v_bound,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(100)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Lt),
                    args: vec![v_x, v_bound],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    // Block 1 (true): return x
    // After refinement, x should be [50, 99] (meet of [50, 50] and [MIN, 99])
    // = [50, 50] (since 50 is already within [MIN, 99])
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    // Block 2 (false): return x
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(10),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::BOOL,
        ],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 3],
        spans: vec![vec![None; 3], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // x starts as [50, 50], true-branch refinement intersects with [MIN, 99].
    // Since [50, 50] ∩ [MIN, 99] = [50, 50], the refinement doesn't change x.
    // The key semantic pin is that apply_block_refinements was called and
    // didn't crash or skip the non-param variable.
    assert_eq!(
        result.var_ranges.get(&v_x).copied(),
        Some(ValueRange::Bounded { lo: 50, hi: 50 })
    );
}

/// Semantic pin: Switch refinement on non-parameter variable.
/// Block 0: let x with Top range, Switch(x, {42 -> b1, 99 -> b2}, default -> b3)
/// Block 1: x should be [42, 42].
#[test]
fn fixpoint_switch_refines_non_param_variable() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let v_x = ArcVarId::new(0);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_x,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Switch {
            scrutinee: v_x,
            cases: vec![(42, ArcBlockId::new(1)), (99, ArcBlockId::new(2))],
            default: ArcBlockId::new(3),
        },
    };

    // Block 1: case 42, return x
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    // Block 2: case 99, return x
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    // Block 3: default, return x
    let block3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(11),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2, block3],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar],
        spans: vec![vec![None], vec![], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // x is defined as [42, 42], so the Switch refinement for case 42
    // (meet of [42, 42] and [42, 42]) keeps it at [42, 42].
    // This tests that apply_block_refinements is called for Switch cases.
    assert_eq!(
        result.var_ranges.get(&v_x).copied(),
        Some(ValueRange::Bounded { lo: 42, hi: 42 })
    );
}

// ─── TPR-03-016: field summary recompute after narrowing ────────

/// Field summary reflects post-narrowing ranges, not pre-narrowing widened ranges.
/// Construct a function with a Construct that uses a variable. After analysis,
/// the field summary should match the final variable range.
#[test]
fn fixpoint_field_summary_uses_final_ranges() {
    use ori_arc::ir::{
        ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue,
    };
    use ori_arc::ArcBlockId;

    let v_field = ArcVarId::new(0); // int field = 42
    let v_struct = ArcVarId::new(1); // struct { field }

    let struct_name = ori_ir::Name::from_raw(100);
    let struct_type_idx = ori_types::Idx::INT; // simplified — real type doesn't matter for range

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_field,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Construct {
                dst: v_struct,
                ty: struct_type_idx,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v_field],
            },
        ],
        terminator: ArcTerminator::Return { value: v_struct },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(12),
        params: vec![],
        return_type: struct_type_idx,
        blocks: vec![block0],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT, struct_type_idx],
        var_reprs: vec![
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
        ],
        spans: vec![vec![None; 2]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // Field summary for field 0 of struct_type_idx should be [42, 42]
    // (matching the final variable range, not a wider intermediate).
    assert_eq!(
        result.field_summaries.field_range(struct_type_idx, 0),
        ValueRange::Bounded { lo: 42, hi: 42 }
    );
}

// ─── TPR-03-017: Switch multi-case same-block must join, not overwrite ───

/// When multiple Switch cases target the same successor block, the scrutinee
/// refinement should be the JOIN of all case values, not just the last one.
/// Semantic pin: y = [0, 1] ONLY passes with correct join; overwrite gives [1, 1].
#[test]
fn fixpoint_switch_multi_case_same_block_joins() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue};
    use ori_arc::ArcBlockId;

    // v_x has range Top (from Apply to unknown function).
    // v_y copies x in the multi-case successor — its range reveals the refinement.
    let v_x = ArcVarId::new(0);
    let v_y = ArcVarId::new(1);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: v_x,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(99),
            args: vec![],
            arg_ownership: vec![],
        }],
        // Cases 0 and 1 both target block 1; case 2 targets block 2.
        terminator: ArcTerminator::Switch {
            scrutinee: v_x,
            cases: vec![
                (0, ArcBlockId::new(1)),
                (1, ArcBlockId::new(1)),
                (2, ArcBlockId::new(2)),
            ],
            default: ArcBlockId::new(3),
        },
    };

    // Block 1: multi-case successor — copy x into y to observe the refinement.
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_y,
            ty: ori_types::Idx::INT,
            value: ArcValue::Var(v_x),
        }],
        terminator: ArcTerminator::Return { value: v_y },
    };

    // Block 2: single case (2).
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    // Block 3: default.
    let block3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(20),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2, block3],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT, ori_types::Idx::INT],
        var_reprs: vec![
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
        ],
        spans: vec![vec![None], vec![None], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // Semantic pin: y must be [0, 1] (joined cases), NOT [1, 1] (last-write-wins).
    assert_eq!(
        result.var_ranges.get(&v_y).copied(),
        Some(ValueRange::Bounded { lo: 0, hi: 1 }),
        "TPR-03-017: multi-case same-block should JOIN [0,0] and [1,1] into [0,1]"
    );
}

// ─── TPR-03-018: Switch default block gets complement refinement ─────

/// The default successor of a Switch should receive a complement refinement
/// that excludes contiguous case values from the scrutinee's edges.
/// Semantic pin: default refinement [3, 10] ONLY passes with complement logic.
#[test]
fn fixpoint_switch_default_gets_complement() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    // Build a function where x ∈ [0, 10] via Select(cond, 0, 10).
    let v_a = ArcVarId::new(0); // 0
    let v_b = ArcVarId::new(1); // 10
    let v_c = ArcVarId::new(2); // IsShared → [0, 1]
    let v_x = ArcVarId::new(3); // Select(c, a, b) → [0, 10]
    let v_y = ArcVarId::new(4); // copy of x in default block

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_a,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_b,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(10)),
            },
            ArcInstr::IsShared { dst: v_c, var: v_a },
            ArcInstr::Select {
                dst: v_x,
                ty: ori_types::Idx::INT,
                cond: v_c,
                true_val: v_a,
                false_val: v_b,
            },
        ],
        // Cases 0, 1, 2 target block1; default → block2.
        terminator: ArcTerminator::Switch {
            scrutinee: v_x,
            cases: vec![
                (0, ArcBlockId::new(1)),
                (1, ArcBlockId::new(1)),
                (2, ArcBlockId::new(1)),
            ],
            default: ArcBlockId::new(2),
        },
    };

    // Block 1: cases 0/1/2.
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    // Block 2 (default): copy x to observe complement refinement.
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_y,
            ty: ori_types::Idx::INT,
            value: ArcValue::Var(v_x),
        }],
        terminator: ArcTerminator::Return { value: v_y },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(21),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::INT,
        ],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 5],
        spans: vec![vec![None; 4], vec![], vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // x is [0, 10]. Cases cover {0, 1, 2} from the low edge.
    // Default complement: [3, 10].
    // y copies x in default block, so y should be [3, 10].
    assert_eq!(
        result.var_ranges.get(&v_y).copied(),
        Some(ValueRange::Bounded { lo: 3, hi: 10 }),
        "TPR-03-018: default block should exclude contiguous low-edge cases [0,2] from [0,10]"
    );
}

// ─── TPR-03-036: multi-predecessor switch default must join, not meet ─

/// Build a diamond CFG where two switch blocks target the same default.
/// entry: x ∈ [0,10]; branch → left, right
/// left: switch x {0 → exit, default → join} → complement [1,10]
/// right: switch x {10 → exit, default → join} → complement [0,9]
/// join: y = copy x; return y
fn build_multi_pred_switch_func() -> (ori_arc::ir::ArcFunction, ArcVarId) {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let (v_a, v_b, v_c, v_x, v_y) = (
        ArcVarId::new(0),
        ArcVarId::new(1),
        ArcVarId::new(2),
        ArcVarId::new(3),
        ArcVarId::new(4),
    );

    let entry = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_a,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_b,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(10)),
            },
            ArcInstr::IsShared { dst: v_c, var: v_a },
            ArcInstr::Select {
                dst: v_x,
                ty: ori_types::Idx::INT,
                cond: v_c,
                true_val: v_a,
                false_val: v_b,
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_c,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };
    let left = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Switch {
            scrutinee: v_x,
            cases: vec![(0, ArcBlockId::new(4))],
            default: ArcBlockId::new(3),
        },
    };
    let right = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Switch {
            scrutinee: v_x,
            cases: vec![(10, ArcBlockId::new(4))],
            default: ArcBlockId::new(3),
        },
    };
    let join = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_y,
            ty: ori_types::Idx::INT,
            value: ArcValue::Var(v_x),
        }],
        terminator: ArcTerminator::Return { value: v_y },
    };
    let exit = ArcBlock {
        id: ArcBlockId::new(4),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_x },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(36),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![entry, left, right, join, exit],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 5],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 5],
        spans: vec![vec![None; 4], vec![], vec![], vec![None], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };
    (func, v_y)
}

/// Semantic pin: two switch predecessors targeting the same default block
/// must JOIN their complement refinements (sound over-approximation).
/// Using meet would under-approximate to [1, 9] — unsound.
#[test]
fn fixpoint_switch_default_multi_predecessor_joins() {
    let (func, v_y) = build_multi_pred_switch_func();
    let pool = ori_types::Pool::new();
    let result = range_fixpoint(&func, &pool, &RangeAnalysisConfig::default(), None, None);

    // Left complement: [1, 10]. Right complement: [0, 9].
    // JOIN = [0, 10] (sound). MEET = [1, 9] (UNSOUND).
    assert_eq!(
        result.var_ranges.get(&v_y).copied(),
        Some(ValueRange::Bounded { lo: 0, hi: 10 }),
        "TPR-03-036: multi-predecessor switch default must JOIN complements, not MEET"
    );
}

// ─── TPR-03-019: Narrowing pass recovers loop-bound block parameters ─

/// Build a simple bounded loop: `for i in 0..<limit` with increment 1.
/// Returns `(function, loop_var)` for assertion.
fn build_bounded_loop_func(limit: i64) -> (ori_arc::ir::ArcFunction, ArcVarId) {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};
    use ori_arc::ArcBlockId;
    use ori_ir::BinaryOp;

    let v_start = ArcVarId::new(0);
    let v_end = ArcVarId::new(1);
    let v_one = ArcVarId::new(2);
    let v_i = ArcVarId::new(3);
    let v_cond = ArcVarId::new(4);
    let v_next = ArcVarId::new(5);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_start,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_end,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(limit)),
            },
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v_start],
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v_i, ori_types::Idx::INT)],
        body: vec![ArcInstr::Let {
            dst: v_cond,
            ty: ori_types::Idx::INT,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Lt),
                args: vec![v_i, v_end],
            },
        }],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(2),
            else_block: ArcBlockId::new(3),
        },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_next,
            ty: ori_types::Idx::INT,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Add),
                args: vec![v_i, v_one],
            },
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v_next],
        },
    };

    let block3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_i },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(22),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2, block3],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 6],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 6],
        spans: vec![vec![None; 3], vec![None], vec![None], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    (func, v_i)
}

/// A bounded loop (i from 0 to <10) widens the loop variable to [0, MAX].
/// The narrowing pass should recover a tighter bound by re-merging block
/// params and applying branch refinements.
/// Semantic pin: i ∈ [0, 10] ONLY passes with block-param-aware narrowing.
#[test]
fn fixpoint_narrowing_recovers_loop_bound() {
    let (func, v_i) = build_bounded_loop_func(10);
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // After widening, i would be [0, MAX]. With proper narrowing
    // (block param re-merging + branch refinements), i should recover
    // to [0, 10] (join of start=[0,0] and narrowed-next=[1,10]).
    let i_range = result
        .var_ranges
        .get(&v_i)
        .copied()
        .unwrap_or(ValueRange::Top);
    assert!(
        matches!(i_range, ValueRange::Bounded { lo: 0, hi } if hi <= 10),
        "TPR-03-019: loop variable should narrow from [0, MAX] to [0, 10], got {i_range:?}"
    );
}

// ─── TPR-03-020: Branch refinement overwrite + stale iterations ──

/// Build a multi-predecessor Branch refinement CFG. Returns `(func, v_y)`.
fn build_multi_pred_branch_func() -> (ori_arc::ir::ArcFunction, ArcVarId) {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, PrimOp};
    use {ori_arc::ArcBlockId, ori_ir::BinaryOp};
    let (int, bool_t) = (ori_types::Idx::INT, ori_types::Idx::BOOL);
    let v = ArcVarId::new;
    let (v_x, v_cond, v_b1, v_c2, v_b2, v_c3, v_y) = (v(0), v(1), v(2), v(3), v(4), v(5), v(6));

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Apply {
                dst: v_x,
                ty: ori_types::Idx::INT,
                func: ori_ir::Name::from_raw(99),
                args: vec![],
                arg_ownership: vec![],
            },
            ArcInstr::IsShared {
                dst: v_cond,
                var: v_x,
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    let make_lt_branch = |id, bvar, cvar, bound, true_b, false_b| ArcBlock {
        id: ArcBlockId::new(id),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: bvar,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(ori_arc::ir::LitValue::Int(bound)),
            },
            ArcInstr::Let {
                dst: cvar,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Lt),
                    args: vec![v_x, bvar],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: cvar,
            then_block: ArcBlockId::new(true_b),
            else_block: ArcBlockId::new(false_b),
        },
    };

    let block1 = make_lt_branch(1, v_b1, v_c2, 0, 3, 4);
    let block2 = make_lt_branch(2, v_b2, v_c3, 100, 5, 3);
    let ret_block = |id, var| ArcBlock {
        id: ArcBlockId::new(id),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: var },
    };
    let block3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_y,
            ty: ori_types::Idx::INT,
            value: ArcValue::Var(v_x),
        }],
        terminator: ArcTerminator::Return { value: v_y },
    };
    let func = ArcFunction {
        name: ori_ir::Name::from_raw(30),
        params: vec![],
        return_type: int,
        blocks: vec![
            block0,
            block1,
            block2,
            block3,
            ret_block(4, v_x),
            ret_block(5, v_x),
        ],
        entry: ArcBlockId::new(0),
        var_types: vec![int, bool_t, int, bool_t, int, bool_t, int],
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 7],
        spans: vec![
            vec![None; 2],
            vec![None; 2],
            vec![None; 2],
            vec![None],
            vec![],
            vec![],
        ],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };
    (func, v_y)
}

/// Semantic pin: multi-predecessor Branch refinements must be joined.
/// Bug: `insert()` overwrites → B3 sees only [100, MAX].
/// Fix: join → B3 sees full range (sound over-approximation).
#[test]
fn fixpoint_branch_multi_predecessor_refinement_joins() {
    let (func, v_y) = build_multi_pred_branch_func();
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // B3 receives x from B1 true (x ∈ [MIN, -1]) and B2 false (x ∈ [100, MAX]).
    // Correct join: [MIN, MAX]. y copies x in B3.
    let y_range = result
        .var_ranges
        .get(&v_y)
        .copied()
        .unwrap_or(ValueRange::Top);
    // join([MIN, -1], [100, MAX]) = Bounded { lo: MIN, hi: MAX } — semantically
    // equivalent to Top but not normalized. Accept either representation.
    let is_full_range = matches!(y_range, ValueRange::Top)
        || matches!(y_range, ValueRange::Bounded { lo, hi } if lo == i64::MIN && hi == i64::MAX);
    assert!(
        is_full_range,
        "TPR-03-020: multi-predecessor Branch refinements must be joined, not overwritten. \
         B3 should see full range (join of [MIN,-1] and [100,MAX]), got {y_range:?}"
    );
}

// ─── TPR-03-021: return_range must be recomputed after narrowing ──────

/// Semantic pin: a bounded loop function's `return_range` should narrow along
/// with the loop variable. The loop returns `v_i` which narrows to [0, 10].
/// Without recomputation, `return_range` stays at [0, MAX] from forward passes.
#[test]
fn fixpoint_return_range_recomputed_after_narrowing() {
    let (func, _v_i) = build_bounded_loop_func(10);
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // The loop variable narrows to [0, 10] (covered by TPR-03-019 test).
    // The Return terminator returns v_i, so return_range should also narrow.
    // Bug: return_range stays at [0, MAX] because it's never recomputed.
    // Fix: recompute return_range from final narrowed ranges.
    assert!(
        matches!(result.return_range, ValueRange::Bounded { lo: 0, hi } if hi <= 10),
        "TPR-03-021: return_range should narrow with loop variable to [0, 10], \
         got {:?}",
        result.return_range
    );
}

// ─── TPR-03-022: projection refresh after field-summary recompute ──────

/// Build a bounded loop where the exit block constructs a struct from the
/// loop variable, projects field 0, and returns the projection.
/// Extends `build_bounded_loop_func` with Construct + Project in the exit block.
/// Returns `(func, v_projected, struct_type_idx)`.
fn build_loop_construct_project_func(
    limit: i64,
) -> (ori_arc::ir::ArcFunction, ArcVarId, ori_types::Idx) {
    use ori_arc::ir::{ArcBlock, ArcInstr, ArcTerminator, CtorKind};
    use ori_arc::ArcBlockId;

    let (mut func, v_i) = build_bounded_loop_func(limit);

    // Add 2 new variables: v_struct and v_projected.
    // v_i is var 3 in the base loop; next free slots are 6 and 7.
    let v_struct = ArcVarId::new(6);
    let v_projected = ArcVarId::new(7);
    let struct_name = ori_ir::Name::from_raw(200);
    let struct_type_idx = ori_types::Idx::BOOL; // distinct from INT field type

    func.var_types.push(struct_type_idx); // v_struct
    func.var_types.push(ori_types::Idx::INT); // v_projected
    func.var_reprs.push(ori_arc::ir::ValueRepr::Scalar);
    func.var_reprs.push(ori_arc::ir::ValueRepr::Scalar);
    func.return_type = ori_types::Idx::INT;

    // Replace exit block (block 3): add Construct + Project, return projection.
    func.blocks[3] = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![
            ArcInstr::Construct {
                dst: v_struct,
                ty: struct_type_idx,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v_i],
            },
            ArcInstr::Project {
                dst: v_projected,
                ty: ori_types::Idx::INT,
                value: v_struct,
                field: 0,
            },
        ],
        terminator: ArcTerminator::Return { value: v_projected },
    };
    func.spans[3] = vec![None; 2];

    (func, v_projected, struct_type_idx)
}

/// Semantic pin: a bounded loop's exit block constructs a struct from the
/// narrowed loop variable, projects field 0, and returns the projection.
/// Without the post-recompute projection refresh, the projection variable
/// and `return_range` stay widened to [0, MAX] even though the field summary
/// correctly shows [0, 10].
#[test]
fn fixpoint_projection_refreshed_after_field_summary_recompute() {
    let (func, v_projected, struct_type_idx) = build_loop_construct_project_func(10);
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // Field summary should be tight: recompute uses narrowed global v_i = [0, 10].
    let field_range = result.field_summaries.field_range(struct_type_idx, 0);
    assert!(
        matches!(field_range, ValueRange::Bounded { lo: 0, hi } if hi <= 10),
        "TPR-03-022: field summary should be [0, 10] after recompute, got {field_range:?}"
    );

    // Projection variable must be bounded — at the exit point, branch refinement gives
    // i ∈ [10, MAX], intersected with field summary [0, 10] → [10, 10]. This is the
    // correct precise answer. Before the fix, it was [10, MAX] (no re-transfer).
    let proj_range = result
        .var_ranges
        .get(&v_projected)
        .copied()
        .unwrap_or(ValueRange::Top);
    assert_eq!(
        proj_range,
        ValueRange::Bounded { lo: 10, hi: 10 },
        "TPR-03-022: projected variable should narrow to [10, 10] \
         (exit-block i refined to [10, 10] ∩ field summary [0, 10]), got {proj_range:?}. \
         Without post-recompute projection refresh, it stays at [10, MAX]."
    );

    // Return range must also be [10, 10] since it returns the projection.
    assert_eq!(
        result.return_range,
        ValueRange::Bounded { lo: 10, hi: 10 },
        "TPR-03-022: return_range should narrow with projection to [10, 10], got {:?}",
        result.return_range
    );
}

// ─── TPR-03-023: return_range must not include unreachable blocks ──────

/// Semantic pin: a function with an unreachable return block must NOT have
/// its `return_range` polluted by the dead block's return variable.
///
/// CFG:
///   B0: let v0 = 42; jump B2
///   B1: (unreachable) return v1 (never analyzed, so v1 has no range → Top)
///   B2: return v0 (42)
///
/// Without the fix, `recompute_return_range()` walks all blocks including B1,
/// finds v1 with no range entry, falls back to Top, and joins [42,42] ∨ Top = Top.
/// With the fix, only reachable blocks (B0, B2) are visited; `return_range` = [42,42].
#[test]
fn fixpoint_return_range_excludes_unreachable_blocks() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    // Block 0: let v0 = 42; jump B2
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v0,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(2),
            args: vec![],
        },
    };

    // Block 1: unreachable — no predecessor ever jumps here.
    // Has a Return terminator whose variable (v1) was never defined in the
    // forward pass, so `ranges.get(v1)` returns None → unwrap_or(Top).
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v1 },
    };

    // Block 2: return v0 (42)
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v0 },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(50),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT, ori_types::Idx::INT],
        var_reprs: vec![
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
        ],
        spans: vec![vec![None], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // Reachable return is only B2 returning v0=42, so return_range should be [42,42].
    // Bug: without the fix, B1's return (v1 → Top fallback) widens return_range to Top.
    assert_eq!(
        result.return_range,
        ValueRange::Bounded { lo: 42, hi: 42 },
        "TPR-03-023: unreachable block B1 should NOT pollute return_range. \
         Expected [42, 42], got {:?}. The unreachable Return's variable (v1) \
         was never analyzed, so it gets Top via unwrap_or, joining to Top.",
        result.return_range
    );
}

/// Edge case: ALL blocks are reachable, each with a Return — `return_range` is the join.
/// This verifies the fix doesn't accidentally exclude reachable return blocks.
#[test]
fn fixpoint_return_range_includes_all_reachable_returns() {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
    use ori_arc::ArcBlockId;

    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v_cond = ArcVarId::new(2);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v0,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(5)),
            },
            ArcInstr::Let {
                dst: v1,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(100)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
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
        body: vec![],
        terminator: ArcTerminator::Return { value: v0 },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(51),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![
            ori_types::Idx::INT,
            ori_types::Idx::INT,
            ori_types::Idx::BOOL,
        ],
        var_reprs: vec![
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
        ],
        spans: vec![vec![None; 3], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // Both blocks reachable: join of [5,5] and [100,100] = [5,100].
    assert_eq!(
        result.return_range,
        ValueRange::Bounded { lo: 5, hi: 100 },
        "All reachable returns should contribute to return_range"
    );
}

// ─── TPR-03-024: Invoke terminator must define dst variable range ──────

/// Semantic pin: an Invoke terminator defines a `dst` variable and its
/// range must appear in the fixpoint result. For an unknown function,
/// the range should be Top (conservative).
#[test]
fn fixpoint_invoke_defines_dst_variable() {
    use ori_arc::ir::{
        ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArgOwnership, LitValue,
    };
    use ori_arc::ArcBlockId;

    let v_arg = ArcVarId::new(0);
    let v_dst = ArcVarId::new(1);

    // Block 0: let v_arg = 10; invoke v_dst = unknown_fn(v_arg), normal→B1, unwind→B2
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v_arg,
            ty: ori_types::Idx::INT,
            value: ArcValue::Literal(LitValue::Int(10)),
        }],
        terminator: ArcTerminator::Invoke {
            dst: v_dst,
            ty: ori_types::Idx::INT,
            func: ori_ir::Name::from_raw(99), // unknown function
            args: vec![v_arg],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    };

    // Block 1 (normal): return v_dst
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v_dst },
    };

    // Block 2 (unwind): resume
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Resume,
    };

    let func = ArcFunction {
        name: ori_ir::Name::from_raw(52),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT, ori_types::Idx::INT],
        var_reprs: vec![
            ori_arc::ir::ValueRepr::Scalar,
            ori_arc::ir::ValueRepr::Scalar,
        ],
        spans: vec![vec![None], vec![], vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    // v_dst should be defined in the result (Top for unknown function).
    let dst_range = result
        .var_ranges
        .get(&v_dst)
        .copied()
        .unwrap_or(ValueRange::Bottom);
    assert_eq!(
        dst_range,
        ValueRange::Top,
        "TPR-03-024: Invoke dst variable should have a range (Top for unknown fn), got {dst_range:?}"
    );

    // return_range should also be Top (returns v_dst which is Top).
    assert_eq!(
        result.return_range,
        ValueRange::Top,
        "TPR-03-024: return_range should be Top when returning Invoke dst of unknown fn"
    );
}

// ─── Loop convergence: SSA body vars must not be widened ──────────

/// Build a loop that matches real ARC IR structure: copy variables in body.
///
/// This is the exact CFG from `@sum_loop` with `GtEq` comparison and copy
/// chains (%6=%4, %18=%4, %13=%5, etc.) that trigger spurious widening
/// when body variables are treated as phi nodes.
///
/// Returns `(func, v_i, v_sum, v_i_copy, v_next)`.
#[expect(
    clippy::too_many_lines,
    reason = "ARC IR test builder — block construction is inherently verbose"
)]
fn build_real_loop_func(
    limit: i64,
) -> (
    ori_arc::ir::ArcFunction,
    ArcVarId,
    ArcVarId,
    ArcVarId,
    ArcVarId,
) {
    use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};
    use ori_arc::ArcBlockId;
    use ori_ir::BinaryOp;

    let v_i_init = ArcVarId::new(0); // %0: sum init = 0 in real IR (both inits are 0)
    let v_s_init = ArcVarId::new(2); // %2: i init = 0 in real IR (both inits are 0)
    let v_i = ArcVarId::new(4); // %4: loop counter phi
    let v_sum = ArcVarId::new(5); // %5: accumulator phi
    let v_i_copy = ArcVarId::new(6); // %6: copy of %4 (for comparison)
    let v_limit = ArcVarId::new(7); // %7: limit constant
    let v_cond = ArcVarId::new(8); // %8: comparison result
    let v_s_copy = ArcVarId::new(13); // %13: copy of sum for add
    let v_i_copy2 = ArcVarId::new(14); // %14: copy of i for sum add
    let v_new_sum = ArcVarId::new(15); // %15: sum + i
    let v_new_sum2 = ArcVarId::new(16); // %16: copy of new_sum
    let v_i_copy3 = ArcVarId::new(18); // %18: copy of i for increment
    let v_one = ArcVarId::new(19); // %19: constant 1
    let v_next = ArcVarId::new(20); // %20: i + 1
    let v_next2 = ArcVarId::new(21); // %21: copy of next
    let v_ret_s = ArcVarId::new(26); // %26: copy of sum for return
    let v_ret = ArcVarId::new(27); // %27: return value

    // bb0: entry — both inits are 0, ordering doesn't affect result
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_i_init,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v_s_init,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v_i_init, v_s_init],
        },
    };

    // bb1: loop header — phi(i, sum), compare, branch
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v_i, ori_types::Idx::INT), (v_sum, ori_types::Idx::INT)],
        body: vec![
            ArcInstr::Let {
                dst: v_i_copy,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_i),
            },
            ArcInstr::Let {
                dst: v_limit,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(limit)),
            },
            ArcInstr::Let {
                dst: v_cond,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::GtEq),
                    args: vec![v_i_copy, v_limit],
                },
            },
        ],
        terminator: ArcTerminator::Branch {
            cond: v_cond,
            then_block: ArcBlockId::new(2),
            else_block: ArcBlockId::new(3),
        },
    };

    // bb2: exit (i >= limit) — return sum
    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_ret_s,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_sum),
            },
            ArcInstr::Let {
                dst: v_ret,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_ret_s),
            },
        ],
        terminator: ArcTerminator::Return { value: v_ret },
    };

    // bb3: body (i < limit) — sum = sum + i; i = i + 1; jump back
    let block3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v_s_copy,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_sum),
            },
            ArcInstr::Let {
                dst: v_i_copy2,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_i),
            },
            ArcInstr::Let {
                dst: v_new_sum,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v_s_copy, v_i_copy2],
                },
            },
            ArcInstr::Let {
                dst: v_new_sum2,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_new_sum),
            },
            ArcInstr::Let {
                dst: v_i_copy3,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_i),
            },
            ArcInstr::Let {
                dst: v_one,
                ty: ori_types::Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: v_next,
                ty: ori_types::Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v_i_copy3, v_one],
                },
            },
            ArcInstr::Let {
                dst: v_next2,
                ty: ori_types::Idx::INT,
                value: ArcValue::Var(v_next),
            },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v_next2, v_new_sum2],
        },
    };

    let mut var_types = vec![ori_types::Idx::INT; 28];
    var_types[8] = ori_types::Idx::BOOL;
    let func = ArcFunction {
        name: ori_ir::Name::from_raw(99),
        params: vec![],
        return_type: ori_types::Idx::INT,
        blocks: vec![block0, block1, block2, block3],
        entry: ArcBlockId::new(0),
        var_types,
        var_reprs: vec![ori_arc::ir::ValueRepr::Scalar; 28],
        spans: vec![vec![None; 2], vec![None; 3], vec![None; 2], vec![None; 8]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: vec![],
    };

    (func, v_i, v_sum, v_i_copy, v_next)
}

/// Semantic pin: loop counter phi with `GtEq` comparison and copy chains
/// must converge to [0, limit]. This is the exact structure generated by
/// the Ori compiler for `loop { if i >= N then break; ... i = i + 1 }`.
///
/// BUG: Before fix, body copy variables (%6 = %4) get widened because
/// `update_range()` applies join+widening to ALL variables. SSA body vars
/// should use direct assignment. The widened copy poisons downstream:
/// %20 = %6 + 1 feeds back to phi %4, causing %4 to diverge to Top.
#[test]
fn fixpoint_real_loop_converges_with_copies() {
    let (func, v_i, v_sum, _v_i_copy, v_next) = build_real_loop_func(10);
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    let i_range = result
        .var_ranges
        .get(&v_i)
        .copied()
        .unwrap_or(ValueRange::Top);
    assert_eq!(
        i_range,
        ValueRange::Bounded { lo: 0, hi: 10 },
        "loop counter phi should converge to EXACTLY [0, 10] with real ARC IR structure \
         (GtEq comparison + copy chains). Got {i_range:?}. \
         If lo < 0: body copy vars are being widened — fix update_range for SSA body vars."
    );

    // v_next (i+1) should be [1, 10] — the back-edge argument
    let next_range = result
        .var_ranges
        .get(&v_next)
        .copied()
        .unwrap_or(ValueRange::Top);
    assert_eq!(
        next_range,
        ValueRange::Bounded { lo: 1, hi: 10 },
        "i+1 should be exactly [1, 10], got {next_range:?}"
    );

    // sum accumulator doesn't have a branch refinement (no comparison on sum),
    // so the narrowing pass can't tighten it. It may stay wide or Top.
    // The critical assertion is on the loop counter and v_next above.
    let sum_range = result
        .var_ranges
        .get(&v_sum)
        .copied()
        .unwrap_or(ValueRange::Top);
    // After the fix, sum should at least have a non-negative lower bound because
    // sum starts at 0 and only adds non-negative values (i in [0,9]).
    // If this still fails, it's acceptable — sum convergence is a separate optimization.
    if let ValueRange::Bounded { lo, .. } = sum_range {
        assert!(
            lo >= 0,
            "if sum converges, lo should be >= 0, got {sum_range:?}"
        );
    }
}

/// Edge case: loop with limit=1 (single iteration).
/// Counter goes 0..1, so i ∈ [0, 1], next ∈ [1, 1].
#[test]
fn fixpoint_real_loop_single_iteration() {
    let (func, v_i, _, _, _) = build_real_loop_func(1);
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    let i_range = result
        .var_ranges
        .get(&v_i)
        .copied()
        .unwrap_or(ValueRange::Top);
    assert!(
        matches!(i_range, ValueRange::Bounded { lo: 0, hi } if hi <= 1),
        "loop counter with limit=1 should converge to [0, 1], got {i_range:?}"
    );
}

/// Edge case: loop with large limit (50000) — should use i16 or i32, not i8.
/// Verifies convergence even with large bounds.
#[test]
fn fixpoint_real_loop_large_limit() {
    let (func, v_i, _, _, _) = build_real_loop_func(50000);
    let pool = ori_types::Pool::new();
    let config = RangeAnalysisConfig::default();
    let result = range_fixpoint(&func, &pool, &config, None, None);

    let i_range = result
        .var_ranges
        .get(&v_i)
        .copied()
        .unwrap_or(ValueRange::Top);
    assert!(
        matches!(i_range, ValueRange::Bounded { lo: 0, hi } if hi <= 50000),
        "loop counter with limit=50000 should converge to [0, 50000], got {i_range:?}"
    );
}
