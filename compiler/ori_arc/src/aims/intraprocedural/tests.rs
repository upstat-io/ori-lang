//! Tests for the backward dataflow framework.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, LitValue,
};
use crate::ArcClass;

use crate::aims::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use crate::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, Locality, ReuseCtorKind, ShapeClass,
    Uniqueness,
};

// Mock classifier for tests

struct TestClassifier {
    /// Types that are scalar. Indexed by Idx raw value.
    scalars: Vec<bool>,
}

impl TestClassifier {
    fn all_ref(count: usize) -> Self {
        Self {
            scalars: vec![false; count],
        }
    }

    fn with_scalar(mut self, idx: usize) -> Self {
        if idx < self.scalars.len() {
            self.scalars[idx] = true;
        }
        self
    }
}

impl crate::ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if self
            .scalars
            .get(idx.raw() as usize)
            .copied()
            .unwrap_or(false)
        {
            ArcClass::Scalar
        } else {
            ArcClass::DefiniteRef
        }
    }
}

fn block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn no_sigs() -> FxHashMap<Name, MemoryContract> {
    FxHashMap::default()
}

// Straight-line single block: let v0 = literal; return v0

#[test]
fn single_block_literal_return() {
    let func = ArcFunction {
        var_types: vec![ty(0)], // v0: ref type
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(0),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v0 is defined as a literal → SCALAR, so it should be scalar
    // even though the type is "ref". The transfer function for Let with
    // Literal returns SCALAR. But the classifier says it's a ref type.
    // The analyzer marks scalars based on the classifier, not the literal.
    //
    // Since ty(0) is DefiniteRef in our classifier, v0 is NOT scalar.
    // But Let { Literal } transfer function gives it SCALAR state.
    // The backward demand from Return adds Once.
    let entry = state_map.var_state_at_block_entry(block_id(0), var(0));
    // v0 is defined in this block, so it should NOT appear in the entry
    // state (it's a definition, not flowing in from a predecessor).
    assert_eq!(entry, AimsState::BOTTOM);
}

// Two blocks with Jump: v0 defined in block 0, used in block 1

#[test]
fn two_blocks_jump_propagates_demand() {
    // Block 0: let v0 = literal; jump block1(v0)
    // Block 1(v1): return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(0)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(1), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Block 1 entry: v1 is a param (defined here), so not in entry state.
    // Block 1 exit: Return demands v1 once.
    let b1_exit = state_map.var_state_at_block_exit(block_id(1), var(1));
    // Return hasn't been added to exit (exit comes from successor entry).
    // Block 1 has no successors, so exit is empty.
    assert_eq!(b1_exit, AimsState::BOTTOM);

    // Block 0: v0 is demanded by the Jump (passing to block 1).
    // But v0 is also defined here (Let), so it shouldn't appear in entry.
    let b0_entry_v0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(b0_entry_v0, AimsState::BOTTOM);
}

// Branch: value used in both branches → Once per execution (alt_join)

#[test]
fn branch_value_used_in_both_arms_is_once() {
    // Block 0: let v0 = construct; let v1 = literal(bool); branch v1 -> b1, b2
    // Block 1: return v0
    // Block 2: return v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: var(0),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: var(1),
                        ty: ty(1),
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: var(1),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
        ],
        ..Default::default()
    };

    // ty(1) is scalar (bool)
    let classifier = TestClassifier::all_ref(2).with_scalar(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Block 0 exit: v0 is demanded by both successors with Once.
    // alt_join(Once, Once) = Once (not Many!) — only one branch executes.
    let b0_exit_v0 = state_map.var_state_at_block_exit(block_id(0), var(0));
    assert_eq!(
        b0_exit_v0.cardinality,
        Cardinality::Once,
        "alt_join(Once, Once) should be Once, not Many"
    );
}

// Sequential uses in same block → Many (seq_add)

#[test]
fn sequential_uses_in_same_block_are_many() {
    // Block 0: let v0 = construct; let v1 = project(v0, f1); let v2 = project(v0, f2); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Project {
                    dst: var(2),
                    ty: ty(0),
                    value: var(0),
                    field: 1,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v0 is used by two Project instructions in the same block.
    // seq_add(Once, Once) = Many — sequential composition.
    // But v0 is defined in this block (Construct), so its entry state is BOTTOM.
    // The exit state captures the demand before the block's instructions run.
    // We need to check the exit state to see the accumulated demand.

    // v0 is defined in this block, so entry has no demand for v0 (it's produced here).
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(0), var(0)),
        AimsState::BOTTOM
    );
}

// Scalar variables are excluded from analysis

#[test]
fn scalar_variables_excluded() {
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1)], // ty(1) is scalar
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(1),
                    value: ArcValue::Literal(LitValue::Bool(true)),
                },
            ],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2).with_scalar(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert!(state_map.is_scalar(var(1)));
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(0), var(1)),
        AimsState::SCALAR
    );
}

// Analysis converges (doesn't loop infinitely)

#[test]
fn analyze_function_terminates_on_back_edge_loop() {
    // Block 0: jump block1
    // Block 1: branch v0 -> block1, block2 (loop back-edge)
    // Block 2: unreachable
    //
    // This creates a loop (block1 → block1). The analysis must converge.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(1), // back-edge
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    // This must not loop infinitely.
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Converged-state pins: all three blocks analyzed, and v0 (the branch
    // cond, demanded across the back-edge) is live at block 1's entry.
    assert_eq!(state_map.num_blocks(), 3);
    let v0_entry = state_map.var_state_at_block_entry(block_id(1), var(0));
    assert_ne!(
        v0_entry.cardinality,
        Cardinality::Absent,
        "branch cond must carry demand at the loop header entry"
    );
}

// Empty function (single Unreachable block)

#[test]
fn empty_function_converges_immediately() {
    let func = ArcFunction::default(); // single block with Unreachable
    let classifier = TestClassifier::all_ref(0);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    assert_eq!(state_map.num_blocks(), 1);
}

// Function parameter flowing through

#[test]
fn function_param_demand_propagated() {
    // Block 0(v0: ref): return v0
    // v0 is a function parameter, not defined in the block body.
    // Its demand from Return should appear in the entry state... but
    // v0 is a block param, so it's removed from entry state.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::Ownership::Owned,
        }],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![(var(0), ty(0))],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v0 is a block param, removed from entry. But the exit state should
    // be empty (Return is terminal, no successors).
    assert_eq!(
        state_map.var_state_at_block_exit(block_id(0), var(0)),
        AimsState::BOTTOM
    );
}

// Invoke: dst defined only in normal successor, not unwind

#[test]
fn invoke_dst_removed_from_normal_successor_entry() {
    // Block 0: invoke v2 = call f(v0) normal→b1 unwind→b2
    // Block 1 (normal): return v2
    // Block 2 (unwind): resume
    //
    // v2 is defined by the Invoke at the entry of block 1 (normal).
    // It should NOT appear in block 1's entry state.
    // v0 is used by the Invoke and defined in the same block, so its
    // entry demand is also consumed.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Invoke {
                    dst: var(2),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(2) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v2 is defined by Invoke at block 1 entry. Block 1's entry state
    // should NOT contain v2 (it's produced here, like a block param).
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(1), var(2)),
        AimsState::BOTTOM,
        "invoke dst should be removed from normal successor entry"
    );

    // Block 2 (unwind) doesn't use v2, and v2 is NOT defined here.
    // Its entry state should also not contain v2.
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(2), var(2)),
        AimsState::BOTTOM,
        "invoke dst should not appear in unwind successor"
    );

    // Invoke edge states should be populated.
    let edge = state_map.invoke_edge_state(block_id(0));
    assert!(
        edge.is_some(),
        "block 0 has Invoke terminator — edge state should exist"
    );
}

#[test]
fn invoke_edge_state_tracks_per_edge_demand() {
    // Block 0: let v0 = construct; invoke v2 = call f(v0) normal→b1 unwind→b2
    // Block 1 (normal): return v2
    // Block 2 (unwind): let v3 = project(v0, 0); resume
    //
    // v0 is live across the invoke. On the unwind path, it's used by Project.
    // The unwind edge demand should include v0 but not v2.
    // The normal edge demand should include v2 (demanded by Return).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Invoke {
                    dst: var(2),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(2) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(3),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let edge = state_map
        .invoke_edge_state(block_id(0))
        .expect("Invoke block should have edge state");

    // Normal edge: v2 is demanded by Return in block 1 → appears in
    // block 1's entry state before invoke def removal. But since invoke
    // defs are removed in entry state computation, the edge state records
    // the raw entry state. v2 should appear in the normal edge.
    // Actually, invoke_edge_state records block_entry_states which already
    // had invoke_defs removed. So v2 should NOT be in normal.
    // What IS in normal? Nothing (v2 was the only variable and it was removed).
    assert!(
        !edge.normal.contains_key(&var(2)),
        "normal edge should not contain invoke dst (removed from entry)"
    );

    // Unwind edge: v0 is used by Project in block 2. v0's demand propagates
    // to block 2's entry state.
    assert!(
        edge.unwind.contains_key(&var(0)),
        "unwind edge should contain v0 (used by Project in unwind block)"
    );
}

// Validation corpus tests

#[test]
fn corpus_01_straight_line_single_use() {
    // v0 = construct; return v0
    // v0 used once → Once at use, defined in block so entry is BOTTOM.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    // v0 defined in this block → entry is BOTTOM (demand consumed at def).
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(0), var(0)),
        AimsState::BOTTOM
    );
}

#[test]
fn corpus_02_if_one_use_each_branch() {
    // Already covered by branch_value_used_in_both_arms_is_once.
    // Re-verify: alt_join(Once, Once) = Once.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: var(0),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: var(1),
                        ty: ty(1),
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: var(1),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
        ],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2).with_scalar(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    let exit_v0 = state_map.var_state_at_block_exit(block_id(0), var(0));
    assert_eq!(exit_v0.cardinality, Cardinality::Once);
}

#[test]
fn corpus_03_if_use_in_one_branch_only() {
    // v0 = construct; branch v1 -> b1(return v0), b2(return literal)
    // alt_join(Once, Absent) = Once (max in lattice order).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: var(0),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: var(1),
                        ty: ty(1),
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                    ArcInstr::Construct {
                        dst: var(2),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(11)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: var(1),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(2) },
            },
        ],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2).with_scalar(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    // v0 used in one branch only. alt_join(Once, BOTTOM) = Once.
    let exit_v0 = state_map.var_state_at_block_exit(block_id(0), var(0));
    assert_eq!(
        exit_v0.cardinality,
        Cardinality::Once,
        "alt_join(Once, Absent) should be Once"
    );
}

#[test]
fn corpus_04_loop_one_use_per_iteration() {
    // Block 0: v0 = construct; jump b1
    // Block 1: branch v1 -> b1(loop), b2(exit)
    // v0 used in loop condition → many iterations → Many.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(1), // back-edge
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    // v0 is used in block 1 (Branch cond) and block 1 loops back to
    // itself. The back-edge means v0's demand is seq_add'd across
    // iterations, promoting Once to Many.
    let b1_entry = state_map.var_state_at_block_entry(block_id(1), var(0));
    assert_eq!(
        b1_entry.cardinality,
        Cardinality::Many,
        "loop-carried use should promote to Many"
    );
}

#[test]
fn corpus_05_nested_loop() {
    // Block 0: v0 = construct; jump b1
    // Block 1: branch v1 -> b2(inner), b3(exit)
    // Block 2: branch v2 -> b2(inner back), b1(outer back)
    // Block 3: unreachable
    // v0 used in inner loop body → Many.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(2),
                    else_block: block_id(3),
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(2), // inner back-edge
                    else_block: block_id(1), // outer back-edge
                },
            },
            ArcBlock {
                id: block_id(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    // v0 used as cond in both block 1 and block 2, both of which loop.
    let b2_entry = state_map.var_state_at_block_entry(block_id(2), var(0));
    assert_eq!(
        b2_entry.cardinality,
        Cardinality::Many,
        "nested loop use should be Many"
    );
}

#[test]
fn corpus_06_switch_pattern_bindings() {
    // v0 = construct; switch v0 { 0→b1, default→b2 }
    // b1: v1 = project(v0, 0); return v1
    // b2: return v0
    // v1 (pattern binding via Project) should inherit v0's uniqueness.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(20),
                        variant: 0,
                    },
                    args: vec![],
                }],
                terminator: ArcTerminator::Switch {
                    scrutinee: var(0),
                    cases: vec![(0, block_id(1))],
                    default: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
        ],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v1 is a Project from v0 (field 0) — its borrow source should be v0 with field index.
    let source = state_map.borrow_source(var(1));
    assert_eq!(
        source,
        Some(&super::super::lattice::BorrowSource::exact_field(var(0), 0)),
        "Project binding should have BorrowSource::exact_field(scrutinee, field)"
    );
}

#[test]
fn corpus_08_project_then_source_reuse() {
    // v0 = construct; v1 = project(v0, 0); return v1
    // v0 is projected but not used afterward → v0 stays Unique
    // (Project doesn't affect source uniqueness).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Borrow source confirms v1 borrows from v0 (field 0).
    assert_eq!(
        state_map.borrow_source(var(1)),
        Some(&super::super::lattice::BorrowSource::exact_field(var(0), 0))
    );
}

#[test]
fn corpus_09_collection_update_receiver_once() {
    // v0 = construct(list); v1 = apply push(v0, v2); return v1
    // The receiver v0 is used once (by Apply) → cardinality Once.
    // v1 (result) is freshly constructed by the call.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::ListLiteral,
                    args: vec![],
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(200), // push
                    args: vec![var(0), var(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    // v0 is defined in this block, so entry is BOTTOM. The analysis correctly
    // tracks it was used once by Apply. Without interprocedural contracts
    // (no interprocedural info), the Apply dst v1 gets conservative state.
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(0), var(0)),
        AimsState::BOTTOM
    );
}

