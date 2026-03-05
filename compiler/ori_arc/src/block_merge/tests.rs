//! Unit tests for the block merge pass.

use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::ir::{
    ArcBlock, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, LitValue, PrimOp,
    RcStrategy,
};
use crate::test_helpers::{b, make_func, owned_param, v};
use crate::uniqueness::CowMode;

use super::{is_trivial_body, merge_blocks};

// ── Phase 2: Invoke Downgrade ───────────────────────────────────────

/// Trivial invoke (empty unwind + Resume, single-pred normal, no params)
/// should be downgraded to Apply + Jump.
#[test]
fn trivial_invoke_downgrades_to_apply_jump() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            // bb0: invoke callee(v0) normal→bb1 unwind→bb2
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            // bb1: return v1
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // bb2: resume (trivial unwind)
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    // After merge: bb0 should have Apply in body and the original bb1
    // return should be merged in. The trivial unwind block is removed.
    assert!(
        func.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { .. })),
        "expected Apply instruction in merged block"
    );
    // Should have fewer blocks (trivial unwind removed, normal merged).
    assert!(
        func.blocks.len() < 3,
        "expected fewer than 3 blocks after merge, got {}",
        func.blocks.len()
    );
}

/// Non-trivial invoke preserved: unwind block has `RcDec` cleanup.
#[test]
fn nontrivial_invoke_unwind_has_cleanup() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // Non-trivial unwind: has RcDec cleanup.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::FatPointer,
                }],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::STR, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    // The invoke should be preserved since unwind has cleanup.
    let has_invoke = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Invoke { .. }));
    assert!(
        has_invoke,
        "invoke with non-trivial unwind should be preserved"
    );
}

/// Non-trivial invoke preserved: unwind terminator is Jump (catch handler).
#[test]
fn nontrivial_invoke_unwind_is_jump() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // Unwind forwards to a catch handler (Jump, not Resume).
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    let has_invoke = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Invoke { .. }));
    assert!(has_invoke, "invoke with Jump unwind should be preserved");
}

/// Invoke preserved when normal block has params.
#[test]
fn invoke_with_normal_params_not_downgraded() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            // Normal block has params — downgrade would produce Jump{args:[]}
            // with arity mismatch.
            ArcBlock {
                id: b(1),
                params: vec![(v(2), Idx::INT)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(2) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    let has_invoke = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Invoke { .. }));
    assert!(
        has_invoke,
        "invoke should be preserved when normal has params"
    );
}

/// Invoke preserved when normal has multiple predecessors.
#[test]
fn invoke_with_multi_pred_normal_not_downgraded() {
    // bb0: branch(cond) → then=bb1, else=bb2
    // bb1: invoke callee(v0) → normal=bb3, unwind=bb4
    // bb2: jump → bb3
    // bb3: return v1 (two predecessors: bb1 via invoke normal, bb2 via jump)
    // bb4: resume (trivial unwind)
    //
    // bb3 has two predecessors (bb1 and bb2), so the invoke in bb1 should
    // NOT be downgraded even though the unwind block is trivial.
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            // bb0: branch to bb1 or bb2
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // bb1: invoke → normal=bb3, unwind=bb4
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(3),
                    unwind: b(4),
                },
            },
            // bb2: alternative path to bb3
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            // bb3: return (two predecessors: bb1 normal, bb2 jump)
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(2) },
            },
            // bb4: trivial unwind
            ArcBlock {
                id: b(4),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::BOOL, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    // The invoke in bb1 should be preserved because bb3 (normal) has two
    // predecessors (bb1 and bb2). All other downgrade criteria are met
    // (trivial unwind, no params on normal), but criterion 4 fails.
    let has_invoke = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Invoke { .. }));
    assert!(
        has_invoke,
        "invoke should be preserved when normal block has multiple predecessors"
    );
}

/// Invoke preserved when normal == unwind (degenerate IR).
#[test]
fn invoke_normal_equals_unwind_not_downgraded() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(1), // same block
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    let has_invoke = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Invoke { .. }));
    assert!(has_invoke, "invoke with normal==unwind should be preserved");
}

// ── Phase 3: Jump Chain Merge ───────────────────────────────────────

