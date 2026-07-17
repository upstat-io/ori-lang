//! Unit tests for `add_invoke_unwind_cleanup()` — `InvokeIndirect` handling.
//!
//! Tests verify that `InvokeIndirect` terminators get iterator drop cleanup on
//! both propagating and catch-transfer unwind blocks.

use ori_ir::StringInterner;
use ori_types::Idx;

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership};
use crate::test_helpers::{
    invoke_indirect_no_args, jump_without_args, make_apply, make_block, make_func_named,
};

/// Semantic pin: `InvokeIndirect` with Resume unwind and live iterator
/// should get `ori_iter_drop` inserted — would fail if `InvokeIndirect`
/// handling is removed.
#[test]
fn invoke_indirect_resume_inserts_iter_drop() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let iter_name = interner.intern("iter");

    // Block 0: create iterator via Apply @iter
    // Block 1: InvokeIndirect terminator, normal->2, unwind->3
    // Block 2: return
    // Block 3: Resume (empty — should get iter drop inserted)
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: iter_name,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(1),
                args: vec![],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::InvokeIndirect {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                closure: ArcVarId::new(2),
                args: vec![],
                arg_ownership: vec![],
                normal: ArcBlockId::new(2),
                unwind: ArcBlockId::new(3),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(3),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    // Block 3 (Resume) should now have an ori_iter_drop instruction
    let unwind_block = &func.blocks[3];
    assert!(
        !unwind_block.body.is_empty(),
        "unwind block should have iter drop inserted"
    );

    // Verify it's an Apply to ori_iter_drop
    let iter_drop_name = interner.intern("ori_iter_drop");
    if let ArcInstr::Apply {
        func: f,
        args,
        arg_ownership,
        ..
    } = &unwind_block.body[0]
    {
        assert_eq!(*f, iter_drop_name, "should call ori_iter_drop");
        assert_eq!(args.len(), 1, "ori_iter_drop takes one arg");
        // `ori_iter_drop` consumes the iterator handle.
        // The ownership contract must match
        // `ProtocolBuiltin::IterDrop.arg_ownership()` which is `Owned`.
        // A `Borrowed` marker here would be a shadow source contradicting
        // the SSOT.
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "ori_iter_drop arg should be Owned (consumes the iterator handle)"
        );
    } else {
        panic!("expected Apply instruction for iter drop");
    }
}

/// A catch-transfer unwind block must release a live iterator before jumping
/// to the enclosing catch handler.
#[test]
fn invoke_indirect_catch_jump_inserts_iter_drop() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let iter_name = interner.intern("iter");

    let blocks = vec![
        make_block(
            ArcBlockId::new(0),
            vec![make_apply(
                ArcVarId::new(1),
                Idx::NONE,
                iter_name,
                vec![ArcVarId::new(0)],
                vec![ArgOwnership::Borrowed],
            )],
            jump_without_args(ArcBlockId::new(1)),
        ),
        make_block(
            ArcBlockId::new(1),
            Vec::new(),
            invoke_indirect_no_args(
                ArcVarId::new(3),
                Idx::INT,
                ArcVarId::new(2),
                ArcBlockId::new(2),
                ArcBlockId::new(3),
            ),
        ),
        make_block(
            ArcBlockId::new(2),
            Vec::new(),
            ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        ),
        make_block(
            ArcBlockId::new(3),
            Vec::new(),
            jump_without_args(ArcBlockId::new(4)),
        ),
        make_block(ArcBlockId::new(4), Vec::new(), ArcTerminator::Unreachable),
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    let iter_drop_name = interner.intern("ori_iter_drop");
    assert!(
        matches!(
            &func.blocks[3].body[..],
            [ArcInstr::Apply {
                func,
                args,
                arg_ownership,
                ..
            }] if *func == iter_drop_name
                && args == &[ArcVarId::new(1)]
                && arg_ownership == &[ArgOwnership::Owned]
        ),
        "catch-transfer unwind block should consume the live iterator"
    );
}