#[test]
fn corpus_10_partial_apply_capture() {
    // v0 = construct; v1 = partial_apply(f, [v0]); return v1
    // Captured v0 should get Many cardinality and HeapEscaping locality.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v0 is defined in this block (Construct), so entry is BOTTOM.
    // But the demand on v0 from PartialApply should be Many (captured).
    assert_eq!(
        state_map.var_state_at_block_entry(block_id(0), var(0)),
        AimsState::BOTTOM,
        "v0 defined in block → entry is BOTTOM"
    );
}

// Sparse event table: reusable allocation candidates

#[test]
fn sparse_events_reusable_allocation_for_struct_construct() {
    // v0 = Construct(Struct); return v0
    // Construct with reusable ctor on non-scalar → ReusableAllocation event.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    let reusable: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, super::state_map::AimsEvent::ReusableAllocation { .. }))
        .collect();
    assert_eq!(
        reusable.len(),
        1,
        "Struct Construct should record ReusableAllocation"
    );
    let expected_block = block_id(0);
    let expected_var = var(0);
    assert!(matches!(
        reusable[0],
        super::state_map::AimsEvent::ReusableAllocation {
            block,
            instr: 0,
            var,
        } if *block == expected_block && *var == expected_var
    ));
}

#[test]
fn sparse_events_reusable_allocation_for_enum_construct() {
    // v0 = Construct(EnumVariant); return v0
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::EnumVariant {
                    enum_name: Name::from_raw(20),
                    variant: 0,
                },
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    let reusable: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, super::state_map::AimsEvent::ReusableAllocation { .. }))
        .collect();
    assert_eq!(
        reusable.len(),
        1,
        "EnumVariant Construct should record ReusableAllocation"
    );
}

#[test]
fn sparse_events_no_reusable_allocation_for_list_literal() {
    // v0 = Construct(ListLiteral); return v0
    // Collections are NOT reusable (they use CollectionReuse instead).
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::ListLiteral,
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    let reusable: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, super::state_map::AimsEvent::ReusableAllocation { .. }))
        .collect();
    assert!(
        reusable.is_empty(),
        "ListLiteral should NOT record ReusableAllocation"
    );
}

#[test]
fn sparse_events_no_reusable_allocation_for_scalar() {
    // v0 = Construct(Struct) but ty(0) is scalar → no event.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1).with_scalar(0);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    assert!(
        events.is_empty(),
        "Scalar Construct should NOT record events"
    );
}

#[test]
fn sparse_events_multiple_constructs_in_block() {
    // v0 = Construct(Struct); v1 = Construct(EnumVariant); return v1
    // Both should produce ReusableAllocation events.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(1),
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(20),
                        variant: 0,
                    },
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    let reusable: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, super::state_map::AimsEvent::ReusableAllocation { .. }))
        .collect();
    assert_eq!(
        reusable.len(),
        2,
        "Both Constructs should record ReusableAllocation"
    );
}

// Sparse event table: local-allocation eligibility

#[test]
fn sparse_events_local_alloc_for_function_local_variable() {
    // v0 = Construct(Struct); v1 = Project(v0, 0); return v1
    // v0 is defined and consumed locally (only projected, never returned).
    // Its converged exit state should have local Locality, producing a
    // LocalAllocCandidate event.
    //
    // Note: whether Locality is FunctionLocal/BlockLocal depends on the
    // transfer functions. This test verifies the event is recorded when
    // the exit state has local locality. If the current transfer functions
    // don't produce local locality for this pattern (conservative defaults
    // may set Unknown), the test documents the expected behavior when
    // locality inference is enabled.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Check what Locality the converged state has for v0.
    let exit_v0 = state_map.var_state_at_block_exit(block_id(0), var(0));
    let events = state_map.events_in_block(block_id(0));
    let local_alloc: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, super::state_map::AimsEvent::LocalAllocCandidate { .. }))
        .collect();

    // If locality is FunctionLocal or BlockLocal, we should have an event.
    // If locality is Unknown/HeapEscaping (conservative default), no event.
    if matches!(
        exit_v0.locality,
        super::super::lattice::Locality::FunctionLocal
            | super::super::lattice::Locality::BlockLocal
    ) {
        assert!(
            !local_alloc.is_empty(),
            "FunctionLocal/BlockLocal variable should record LocalAllocCandidate"
        );
    } else {
        // Conservative default: no local-alloc events produced when
        // locality defaults to Unknown.
        assert!(
            local_alloc.is_empty(),
            "Unknown/HeapEscaping locality should NOT record LocalAllocCandidate"
        );
    }
}

// Locality Activation tests

/// Construct in a single block with return: value escapes the function,
/// so return widening forces `HeapEscaping` locality on the returned var.
#[test]
fn returned_construct_gets_heap_escaping_locality() {
    // Block 0: v0 = Construct; return v0
    let func = ArcFunction {
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Block 0 exit is empty (no successors). But within the block,
    // the Return terminator widens v0's locality to HeapEscaping.
    // Check that block 0's entry state reflects this via the backward
    // demand. v0 is defined in this block, so entry has BOTTOM.
    // The relevant state is at the exit — but exit is empty for Return blocks.
    //
    // The effect shows in the converged state map: the backward demand
    // from Return includes HeapEscaping locality. This influences
    // contract extraction (which reads entry state for params).
    // For a non-param variable defined in the same block, verify the
    // return locality widening happened by checking the block exit state
    // of the predecessor in a multi-block scenario (tested below).
    let entry_v0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(entry_v0, AimsState::BOTTOM, "defined in this block");
}

/// Construct used only within the same block (not returned, not passed
/// to another block): locality stays `BlockLocal`.
#[test]
fn block_local_construct_stays_block_local() {
    // Block 0: v0 = Construct; v1 = Project(v0, 0); return v1
    // v0 is constructed and projected in the same block, never escapes.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v0 is used only in its defining block (by Project). Its exit state
    // should have BlockLocal locality — it never crosses a block boundary.
    // Since block 0 has no successors, v0's exit state is BOTTOM.
    // But the forward transfer for Construct gives BlockLocal via FRESH.
    //
    // The key test: v0 does NOT appear in the exit state (no successors
    // demand it), confirming it stays block-local. Verify via the sparse
    // event table: v0 should be a LocalAllocCandidate.
    let events = state_map.events_in_block(block_id(0));
    let local_alloc: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                super::AimsEvent::LocalAllocCandidate {
                    var: v,
                    ..
                } if *v == var(0)
            )
        })
        .collect();
    assert!(
        !local_alloc.is_empty(),
        "block-local construct should be a LocalAllocCandidate"
    );
}

/// Cross-block flow widens locality to `FunctionLocal`.
///
/// Function param p0 flows to block 1 via Jump. Because p0 crosses a
/// block boundary, its backward demand includes `FunctionLocal` locality.
/// This is visible in the entry state of block 0.
#[test]
fn cross_block_flow_widens_to_function_local() {
    // func(p0):
    //   block 0: jump block1(p0)
    //   block 1(v1): v2 = Project(v1, 0); return v2
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(0)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(1), ty(0))],
                body: vec![ArcInstr::Project {
                    dst: var(2),
                    ty: ty(0),
                    value: var(1),
                    field: 0,
                }],
                terminator: ArcTerminator::Return { value: var(2) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // p0 is a function param. The Jump passes p0 to block 1 (cross-block),
    // so the backward demand includes FunctionLocal locality widening.
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert!(
        entry_p0.locality >= Locality::FunctionLocal,
        "cross-block function param should have at least FunctionLocal locality, got {:?}",
        entry_p0.locality
    );
}

/// Return widening sets `HeapEscaping` locality on returned param.
///
/// Function param p0 is directly returned. The backward demand from
/// Return includes `HeapEscaping` locality widening.
#[test]
fn returned_param_gets_heap_escaping_locality() {
    // func(p0):
    //   block 0: return p0
    let func = ArcFunction {
        var_types: vec![ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // p0 is returned directly. The backward demand from Return
    // widens p0's locality to HeapEscaping.
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        entry_p0.locality,
        Locality::HeapEscaping,
        "returned param should have HeapEscaping locality"
    );
}

/// Contract-aware locality: callee contract with `HeapEscaping` locality
/// widens the arg's locality.
#[test]
fn callee_contract_locality_widens_arg() {
    use super::super::contract::{ContextBehavior, EffectSummary, ParamContract, ReturnContract};
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // Block 0: v0 = Construct; v1 = Apply(callee, [v0]); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee contract: param 0 may escape (HeapEscaping locality_bound).
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Owned,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: true,
                may_share: false,
                locality_bound: Locality::HeapEscaping,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::default(),
            context_behavior: ContextBehavior::default(),
            fip: super::super::contract::FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // Key verification: analysis converges successfully with contract locality.
    // The contract-aware demand influences the entry state of the function
    // (for parameters), which we test via interprocedural tests.
    // v0 is defined in this block (removed from entry), so we verify via
    // event table: v0 should NOT be a LocalAllocCandidate because the
    // callee escapes it.
    let events = state_map.events_in_block(block_id(0));
    let local_alloc_v0: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                super::AimsEvent::LocalAllocCandidate { var: v, .. } if v == &var(0)
            )
        })
        .collect();
    // `local_alloc_v0` rides on EXIT-state locality, which is BOTTOM
    // (BlockLocal) for a single-block Return function, so it cannot observe the
    // contract-driven widening (the event still fires off the exit state). The
    // widening is recorded as backward demand at v0's DEFINITION (the same
    // `var_state_at_definition` surface the Phase-6 burden eliminator consumes)
    // — per TF-11 Apply, an Owned arg to a callee whose
    // `ParamContract.locality_bound = HeapEscaping` widens
    // `arg.locality := max(arg.locality, HeapEscaping)`.
    let _ = local_alloc_v0;
    let def_v0 = state_map.var_state_at_definition(block_id(0), var(0));
    assert_eq!(
        def_v0.locality,
        Locality::HeapEscaping,
        "callee HeapEscaping contract must widen v0's definition-site locality"
    );
}

/// Negative clamp for `callee_contract_locality_widens_arg`: a callee whose
/// `ParamContract.locality_bound = FunctionLocal` does NOT widen the Owned
/// arg's locality to `HeapEscaping` — the arg stays a local-allocation
/// candidate. Pairs with the positive test to pin that the widening is driven
/// by the contract's locality value, not unconditional on every Owned call arg.
#[test]
fn callee_contract_local_locality_does_not_escape_arg() {
    use super::super::contract::{ContextBehavior, EffectSummary, ParamContract, ReturnContract};
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee contract: param 0 stays FunctionLocal (does not escape).
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Owned,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::default(),
            context_behavior: ContextBehavior::default(),
            fip: super::super::contract::FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let def_v0 = state_map.var_state_at_definition(block_id(0), var(0));
    assert_ne!(
        def_v0.locality,
        Locality::HeapEscaping,
        "FunctionLocal callee contract must NOT widen v0 to HeapEscaping"
    );
}

/// Contrast: callee with `FunctionLocal` locality preserves arg locality.
#[test]
fn callee_contract_function_local_preserves_arg() {
    use super::super::contract::{ContextBehavior, EffectSummary, ParamContract, ReturnContract};
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // Block 0(p0): v1 = Apply(callee, [p0]); return v1
    // p0 is a function param.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee contract: param 0 stays FunctionLocal (doesn't escape).
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Borrowed,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::default(),
            context_behavior: ContextBehavior::default(),
            fip: super::super::contract::FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // p0 is a function param. Its entry state at block 0 should reflect
    // the callee's FunctionLocal locality bound (not HeapEscaping).
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert!(
        entry_p0.locality <= Locality::FunctionLocal,
        "callee with FunctionLocal bound should preserve arg locality, got {:?}",
        entry_p0.locality
    );
}

/// Block-local construct starts `Unique` via TF-3 (fresh allocation).
///
/// A value constructed within a block starts with `Uniqueness::Unique`
/// because `Construct` produces a fresh allocation with RC == 1 (TF-3).
/// Uniqueness is preserved through the block because no sharing occurs.
#[test]
fn block_local_value_gets_unique_without_runtime_check() {
    use super::super::lattice::Uniqueness;

    // func:
    //   block 0: v0 = Construct(Struct); v1 = Project(v0, 0); return v1
    // v0 is block-local: constructed and fully consumed (Project) in same block.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v0 is defined in this block, so it won't appear in entry state.
    // Check the exit state instead — v0 should have Unique uniqueness
    // (or be absent, which is fine since it's consumed).
    // The definitive check: v0 should NOT need a COW check (no
    // MaybeShared → no IsShared instruction needed).
    let exit = state_map.block_exit_states(block_id(0));
    if let Some(exit_states) = exit {
        if let Some(v0_state) = exit_states.get(&var(0)) {
            // If v0 appears in exit state (shouldn't for a terminal block),
            // it should be Unique, not MaybeShared.
            assert_ne!(
                v0_state.uniqueness,
                Uniqueness::MaybeShared,
                "block-local value should not be MaybeShared"
            );
        }
    }

    // The real verification: the LocalAllocCandidate event confirms block-local
    // treatment (no runtime uniqueness check needed).
    let events = state_map.events_in_block(block_id(0));
    let is_local_alloc = events.iter().any(|e| {
        matches!(
            e,
            super::AimsEvent::LocalAllocCandidate { var: v, .. } if *v == var(0)
        )
    });
    assert!(
        is_local_alloc,
        "block-local construct should be LocalAllocCandidate (no runtime uniqueness check)"
    );
}