/// Single-predecessor Jump merge (no params).
#[test]
fn single_pred_jump_merge_no_params() {
    // bb0: let v0 = 42; jump → bb1
    // bb1: return v0
    let func = make_func(
        vec![],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1, "should merge into single block");
    assert!(
        matches!(func.blocks[0].terminator, ArcTerminator::Return { value } if value == v(0)),
        "merged block should return v0"
    );
    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "body should have 1 instruction"
    );
}

/// Jump merge with block params (no overlap) → direct Let bindings.
#[test]
fn jump_merge_with_params_no_overlap() {
    // bb0: let v0 = 1; let v1 = 2; jump(v0, v1) → bb1(v2, v3)
    // bb1(v2: int, v3: int): return v2
    let func = make_func(
        vec![],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: Idx::INT,
                        value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(crate::ir::LitValue::Int(2)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(0), v(1)],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![(v(2), Idx::INT), (v(3), Idx::INT)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1);
    // Body: 2 original lets + 2 param lets = 4 instructions.
    assert_eq!(func.blocks[0].body.len(), 4);
    // The two param Let bindings: v2 = Var(v0), v3 = Var(v1).
    assert!(matches!(
        &func.blocks[0].body[2],
        ArcInstr::Let { dst, value: ArcValue::Var(arg), .. } if *dst == v(2) && *arg == v(0)
    ));
    assert!(matches!(
        &func.blocks[0].body[3],
        ArcInstr::Let { dst, value: ArcValue::Var(arg), .. } if *dst == v(3) && *arg == v(1)
    ));
}

/// Jump merge with overlapping params (swap pattern) → temp-based parallel copy.
#[test]
fn jump_merge_with_overlapping_params_swap() {
    // bb0: jump(v1, v0) → bb1(v0, v1)  — swap pattern
    // bb1(v0: int, v1: int): return v0
    // Without temps, sequential `v0 = v1; v1 = v0` would clobber v0.
    let func = make_func(
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(1), v(0)], // swap: pass v1→p0, v0→p1
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![(v(0), Idx::INT), (v(1), Idx::INT)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1);
    // Should have 4 Let bindings (2 temps + 2 param copies).
    assert_eq!(func.blocks[0].body.len(), 4);

    // Verify the temp-based parallel copy pattern:
    // temp0 = Var(v1), temp1 = Var(v0), v0 = Var(temp0), v1 = Var(temp1)
    let body = &func.blocks[0].body;

    // First two: args → temps
    let ArcInstr::Let {
        dst: t0,
        value: ArcValue::Var(src0),
        ..
    } = &body[0]
    else {
        panic!("expected Let at body[0]");
    };
    assert_eq!(*src0, v(1), "first temp should read v1");

    let ArcInstr::Let {
        dst: t1,
        value: ArcValue::Var(src1),
        ..
    } = &body[1]
    else {
        panic!("expected Let at body[1]");
    };
    assert_eq!(*src1, v(0), "second temp should read v0");

    // Next two: temps → params
    let ArcInstr::Let {
        dst: p0,
        value: ArcValue::Var(from0),
        ..
    } = &body[2]
    else {
        panic!("expected Let at body[2]");
    };
    assert_eq!(*p0, v(0), "should write to v0");
    assert_eq!(*from0, *t0, "should read from first temp");

    let ArcInstr::Let {
        dst: p1,
        value: ArcValue::Var(from1),
        ..
    } = &body[3]
    else {
        panic!("expected Let at body[3]");
    };
    assert_eq!(*p1, v(1), "should write to v1");
    assert_eq!(*from1, *t1, "should read from second temp");
}

/// Self-loop block is not merged.
#[test]
fn self_loop_not_merged() {
    // bb0: jump → bb0 (infinite loop)
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(0),
                args: vec![],
            },
        }],
        vec![],
    );

    let mut func = func;
    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1, "self-loop block should remain");
    assert!(
        matches!(&func.blocks[0].terminator, ArcTerminator::Jump { target, .. } if *target == b(0)),
        "self-loop should be preserved"
    );
}