/// A shared normal/unwind destination is not an unwind cleanup edge.
#[test]
fn invoke_indirect_shared_normal_unwind_no_cleanup() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let iter_name = interner.intern("iter");

    let blocks = vec![
        make_block(
            ArcBlockId::new(0),
            vec![make_apply(
                ArcVarId::new(1),
                Idx::NONE,
                iter_name,
                vec![ArcVarId::new(0)],
                vec![ArgOwnership::Borrowed],
            )],
            jump_without_args(ArcBlockId::new(1)),
        ),
        make_block(
            ArcBlockId::new(1),
            Vec::new(),
            invoke_indirect_no_args(
                ArcVarId::new(3),
                Idx::INT,
                ArcVarId::new(2),
                ArcBlockId::new(2),
                ArcBlockId::new(2),
            ),
        ),
        make_block(
            ArcBlockId::new(2),
            Vec::new(),
            ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        ),
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    assert!(
        func.blocks[2].body.is_empty(),
        "shared normal/unwind destination should not get unwind cleanup"
    );
}

/// A live for-yield scratch list must be freed on a catch-transfer unwind
/// before its normal-path `ori_list_take` can consume it.
#[test]
fn invoke_catch_jump_frees_live_yield_scratch() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let list_new = interner.intern("ori_list_new");
    let list_take = interner.intern("ori_list_take");

    let blocks = vec![
        make_block(
            ArcBlockId::new(0),
            vec![make_apply(
                ArcVarId::new(2),
                Idx::INT,
                list_new,
                vec![ArcVarId::new(0), ArcVarId::new(1)],
                vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
            )],
            jump_without_args(ArcBlockId::new(1)),
        ),
        make_block(
            ArcBlockId::new(1),
            Vec::new(),
            invoke_indirect_no_args(
                ArcVarId::new(3),
                Idx::INT,
                ArcVarId::new(4),
                ArcBlockId::new(2),
                ArcBlockId::new(3),
            ),
        ),
        make_block(
            ArcBlockId::new(2),
            vec![make_apply(
                ArcVarId::new(5),
                Idx::INT,
                list_take,
                vec![ArcVarId::new(2)],
                vec![ArgOwnership::Owned],
            )],
            ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        ),
        make_block(
            ArcBlockId::new(3),
            Vec::new(),
            jump_without_args(ArcBlockId::new(4)),
        ),
        make_block(ArcBlockId::new(4), Vec::new(), ArcTerminator::Unreachable),
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 6]);
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    let list_free = interner.intern("ori_list_free");
    assert!(
        matches!(
            &func.blocks[3].body[..],
            [ArcInstr::Apply {
                func,
                args,
                arg_ownership,
                ..
            }] if *func == list_free
                && args == &[ArcVarId::new(2), ArcVarId::new(1)]
                && arg_ownership == &[ArgOwnership::Owned, ArgOwnership::Borrowed]
        ),
        "catch-transfer unwind block should free the live yield scratch exactly once"
    );
}

/// Inline checked operations need their own landing block: the shared catch
/// handler is also reached by ordinary branches from Invoke cleanup blocks and
/// therefore cannot itself contain an LLVM landingpad.
#[test]
fn checked_op_retargets_to_cleanup_landing_block() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let list_new = interner.intern("ori_list_new");

    let blocks = vec![
        make_block(
            ArcBlockId::new(0),
            vec![
                make_apply(
                    ArcVarId::new(2),
                    Idx::INT,
                    list_new,
                    vec![ArcVarId::new(0), ArcVarId::new(1)],
                    vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                ),
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    value: ArcValue::PrimOp {
                        op: crate::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                },
            ],
            ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        ),
        make_block(ArcBlockId::new(1), Vec::new(), ArcTerminator::Unreachable),
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
    func.catch_scoped_checked_ops = vec![(ArcVarId::new(3), ArcBlockId::new(1))];
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    let landing = func.catch_scoped_checked_ops[0].1;
    assert_ne!(
        landing,
        ArcBlockId::new(1),
        "checked op must not unwind directly to the shared catch handler"
    );
    assert!(
        matches!(
            func.blocks[landing.index()].terminator,
            ArcTerminator::Jump { target, ref args }
                if target == ArcBlockId::new(1) && args.is_empty()
        ),
        "checked-op landing block should transfer to the original catch handler"
    );

    let list_free = interner.intern("ori_list_free");
    assert_eq!(
        func.blocks[landing.index()]
            .body
            .iter()
            .filter(|instr| matches!(instr, ArcInstr::Apply { func, .. } if *func == list_free))
            .count(),
        1,
        "checked-op landing block should free the live yield scratch exactly once"
    );
}