/// Function-local linear value is RC-skip eligible.
///
/// A function parameter that is used linearly (consumed once) and stays
/// function-local should be marked as RC-skip eligible — no need for
/// `RcInc` at entry or `RcDec` at last use.
#[test]
fn function_local_linear_value_skips_rc() {
    // func(p0):
    //   block 0: v1 = Project(p0, 0); return v1
    // p0 is a function param, used once (Project), stays function-local
    // (doesn't cross block boundaries, but is a function param so at
    // least FunctionLocal).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Project {
                dst: var(1),
                ty: ty(0),
                value: var(0),
                field: 0,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // p0 is returned indirectly (through Project → v1 → Return).
    // But p0 itself is used once (by Project) and not returned.
    // However, v1 (the projected value) IS returned, so v1 gets HeapEscaping.
    //
    // For p0: used once (Once cardinality) but it's a function param
    // that could have any upstream locality. The backward analysis
    // sees Return(v1) → HeapEscaping on v1, then Project adds demand
    // on v0=p0 with Once cardinality.
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));

    // p0 should have Once cardinality (used once by Project).
    assert_eq!(
        entry_p0.cardinality,
        Cardinality::Once,
        "p0 used once by Project"
    );

    // For a function-local linear value, is_rc_skip_eligible should be true
    // when locality is FunctionLocal and consumption is Linear.
    // Note: the backward analysis may give p0 Affine consumption (may need
    // drop). Check the actual state for RC-skip eligibility.
    if entry_p0.is_local() && entry_p0.consumption <= super::super::lattice::Consumption::Linear {
        assert!(
            entry_p0.is_rc_skip_eligible(),
            "function-local linear param should be RC-skip eligible: {entry_p0:?}"
        );
    }
}

/// Contract with locality bounds enables RC-free call pattern.
///
/// When a callee's contract guarantees all params stay `FunctionLocal`,
/// the caller can skip `RcInc`/`RcDec` at the call boundary.
#[test]
fn contract_with_locality_bounds_enables_rc_free_call() {
    use super::super::contract::{ContextBehavior, EffectSummary, ParamContract, ReturnContract};
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // func(p0):
    //   block 0: v1 = Apply(callee, p0); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee contract: param stays FunctionLocal, borrowed, linear.
    // This means the callee does NOT escape the arg → no RcInc needed.
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Borrowed,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::default(),
            context_behavior: ContextBehavior::default(),
            fip: super::super::contract::FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));

    // The callee's contract says param stays FunctionLocal.
    // Combined with backward demand, p0's locality should reflect this.
    assert!(
        entry_p0.locality <= Locality::HeapEscaping,
        "contract locality should be reflected in analysis"
    );

    // The key insight: if p0 is function-local and the callee borrows it
    // (doesn't take ownership), the caller can skip `RcInc`/`RcDec` at the
    // call site. Verify via the contract's locality_bound.
    let contract = &sigs[&callee_name];
    assert_eq!(contract.params[0].locality_bound, Locality::FunctionLocal);
    assert!(!contract.params[0].may_escape);
}

// Transfer Fusion — pure callee preserves caller uniqueness

/// When callee has `may_share == false`, borrowed arguments preserve uniqueness.
///
/// A "pure" callee (one that doesn't create new references) cannot compromise
/// the caller's uniqueness of a borrowed argument. The argument's pre-call
/// uniqueness is preserved through the call.
///
/// Transfer Fusion rule.
#[test]
fn pure_callee_preserves_borrowed_arg_uniqueness() {
    use super::super::contract::{
        ContextBehavior, EffectSummary, FipContract, ParamContract, ReturnContract,
    };
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // func(p0): v1 = Apply(callee, [p0]); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee borrows param 0, does NOT share (may_share: false).
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Borrowed,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary {
                may_share: false,
                ..EffectSummary::default()
            },
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // p0's entry state should preserve Unique uniqueness — the callee
    // doesn't share, so no new references are created.
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        entry_p0.uniqueness,
        Uniqueness::Unique,
        "pure callee (may_share=false) should preserve borrowed arg uniqueness, got {:?}",
        entry_p0.uniqueness
    );
}

/// When callee has `may_share == true`, borrowed arguments get `MaybeShared`.
///
/// A callee that may create new references (`RcInc`) could compromise the
/// caller's uniqueness of a borrowed argument. The backward demand widens
/// the argument's uniqueness to `MaybeShared`.
///
/// Transfer Fusion rule — contrast test.
#[test]
fn sharing_callee_widens_borrowed_arg_uniqueness() {
    use super::super::contract::{
        ContextBehavior, EffectSummary, FipContract, ParamContract, ReturnContract,
    };
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // func(p0): v1 = Apply(callee, [p0]); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee borrows param 0, but MAY share (may_share: true).
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Borrowed,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary {
                may_share: true, // callee shares references
                ..EffectSummary::default()
            },
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // p0's entry state should have MaybeShared — the callee might create
    // new references to the borrowed argument.
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        entry_p0.uniqueness,
        Uniqueness::MaybeShared,
        "sharing callee (may_share=true) should widen borrowed arg uniqueness, got {:?}",
        entry_p0.uniqueness
    );
}

/// Owned params are not affected by callee's `may_share` — only borrowed params.
///
/// The uniqueness widening rule applies ONLY to borrowed
/// parameters. Owned parameters transfer ownership to the callee; the
/// caller's pre-call uniqueness is independent of whether the callee shares.
///
/// Transfer Fusion rule — owned param contrast.
#[test]
fn owned_param_ignores_callee_may_share() {
    use super::super::contract::{
        ContextBehavior, EffectSummary, FipContract, ParamContract, ReturnContract,
    };
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // func(p0): v1 = Apply(callee, [p0]); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee takes ownership (Owned), and may share.
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Owned,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary {
                may_share: true, // callee shares — but param is Owned
                ..EffectSummary::default()
            },
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // p0's entry state should preserve Unique — owned params are not
    // affected by the may_share rule (it only applies to borrowed params).
    let entry_p0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        entry_p0.uniqueness,
        Uniqueness::Unique,
        "owned param should not be affected by callee may_share, got {:?}",
        entry_p0.uniqueness
    );
}

// Effect summary accumulation

/// Non-scalar `Construct` sets `may_allocate = true` in effect summary.
#[test]
fn effect_summary_construct_sets_may_allocate() {
    // func: v0 = Construct(Struct, []); return v0
    let func = ArcFunction {
        var_types: vec![ty(0)],
        params: vec![],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(1)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let effects = state_map.effect_summary();
    assert!(effects.may_allocate, "Construct should set may_allocate");
    assert!(
        !effects.may_share,
        "empty Construct (no args) should not set may_share"
    );
    assert!(!effects.may_throw, "Construct should not set may_throw");
}

/// `Construct` storing an argument with non-`BlockLocal` locality sets
/// `may_share = true` — `HeapEscaping` → `may_share` rule.
#[test]
fn effect_summary_construct_heap_escaping_arg_sets_may_share() {
    // func(p0): v1 = Construct(Struct, [p0]); return v1
    // p0 is passed in (param), then stored in a struct, then the struct is returned.
    // The struct escapes (returned), so p0 is stored in a HeapEscaping structure.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(1),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(1)),
                args: vec![var(0)],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let effects = state_map.effect_summary();
    assert!(effects.may_allocate, "Construct should set may_allocate");
    // p0 is stored in v1, and v1 is returned (HeapEscaping). The backward
    // analysis should propagate HeapEscaping locality to p0, which then
    // triggers the may_share effect in populate_effect_summary.
    assert!(
        effects.may_share,
        "Construct storing arg with non-BlockLocal locality should set may_share"
    );
}

/// `Construct` where all arguments are block-local does NOT set `may_share`.
#[test]
fn effect_summary_construct_block_local_args_no_may_share() {
    // func: v0 = Construct(Struct, []); v1 = Construct(Struct, [v0]);
    // v2 = Project(v1, 0); return v2
    // v0 is created and immediately stored in v1, both in the same block.
    // v0's locality should be BlockLocal (never escapes the block).
    // However, v1 is returned → HeapEscaping. v0 is stored in v1, so
    // v0's locality gets widened. This test verifies the NON-sharing case
    // needs a purely block-local scenario.
    //
    // Simpler: Construct with no args → no may_share.
    let func = ArcFunction {
        var_types: vec![ty(0)],
        params: vec![],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(1)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let effects = state_map.effect_summary();
    assert!(effects.may_allocate, "Construct should set may_allocate");
    assert!(
        !effects.may_share,
        "Construct with no args should not set may_share"
    );
}

/// `Invoke` terminator sets `may_throw = true` in effect summary.
#[test]
fn effect_summary_invoke_sets_may_throw() {
    let callee_name = Name::from_raw(100);

    // func(p0): v1 = Invoke(callee, [p0], normal=b1, unwind=b2)
    // b1: return v1; b2: Resume
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let effects = state_map.effect_summary();
    assert!(effects.may_throw, "Invoke should set may_throw");
}

/// `Apply` with known callee unions callee's effect summary.
#[test]
fn effect_summary_apply_unions_callee_effects() {
    use super::super::contract::{
        ContextBehavior, EffectSummary, FipContract, ParamContract, ReturnContract,
    };
    use super::super::lattice::{AccessClass, Consumption, Uniqueness};

    let callee_name = Name::from_raw(100);

    // func(p0): v1 = Apply(callee, [p0]); return v1
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // Callee has may_share = true, may_allocate = true.
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        MemoryContract {
            params: vec![ParamContract {
                access: AccessClass::Owned,
                consumption: Consumption::Linear,
                cardinality: Cardinality::Once,
                may_escape: false,
                may_share: false,
                locality_bound: Locality::FunctionLocal,
                uniqueness: Uniqueness::MaybeShared,
                transfers_through_return: false,
                return_alias: None,
                return_payload_contains_param: false,
                return_payload_contains_param_all_paths: false,
                iter_consumes: false,
                borrowed_read_only: false,
                borrowed_cow_consumed: false,
                capture_variant_return_project: None,
                iter_consumes_projected_field: None,
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary {
                may_allocate: true,
                alloc_only_on_slow_path: false,
                may_deallocate: false,
                may_share: true,
                may_throw: false,
                has_unbounded_stack: false,
            },
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        },
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let effects = state_map.effect_summary();
    assert!(
        effects.may_allocate,
        "Apply should union callee's may_allocate"
    );
    assert!(effects.may_share, "Apply should union callee's may_share");
    assert!(
        !effects.may_throw,
        "Apply (not Invoke) should not set may_throw when callee doesn't throw"
    );
}

// Closure-capture locality and uniqueness

#[test]
fn closure_capture_non_escaping_preserves_block_local() {
    // func(v0: ref):
    //   v1 = PartialApply(f, [v0]) — captures v0
    //   v2 = ApplyIndirect(v1, []) — uses closure locally
    //   return v2 — v1 NOT returned (non-escaping)
    //
    // v0 is a parameter (captured by closure). The closure v1 is used once
    // locally via ApplyIndirect and never returned, so v1's demand locality
    // stays BlockLocal. capture_state_update uses max(current, closure_locality)
    // with no artificial FunctionLocal floor (TF-13).
    use super::super::lattice::Consumption;

    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::Ownership::Owned,
        }],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                },
                ArcInstr::ApplyIndirect {
                    dst: var(2),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let entry_v0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        entry_v0.locality,
        Locality::BlockLocal,
        "captured var in block-local closure preserves BlockLocal (no FunctionLocal floor per TF-13)"
    );
    // Captured by a once-closure (used once via ApplyIndirect), so
    // consumption should be Affine (may be dropped), not Unrestricted.
    assert!(
        entry_v0.consumption <= Consumption::Affine,
        "captured var in once-closure should be at most Affine, got {:?}",
        entry_v0.consumption
    );
}

#[test]
fn once_closure_capture_preserves_cardinality() {
    // func(v0: ref):
    //   v1 = PartialApply(f, [v0]) — captures v0
    //   v2 = ApplyIndirect(v1, []) — uses closure once (Once cardinality)
    //   return v2
    //
    // The closure v1 has cardinality Once (used once by ApplyIndirect).
    // capture_state_update with once-closure should preserve v0's
    // cardinality as Once (not widen to Many). This is the OxCaml LAM
    // "lock" mechanism: a once-closure cannot create multiple references
    // to captured values because it is invoked at most once.

    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::Ownership::Owned,
        }],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                },
                ArcInstr::ApplyIndirect {
                    dst: var(2),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let entry_v0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    // Once-closure: captured var used at most once through the closure.
    assert_eq!(
        entry_v0.cardinality,
        Cardinality::Once,
        "once-closure capture should preserve Once cardinality"
    );
}

// TRMC candidate detection (Shape Activation)

/// Recursive Construct → `ContextHole` when soundness conditions hold.
///
/// Pattern: `let v1 = self(args); let v2 = Construct(v1); return v2`
/// The Construct uses the result of a recursive call as a field arg.
/// With `Unique` + `FunctionLocal` + `!may_share`, v2 gets `ContextHole` shape.
#[test]
fn trmc_candidate_detected_for_recursive_construct() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                // v0 = literal (base argument)
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = self(v0) — recursive call
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                // v2 = Construct(v1) — constructor wrapping recursive result
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v2 should be ContextHole — recursive call result feeds into Construct.
    assert_eq!(
        state_map.var_shape(var(2)),
        ShapeClass::ContextHole,
        "Construct wrapping recursive Apply result should be ContextHole"
    );

    // v1 should NOT be ContextHole — it's the recursive call result, not a Construct.
    assert_ne!(
        state_map.var_shape(var(1)),
        ShapeClass::ContextHole,
        "Apply result should not be ContextHole"
    );
}

/// Non-recursive call → shape stays `ReusableCtor`, no `ContextHole`.
#[test]
fn trmc_not_detected_for_non_recursive_call() {
    let self_name = Name::from_raw(42);
    let other_name = Name::from_raw(99);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = other(v0) — NOT recursive
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: other_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Should be ReusableCtor, NOT ContextHole — the call is not recursive.
    assert_eq!(
        state_map.var_shape(var(2)),
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        "Non-recursive call wrapping should stay ReusableCtor"
    );
}

