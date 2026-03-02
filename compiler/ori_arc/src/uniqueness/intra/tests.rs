//! Tests for intraprocedural uniqueness analysis.

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue,
};
use crate::liveness::compute_liveness;
use crate::test_helpers::{make_func, owned_param, v};
use crate::{ArcClass, ArcClassification};

use super::{analyze_intraprocedural, Uniqueness};

// -- Test classifier --

/// Test classifier: `Idx::STR` (index 3) is `DefiniteRef`, everything else Scalar.
/// This matches reality: str is heap-allocated, int/bool/float are scalars.
struct TestClassifier;

impl ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if idx == Idx::STR {
            ArcClass::DefiniteRef
        } else {
            ArcClass::Scalar
        }
    }
}

/// Type shorthand: RC'd type (str).
const RC: Idx = Idx::STR;
/// Type shorthand: scalar type (int).
const SCALAR: Idx = Idx::INT;

// -- Helpers --

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn block_with_params(
    id: u32,
    params: Vec<(ArcVarId, Idx)>,
    body: Vec<ArcInstr>,
    terminator: ArcTerminator,
) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params,
        body,
        terminator,
    }
}

fn construct(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: v(dst),
        ty: RC,
        ctor: CtorKind::ListLiteral,
        args: Vec::new(),
    }
}

fn let_var(dst: u32, src: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: RC,
        value: ArcValue::Var(v(src)),
    }
}

fn let_literal(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: RC,
        value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
    }
}

fn apply(dst: u32, args: Vec<u32>) -> ArcInstr {
    ArcInstr::Apply {
        dst: v(dst),
        ty: RC,
        func: Name::from_raw(99),
        args: args.into_iter().map(v).collect(),
        arg_ownership: Vec::new(),
    }
}

fn ret(var: u32) -> ArcTerminator {
    ArcTerminator::Return { value: v(var) }
}

fn jump(target: u32, args: Vec<u32>) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args: args.into_iter().map(v).collect(),
    }
}

fn branch(cond: u32, then_block: u32, else_block: u32) -> ArcTerminator {
    ArcTerminator::Branch {
        cond: v(cond),
        then_block: ArcBlockId::new(then_block),
        else_block: ArcBlockId::new(else_block),
    }
}

// -- Tests --

#[test]
fn fresh_allocation_is_unique() {
    // v0 = Construct([])  → Unique
    // Return(v0)
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![construct(0)], ret(0))],
        vec![RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::Unique);
}

#[test]
fn function_param_is_maybe_shared() {
    // @f(v0: str) -> str
    //   Return(v0)
    let func = make_func(
        vec![owned_param(0, RC)],
        RC,
        vec![block(0, vec![], ret(0))],
        vec![RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_in[0].get(v(0)), Uniqueness::MaybeShared);
    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::MaybeShared);
}

#[test]
fn alias_with_dead_source_is_move() {
    // v0 = Construct([])   → Unique
    // v1 = Let(Var(v0))    → v0 dead after → move → v1 = Unique
    // Return(v1)
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![construct(0), let_var(1, 0)], ret(1))],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    // v0 was Unique, v1 inherits via move (v0 is dead after the Let)
    assert_eq!(result.block_out[0].get(v(1)), Uniqueness::Unique);
}

#[test]
fn alias_with_live_source_is_shared() {
    // v0 = Construct([])   → Unique
    // v1 = Let(Var(v0))    → v0 still live → both Shared
    // Return(v0)           ← v0 is live after the Let
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![construct(0), let_var(1, 0)], ret(0))],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::Shared);
    assert_eq!(result.block_out[0].get(v(1)), Uniqueness::Shared);
}

#[test]
fn function_call_result_is_maybe_shared() {
    // v0 = Apply(some_fn)  → MaybeShared
    // Return(v0)
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![apply(0, vec![])], ret(0))],
        vec![RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::MaybeShared);
}

#[test]
fn string_literal_is_unique() {
    // v0 = Let(Literal("hello"))  → Unique
    // Return(v0)
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![let_literal(0)], ret(0))],
        vec![RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::Unique);
}

