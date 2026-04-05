//! Unit tests for `collect_borrowed_call_args()` — indirect call handling.
//!
//! Tests verify that `ApplyIndirect` and `InvokeIndirect` correctly use
//! `arg_ownership` annotations to determine borrowed args, with conservative
//! fallback when annotations are missing (empty `arg_ownership`).

use ori_ir::StringInterner;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::test_helpers::make_func_named;
use crate::BuiltinOwnershipSets;

/// Semantic pin: `ApplyIndirect` with populated `arg_ownership` marks only
/// Borrowed args — would fail if reverted to old all-borrowed workaround.
#[test]
fn apply_indirect_populated_marks_only_borrowed() {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::empty();
    let func_name = interner.intern("caller");

    // ApplyIndirect with 3 args: Owned, Borrowed, Owned
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(4),
            ty: Idx::INT,
            closure: ArcVarId::new(0),
            args: vec![ArcVarId::new(1), ArcVarId::new(2), ArcVarId::new(3)],
            arg_ownership: vec![
                ArgOwnership::Owned,
                ArgOwnership::Borrowed,
                ArgOwnership::Owned,
            ],
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(4),
        },
    }];

    let func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
    let contracts = FxHashMap::default();
    let borrowed = super::collect_borrowed_call_args(&func, &contracts, &builtins);

    // Only var 2 (Borrowed) should be in the set, not vars 1 or 3 (Owned)
    assert!(
        borrowed.contains(&ArcVarId::new(2)),
        "Borrowed arg should be marked"
    );
    assert!(
        !borrowed.contains(&ArcVarId::new(1)),
        "Owned arg should NOT be marked borrowed"
    );
    assert!(
        !borrowed.contains(&ArcVarId::new(3)),
        "Owned arg should NOT be marked borrowed"
    );
}

/// Semantic pin: `InvokeIndirect` terminator with populated `arg_ownership`
/// contributes only Borrowed args — would fail if `InvokeIndirect` handling removed.
#[test]
fn invoke_indirect_populated_marks_only_borrowed() {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::empty();
    let func_name = interner.intern("caller");

    // InvokeIndirect terminator with 2 args: Borrowed, Owned
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::InvokeIndirect {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1), ArcVarId::new(2)],
                arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Owned],
                normal: ArcBlockId::new(1),
                unwind: ArcBlockId::new(2),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let contracts = FxHashMap::default();
    let borrowed = super::collect_borrowed_call_args(&func, &contracts, &builtins);

    assert!(
        borrowed.contains(&ArcVarId::new(1)),
        "Borrowed arg should be marked"
    );
    assert!(
        !borrowed.contains(&ArcVarId::new(2)),
        "Owned arg should NOT be marked borrowed"
    );
}

/// Negative pin: empty `arg_ownership` on `ApplyIndirect` falls back to
/// all-borrowed (conservative safety in release builds).
#[test]
fn apply_indirect_empty_ownership_falls_back_to_all_borrowed() {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::empty();
    let func_name = interner.intern("caller");

    // ApplyIndirect with empty arg_ownership (unannotated)
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            closure: ArcVarId::new(0),
            args: vec![ArcVarId::new(1), ArcVarId::new(2)],
            arg_ownership: vec![],
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    }];

    let func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let contracts = FxHashMap::default();
    let borrowed = super::collect_borrowed_call_args(&func, &contracts, &builtins);

    // Both args should be marked borrowed (conservative fallback)
    assert!(
        borrowed.contains(&ArcVarId::new(1)),
        "all args should be borrowed when ownership is empty"
    );
    assert!(
        borrowed.contains(&ArcVarId::new(2)),
        "all args should be borrowed when ownership is empty"
    );
}

/// Negative pin: empty `arg_ownership` on `InvokeIndirect` terminator falls
/// back to all-borrowed.
#[test]
fn invoke_indirect_empty_ownership_falls_back_to_all_borrowed() {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::empty();
    let func_name = interner.intern("caller");

    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::InvokeIndirect {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1), ArcVarId::new(2)],
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
                value: ArcVarId::new(3),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let contracts = FxHashMap::default();
    let borrowed = super::collect_borrowed_call_args(&func, &contracts, &builtins);

    assert!(
        borrowed.contains(&ArcVarId::new(1)),
        "all args should be borrowed when ownership is empty"
    );
    assert!(
        borrowed.contains(&ArcVarId::new(2)),
        "all args should be borrowed when ownership is empty"
    );
}

/// Alias propagation: borrowed arg aliased via Let should also be marked.
#[test]
fn alias_chain_propagates_borrowed_from_indirect() {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::empty();
    let func_name = interner.intern("caller");

    // %2 = ApplyIndirect(.., args=[%1], arg_ownership=[Borrowed])
    // %3 = Let %1  (alias)
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(2),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1)],
                arg_ownership: vec![ArgOwnership::Borrowed],
            },
            ArcInstr::Let {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                value: crate::ir::ArcValue::Var(ArcVarId::new(1)),
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    }];

    let func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let contracts = FxHashMap::default();
    let borrowed = super::collect_borrowed_call_args(&func, &contracts, &builtins);

    assert!(
        borrowed.contains(&ArcVarId::new(1)),
        "directly borrowed arg should be marked"
    );
    assert!(
        borrowed.contains(&ArcVarId::new(3)),
        "alias of borrowed arg should also be marked"
    );
}
