//! Unit tests for `add_invoke_unwind_cleanup()` — `InvokeIndirect` handling.
//!
//! Tests verify that `InvokeIndirect` terminators with Resume unwind blocks
//! get iterator drop cleanup, matching the existing Invoke behavior.

use ori_ir::StringInterner;
use ori_types::Idx;

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::test_helpers::make_func_named;

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
        // TPR-07-012: `ori_iter_drop` consumes the iterator handle.
        // The ownership contract must match
        // `ProtocolBuiltin::IterDrop.arg_ownership()` which is `Owned`
        // post-TPR-07-008. Previously `Borrowed` here was a shadow
        // source contradicting the SSOT.
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "ori_iter_drop arg should be Owned (consumes the iterator handle)"
        );
    } else {
        panic!("expected Apply instruction for iter drop");
    }
}

/// No cleanup when unwind block is not Resume.
#[test]
fn invoke_indirect_non_resume_no_cleanup() {
    let interner = StringInterner::new();
    let func_name = interner.intern("test_fn");
    let iter_name = interner.intern("iter");

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
                // unwind points to block 2 (Return, not Resume)
                unwind: ArcBlockId::new(2),
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
    ];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let original_body_len = func.blocks[2].body.len();
    super::add_invoke_unwind_cleanup(&mut func, &interner);

    // Block 2 should not have any new instructions (normal==unwind or non-Resume)
    assert_eq!(
        func.blocks[2].body.len(),
        original_body_len,
        "no cleanup should be inserted for non-Resume unwind"
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