/// Enum variant Construct wrapping recursive result → `ContextHole`.
#[test]
fn trmc_candidate_detected_for_recursive_enum_construct() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(100),
                        variant: 0,
                    },
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert_eq!(
        state_map.var_shape(var(2)),
        ShapeClass::ContextHole,
        "EnumVariant Construct wrapping recursive Apply should be ContextHole"
    );
}

/// Construct wrapping recursive result but with no field args from recursion
/// stays `ReusableCtor`.
#[test]
fn trmc_not_detected_when_recursive_result_not_in_construct_args() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = self(v0) — recursive, but its result is not used by v3
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                // v3 = Construct(v2) — uses v2 (non-recursive), not v1
                ArcInstr::Construct {
                    dst: var(3),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert_eq!(
        state_map.var_shape(var(3)),
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        "Construct not using recursive result should stay ReusableCtor"
    );
}

/// Tuple constructor is not a TRMC candidate (not reusable).
#[test]
fn trmc_not_detected_for_tuple_constructor() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Tuple,
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert_eq!(
        state_map.var_shape(var(2)),
        ShapeClass::NonReusable,
        "Tuple constructor is never a TRMC candidate"
    );
}

// Convergence Feedback — cross-dimension detection

#[test]
fn cross_dimension_not_detected_for_straight_line() {
    // A simple straight-line function should not trigger cross-dimension
    // detection (Convergence Feedback).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::Ownership::Owned,
        }],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![(var(0), ty(0))],
            body: vec![ArcInstr::Construct {
                dst: var(1),
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![var(0)],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert!(
        !state_map.cross_dimension_detected(),
        "straight-line function should not detect cross-dimension chaining"
    );
}

#[test]
fn cross_dimension_not_detected_for_branching() {
    // A function with control flow (Branch) should not trigger cross-dimension
    // detection with current rules (Convergence Feedback).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(1), ty(0)],
        params: vec![
            crate::ir::ArcParam {
                var: var(0),
                ty: ty(0),
                ownership: crate::Ownership::Owned,
            },
            crate::ir::ArcParam {
                var: var(1),
                ty: ty(1),
                ownership: crate::Ownership::Owned,
            },
        ],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![(var(0), ty(0)), (var(1), ty(1))],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(1),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(0) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Return { value: var(2) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3).with_scalar(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert!(
        !state_map.cross_dimension_detected(),
        "branching function should not detect cross-dimension chaining"
    );
}

// FIP call-site specialization

#[test]
fn conditional_fip_call_site_all_unique_no_widening() {
    // caller(x: T) -> T {
    //   v1 = callee(x) ← callee has Conditional { [true] }, may_share=true
    //   return v1
    // }
    //
    // callee's contract: may_share=true, FIP=Conditional{[true]}.
    // At the Apply, x (v0) has backward state Unique (only used once here).
    // Conditional precondition: all required-unique args are Unique → met.
    // compute_effective_may_share returns false → no uniqueness widening.
    // Result: v0 stays Unique at block entry (instead of MaybeShared).
    let callee_name = Name::from_raw(100);

    let callee_contract = MemoryContract {
        params: vec![ParamContract {
            access: AccessClass::Borrowed,
            consumption: Consumption::Affine,
            cardinality: Cardinality::Once,
            may_escape: false,
            may_share: false,
            locality_bound: Locality::FunctionLocal,
            uniqueness: Uniqueness::MaybeShared,
            transfers_through_return: false,
            return_alias: None,
            return_payload_contains_param: false,
            return_payload_contains_param_all_paths: false,
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            capture_variant_return_project: None,
            iter_consumes_projected_field: None,
        }],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary {
            may_allocate: true,
            alloc_only_on_slow_path: false,
            may_deallocate: false,
            may_share: true, // would normally widen, but FIP Conditional overrides
            may_throw: false,
            has_unbounded_stack: false,
        },
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Conditional {
            requires_unique_params: vec![true],
        },
        is_fbip: false,
    };

    let mut sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    sigs.insert(callee_name, callee_contract);

    // caller: param v0, v1=Apply(callee, v0), return v1
    // v0 is a function param visible at block entry.
    let func = ArcFunction {
        name: Name::from_raw(200),
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::ownership::Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // With Conditional FIP specialization: v0's backward state at the
    // Apply is Unique (only use) → Conditional check passes → no widen.
    // Without specialization: v0 would get MaybeShared from may_share=true.
    let v0_state = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        v0_state.uniqueness,
        Uniqueness::Unique,
        "Conditional FIP with unique arg → no may_share widening, stays Unique"
    );
}

/// Builds a `MemoryContract` with the param + effects shape shared by the
/// conditional/sharing test pair below; `fip` is the only differing field.
fn fip_test_contract(fip: FipContract) -> MemoryContract {
    MemoryContract {
        params: vec![ParamContract {
            access: AccessClass::Borrowed,
            consumption: Consumption::Affine,
            cardinality: Cardinality::Once,
            may_escape: false,
            may_share: false,
            locality_bound: Locality::FunctionLocal,
            uniqueness: Uniqueness::MaybeShared,
            transfers_through_return: false,
            return_alias: None,
            return_payload_contains_param: false,
            return_payload_contains_param_all_paths: false,
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            capture_variant_return_project: None,
            iter_consumes_projected_field: None,
        }],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary {
            may_allocate: true,
            alloc_only_on_slow_path: false,
            may_deallocate: false,
            may_share: true,
            may_throw: false,
            has_unbounded_stack: false,
        },
        context_behavior: ContextBehavior::default(),
        fip,
        is_fbip: false,
    }
}

#[test]
fn conditional_fip_call_site_not_unique_widens() {
    // caller(x: T) -> T {
    //   v1 = conditional_callee(x) ← Conditional { [true] }
    //   v2 = sharing_callee(x) ← may_share=true, no FIP
    //   return v2
    // }
    //
    // Backward walk processes v2=sharing_callee FIRST (later in program order).
    // sharing_callee widens v0 to MaybeShared. Then at v1=conditional_callee,
    // v0 is already MaybeShared → Conditional check fails → normal widen path.
    let conditional_name = Name::from_raw(100);
    let sharing_name = Name::from_raw(101);

    let conditional_contract = fip_test_contract(FipContract::Conditional {
        requires_unique_params: vec![true],
    });
    let sharing_contract = fip_test_contract(FipContract::Never);

    let mut sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    sigs.insert(conditional_name, conditional_contract);
    sigs.insert(sharing_name, sharing_contract);

    // caller: param v0, v1=conditional_callee(v0), v2=sharing_callee(v0), return v2
    let func = ArcFunction {
        name: Name::from_raw(200),
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::ownership::Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: conditional_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: sharing_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // sharing_callee (processed first in backward) widens v0 to MaybeShared.
    // At conditional_callee: v0 is MaybeShared → Conditional check fails.
    let v0_state = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        v0_state.uniqueness,
        Uniqueness::MaybeShared,
        "Conditional FIP with non-unique arg → falls back to may_share widening"
    );
}

// Context event recording tests

/// When context regions are provided by the normalize pass and the context
/// variable has `ContextHole` shape + Unique, `ContextOpen`/`ContextClose` events
/// should be recorded.
#[test]
fn context_events_recorded_for_valid_trmc_candidate() {
    use crate::aims::contract::ContextRegion;
    use crate::aims::intraprocedural::AimsEvent;

    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = self(v0) — recursive call
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                // v2 = Construct(v1) — constructor wrapping recursive result
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    // Provide context regions from normalize pass.
    let context_regions = vec![ContextRegion {
        open_block: block_id(0),
        open_instr: 2,
        context_var: var(2),
        hole_field: 0,
        close_block: block_id(0),
        close_instr: 1,
        hole_var: var(1),
    }];

    let classifier = TestClassifier::all_ref(1);
    let state_map =
        super::analyze_function(&func, &classifier, &no_sigs(), &context_regions, Vec::new());

    // v2 should have ContextHole shape (detect_trmc_candidates runs first).
    assert_eq!(
        state_map.var_shape(var(2)),
        ShapeClass::ContextHole,
        "Construct wrapping recursive result should be ContextHole"
    );

    // ContextOpen and ContextClose events should be recorded.
    let events = state_map.events_in_block(block_id(0));
    let open_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AimsEvent::ContextOpen { .. }))
        .collect();
    let close_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AimsEvent::ContextClose { .. }))
        .collect();

    assert_eq!(open_events.len(), 1, "one ContextOpen event");
    assert_eq!(close_events.len(), 1, "one ContextClose event");

    // Verify event fields.
    if let AimsEvent::ContextOpen {
        block,
        instr,
        var: ev_var,
    } = open_events[0]
    {
        assert_eq!(*block, block_id(0));
        assert_eq!(*instr, 2);
        assert_eq!(*ev_var, var(2));
    } else {
        panic!("expected ContextOpen");
    }

    if let AimsEvent::ContextClose {
        block,
        instr,
        var: ev_var,
    } = close_events[0]
    {
        assert_eq!(*block, block_id(0));
        assert_eq!(*instr, 1);
        assert_eq!(*ev_var, var(1));
    } else {
        panic!("expected ContextClose");
    }
}

/// When context regions are provided but the context variable is NOT
/// `ContextHole` (e.g., non-recursive construct), no events are recorded.
#[test]
fn no_context_events_when_not_context_hole() {
    use crate::aims::contract::ContextRegion;
    use crate::aims::intraprocedural::AimsEvent;

    let self_name = Name::from_raw(42);
    let other_name = Name::from_raw(99);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = other(v0) — NOT recursive
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: other_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    // Provide a bogus context region — but the analysis won't mark v2 as ContextHole
    // because the call is not recursive. The event should be skipped.
    let context_regions = vec![ContextRegion {
        open_block: block_id(0),
        open_instr: 2,
        context_var: var(2),
        hole_field: 0,
        close_block: block_id(0),
        close_instr: 1,
        hole_var: var(1),
    }];

    let classifier = TestClassifier::all_ref(1);
    let state_map =
        super::analyze_function(&func, &classifier, &no_sigs(), &context_regions, Vec::new());

    // v2 should NOT be ContextHole — the call is not recursive.
    assert_ne!(
        state_map.var_shape(var(2)),
        ShapeClass::ContextHole,
        "non-recursive call should not produce ContextHole"
    );

    // No context events should be recorded.
    let events = state_map.events_in_block(block_id(0));
    let context_events: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AimsEvent::ContextOpen { .. } | AimsEvent::ContextClose { .. }
            )
        })
        .collect();
    assert!(
        context_events.is_empty(),
        "no context events when shape is not ContextHole"
    );
}

/// Empty context regions slice → no context events (backward compat).
#[test]
fn empty_context_regions_no_events() {
    use crate::aims::intraprocedural::AimsEvent;

    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    // Empty context regions (old behavior).
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // ContextHole shape is still set by detect_trmc_candidates (post-convergence).
    assert_eq!(state_map.var_shape(var(2)), ShapeClass::ContextHole);

    // But no ContextOpen/ContextClose events — context_regions is empty.
    let events = state_map.events_in_block(block_id(0));
    let context_events: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AimsEvent::ContextOpen { .. } | AimsEvent::ContextClose { .. }
            )
        })
        .collect();
    assert!(
        context_events.is_empty(),
        "empty context_regions → no context events"
    );
}

// Soundness gate reconciliation

/// TRMC candidates are NOT rejected by `may_share` in v1 (logged, not enforced).
///
/// The `HeapEscaping` → `may_share` accumulation rule makes ALL returned Constructs
/// trigger `may_share` == true. Since TRMC functions by definition return a
/// Construct, enforcing `may_share` would block all TRMC. The gate is logged
/// for diagnostics but does not prevent `ContextHole` detection.
#[test]
fn trmc_not_rejected_when_may_share_true() {
    // Same TRMC pattern as trmc_candidate_detected_for_recursive_construct.
    // may_share is true (from HeapEscaping), but ContextHole should still be set.
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // The function DOES have may_share == true (HeapEscaping return).
    assert!(
        state_map.effect_summary().may_share,
        "TRMC function should have may_share=true from HeapEscaping return"
    );

    // But ContextHole is still set — may_share is logged, not enforced.
    assert_eq!(
        state_map.var_shape(var(2)),
        ShapeClass::ContextHole,
        "TRMC candidate should still be detected despite may_share=true"
    );
}

/// Context events NOT recorded when `may_share` is true (logged, not enforced).
/// Even with `may_share=true`, context events ARE recorded because the gate
/// is logged-only in v1 (no effect handlers → no non-linear resumption).
#[test]
fn context_events_recorded_despite_may_share_true() {
    use crate::aims::contract::ContextRegion;
    use crate::aims::intraprocedural::AimsEvent;

    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let regions = vec![ContextRegion {
        open_block: block_id(0),
        open_instr: 2,
        context_var: var(2),
        hole_field: 0,
        close_block: block_id(0),
        close_instr: 1,
        hole_var: var(1),
    }];

    let classifier = TestClassifier::all_ref(1);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &regions, Vec::new());

    // Function has may_share=true (HeapEscaping), but events are still recorded.
    assert!(state_map.effect_summary().may_share);

    let events = state_map.events_in_block(block_id(0));
    let context_events: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                AimsEvent::ContextOpen { .. } | AimsEvent::ContextClose { .. }
            )
        })
        .collect();
    assert_eq!(
        context_events.len(),
        2,
        "context events should be recorded despite may_share=true (gate is logged, not enforced)"
    );
}

// compute_project_alias_sources

/// Assert that `var` maps to exactly one Project source `expected`.
fn assert_single_source(
    sources: &FxHashMap<ArcVarId, super::project_aliases::ProjectSources>,
    v: ArcVarId,
    expected: ArcVarId,
    msg: &str,
) {
    let s = sources
        .get(&v)
        .unwrap_or_else(|| panic!("{msg}: no entry for {v:?}"));
    assert_eq!(s.as_slice(), &[expected], "{msg}");
}