/// Transitive chain merge: A → B → C all merge into A.
#[test]
fn transitive_chain_merge() {
    // bb0 → bb1 → bb2 → return
    let func = make_func(
        vec![],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(2)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(2),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(3)),
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1, "3-block chain should merge into 1");
    assert_eq!(
        func.blocks[0].body.len(),
        3,
        "all 3 Let instructions should be in merged block"
    );
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Return { value } if value == v(2)
    ));
}

// ── Phase 1: Dead Block Compaction ──────────────────────────────────

/// Dead block compaction removes unreachable blocks and remaps IDs.
#[test]
fn dead_block_compaction() {
    // bb0 → bb2 (bb1 unreachable)
    let func = make_func(
        vec![],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(2),
                    args: vec![],
                },
            },
            // bb1: unreachable (no predecessor)
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    // bb1 removed, bb2 renumbered to bb1, then merged with bb0.
    assert_eq!(func.blocks.len(), 1);
}

/// Dead cycle: unreachable SCC (dead blocks targeting each other).
#[test]
fn dead_cycle_compaction() {
    // bb0: return v0 (entry)
    // bb1 → bb2 → bb1 (dead cycle, unreachable from entry)
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(2),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
        ],
        vec![Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1, "dead cycle should be removed");
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Return { .. }
    ));
}

// ── Span Consistency ────────────────────────────────────────────────

/// Span entries match instruction count after merge.
#[test]
fn spans_consistent_after_merge() {
    let func = make_func(
        vec![],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(2)),
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    for (i, block) in func.blocks.iter().enumerate() {
        assert_eq!(
            func.spans[i].len(),
            block.body.len(),
            "span count must match body length for block {i}"
        );
    }
}

// ── COW Annotation Preservation ─────────────────────────────────────

/// COW annotations survive body Apply merge.
#[test]
fn cow_annotations_preserved_after_body_apply_merge() {
    // bb0: jump → bb1
    // bb1: Apply (cow-annotated StaticUnique); return
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(200),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    // Set annotation: block 1, instr 0 = StaticUnique.
    func.cow_annotations.set(1, 0, CowMode::StaticUnique);

    merge_blocks(&mut func);

    // After merge: bb1 merged into bb0. The Apply (was at (1,0)) should
    // now be at (0, 0) since bb0 had empty body.
    assert_eq!(func.blocks.len(), 1);
    let mode = func.cow_annotations.get(0, 0);
    assert_eq!(
        mode,
        CowMode::StaticUnique,
        "annotation should survive merge at remapped coordinate"
    );
}

/// COW annotations survive invoke downgrade + merge sequence.
#[test]
fn cow_annotations_preserved_after_invoke_downgrade_and_merge() {
    // bb0: jump → bb1
    // bb1: invoke (cow-annotated at terminator position) → bb2, bb3
    // bb2: return
    // bb3: resume
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(5)),
                }],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(200),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(2),
                    unwind: b(3),
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    // Annotation on the Invoke terminator: (block=1, instr=body.len()=1).
    func.cow_annotations.set(1, 1, CowMode::StaticUnique);

    merge_blocks(&mut func);

    // After Phase 2: Invoke at bb1 → Apply at bb1.body[1] (index preserved).
    // After Phase 3: bb1 merged into bb0; bb2 merged in too.
    // bb1's body started at offset 0 in bb0 (bb0 was empty).
    // The Apply (originally at (1,1)) → should be at (0, 1).
    assert_eq!(func.blocks.len(), 1);

    // The annotation should be at (0, 1) — the Apply's position after merge.
    let mode = func.cow_annotations.get(0, 1);
    assert_eq!(
        mode,
        CowMode::StaticUnique,
        "annotation should survive invoke downgrade + merge"
    );
}

/// COW annotations for dead blocks are dropped by compaction.
#[test]
fn cow_annotations_compaction_remaps_and_drops() {
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(200),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // bb1: unreachable (dead unwind block)
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::INT,
                    func: Name::from_raw(201),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                }],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    // Annotations on both blocks.
    func.cow_annotations.set(0, 0, CowMode::StaticUnique);
    func.cow_annotations.set(1, 0, CowMode::StaticShared);

    merge_blocks(&mut func);

    // Dead block's annotation should be dropped.
    assert_eq!(func.blocks.len(), 1);
    assert_eq!(
        func.cow_annotations.get(0, 0),
        CowMode::StaticUnique,
        "surviving annotation should be preserved"
    );
    assert_eq!(
        func.cow_annotations.len(),
        1,
        "dead block annotation should be dropped"
    );
}