/// No cleanup when no iterators are live at the `InvokeIndirect`.
#[test]
fn invoke_indirect_no_live_iterators_no_cleanup() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");

    // No iterator creation — just an InvokeIndirect with Resume unwind
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::InvokeIndirect {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![],
                arg_ownership: vec![],
                normal: ArcBlockId::new(1),
                unwind: ArcBlockId::new(2),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 2]);
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    // Block 2 (Resume) should remain empty — no live iterators
    assert!(
        func.blocks[2].body.is_empty(),
        "no cleanup when no iterators are live"
    );
}

/// semantic pin: an iterator created on a sibling branch
/// that CANNOT reach the Invoke block must NOT be treated as live
/// at that Invoke. The previous check used block-ordering
/// (`create_block <= invoke_block_idx`), which silently treated
/// earlier-numbered sibling branches as live — causing spurious
/// `ori_iter_drop` synthesis on unwind edges for variables that are
/// uninitialized at that point.
///
/// CFG shape:
/// ```text
///   bb0: Branch cond → bb1 | bb2
///   bb1: Apply @iter %iter  →  Return  (sibling branch, creates iterator)
///   bb2: InvokeIndirect (normal=bb3, unwind=bb4)
///   bb3: Return
///   bb4: Resume (unwind block — must NOT get a drop for bb1's iterator)
/// ```
///
/// bb1 cannot reach bb2 via forward edges — they are siblings off bb0.
/// Therefore bb1's iterator is never live at bb2's Invoke and no
/// cleanup should be inserted into bb4.
#[test]
fn sibling_branch_iterator_not_live_at_invoke() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let iter_name = interner.intern("iter");

    let blocks = vec![
        // bb0: Branch cond → bb1 | bb2
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Branch {
                cond: ArcVarId::new(0),
                then_block: ArcBlockId::new(1),
                else_block: ArcBlockId::new(2),
            },
        },
        // bb1: Apply @iter → Return (sibling branch creates iterator)
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: iter_name,
                args: vec![ArcVarId::new(2)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        },
        // bb2: InvokeIndirect (normal=bb3, unwind=bb4)
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::InvokeIndirect {
                dst: ArcVarId::new(4),
                ty: Idx::INT,
                closure: ArcVarId::new(3),
                args: vec![],
                arg_ownership: vec![],
                normal: ArcBlockId::new(3),
                unwind: ArcBlockId::new(4),
            },
        },
        // bb3: Return (normal path from bb2's invoke)
        ArcBlock {
            id: ArcBlockId::new(3),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(4),
            },
        },
        // bb4: Resume (unwind path from bb2's invoke)
        ArcBlock {
            id: ArcBlockId::new(4),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    // bb4 (the Resume unwind block) must remain empty. The iterator
    // created in bb1 is on a sibling branch that never reaches bb2,
    // so it cannot be "live" at bb2's Invoke. Before the
    // filter used `create_block <= invoke_block_idx` (1 <= 2 → true)
    // and synthesized a spurious `ori_iter_drop` here.
    assert!(
        func.blocks[4].body.is_empty(),
        "sibling-branch iterator must not be treated as live at an \
         Invoke it cannot forward-reach"
    );
}