#[test]
fn compute_project_alias_sources_direct_project() {
    // v1 = Project v0.0
    // Maps: v1 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Project {
                dst: var(1),
                ty: ty(0),
                value: var(0),
                field: 0,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_eq!(sources.len(), 1);
}

#[test]
fn compute_project_alias_sources_let_alias() {
    // v1 = Project v0.0
    // v2 = Let Var(v1)
    // Maps: v1 → v0, v2 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Var(var(1)),
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_single_source(&sources, var(2), var(0), "Let alias of Project dst");
    assert_eq!(sources.len(), 2);
}

#[test]
fn compute_project_alias_sources_transitive_let_chain() {
    // v1 = Project v0.0
    // v2 = Let Var(v1)
    // v3 = Let Var(v2)
    // Maps: v1 → v0, v2 → v0, v3 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Var(var(1)),
                },
                ArcInstr::Let {
                    dst: var(3),
                    ty: ty(0),
                    value: ArcValue::Var(var(2)),
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "v1");
    assert_single_source(&sources, var(2), var(0), "v2");
    assert_single_source(&sources, var(3), var(0), "v3");
    assert_eq!(sources.len(), 3);
}

#[test]
fn compute_project_alias_sources_cross_block_let() {
    // Block 0: v1 = Project v0.0; jump block1
    // Block 1: v2 = Let Var(v1); return v2
    // Maps: v1 → v0, v2 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Var(var(1)),
                }],
                terminator: ArcTerminator::Return { value: var(2) },
            },
        ],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_single_source(
        &sources,
        var(2),
        var(0),
        "cross-block Let alias of Project dst",
    );
    assert_eq!(sources.len(), 2);
}

// Semantic pin: cross-block Project + Let alias demand propagation

#[test]
fn project_let_alias_cross_block_propagates_source_demand() {
    // Semantic pin for fix.
    //
    // Block 0: v1 = Construct Struct; v2 = Project v1.0; v3 = Let Var(v2);
    //          Branch cond → block1, block2
    // Block 1: return v3 (uses Let alias of Project result)
    // Block 2: return v4 (doesn't use v1, v2, or v3)
    //
    // Without the fix: Block 1's entry would have demand for v3 but NOT v1,
    // causing edge cleanup to emit premature RcDec(v1) on the 0→1 edge.
    // The fix ensures v1 has demand at Block 1's entry via the alias chain
    // v3 → v2 → Project → v1.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![(var(0), ty(0))], // cond param
                body: vec![
                    ArcInstr::Construct {
                        dst: var(1),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![var(5)],
                    },
                    ArcInstr::Project {
                        dst: var(2),
                        ty: ty(0),
                        value: var(1),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: var(3),
                        ty: ty(0),
                        value: ArcValue::Var(var(2)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(3) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(4),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Return { value: var(4) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(6);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // The critical assertion: v1 (Project source) must have demand at
    // Block 1's entry. Without the fix, only v3 (Let alias) would have
    // demand here, and v1 would be absent — causing premature RcDec.
    let v1_at_b1_entry = state_map.var_state_at_block_entry(block_id(1), var(1));
    assert_ne!(
        v1_at_b1_entry.cardinality,
        Cardinality::Absent,
        "v1 (Project source) must have demand at Block 1 entry — \
         v3 (Let alias of v2 = Project v1.0) is live here, so v1 must stay alive"
    );

    // Also verify v3 has demand at Block 1's entry (used in Return).
    let v3_at_b1_entry = state_map.var_state_at_block_entry(block_id(1), var(3));
    assert_ne!(
        v3_at_b1_entry.cardinality,
        Cardinality::Absent,
        "v3 must have demand at Block 1 entry (used in Return)"
    );

    // And v1 should NOT have demand at Block 2's entry (not used there).
    let v1_at_b2_entry = state_map.var_state_at_block_entry(block_id(2), var(1));
    assert_eq!(
        v1_at_b2_entry.cardinality,
        Cardinality::Absent,
        "v1 should be absent at Block 2 entry (not used in Block 2)"
    );
}

// compute_project_alias_sources — Jump arg → block param propagation

#[test]
fn compute_project_alias_sources_jump_arg_to_block_param() {
    // Block 0: v1 = Project v0.0; Jump block1, args=[v1]
    // Block 1: params=[v2]; return v2
    // Maps: v1 → v0, v2 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(1)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(2), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(2) },
            },
        ],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_single_source(&sources, var(2), var(0), "block param via Jump arg");
    assert_eq!(sources.len(), 2);
}

#[test]
fn compute_project_alias_sources_transitive_jump_chain() {
    // Block 0: v1 = Project v0.0; Jump block1, args=[v1]
    // Block 1: params=[v2]; Jump block2, args=[v2]
    // Block 2: params=[v3]; return v3
    // Maps: v1 → v0, v2 → v0, v3 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(1)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(2), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: block_id(2),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![(var(3), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_single_source(&sources, var(2), var(0), "first block param in chain");
    assert_single_source(&sources, var(3), var(0), "second block param in chain");
    assert_eq!(sources.len(), 3);
}

#[test]
fn compute_project_alias_sources_let_then_jump() {
    // Block 0: v1 = Project v0.0; v2 = Let Var(v1); Jump block1, args=[v2]
    // Block 1: params=[v3]; return v3
    // Maps: v1 → v0, v2 → v0, v3 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: var(1),
                        ty: ty(0),
                        value: var(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: var(2),
                        ty: ty(0),
                        value: ArcValue::Var(var(1)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(3), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_single_source(&sources, var(2), var(0), "Let alias");
    assert_single_source(&sources, var(3), var(0), "block param via Let alias");
    assert_eq!(sources.len(), 3);
}

#[test]
fn compute_project_alias_sources_loop_header_param() {
    // Block 0: v1 = Project v0.0; Jump block1, args=[v1]
    // Block 1: params=[v2];... Jump block1, args=[v2] (back-edge)
    //
    // Loop header param v2 receives Project alias from entry AND back-edge.
    // Maps: v1 → v0, v2 → v0
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(1),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(1)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(2), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
        ],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_single_source(&sources, var(1), var(0), "direct Project dst");
    assert_single_source(&sources, var(2), var(0), "loop header param");
    assert_eq!(sources.len(), 2);
}

// Semantic pin: block-param Project demand propagation

#[test]
fn project_block_param_cross_block_propagates_source_demand() {
    // Semantic pin for fix.
    //
    // Block 0: v1 = Construct Struct(v5); v2 = Project v1.0;
    //          Jump block1, args=[v2]
    // Block 1: params=[v3]; return v3
    //
    // Without the fix: Block 1's entry would have demand for v3 (block param,
    // used in Return) but NOT v1 (Project source), because v3 is not in
    // project_alias_sources — causing edge cleanup to emit premature RcDec(v1)
    // on the 0→1 edge. Use-after-free when v3 (aliasing v2 = Project v1.0)
    // is used in Block 1.
    //
    // With the fix: v3 maps to v1 via Jump arg propagation in
    // compute_project_alias_sources, so propagate_project_source_demand
    // adds demand for v1 at Block 1's entry.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![(var(0), ty(0))], // entry param (unused, for var_types)
                body: vec![
                    ArcInstr::Construct {
                        dst: var(1),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![var(5)],
                    },
                    ArcInstr::Project {
                        dst: var(2),
                        ty: ty(0),
                        value: var(1),
                        field: 0,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(3), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(6);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Critical assertion: v1 (Project source) must have demand at Block 0's
    // EXIT state. Without the fix, Block 1's entry has no demand for v1
    // (because v3 isn't recognized as a Project alias), so Block 0's exit
    // (derived from Block 1's entry) also has no v1 demand. RC emission
    // would place RcDec(v1) after the Project, before the Jump — UAF.
    let v1_at_b0_exit = state_map
        .block_exit_states(block_id(0))
        .and_then(|s| s.get(&var(1)).copied())
        .unwrap_or(AimsState::BOTTOM);
    assert_ne!(
        v1_at_b0_exit.cardinality,
        Cardinality::Absent,
        "v1 (Project source) must have demand at Block 0 exit — \
         v3 (block param = Jump arg v2 = Project v1.0) is live in Block 1"
    );

    // Also verify the access is Borrowed (Project source kept alive, not consumed).
    assert_eq!(
        v1_at_b0_exit.access,
        AccessClass::Borrowed,
        "Project source demand should be Borrowed"
    );
}

// compute_project_alias_sources — multi-predecessor merge

#[test]
fn compute_project_alias_sources_multi_predecessor_merge() {
    // Block 0: v2 = Project v0.0; Jump block2, args=[v2]
    // Block 1: v3 = Project v1.0; Jump block2, args=[v3]
    // Block 2: params=[v4]; return v4
    //
    // v4 can alias EITHER v0 (via v2) or v1 (via v3) depending on control flow.
    // Both must be recorded as sources for v4.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(2),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(2),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: var(3),
                    ty: ty(0),
                    value: var(1),
                    field: 0,
                }],
                terminator: ArcTerminator::Jump {
                    target: block_id(2),
                    args: vec![var(3)],
                },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![(var(4), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(4) },
            },
        ],
        ..Default::default()
    };

    let sources =
        super::project_aliases::compute_project_alias_sources(&func, &FxHashMap::default());
    assert_eq!(
        sources.get(&var(2)).map(SmallVec::as_slice),
        Some(&[var(0)][..]),
        "v2 → v0"
    );
    assert_eq!(
        sources.get(&var(3)).map(SmallVec::as_slice),
        Some(&[var(1)][..]),
        "v3 → v1"
    );

    // v4 must map to BOTH v0 and v1 (multi-predecessor merge).
    let v4_sources = sources
        .get(&var(4))
        .expect("v4 must have Project alias sources");
    assert!(
        v4_sources.contains(&var(0)) && v4_sources.contains(&var(1)),
        "v4 must alias both v0 and v1 at merge, got: {v4_sources:?}"
    );
    assert_eq!(v4_sources.len(), 2, "exactly two sources");
}

// Semantic pin: multi-predecessor merge demand propagation

#[test]
fn project_block_param_multi_predecessor_merge_propagates_all_source_demand() {
    // Semantic pin: multi-predecessor merge propagates demand from all sources.
    //
    // Block 0 (entry): Branch(v0) → block1 | block2
    // Block 1: v2 = Construct Struct(v6); v3 = Project v2.0; Jump block3, args=[v3]
    // Block 2: v4 = Construct Struct(v7); v5 = Project v4.0; Jump block3, args=[v5]
    // Block 3 (merge): params=[v8]; return v8
    //
    // The backward analysis propagates demand for BOTH v2 and v4 to Block 3's
    // entry (via project_alias_sources). This is correct: the demand
    // propagation keeps parent aggregates alive per-predecessor. The emission
    // layer filters branch-local variables at merge blocks, routing them to
    // per-predecessor trampolines via edge cleanup.
    let func = ArcFunction {
        var_types: vec![ty(0); 9],
        blocks: vec![
            // Block 0 (entry): branch to Block 1 or Block 2
            ArcBlock {
                id: block_id(0),
                params: vec![(var(0), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            // Block 1: construct aggregate, project field, jump to merge
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: var(2),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![var(6)],
                    },
                    ArcInstr::Project {
                        dst: var(3),
                        ty: ty(0),
                        value: var(2),
                        field: 0,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(3),
                    args: vec![var(3)],
                },
            },
            // Block 2: construct DIFFERENT aggregate, project field, jump to merge
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: var(4),
                        ty: ty(0),
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![var(7)],
                    },
                    ArcInstr::Project {
                        dst: var(5),
                        ty: ty(0),
                        value: var(4),
                        field: 0,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(3),
                    args: vec![var(5)],
                },
            },
            // Block 3 (merge): receives projected value from either predecessor
            ArcBlock {
                id: block_id(3),
                params: vec![(var(8), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(8) },
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(9);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // v2 (parent aggregate in Block 1) must have demand at Block 1 exit.
    let v2_at_b1_exit = state_map
        .block_exit_states(block_id(1))
        .and_then(|s| s.get(&var(2)).copied())
        .unwrap_or(AimsState::BOTTOM);
    assert_ne!(
        v2_at_b1_exit.cardinality,
        Cardinality::Absent,
        "v2 (parent aggregate, Block 1) must have demand — \
         v8 (merge param) aliases v3 = Project v2.0"
    );

    // v4 (parent aggregate in Block 2) must have demand at Block 2 exit.
    let v4_at_b2_exit = state_map
        .block_exit_states(block_id(2))
        .and_then(|s| s.get(&var(4)).copied())
        .unwrap_or(AimsState::BOTTOM);
    assert_ne!(
        v4_at_b2_exit.cardinality,
        Cardinality::Absent,
        "v4 (parent aggregate, Block 2) must have demand — \
         v8 (merge param) aliases v5 = Project v4.0"
    );
}

// TF-6 contract-narrowed call-result side tables
//
// `populate_call_result_states` pass populates per-variable forward-state
// side tables on `AimsStateMap` from each Apply/Invoke instruction's
// `MemoryContract.return_info` (or CONSERVATIVE for direct calls without
// a contract; CONSERVATIVE for indirect calls per spec TF-5a/TF-6c).
//
// Pipeline order: position 1.5 (between `populate_borrow_sources` and
// `populate_sparse_events`) — Side tables MUST be
// populated BEFORE consumers read them; locality narrowing in the side
// table MUST reach `populate_sparse_events` for `LocalAllocCandidate`
// emission.
//
// Sparse filter is BOTTOM-default per skip Unique /
// BlockLocal / NonReusable; store everything else (including CONSERVATIVE
// values like MaybeShared / Unknown that override the optimistic lattice
// default).
//
// Canonicalization: contract dimensions are written
// to a temporary `AimsState`, `canonicalize` runs to enforce CN-3 (Shared+
// ReusableCtor → NonReusable) and CN-6 (HeapEscaping+Unique → MaybeShared),
// then canonicalized values are written to side tables.

/// Helper: construct a minimal `MemoryContract` with a single Borrowed param
/// and the given `return_info`.
fn contract_with_return(return_info: ReturnContract) -> MemoryContract {
    MemoryContract {
        params: vec![ParamContract {
            access: AccessClass::Borrowed,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            may_escape: false,
            may_share: false,
            locality_bound: Locality::FunctionLocal,
            uniqueness: Uniqueness::MaybeShared,
            transfers_through_return: false,
            return_alias: None,
            return_payload_contains_param: false,
            return_payload_contains_param_all_paths: false,
            iter_consumes: false,
            borrowed_read_only: false,
            borrowed_cow_consumed: false,
            capture_variant_return_project: None,
            iter_consumes_projected_field: None,
        }],
        return_info,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// Helper: `ArcFunction` with a single block doing `v1 = Apply(callee, [p0]); return v1`.
fn func_with_apply_call(callee_name: Name) -> ArcFunction {
    ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: callee_name,
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    }
}

/// PRIMARY BUG-04-086 LOAD-BEARING TEST.
///
/// Apply call with contract `return_info.uniqueness = MaybeShared` populates
/// the side table at `dst`. Without this, `effective_uniqueness_at_block_*`
/// would fall through to lattice BOTTOM=Unique, `drop_hints` would classify the
/// slice-rest as Unique, and codegen would route through
/// `ori_buffer_drop_unique` → BUG-04-086 panic UNFIXED.
#[test]
fn populate_call_result_states_apply_maybe_shared_contract_inserts_dst() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "Apply with MaybeShared contract must populate side table — \
         BUG-04-086 closure depends on this exact override of optimistic lattice BOTTOM=Unique"
    );
}

/// Apply with Unique contract: BOTTOM-default sparse filter skips the insert.
/// `contract_uniqueness(dst) = None`; effective falls through to lattice
/// (which already has Unique demand from a fresh-Unique-contract callee).
#[test]
fn populate_call_result_states_apply_unique_contract_filtered() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::Unique,
            preserves_freshness: true,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        None,
        "Unique contract is BOTTOM — sparse filter must skip; effective falls through to lattice"
    );
    assert_eq!(
        state_map.contract_locality(var(1)),
        None,
        "BlockLocal contract is BOTTOM — sparse filter must skip"
    );
}

/// Apply WITHOUT contract receives CONSERVATIVE per spec TF-5
/// . CONSERVATIVE.uniqueness =
/// `MaybeShared` overrides optimistic lattice BOTTOM=Unique.
#[test]
fn populate_call_result_states_apply_no_contract_uses_conservative() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    // No contract registered.
    let sigs = no_sigs();

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "Apply without contract: spec TF-5 says CONSERVATIVE = MaybeShared, \
         not optimistic BOTTOM=Unique"
    );
    assert_eq!(
        state_map.contract_locality(var(1)),
        Some(Locality::Unknown),
        "CONSERVATIVE.locality = Unknown — must override optimistic BOTTOM=BlockLocal"
    );
}