/// Auto FBIP preserved after merge.
#[test]
fn auto_fbip_preserved_after_merge() {
    use crate::fbip::is_auto_fbip;

    // Two blocks, each with a COW Apply annotated as StaticUnique.
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(200),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::INT,
                    func: Name::from_raw(201),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    func.cow_annotations.set(0, 0, CowMode::StaticUnique);
    func.cow_annotations.set(1, 0, CowMode::StaticUnique);

    assert!(is_auto_fbip(&func), "should be auto FBIP before merge");

    merge_blocks(&mut func);

    assert_eq!(func.blocks.len(), 1);
    assert_eq!(
        func.cow_annotations.len(),
        2,
        "both annotations should survive"
    );
    assert!(is_auto_fbip(&func), "should still be auto FBIP after merge");

    // Verify specific keys exist.
    assert_eq!(func.cow_annotations.get(0, 0), CowMode::StaticUnique);
    assert_eq!(func.cow_annotations.get(0, 1), CowMode::StaticUnique);
}

// ── Drop Hints Stability ────────────────────────────────────────────

/// Drop hints computed after merge have valid coordinates.
#[test]
fn drop_hints_valid_after_merge() {
    // Build a function with multiple blocks that will merge, then
    // verify drop_hints coordinates point to actual RcDec instructions.
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT],
    );

    merge_blocks(&mut func);

    // After merge, drop_hints should be empty (defensively cleared).
    assert!(
        func.drop_hints.is_empty(),
        "merge should clear stale drop hints"
    );

    // Now compute drop hints on the merged function — should not panic.
    let pool = Pool::new();
    func.drop_hints = crate::uniqueness::compute_drop_hints(&func, &pool);

    // Verify all hint coordinates are valid.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, _instr) in block.body.iter().enumerate() {
            if func.drop_hints.is_unique_drop(block_idx, instr_idx) {
                assert!(
                    matches!(block.body[instr_idx], ArcInstr::RcDec { .. }),
                    "drop hint at ({block_idx}, {instr_idx}) should point to RcDec"
                );
            }
        }
    }
}

// ── Full Pipeline: Invoke Downgrade + Merge ─────────────────────────

/// 3 sequential calls: invoke/normal/invoke/normal/invoke/normal → single block.
#[test]
fn three_sequential_calls_merge_to_single_block() {
    // Simulates: let a = f(x); let b = g(a); let c = h(b); return c
    // With invokes: bb0→bb1, bb2→bb3, bb4→bb5, plus unwinds bb6,bb7,bb8
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(100),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(4),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::INT,
                    func: Name::from_raw(101),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(2),
                    unwind: b(5),
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(3),
                    ty: Idx::INT,
                    func: Name::from_raw(102),
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(3),
                    unwind: b(6),
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(3) },
            },
            ArcBlock {
                id: b(4),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: b(5),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: b(6),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT],
    );

    let mut func = func;
    merge_blocks(&mut func);

    // All 3 invokes should downgrade to Apply, all blocks should merge.
    assert_eq!(
        func.blocks.len(),
        1,
        "3 sequential trivial invokes should merge to 1 block, got {}",
        func.blocks.len()
    );
    assert_eq!(
        func.blocks[0]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::Apply { .. }))
            .count(),
        3,
        "should have 3 Apply instructions"
    );
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Return { value } if value == v(3)
    ));
}

// ── Phase 3: Select Diamond Folding — is_trivial_body ───────────────

/// Empty body is trivial.
#[test]
fn trivial_body_empty() {
    assert!(is_trivial_body(&[]));
}

/// Single literal Let is trivial.
#[test]
fn trivial_body_single_literal() {
    assert!(is_trivial_body(&[ArcInstr::Let {
        dst: v(0),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(42)),
    }]));
}

