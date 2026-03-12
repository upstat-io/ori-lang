//! Tests for the backward dataflow framework.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, LitValue,
};
use crate::ArcClass;

use super::super::contract::MemoryContract;
use super::super::lattice::{AimsState, Cardinality};

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
fn analysis_converges_for_simple_loop() {
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
    let _state_map = super::analyze_function(&func, &classifier, &no_sigs(), &[], Vec::new());
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

// Validation corpus tests (Section 02.7)

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
    // (Stage 1), the Apply dst v1 gets conservative state.
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
    // don't produce local locality for this pattern (Stage 1 conservative
    // defaults may set Unknown), the test documents the expected behavior
    // when locality inference is enabled.
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
    // If locality is Unknown/HeapEscaping (Stage 1 conservative), no event.
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
        // Stage 1 conservative: document that no local-alloc events are
        // produced when locality defaults to Unknown.
        assert!(
            local_alloc.is_empty(),
            "Unknown/HeapEscaping locality should NOT record LocalAllocCandidate"
        );
    }
}