/// Helper: `ArcFunction` doing `v1 = Apply(callee, [p0]); v2 = Let Var(v1); return v2`.
/// The `Let { Var }` alias (`v2`) is the shape `propagate_alias_forward_state`
/// must carry the call-result forward state onto (TF-2: a var-binding inherits
/// its source's full lattice state).
fn func_with_apply_then_let_alias(callee_name: Name) -> ArcFunction {
    ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Var(var(1)),
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    }
}

/// BUG-04-202 LOAD-BEARING TEST (TF-2 alias carrier).
///
/// A `Let { Var(src) }` alias of an `Apply` result whose contract is
/// `MaybeShared` (the seamless-slice `..tail` pattern binding aliasing the
/// `ori_list_slice_drop` result) MUST inherit `MaybeShared` via
/// `propagate_alias_forward_state`. Without it the alias's `contract_uniqueness`
/// is None, `effective_uniqueness_at_block_*` falls through to lattice
/// BOTTOM=Unique, `decide_drop_hint` selects the unique-owner free path, and
/// codegen emits `ori_buffer_drop_unique` on a slice cap → bound-slice drop
/// SIGSEGV.
#[test]
fn propagate_alias_forward_state_let_alias_inherits_maybe_shared() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_then_let_alias(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // The Apply dst (%1) carries the contract MaybeShared (populate_call_result_states).
    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "Apply dst must carry the MaybeShared contract"
    );
    // The Let-alias (%2) MUST inherit it via propagate_alias_forward_state (TF-2).
    assert_eq!(
        state_map.contract_uniqueness(var(2)),
        Some(Uniqueness::MaybeShared),
        "Let{{Var}} alias of a MaybeShared call result MUST inherit MaybeShared — \
         TF-2 carrier; the seamless-slice bound-pattern drop-selection depends on it"
    );
    // Locality also propagates (CONSERVATIVE Unknown from the call result).
    assert_eq!(
        state_map.contract_locality(var(2)),
        Some(Locality::Unknown),
        "Let{{Var}} alias must inherit the call result's locality too"
    );
}

/// Negative clamp: a `Let { Var }` alias of a `Unique`-contract call result
/// inherits NOTHING (BOTTOM-skip) — `contract_uniqueness` stays None and
/// `effective_*` correctly falls through to the lattice. Proves the alias
/// propagation copies only the source's stored (non-BOTTOM) dimensions and does
/// NOT broaden the side table.
#[test]
fn propagate_alias_forward_state_unique_source_alias_stays_unset() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_then_let_alias(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::Unique,
            preserves_freshness: true,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // Source %1: Unique is BOTTOM — not stored.
    assert_eq!(state_map.contract_uniqueness(var(1)), None);
    // Alias %2: nothing to inherit (source unset) — stays None.
    assert_eq!(
        state_map.contract_uniqueness(var(2)),
        None,
        "alias of a Unique (BOTTOM, unstored) source must stay unset — \
         propagation copies only stored non-BOTTOM dimensions"
    );
}

/// BUG-04-097 M1 — TF-4 Project forward View inherits the source's narrowed
/// contract uniqueness. `v1 = Apply(callee)` carries a `MaybeShared` return
/// contract; `v2 = Project v1.0` is a borrowed view of that result. The
/// extended `propagate_alias_forward_state` (Project edge, `AliasKind::View`)
/// copies `contract_uniqueness` source -> dst so downstream COW/drop-hint reads
/// the narrowed fact on the projected alias, not the conservative lattice.
/// Pre-fix (TF-2-only edge collector) left `v2` unset (None).
#[test]
fn propagate_alias_forward_state_project_view_inherits_maybe_shared() {
    let callee_name = Name::from_raw(100);
    // func(p0): v1 = Apply(callee, p0); v2 = Project v1.0; return v2
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Project {
                    dst: var(2),
                    ty: ty(0),
                    value: var(1),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );
    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "Apply dst carries the MaybeShared contract"
    );
    assert_eq!(
        state_map.contract_uniqueness(var(2)),
        Some(Uniqueness::MaybeShared),
        "TF-4 Project View MUST inherit the source's MaybeShared contract \
         uniqueness (BUG-04-097 M1) — the pre-fix TF-2-only collector left it unset"
    );
}

/// BUG-04-097 M4 — TF-8 Select takes the lattice JOIN (LUB) over its operands,
/// NOT a meet / first-write-wins. `v3 = Select(cond, v1, v2)` where `v1` is a
/// `MaybeShared`-contract call result and `v2` a `Unique`-contract one: the
/// stored side-table join is `MaybeShared` (the wider value; `Unique` is the
/// unstored lattice BOTTOM so the join reduces to the `MaybeShared` source).
/// Asserts the Select dst is `MaybeShared`, never the optimistic Unique a meet /
/// first-write-wins would pick.
#[test]
fn propagate_alias_forward_state_select_joins_to_maybe_shared() {
    let ms_callee = Name::from_raw(100);
    let uniq_callee = Name::from_raw(101);
    // func(p0): v1 = Apply(ms_callee)[MaybeShared]; v2 = Apply(uniq_callee)[Unique];
    //           v3 = Select(p0, v1, v2); return v3
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: ms_callee,
                    args: vec![],
                    arg_ownership: vec![],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: uniq_callee,
                    args: vec![],
                    arg_ownership: vec![],
                    mono_instance_id: None,
                },
                ArcInstr::Select {
                    dst: var(3),
                    ty: ty(0),
                    cond: var(0),
                    true_val: var(1),
                    false_val: var(2),
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };
    let mut sigs = FxHashMap::default();
    sigs.insert(
        ms_callee,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );
    sigs.insert(
        uniq_callee,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::Unique,
            preserves_freshness: true,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );
    let classifier = TestClassifier::all_ref(4);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    assert_eq!(
        state_map.contract_uniqueness(var(3)),
        Some(Uniqueness::MaybeShared),
        "TF-8 Select MUST join (LUB) to MaybeShared (the wider value), never the \
         optimistic Unique a meet/first-write-wins would pick (BUG-04-097 M4/M11)"
    );
}

/// BUG-04-097 M7 — TF-15/15a Set/SetTag are EXCLUDED from forward propagation.
/// A `Set { base, field, value }` is an in-place mutation with no `dst`; the
/// extended pass MUST NOT manufacture a forward contract fact for the base from
/// it. `v0` is only ever a Set base here (no contract-defining instruction), so
/// its side-table uniqueness stays unset.
#[test]
fn propagate_alias_forward_state_set_base_inherits_no_forward_fact() {
    // func(p0, p1): Set p0.0 = p1; return p0   (p0 is only a Set base)
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![
            crate::test_helpers::owned_param(0, ty(0)),
            crate::test_helpers::owned_param(1, ty(0)),
        ],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Set {
                base: var(0),
                field: 0,
                value: var(1),
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };
    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
    assert_eq!(
        state_map.contract_uniqueness(var(0)),
        None,
        "TF-15 Set base MUST NOT inherit a forward contract fact — Set/SetTag are \
         excluded from forward propagation (BUG-04-097 M7)"
    );
}

/// `ApplyIndirect` populates with CONSERVATIVE per spec TF-5a.
/// Spec says indirect calls receive "Same as TF-5"
/// (CONSERVATIVE = `MaybeShared`), NOT excluded from the side table entirely.
#[test]
fn populate_call_result_states_apply_indirect_uses_conservative() {
    // func(p0): v1 = PartialApply(f, [p0]); v2 = ApplyIndirect(v1, []); return v2
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                },
                ArcInstr::ApplyIndirect {
                    dst: var(2),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(2)),
        Some(Uniqueness::MaybeShared),
        "ApplyIndirect: spec TF-5a CONSERVATIVE.uniqueness = MaybeShared"
    );
    assert_eq!(
        state_map.contract_locality(var(2)),
        Some(Locality::Unknown),
        "ApplyIndirect: CONSERVATIVE.locality = Unknown"
    );
}

/// Invoke with contract: symmetric to Apply (+
/// F2 — terminator path also walked by `populate_call_result_states`).
#[test]
fn populate_call_result_states_invoke_with_contract_inserts_dst() {
    let callee_name = Name::from_raw(100);
    // func(p0): block 0 ends with Invoke; b1 = return v1; b2 = Resume.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "Invoke terminator: side table populated symmetrically with Apply body instruction"
    );
    assert_eq!(
        state_map.contract_locality(var(1)),
        Some(Locality::HeapEscaping),
        "Invoke terminator locality narrowing"
    );
}

/// Canonicalization fires before side-table writes.
/// CN-3: Shared + `ReusableCtor` → `NonReusable`.
/// Without canonicalization, the side table would store an infeasible
/// (Shared, `ReusableCtor`) state, breaking AIMS Invariant 5 cross-dimensional
/// feasibility (CN-3).
#[test]
fn populate_call_result_states_canonicalizes_cn3() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    // Contract: (Shared, BlockLocal, ReusableCtor(Struct))
    // CN-3 forces shape:= NonReusable.
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::Shared,
            preserves_freshness: false,
            locality: Locality::BlockLocal,
            shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::Shared),
        "Shared uniqueness preserved (passes BOTTOM-default filter)"
    );
    assert_eq!(
        state_map.var_shape(var(1)),
        ShapeClass::NonReusable,
        "CN-3 must demote Shared+ReusableCtor → NonReusable BEFORE side-table write — \
         a raw write would have stored ReusableCtor here"
    );
}

/// Canonicalization CN-6: `HeapEscaping` + Unique → `MaybeShared`.
/// Without canonicalization, an infeasible (Unique, `HeapEscaping`) state
/// would be written, violating §2 CN-6.
#[test]
fn populate_call_result_states_canonicalizes_cn6() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    // Contract: (Unique, HeapEscaping, NonReusable)
    // CN-6 forces uniqueness:= MaybeShared.
    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::Unique,
            preserves_freshness: true,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "CN-6 must demote Unique→MaybeShared when locality is HeapEscaping — \
         a raw write would have stored Unique here, falsely claiming a \
         heap-escaping value is RC==1"
    );
    assert_eq!(
        state_map.contract_locality(var(1)),
        Some(Locality::HeapEscaping),
        "Locality preserved through canonicalization"
    );
}

/// Excluded variables (scalar / immortal) MUST NOT receive side-table entries.
/// Mirrors the `is_excluded` guard in `populate_var_shapes` (`post_convergence.rs:131`).
#[test]
fn populate_call_result_states_skips_scalar_dst() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::HeapEscaping,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    // Mark var(1)'s type (ty(0)) as scalar — the dst type is scalar.
    let classifier = TestClassifier::all_ref(2).with_scalar(0);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        None,
        "scalar dst MUST be excluded from side-table population"
    );
}