/// Var referencing a pre-branch variable is trivial.
#[test]
fn trivial_body_var_pre_branch() {
    // v(10) is NOT defined in this body, so it's a pre-branch variable.
    assert!(is_trivial_body(&[ArcInstr::Let {
        dst: v(0),
        ty: Idx::INT,
        value: ArcValue::Var(v(10)),
    }]));
}

/// `PrimOp` is NOT trivial (even though it's a Let).
#[test]
fn trivial_body_primop_rejected() {
    assert!(!is_trivial_body(&[ArcInstr::Let {
        dst: v(0),
        ty: Idx::INT,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(ori_ir::BinaryOp::Add),
            args: vec![v(1), v(2)],
        },
    }]));
}

/// Chained var reference (b refs a, which is arm-local) is NOT trivial.
#[test]
fn trivial_body_chained_var_rejected() {
    assert!(!is_trivial_body(&[
        ArcInstr::Let {
            dst: v(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        },
        ArcInstr::Let {
            dst: v(1),
            ty: Idx::INT,
            value: ArcValue::Var(v(0)), // v(0) is defined in same body
        },
    ]));
}

/// Mixed instructions (Let + `RcInc`) are NOT trivial.
#[test]
fn trivial_body_mixed_instructions() {
    assert!(!is_trivial_body(&[
        ArcInstr::Let {
            dst: v(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        },
        ArcInstr::RcInc {
            var: v(0),
            count: 1,
            strategy: RcStrategy::FatPointer,
        },
    ]));
}

// ── Phase 3: Select Diamond Folding — Positive (fold occurs) ────────

/// Configuration for a 4-block diamond test pattern.
struct DiamondConfig {
    cond_var: u32,
    then_body: Vec<ArcInstr>,
    then_args: Vec<ArcVarId>,
    else_body: Vec<ArcInstr>,
    else_args: Vec<ArcVarId>,
    merge_params: Vec<(ArcVarId, Idx)>,
    merge_result: ArcVarId,
    var_count: usize,
}

/// Build a 4-block diamond pattern from a config.
///
/// `B0`: branch(cond) → `B1`, `B2` |
/// `B1` / `B2`: arm body + jump → `B3` |
/// `B3`: merge params + return
fn make_diamond(cfg: DiamondConfig) -> crate::ir::ArcFunction {
    let var_types = vec![Idx::INT; cfg.var_count];
    make_func(
        vec![owned_param(cfg.cond_var, Idx::BOOL)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(cfg.cond_var),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: cfg.then_body,
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: cfg.then_args,
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: cfg.else_body,
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: cfg.else_args,
                },
            },
            ArcBlock {
                id: b(3),
                params: cfg.merge_params,
                body: vec![],
                terminator: ArcTerminator::Return {
                    value: cfg.merge_result,
                },
            },
        ],
        var_types,
    )
}

/// Empty arm bodies with pre-branch variables as jump args → Select emitted.
#[test]
fn select_fold_trivial_if_else() {
    // v0 = cond, v1/v2 = pre-branch values
    // B0: branch(v0) → B1, B2
    // B1: jump(v1) → B3
    // B2: jump(v2) → B3
    // B3(v3): return v3
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(2)],
        merge_params: vec![(v(3), Idx::INT)],
        merge_result: v(3),
        var_count: 4,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(2, Idx::INT));

    merge_blocks(&mut func);

    // Should have Select in some block.
    let has_select = func
        .blocks
        .iter()
        .any(|bl| bl.body.iter().any(|i| matches!(i, ArcInstr::Select { .. })));
    assert!(has_select, "expected Select instruction after folding");

    // No Branch should remain.
    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(!has_branch, "expected no Branch after folding");
}

/// Both arms have literal Let bodies → Select emitted with correct values.
#[test]
fn select_fold_with_literal_arms() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![ArcInstr::Let {
            dst: v(1),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(10)),
        }],
        then_args: vec![v(1)],
        else_body: vec![ArcInstr::Let {
            dst: v(2),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(20)),
        }],
        else_args: vec![v(2)],
        merge_params: vec![(v(3), Idx::INT)],
        merge_result: v(3),
        var_count: 4,
    });

    merge_blocks(&mut func);

    let has_select = func
        .blocks
        .iter()
        .any(|bl| bl.body.iter().any(|i| matches!(i, ArcInstr::Select { .. })));
    assert!(has_select, "expected Select with literal arms");
}