#[test]
fn branch_same_uniqueness_preserves() {
    // block0:
    //   v0 = Construct([])    → Unique
    //   v1 = Let(Int(0))      (scalar cond)
    //   Branch(v1, block1, block2)
    //
    // block1: Jump(block3, [v0])
    // block2: Jump(block3, [v0])
    //
    // block3(v2):
    //   Return(v2)
    //
    // v0 is Unique on both paths → v2 = join(Unique, Unique) = Unique
    let func = make_func(
        vec![],
        RC,
        vec![
            block(
                0,
                vec![
                    construct(0),
                    ArcInstr::Let {
                        dst: v(1),
                        ty: SCALAR,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                branch(1, 1, 2),
            ),
            block(1, vec![], jump(3, vec![0])),
            block(2, vec![], jump(3, vec![0])),
            block_with_params(3, vec![(v(2), RC)], vec![], ret(2)),
        ],
        vec![RC, SCALAR, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_in[3].get(v(2)), Uniqueness::Unique);
}

#[test]
fn branch_different_uniqueness_gives_maybe_shared() {
    // block0:
    //   v0 = Construct([])    → Unique
    //   v1 = Apply(fn)        → MaybeShared
    //   v2 = Let(Bool(true))  (scalar cond)
    //   Branch(v2, block1, block2)
    //
    // block1: Jump(block3, [v0])   → passes Unique
    // block2: Jump(block3, [v1])   → passes MaybeShared
    //
    // block3(v3):
    //   Return(v3)
    //
    // v3 = join(Unique, MaybeShared) = MaybeShared
    let func = make_func(
        vec![],
        RC,
        vec![
            block(
                0,
                vec![
                    construct(0),
                    apply(1, vec![]),
                    ArcInstr::Let {
                        dst: v(2),
                        ty: SCALAR,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                branch(2, 1, 2),
            ),
            block(1, vec![], jump(3, vec![0])),
            block(2, vec![], jump(3, vec![1])),
            block_with_params(3, vec![(v(3), RC)], vec![], ret(3)),
        ],
        vec![RC, RC, SCALAR, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_in[3].get(v(3)), Uniqueness::MaybeShared);
}

#[test]
fn cow_rebinding_pattern_stays_unique() {
    // The canonical value-semantics rebinding pattern:
    //   let list = []         → Unique
    //   let list = push(list) → push consumes old list, returns new → Unique
    //
    // In ARC IR (SSA):
    //   v0 = Construct([])    → Unique
    //   v1 = Apply(push, v0)  → MaybeShared (conservative for now)
    //   Return(v1)
    //
    // Note: v0 is dead after the Apply (it's consumed). v1 is a fresh
    // result from push. With interprocedural analysis (§07.3), v1 would
    // be Unique. For now, it's MaybeShared (conservative).
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![construct(0), apply(1, vec![0])], ret(1))],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    // v0 starts Unique
    // v1 is MaybeShared (Apply result; §07.3 will refine to Unique for COW ops)
    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::Unique);
    assert_eq!(result.block_out[0].get(v(1)), Uniqueness::MaybeShared);
}

#[test]
fn scalar_variables_not_tracked() {
    // v0 = Let(Int(42))  → scalar, not tracked
    // Return(v0)
    let func = make_func(
        vec![],
        SCALAR,
        vec![block(
            0,
            vec![ArcInstr::Let {
                dst: v(0),
                ty: SCALAR,
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            ret(0),
        )],
        vec![SCALAR],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    // Scalar variables are not explicitly tracked — default is MaybeShared
    // but it doesn't matter since they're never used for COW operations.
    assert_eq!(result.block_out[0].get(v(0)), Uniqueness::MaybeShared);
}

#[test]
fn loop_fresh_allocation_stays_unique() {
    // A loop that creates a fresh value each iteration:
    //
    // block0:
    //   v0 = Construct([])    → Unique
    //   Jump(block1, [v0])
    //
    // block1(v1):  ← loop header
    //   v2 = Construct([])    → Unique
    //   v3 = Let(Bool(true))  (cond)
    //   Branch(v3, block1_back, block2)
    //
    // block1_back:
    //   Jump(block1, [v2])    ← back edge with fresh value
    //
    // block2:
    //   Return(v1)
    //
    // v1 = join(Unique from block0, Unique from block1_back) = Unique
    let func = make_func(
        vec![],
        RC,
        vec![
            // block0: entry
            block(0, vec![construct(0)], jump(1, vec![0])),
            // block1: loop header
            block_with_params(
                1,
                vec![(v(1), RC)],
                vec![
                    construct(2),
                    ArcInstr::Let {
                        dst: v(3),
                        ty: SCALAR,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                branch(3, 3, 2),
            ),
            // block2: exit
            block(2, vec![], ret(1)),
            // block3 (block1_back): back edge
            block(3, vec![], jump(1, vec![2])),
        ],
        vec![RC, RC, RC, SCALAR],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    // v1 (loop header param) receives Unique from both paths
    assert_eq!(result.block_in[1].get(v(1)), Uniqueness::Unique);
    // v2 (constructed in loop body) is always Unique
    assert_eq!(result.block_out[1].get(v(2)), Uniqueness::Unique);
}

#[test]
fn select_joins_both_values() {
    // v0 = Construct([])      → Unique
    // v1 = Apply(fn)          → MaybeShared
    // v2 = Let(Bool(true))    (cond, scalar)
    // v3 = Select(v2, v0, v1) → join(Unique, MaybeShared) = MaybeShared
    // Return(v3)
    let func = make_func(
        vec![],
        RC,
        vec![block(
            0,
            vec![
                construct(0),
                apply(1, vec![]),
                ArcInstr::Let {
                    dst: v(2),
                    ty: SCALAR,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                },
                ArcInstr::Select {
                    dst: v(3),
                    ty: RC,
                    cond: v(2),
                    true_val: v(0),
                    false_val: v(1),
                },
            ],
            ret(3),
        )],
        vec![RC, RC, SCALAR, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(3)), Uniqueness::MaybeShared);
}

#[test]
fn select_both_unique_stays_unique() {
    // v0 = Construct([])
    // v1 = Construct([])
    // v2 = Let(Bool(true))
    // v3 = Select(v2, v0, v1) → join(Unique, Unique) = Unique
    // Return(v3)
    let func = make_func(
        vec![],
        RC,
        vec![block(
            0,
            vec![
                construct(0),
                construct(1),
                ArcInstr::Let {
                    dst: v(2),
                    ty: SCALAR,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                },
                ArcInstr::Select {
                    dst: v(3),
                    ty: RC,
                    cond: v(2),
                    true_val: v(0),
                    false_val: v(1),
                },
            ],
            ret(3),
        )],
        vec![RC, RC, SCALAR, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(3)), Uniqueness::Unique);
}

#[test]
fn convergence_with_single_block() {
    // Single block, no loops → should converge in 1 iteration.
    let func = make_func(
        vec![],
        RC,
        vec![block(0, vec![construct(0)], ret(0))],
        vec![RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    // Just verify it doesn't panic and produces a result.
    assert_eq!(result.block_in.len(), 1);
    assert_eq!(result.block_out.len(), 1);
}

#[test]
fn chained_moves_preserve_uniqueness() {
    // v0 = Construct([])   → Unique
    // v1 = Let(Var(v0))    → v0 dead → move → v1 = Unique
    // v2 = Let(Var(v1))    → v1 dead → move → v2 = Unique
    // Return(v2)
    let func = make_func(
        vec![],
        RC,
        vec![block(
            0,
            vec![construct(0), let_var(1, 0), let_var(2, 1)],
            ret(2),
        )],
        vec![RC, RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(2)), Uniqueness::Unique);
}

#[test]
fn project_is_maybe_shared() {
    // v0 = Construct(Struct)   → Unique
    // v1 = Project(v0, field0) → MaybeShared (borrows from parent)
    // Return(v1)
    let func = make_func(
        vec![],
        RC,
        vec![block(
            0,
            vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: RC,
                    ctor: CtorKind::Struct(Name::from_raw(50)),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: v(1),
                    ty: RC,
                    value: v(0),
                    field: 0,
                },
            ],
            ret(1),
        )],
        vec![RC, RC],
    );
    let classifier = TestClassifier;
    let liveness = compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(result.block_out[0].get(v(1)), Uniqueness::MaybeShared);
}