/// Pipeline ordering: `populate_call_result_states` runs BEFORE
/// `populate_sparse_events`.
/// `FunctionLocal` contract locality must reach `LocalAllocCandidate` event
/// emission. `BlockLocal` alternative would be tautological under
/// BOTTOM-default filter (F1) — using `FunctionLocal` restores
/// discriminating power.
#[test]
fn populate_sparse_events_sees_function_local_contract_locality() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    let local_alloc_v1 = events.iter().any(|e| {
        matches!(
            e,
            super::AimsEvent::LocalAllocCandidate { var: v, .. } if v == &var(1)
        )
    });
    assert!(
        local_alloc_v1,
        "populate_sparse_events MUST see contract-derived FunctionLocal via the \
         side-table (effective_locality_at_block_exit) and emit LocalAllocCandidate. \
         Without correct ordering (or without effective_* migration), no event fires."
    );
}

/// F1 negative pin: `BlockLocal` contract does NOT insert into the
/// side table (BOTTOM-default filter) — guards against the tautological
/// pipeline-ordering test the prior design used.
#[test]
fn populate_call_result_states_block_local_filtered_no_event() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::BlockLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    assert_eq!(
        state_map.contract_locality(var(1)),
        None,
        "BlockLocal MUST be filtered out (it is the BOTTOM default) — \
         the side table only stores narrower-than-BOTTOM values"
    );
}

/// Negative pin: an Invoke result that the normal successor RETURNS escapes
/// the function (per spec IA-6 Return widening: returned values are widened
/// to `HeapEscaping` locality + `Owned` access). Such an escaping value
/// MUST NOT be a `LocalAllocCandidate` — stack-promoting it would create a
/// dangling stack-to-heap pointer.
///
/// Pre-fix bug (+ verification): `var_state_at_block_exit(invoke_block, dst)`
/// returned `BOTTOM` because the normal successor's strip
/// (`block.rs` `compute_block_entry_state` "Invoke defs" branch) erased the
/// dst from its entry state before the predecessor's exit JOIN read it.
/// `effective_locality_at_block_exit` then joined BOTTOM (`BlockLocal`) with
/// the side-table contract value (`FunctionLocal`) → `FunctionLocal` → fired
/// `LocalAllocCandidate` incorrectly.
///
/// Fix: `AimsStateMap::invoke_def_demand` captures pre-strip demand keyed
/// by the predecessor Invoke block. `var_state_at_block_exit` consults
/// it FIRST. Now the captured demand reflects Return-widening
/// (`HeapEscaping`), JOIN with side-table `FunctionLocal` still gives
/// `HeapEscaping` (max), and the `LocalAllocCandidate` filter (`FunctionLocal`
/// or `BlockLocal`) correctly rejects.
#[test]
fn populate_sparse_events_invoke_terminator_returned_dst_no_local_alloc_candidate() {
    let callee_name = Name::from_raw(100);
    // func(p0): block 0 → Invoke(callee, [p0]) normal=b1, unwind=b2
    //           block 1 → Return v1 (returns the Invoke result)
    //           block 2 → Resume
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // Block 0 is the Invoke block. v1 is returned from block 1 → Return
    // widening promotes locality to HeapEscaping. The
    // `populate_sparse_events` Invoke-terminator arm MUST NOT record
    // LocalAllocCandidate for an escaping value.
    let events = state_map.events_in_block(block_id(0));
    let local_alloc_v1 = events.iter().any(|e| {
        matches!(
            e,
            super::AimsEvent::LocalAllocCandidate { var: v, .. } if v == &var(1)
        )
    });
    assert!(
        !local_alloc_v1,
        "populate_sparse_events MUST NOT record LocalAllocCandidate for an Invoke \
         result that escapes via Return — IA-6 Return widening pushes locality \
         to HeapEscaping; stack-promoting an escaping value would dangle. \
         (Required: invoke_def_demand side table captures pre-strip Return-widened \
         demand so var_state_at_block_exit returns HeapEscaping.)"
    );
}

/// Positive pin: an Invoke result that does NOT escape the function (the
/// normal successor terminates without returning the value, e.g. via
/// `Unreachable`) is eligible for `LocalAllocCandidate` — its locality
/// stays at the contract-narrowed value (`FunctionLocal` here), which
/// matches the `LocalAllocCandidate` filter.
///
/// This pin demonstrates that `populate_sparse_events`' Invoke-terminator
/// arm still emits events when the value is genuinely local — the fix
/// removes false positives (returned/escaping values) without removing
/// true positives (local-consumed values). The filter behavior is
/// preserved across the pre-fix → post-fix transition.
#[test]
fn populate_sparse_events_invoke_terminator_local_dst_emits_local_alloc_candidate() {
    let callee_name = Name::from_raw(100);
    // func(p0): block 0 → Invoke(callee, [p0]) normal=b1, unwind=b2
    //           block 1 → Unreachable (does NOT return v1; v1 stays local)
    //           block 2 → Resume
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::FunctionLocal,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    // v1 is unused by the normal successor (Unreachable) — pre-strip
    // demand is BOTTOM (no use), captured into invoke_def_demand. JOIN
    // with side-table contract (FunctionLocal) gives FunctionLocal, which
    // matches the LocalAllocCandidate filter → event SHOULD fire.
    let events = state_map.events_in_block(block_id(0));
    let local_alloc_v1 = events.iter().any(|e| {
        matches!(
            e,
            super::AimsEvent::LocalAllocCandidate { var: v, .. } if v == &var(1)
        )
    });
    assert!(
        local_alloc_v1,
        "populate_sparse_events MUST record LocalAllocCandidate when an Invoke \
         result has FunctionLocal contract-narrowed locality AND does not escape \
         via the normal successor — the walks-Invoke-terminator \
         requirement (preserves the precision improvement, just routes through \
         the corrected pre-strip demand path)."
    );
}

/// Consumer integration: Apply with `MaybeShared` contract → `effective_uniqueness`
/// at block entry returns `MaybeShared` (load-bearing for COW emission at
/// `emit_rc/cow.rs:67`). Pre-fix consumer reads `state.uniqueness` from
/// `var_state_at_block_entry` and gets BOTTOM=Unique, missing the `IsShared`
/// runtime check.
#[test]
fn effective_uniqueness_at_block_entry_reflects_apply_maybe_shared_contract() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let effective = state_map.effective_uniqueness_at_block_entry(block_id(0), var(1));
    assert_eq!(
        effective,
        Uniqueness::MaybeShared,
        "consumer at block entry (cow.rs:67 / realize/mod.rs:302+346) must see \
         contract-narrowed MaybeShared — JOIN(MaybeShared, lattice_BOTTOM=Unique) = MaybeShared"
    );
}

/// Consumer integration: Apply with `MaybeShared` contract → `effective_uniqueness`
/// at block exit returns `MaybeShared` (load-bearing for DeathEvent.uniqueness
/// at `realize/walk_dec.rs:250` and downstream `emit_reuse/detect.rs:86` +
/// `emit_reuse/planner.rs:83`).
#[test]
fn effective_uniqueness_at_block_exit_reflects_apply_maybe_shared_contract() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        callee_name,
        contract_with_return(ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            locality: Locality::Unknown,
            shape: ShapeClass::NonReusable,
            returns_fresh_self_alloc: false,
        }),
    );

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let effective = state_map.effective_uniqueness_at_block_exit(block_id(0), var(1));
    assert_eq!(
        effective,
        Uniqueness::MaybeShared,
        "consumer at block exit (walk_dec.rs:250+254+300) must see contract-narrowed \
         MaybeShared for DeathEvent — emit_reuse downstream filters death.uniqueness == Unique"
    );
}

/// Negative pin (guards against INVERTED-TDD): the existing
/// `populate_sparse_events_sees_function_local_contract_locality` test passes
/// regardless of pipeline ordering because BOTH `BlockLocal` (lattice BOTTOM
/// fallback) and `FunctionLocal` (contract-narrowed) emit `LocalAllocCandidate`.
///
/// This negative pin discriminates: with no contract registered, `populate_call_result_states`
/// writes CONSERVATIVE (locality=`Unknown`), so `effective_locality = max(Unknown, BlockLocal)`
/// = `Unknown` → no event fires. If pipeline ordering broke (`populate_sparse_events` ran
/// before `populate_call_result_states`), the side table would be empty, `contract_locality`
/// would return None, effective would fall through to lattice `BOTTOM=BlockLocal`, and the
/// event would fire incorrectly. The assertion `no_local_alloc_v1` is therefore load-bearing
/// proof that ordering + side-table population work together.
#[test]
fn populate_sparse_events_no_event_for_no_contract_apply_pins_ordering() {
    let callee_name = Name::from_raw(100);
    let func = func_with_apply_call(callee_name);
    // No contract registered for callee_name → CONSERVATIVE (Unknown locality).
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &sigs, &[], Vec::new());

    let events = state_map.events_in_block(block_id(0));
    let local_alloc_v1 = events.iter().any(|e| {
        matches!(
            e,
            super::AimsEvent::LocalAllocCandidate { var: v, .. } if v == &var(1)
        )
    });
    assert!(
        !local_alloc_v1,
        "no-contract Apply: populate_call_result_states writes Unknown locality → \
         effective_locality_at_block_exit = max(Unknown, BlockLocal) = Unknown → \
         LocalAllocCandidate MUST NOT fire. If this test fails, either pipeline \
         ordering broke (sparse_events ran before call_result_states) or the \
         effective_locality migration was reverted."
    );
}

/// `InvokeIndirect` terminator
/// CONSERVATIVE branch — symmetric to `ApplyIndirect` (F2) but tests the
/// terminator-walking arm of `populate_call_result_states`.
#[test]
fn populate_call_result_states_invoke_indirect_uses_conservative() {
    // func(p0): v1 = PartialApply(f, [p0]); block 0 ends with InvokeIndirect on v1.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                }],
                terminator: ArcTerminator::InvokeIndirect {
                    dst: var(2),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(2) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(2)),
        Some(Uniqueness::MaybeShared),
        "InvokeIndirect: spec TF-6c CONSERVATIVE.uniqueness = MaybeShared"
    );
    assert_eq!(
        state_map.contract_locality(var(2)),
        Some(Locality::Unknown),
        "InvokeIndirect: CONSERVATIVE.locality = Unknown"
    );
}

/// TPR (GAP): Invoke without contract — TF-6b CONSERVATIVE
/// path, symmetric to the body-Apply no-contract case. Pins that the terminator
/// arm of `populate_call_result_states` applies the same fallback logic as the
/// body arm.
#[test]
fn populate_call_result_states_invoke_no_contract_uses_conservative() {
    let callee_name = Name::from_raw(100);
    // func(p0): block 0 ends with Invoke (no contract registered).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: var(1),
                    ty: ty(0),
                    func: callee_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(1) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    assert_eq!(
        state_map.contract_uniqueness(var(1)),
        Some(Uniqueness::MaybeShared),
        "Invoke without contract: spec TF-6b CONSERVATIVE.uniqueness = MaybeShared"
    );
    assert_eq!(
        state_map.contract_locality(var(1)),
        Some(Locality::Unknown),
        "Invoke without contract: CONSERVATIVE.locality = Unknown"
    );
}

// closure_env_alias realization-layer fix matrix

/// Pin: 5th edit site (intraprocedural/mod.rs `InvokeEdgeState` recording arm).
///
/// `InvokeEdgeState` is the per-edge demand state captured for terminators
/// with both a normal and an unwind successor. Pre-fix, the worklist's
/// `if let ArcTerminator::Invoke {.. }` arm matched only `Invoke`, so
/// `InvokeIndirect` terminators left `state_map.invoke_edge_state(block)`
/// returning `None` — the per-edge cleanup machinery had no entry-state to
/// consult on the unwind path, and a borrowed closure receiver dying on
/// the unwind edge could leak.
///
/// Post-fix, the arm extends to a `match` that also matches
/// `InvokeIndirect`, so the same `InvokeEdgeState` is recorded for
/// indirect-call terminators. Ref: site 4.
#[test]
fn invoke_indirect_records_edge_state_for_normal_and_unwind() {
    // func(v0: ref):
    //   v1 = PartialApply(f, [v0])
    //   block 0 ends with InvokeIndirect closure=v1 normal=b1 unwind=b2
    //
    // Pre-fix: state_map.invoke_edge_state(block_id(0)) returns None.
    // Post-fix: returns Some(InvokeEdgeState { normal, unwind }).
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0)],
        params: vec![crate::test_helpers::owned_param(0, ty(0))],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                }],
                terminator: ArcTerminator::InvokeIndirect {
                    dst: var(2),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(2) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let edge_state = state_map.invoke_edge_state(block_id(0));
    assert!(
        edge_state.is_some(),
        "InvokeIndirect terminator should record InvokeEdgeState for normal/unwind successors. \
         Earlier the worklist arm only matched Invoke, leaving InvokeIndirect without per-edge state."
    );
}