/// Both arms have Var Let bodies → Select emitted.
#[test]
fn select_fold_with_var_arms() {
    // v0 = cond, v1/v2 = pre-branch params
    // then: let v3 = v1; jump(v3)
    // else: let v4 = v2; jump(v4)
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![ArcInstr::Let {
            dst: v(3),
            ty: Idx::INT,
            value: ArcValue::Var(v(1)),
        }],
        then_args: vec![v(3)],
        else_body: vec![ArcInstr::Let {
            dst: v(4),
            ty: Idx::INT,
            value: ArcValue::Var(v(2)),
        }],
        else_args: vec![v(4)],
        merge_params: vec![(v(5), Idx::INT)],
        merge_result: v(5),
        var_count: 6,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(2, Idx::INT));

    merge_blocks(&mut func);

    let has_select = func
        .blocks
        .iter()
        .any(|bl| bl.body.iter().any(|i| matches!(i, ArcInstr::Select { .. })));
    assert!(has_select, "expected Select with var arms");
}

/// Multiple merge params → multiple Selects emitted.
#[test]
fn select_fold_with_multiple_merge_params() {
    // Both arms pass two values to merge block.
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![],
        then_args: vec![v(1), v(2)],
        else_body: vec![],
        else_args: vec![v(3), v(4)],
        merge_params: vec![(v(5), Idx::INT), (v(6), Idx::INT)],
        merge_result: v(5),
        var_count: 7,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(2, Idx::INT));
    func.params.push(owned_param(3, Idx::INT));
    func.params.push(owned_param(4, Idx::INT));

    merge_blocks(&mut func);

    let select_count: usize = func
        .blocks
        .iter()
        .map(|bl| {
            bl.body
                .iter()
                .filter(|i| matches!(i, ArcInstr::Select { .. }))
                .count()
        })
        .sum();
    assert_eq!(select_count, 2, "expected 2 Select instructions");
}

/// Both arms define the same `ArcVarId` with different values → fresh-renamed.
#[test]
fn select_fold_same_var_different_defs() {
    // Both arms define v(1), but with different literals.
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![ArcInstr::Let {
            dst: v(1),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(10)),
        }],
        then_args: vec![v(1)],
        else_body: vec![ArcInstr::Let {
            dst: v(1),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(20)),
        }],
        else_args: vec![v(1)],
        merge_params: vec![(v(2), Idx::INT)],
        merge_result: v(2),
        var_count: 3,
    });

    merge_blocks(&mut func);

    // Should have a Select (the two values are different literals).
    let has_select = func
        .blocks
        .iter()
        .any(|bl| bl.body.iter().any(|i| matches!(i, ArcInstr::Select { .. })));
    assert!(
        has_select,
        "expected Select when same var has different defs"
    );

    // Find the Select and verify its true_val != false_val (fresh-renamed).
    for bl in &func.blocks {
        for instr in &bl.body {
            if let ArcInstr::Select {
                true_val,
                false_val,
                ..
            } = instr
            {
                assert_ne!(true_val, false_val, "fresh-renamed vars should be distinct");
            }
        }
    }
}

/// After full `merge_blocks()`, dead arm blocks are removed (block count drops).
#[test]
fn select_fold_dead_blocks_compacted() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(2)],
        merge_params: vec![(v(3), Idx::INT)],
        merge_result: v(3),
        var_count: 4,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(2, Idx::INT));

    assert_eq!(func.blocks.len(), 4, "should start with 4 blocks");

    merge_blocks(&mut func);

    assert!(
        func.blocks.len() < 4,
        "expected fewer than 4 blocks after merge, got {}",
        func.blocks.len()
    );
}

