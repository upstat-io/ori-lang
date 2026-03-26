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
    let result = range_fixpoint(&func, &pool, &config);

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
    let result = range_fixpoint(&func, &pool, &config);

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
    let result = range_fixpoint(&func, &pool, &config);

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
    let result = range_fixpoint(&func, &pool, &config);

    // v_param should be join of [5,5] and [15,15] = [5,15]
    assert_eq!(
        result.var_ranges.get(&v_param).copied(),
        Some(ValueRange::Bounded { lo: 5, hi: 15 })
    );
}