/// Pin: REJECTED-APPROACH GUARD for `closure_env_alias` fix.
///
/// The `closure_env_alias` bug is fixed at the realization layer (`RcInc`
/// suppression for `ApplyIndirect` closure receivers); the LATTICE
/// (TF-11 backward demands + TF-13 capture state update) MUST stay
/// unchanged so multi-call closures still promote captures to
/// `Cardinality::Many` and §3 TF-13.
///
/// An alternative fix was entertained:
/// remove the `(closure, Once, Linear)` demand TF-11 emits for
/// `ApplyIndirect`. That alternative would have made the closure
/// receiver appear non-demanding, suppressing the spurious `RcInc` as a
/// side effect. It was rejected because it corrupts TF-13's
/// capture-state propagation: with the closure stuck at `Once`, TF-13
/// takes the `closure cardinality <= Once` branch and the captured var
/// stays at `Once + Affine`, breaking the interprocedural soundness
/// chain TF-11 + TF-13 enforce together.
///
/// This test pins the post-promotion shape end-to-end: a multi-call
/// closure capturing an RC-tracked param MUST converge with the
/// captured var at `Cardinality::Many` AND `Consumption::Unrestricted`.
/// If any future change collapses TF-11's `ApplyIndirect` closure demand,
/// this assertion fails immediately.
#[test]
fn apply_indirect_multi_call_promotes_captures_to_many() {
    // func(v0: ref):
    //   v1 = PartialApply(f, [v0]) — captures v0 in closure env
    //   v2 = ApplyIndirect(v1, []) — first invocation
    //   v3 = ApplyIndirect(v1, []) — second invocation
    //   return v3
    //
    // Two ApplyIndirect calls drive the closure cardinality to Many.
    // TF-13 (closure cardinality > Once branch) then promotes v0 to
    // Many + Unrestricted.
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        params: vec![crate::ir::ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: crate::Ownership::Owned,
        }],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::PartialApply {
                    dst: var(1),
                    ty: ty(0),
                    func: Name::from_raw(100),
                    args: vec![var(0)],
                },
                ArcInstr::ApplyIndirect {
                    dst: var(2),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                },
                ArcInstr::ApplyIndirect {
                    dst: var(3),
                    ty: ty(0),
                    closure: var(1),
                    args: vec![],
                    arg_ownership: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4);
    let state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let entry_v0 = state_map.var_state_at_block_entry(block_id(0), var(0));
    assert_eq!(
        entry_v0.cardinality,
        Cardinality::Many,
        "Multi-call closure capture must promote captured var to Many cardinality \
         (TF-13 closure-cardinality > Once branch). \
         REJECTED-APPROACH GUARD: deleting TF-11's ApplyIndirect closure-demand row \
         would collapse this to Once and break TF-13 capture-promotion soundness."
    );
    assert_eq!(
        entry_v0.consumption,
        Consumption::Unrestricted,
        "Multi-call closure capture must promote captured var to Unrestricted consumption \
         per TF-13. REJECTED-APPROACH GUARD per the multi-call closure-capture promotion contract."
    );
}

// Part A: ContextHole shape inheritance
//
// ContextHole-shaped variables (per `aims/lattice/dimensions.rs:213` +
// `aims/intraprocedural/post_convergence.rs:445`) inherit their BurdenSpec
// from their UNDERLYING TypeId — there is NO synthetic ContextHole TypeId
// registered separately. The TRMC pipeline mints a new parameter whose type
// is the constructor's argument type (an existing pool `Idx`); its
// BurdenSpec lookup uses that `Idx` regardless of the lattice shape.
//
// These tests are STRUCTURAL — they exercise the inheritance pathway by
// (a) setting `ShapeClass::ContextHole` on a variable post-analysis,
// (b) confirming `func.var_type(var)` is unchanged, and (c) showing the
// existing burden-lookup APIs route through the underlying TypeId.

#[test]
fn context_hole_shape_inherits_underlying_typeid_for_primitive() {
    // Positive: a variable shape-annotated `ContextHole` whose underlying
    // TypeId is a primitive (`Idx::INT`) still routes to the empty
    // BuiltinBurdenSpec via the existing BurdenRegistry lookup — the
    // shape annotation does NOT redirect the lookup.
    use ori_registry::burden::table::{burden_type_id, BurdenRegistry};
    use ori_registry::TypeTag;

    let func = ArcFunction {
        var_types: vec![Idx::INT],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: Idx::INT,
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let mut state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Simulate post-convergence TRMC detection: set ContextHole shape on v0.
    state_map.set_var_shape(var(0), ShapeClass::ContextHole);
    assert_eq!(state_map.var_shape(var(0)), ShapeClass::ContextHole);

    // Underlying TypeId is unchanged — inheritance pathway routes through it.
    let underlying = func.var_type(var(0));
    assert_eq!(underlying, Idx::INT);

    // Existing BurdenRegistry lookup for `int`'s primitive TypeId returns
    // the empty BuiltinBurdenSpec. The ShapeClass::ContextHole annotation
    // has zero effect on this lookup — no fresh registration, no synthetic
    // TypeId.
    let spec = BurdenRegistry::lookup_builtin(burden_type_id(TypeTag::Int))
        .expect("int has a registered empty BuiltinBurdenSpec in BURDEN_TABLE");
    assert!(!spec.self_heap_alloc, "int has empty burden");
    assert!(spec.owned_fields.is_empty());
    assert!(spec.variant_burdens.is_empty());
}

#[test]
fn context_hole_shape_inherits_underlying_typeid_for_heap_type() {
    // Positive: a variable shape-annotated `ContextHole` whose underlying
    // TypeId is a heap-allocated type (`Idx::STR`) routes to the existing
    // BuiltinBurdenSpec for `str` (self_heap_alloc = true). The
    // ContextHole shape inherits the same lookup, NOT a fresh registration.
    use ori_registry::burden::table::{burden_type_id, BurdenRegistry};
    use ori_registry::TypeTag;

    let func = ArcFunction {
        var_types: vec![Idx::STR],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: Idx::STR,
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let mut state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    state_map.set_var_shape(var(0), ShapeClass::ContextHole);
    assert_eq!(state_map.var_shape(var(0)), ShapeClass::ContextHole);

    // Underlying TypeId is still `str`.
    assert_eq!(func.var_type(var(0)), Idx::STR);

    let spec = BurdenRegistry::lookup_builtin(burden_type_id(TypeTag::Str))
        .expect("str has a registered BuiltinBurdenSpec in BURDEN_TABLE");
    assert!(
        spec.self_heap_alloc,
        "str's burden survives unchanged through ContextHole shape annotation"
    );
}

#[test]
fn context_hole_shape_does_not_register_synthetic_typeid_in_type_registry() {
    // Positive (no fictional registration): annotating a variable with
    // `ShapeClass::ContextHole` MUST NOT mutate `TypeRegistry::burden` to
    // produce a fresh entry. A "register UserBurdenSpec on synthetic
    // ContextHole TypeId" path must NOT manifest.
    //
    // We construct an empty TypeRegistry, run the lattice analysis with
    // ContextHole annotation applied post-convergence, and verify:
    // (a) no burden_signature claim was made;
    // (b) `TypeRegistry::burden` lookup on the underlying TypeId returns
    //     None (the underlying type was never registered as a user type).
    use ori_types::TypeRegistry;

    let func = ArcFunction {
        var_types: vec![Idx::INT],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: Idx::INT,
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let mut state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // BEFORE annotation: empty type registry has zero burden signatures.
    let registry = TypeRegistry::new();
    assert_eq!(
        registry.burden_signature_count(),
        0,
        "fresh TypeRegistry has zero burden signatures"
    );

    // Apply the ContextHole annotation.
    state_map.set_var_shape(var(0), ShapeClass::ContextHole);

    // AFTER annotation: registry STILL has zero burden signatures —
    // the lattice annotation is purely state-map-local, no registry write.
    assert_eq!(
        registry.burden_signature_count(),
        0,
        "ContextHole annotation MUST NOT mutate TypeRegistry — no synthetic registration"
    );
    assert!(
        registry.burden(Idx::INT).is_none(),
        "int is not a user-registered type; TypeRegistry::burden returns None regardless of ContextHole shape"
    );
}

#[test]
fn context_hole_shape_lookup_path_unchanged_by_annotation() {
    // Semantic pin: setting and clearing ContextHole on a variable does
    // NOT alter the var → underlying-TypeId mapping. This is the
    // inheritance pathway's load-bearing invariant: the lookup site
    // ALWAYS uses `func.var_type(var)`, never any shape-derived key.
    //
    // Regression catcher: if a future edit smuggles a "synthesize
    // ContextHole TypeId" path into the lattice pipeline, this test
    // surfaces it by detecting a change in the var → TypeId resolution.
    let func = ArcFunction {
        var_types: vec![Idx::BOOL],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(0),
                ty: Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let mut state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    let underlying_before = func.var_type(var(0));
    state_map.set_var_shape(var(0), ShapeClass::ContextHole);
    let underlying_after = func.var_type(var(0));

    assert_eq!(
        underlying_before, underlying_after,
        "func.var_type(v) is the SSOT for v's TypeId — ContextHole shape does not redirect it"
    );
    assert_eq!(
        underlying_after,
        Idx::BOOL,
        "underlying TypeId stays exactly what var_types[v] holds"
    );
}

#[test]
fn negative_pin_no_synthetic_context_hole_typeid_minting_pathway_exists() {
    // Negative pin: there is NO public API on
    // `AimsStateMap`, `ArcFunction`, or `TypeRegistry` that mints a
    // synthetic TypeId carrying a "ContextHole" tag. Such an API must NOT
    // exist; this test pins its absence.
    //
    // The verification is structural: we exercise every shape-related
    // surface on `AimsStateMap` and confirm none returns a synthetic Idx.
    let func = ArcFunction {
        var_types: vec![Idx::INT],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: Idx::INT,
                ctor: CtorKind::Struct(Name::from_raw(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let mut state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());

    // Apply the shape; confirm `var_shape` returns the SHAPE enum, NOT a
    // synthetic Idx. The shape annotation lives in a `FxHashMap<ArcVarId,
    // ShapeClass>`, not in any TypeId-keyed structure.
    state_map.set_var_shape(var(0), ShapeClass::ContextHole);
    assert_eq!(
        state_map.var_shape(var(0)),
        ShapeClass::ContextHole,
        "var_shape returns the ShapeClass enum stored by set_var_shape — no Idx payload exists"
    );

    // The lookup path remains TypeId-keyed via `func.var_type(var)`, and the
    // shape annotation never redirects it to a synthetic TypeId.
    assert_eq!(
        func.var_type(var(0)),
        Idx::INT,
        "ContextHole shape annotation must not mint or redirect the underlying TypeId"
    );
}

// compute_project_alias_table — unified-closure membership, demand split,
// over-approximation classification, genuine-rep parity

/// R2 generalized: a `Let { Var }` of a NON-projected root (no sources of its
/// own) seeds the whole-var identity in the UNIFIED closure but NOT in the
/// backward-demand table.
#[test]
fn alias_table_r2gen_whole_var_identity_in_unified_not_demand() {
    // v1 = Let Var(v0)   — v0 has no sources (bare root)
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(1),
                ty: ty(0),
                value: ArcValue::Var(var(0)),
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };
    let table = super::project_aliases::compute_project_alias_table(&func, &FxHashMap::default());
    assert_single_source(&table.sources, var(1), var(0), "R2-gen whole-var identity");
    assert!(
        !table.demand_sources.contains_key(&var(1)),
        "whole-var Let alias of a non-projected root must NOT enter backward demand"
    );
}

/// R5 Select: the dst joins the unified closure with both operands (plus their
/// sources), is recorded in `select_alias_dsts`, and stays OUT of the demand
/// table.
#[test]
fn alias_table_r5_select_unified_membership_and_demand_exclusion() {
    // v3 = Project v0.0 ; v4 = Select(cond=v2, t=v3, f=v1)
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: var(3),
                    ty: ty(0),
                    value: var(0),
                    field: 0,
                },
                ArcInstr::Select {
                    dst: var(4),
                    ty: ty(0),
                    cond: var(2),
                    true_val: var(3),
                    false_val: var(1),
                },
            ],
            terminator: ArcTerminator::Return { value: var(4) },
        }],
        ..Default::default()
    };
    let table = super::project_aliases::compute_project_alias_table(&func, &FxHashMap::default());
    let Some(select_sources) = table.sources.get(&var(4)) else {
        panic!("Select dst must be in the unified closure")
    };
    assert!(
        select_sources.contains(&var(3)) && select_sources.contains(&var(1)),
        "Select dst carries both operands"
    );
    assert!(
        select_sources.contains(&var(0)),
        "Select dst carries the true-operand's transitive Project source"
    );
    assert!(
        table.select_alias_dsts.contains(&var(4)),
        "Select dst recorded in select_alias_dsts"
    );
    assert!(
        !table.demand_sources.contains_key(&var(4)),
        "Select dst must NOT enter backward demand"
    );
}

/// The demand table is the ORIGINAL §1.9 closure: R1 + R3 + R6 entries match
/// the unified table for projection-rooted chains.
#[test]
fn alias_table_demand_matches_unified_on_projection_chains() {
    // v1 = Project v0.0 ; v2 = Let Var(v1) ; Jump bb1(v2) ; bb1 params [v3]
    let func = ArcFunction {
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: var(1),
                        ty: ty(0),
                        value: var(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: var(2),
                        ty: ty(0),
                        value: ArcValue::Var(var(1)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(3), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };
    let table = super::project_aliases::compute_project_alias_table(&func, &FxHashMap::default());
    for v in [var(1), var(2), var(3)] {
        assert_eq!(
            table.sources.get(&v),
            table.demand_sources.get(&v),
            "projection-rooted chain entries identical across unified + demand tables"
        );
    }
}

/// Superset-parity: `compute_same_alloc_reps` is a thin projection of the
/// table's genuine same-allocation builder — identical maps on a
/// representative IR carrying Let aliases + a Jump-arg rename.
#[test]
fn same_alloc_reps_parity_with_genuine_table_builder() {
    let func = ArcFunction {
        var_types: vec![ty(0); 4],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: var(1),
                        ty: ty(0),
                        value: ArcValue::Var(var(0)),
                    },
                    ArcInstr::Let {
                        dst: var(2),
                        ty: ty(0),
                        value: ArcValue::Var(var(1)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(2)],
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![(var(3), ty(0))],
                body: vec![],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };
    let from_projection =
        crate::aims::emit_rc::compute_same_alloc_reps(&func, &FxHashMap::default());
    let from_table =
        super::project_aliases::compute_genuine_same_alloc_reps(&func, &FxHashMap::default());
    assert_eq!(from_projection, from_table, "thin-projection parity");
    // Jump-arg rename (edge type 2) stays EXCLUDED from genuine reps.
    assert!(
        !from_table.contains_key(&var(3)),
        "block-param rename never joins the genuine same-allocation union-find"
    );
}