/// Zero merge params → Branch becomes bare Jump, zero Selects emitted.
#[test]
fn select_fold_zero_merge_params() {
    // B3 has no params, both arms jump with no args.
    let mut func = make_func(
        vec![owned_param(0, Idx::BOOL)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(0),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::BOOL, Idx::INT],
    );

    merge_blocks(&mut func);

    // No Select should exist.
    let select_count: usize = func
        .blocks
        .iter()
        .map(|bl| {
            bl.body
                .iter()
                .filter(|i| matches!(i, ArcInstr::Select { .. }))
                .count()
        })
        .sum();
    assert_eq!(
        select_count, 0,
        "expected zero Selects with zero merge params"
    );

    // No Branch should remain.
    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(!has_branch, "expected no Branch after folding");
}

/// Both arms jump with the same variable → Let { Var } emitted, not Select.
#[test]
fn select_fold_identical_args_passthrough() {
    // Both arms pass v(1) to merge.
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(1)],
        merge_params: vec![(v(2), Idx::INT)],
        merge_result: v(2),
        var_count: 3,
    });
    func.params.push(owned_param(1, Idx::INT));

    merge_blocks(&mut func);

    // No Select — should use Let { Var } passthrough.
    let select_count: usize = func
        .blocks
        .iter()
        .map(|bl| {
            bl.body
                .iter()
                .filter(|i| matches!(i, ArcInstr::Select { .. }))
                .count()
        })
        .sum();
    assert_eq!(
        select_count, 0,
        "expected zero Selects for identical args (should use Var passthrough)"
    );

    // A Let { Var } should exist for the passthrough.
    let has_var_let = func.blocks.iter().any(|bl| {
        bl.body.iter().any(|i| {
            matches!(
                i,
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
            )
        })
    });
    assert!(has_var_let, "expected Let {{ Var }} passthrough");
}

/// After `merge_blocks()`, branch and merge blocks are merged into one
/// (validates Phase 3 → Phase 4 interaction end-to-end).
#[test]
fn select_fold_then_phase4_merges() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(2)],
        merge_params: vec![(v(3), Idx::INT)],
        merge_result: v(3),
        var_count: 4,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(2, Idx::INT));

    merge_blocks(&mut func);

    // Phase 3 folds diamond → B0 gets Select + Jump to B3.
    // Phase 3b compacts → B3 renumbered.
    // Phase 4 merges B0 + B3 (single predecessor) → 1 block.
    assert_eq!(
        func.blocks.len(),
        1,
        "expected 1 block after select fold + phase 4 merge, got {}",
        func.blocks.len()
    );

    // The single block should have a Select and a Return.
    let has_select = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Select { .. }));
    assert!(has_select, "merged block should contain Select");

    assert!(
        matches!(func.blocks[0].terminator, ArcTerminator::Return { .. }),
        "merged block should have Return terminator"
    );
}

/// Branch block with existing COW annotations preserves them after folding.
#[test]
fn select_fold_preserves_branch_block_cow_annotations() {
    let mut func = make_func(
        vec![
            owned_param(0, Idx::BOOL),
            owned_param(1, Idx::INT),
            owned_param(2, Idx::INT),
        ],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::INT,
                    func: Name::from_raw(200),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(0),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![v(1)],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![v(2)],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![(v(4), Idx::INT)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(4) },
            },
        ],
        vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT, Idx::INT],
    );

    // Set annotation on branch block's existing Apply at (0, 0).
    func.cow_annotations.set(0, 0, CowMode::StaticUnique);

    merge_blocks(&mut func);

    // After merge, the Apply should still have its annotation at (0, 0).
    assert_eq!(
        func.cow_annotations.get(0, 0),
        CowMode::StaticUnique,
        "branch block COW annotation should survive select folding"
    );
}

// ── Phase 3: Select Diamond Folding — Negative (fold does NOT occur) ─

/// Arm has Apply instruction → Branch preserved.
#[test]
fn select_not_folded_with_apply() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::INT,
            func: Name::from_raw(100),
            args: vec![v(0)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(2)],
        merge_params: vec![(v(3), Idx::INT)],
        merge_result: v(3),
        var_count: 4,
    });
    func.params.push(owned_param(2, Idx::INT));

    // Don't use merge_blocks — that includes Phase 4 which may eliminate
    // the Branch by other means. Test the select fold specifically.
    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(has_branch, "Branch should be preserved when arm has Apply");
}

/// Arm has `RcInc` → Branch preserved (most common negative case).
#[test]
fn select_not_folded_with_rc_ops() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![ArcInstr::RcInc {
            var: v(1),
            count: 1,
            strategy: RcStrategy::FatPointer,
        }],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(2)],
        merge_params: vec![(v(3), Idx::INT)],
        merge_result: v(3),
        var_count: 4,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(2, Idx::INT));

    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(has_branch, "Branch should be preserved when arm has RcInc");
}

/// Arm has `Let { PrimOp }` → Branch preserved (IS a `Let`, but not `Literal`/`Var`).
#[test]
fn select_not_folded_with_primop() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![ArcInstr::Let {
            dst: v(1),
            ty: Idx::INT,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                args: vec![v(2), v(3)],
            },
        }],
        then_args: vec![v(1)],
        else_body: vec![],
        else_args: vec![v(4)],
        merge_params: vec![(v(5), Idx::INT)],
        merge_result: v(5),
        var_count: 6,
    });
    func.params.push(owned_param(2, Idx::INT));
    func.params.push(owned_param(3, Idx::INT));
    func.params.push(owned_param(4, Idx::INT));

    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(has_branch, "Branch should be preserved when arm has PrimOp");
}

/// Then block has 2 predecessors → Branch preserved.
#[test]
fn select_not_folded_multi_predecessor() {
    // B0: branch → B1, B2
    // B4: jump → B1 (gives B1 two predecessors)
    // B1: jump(v1) → B3
    // B2: jump(v2) → B3
    // B3(v3): return v3
    let mut func = make_func(
        vec![
            owned_param(0, Idx::BOOL),
            owned_param(1, Idx::INT),
            owned_param(2, Idx::INT),
        ],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(0),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![v(1)],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![v(2)],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![(v(3), Idx::INT)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(3) },
            },
            // Extra block jumping to B1, giving it 2 predecessors.
            ArcBlock {
                id: b(4),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
        ],
        vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT],
    );

    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(
        has_branch,
        "Branch should be preserved when then block has multiple predecessors"
    );
}

/// Arms jump to different merge blocks → Branch preserved.
#[test]
fn select_not_folded_mismatched_merge_targets() {
    // B1 → B3, B2 → B4 (different targets).
    let mut func = make_func(
        vec![
            owned_param(0, Idx::BOOL),
            owned_param(1, Idx::INT),
            owned_param(2, Idx::INT),
        ],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(0),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(4),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(4),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::BOOL, Idx::INT, Idx::INT],
    );

    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(
        has_branch,
        "Branch should be preserved when arms jump to different targets"
    );
}

/// Then is trivial, else has Apply → Branch preserved (per-arm check).
#[test]
fn select_not_folded_one_arm_nontrivial() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![],
        then_args: vec![v(1)],
        else_body: vec![ArcInstr::Apply {
            dst: v(2),
            ty: Idx::INT,
            func: Name::from_raw(100),
            args: vec![v(3)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        else_args: vec![v(2)],
        merge_params: vec![(v(4), Idx::INT)],
        merge_result: v(4),
        var_count: 5,
    });
    func.params.push(owned_param(1, Idx::INT));
    func.params.push(owned_param(3, Idx::INT));

    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(
        has_branch,
        "Branch should be preserved when one arm is non-trivial"
    );
}

/// Arm has chained Let { Var } → Branch preserved.
#[test]
fn select_not_folded_chained_let() {
    let mut func = make_diamond(DiamondConfig {
        cond_var: 0,
        then_body: vec![
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: Idx::INT,
                value: ArcValue::Var(v(1)), // v(1) is arm-local
            },
        ],
        then_args: vec![v(2)],
        else_body: vec![],
        else_args: vec![v(3)],
        merge_params: vec![(v(4), Idx::INT)],
        merge_result: v(4),
        var_count: 5,
    });
    func.params.push(owned_param(3, Idx::INT));

    super::fold_select_diamonds(&mut func);

    let has_branch = func
        .blocks
        .iter()
        .any(|bl| matches!(bl.terminator, ArcTerminator::Branch { .. }));
    assert!(
        has_branch,
        "Branch should be preserved when arm has chained Let"
    );
}
