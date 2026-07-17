//! Unit tests for `collect_borrowed_call_args()` — indirect call handling.
//!
//! Tests verify that `ApplyIndirect` and `InvokeIndirect` correctly use
//! `arg_ownership` annotations to determine borrowed args, with conservative
//! fallback when annotations are missing (empty `arg_ownership`).

use ori_ir::StringInterner;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::test_helpers::make_func_named;
use crate::BuiltinOwnershipSets;
use ori_ir::Name;

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
    let borrowed =
        super::collect_borrowed_call_args(&func, &contracts, &builtins, &FxHashSet::default());

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
    let borrowed =
        super::collect_borrowed_call_args(&func, &contracts, &builtins, &FxHashSet::default());

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
    let borrowed =
        super::collect_borrowed_call_args(&func, &contracts, &builtins, &FxHashSet::default());

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
    let borrowed =
        super::collect_borrowed_call_args(&func, &contracts, &builtins, &FxHashSet::default());

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
    let borrowed =
        super::collect_borrowed_call_args(&func, &contracts, &builtins, &FxHashSet::default());

    assert!(
        borrowed.contains(&ArcVarId::new(1)),
        "directly borrowed arg should be marked"
    );
    assert!(
        borrowed.contains(&ArcVarId::new(3)),
        "alias of borrowed arg should also be marked"
    );
}

// Site 8 — is_safe_non_sharing_callee: builtin / contract / IC-1 discrimination.

/// 8a: a builtin callee is always conservative (returns false), even when the
/// contract would say otherwise. Builtin runtime impls may do hidden RC ops.
#[test]
fn is_safe_non_sharing_callee_returns_false_for_builtin() {
    let interner = StringInterner::new();
    // `len` is a known borrowing builtin registered by `new`.
    let builtins = BuiltinOwnershipSets::new(&interner);
    let callee = interner.intern("len");
    assert!(
        builtins.contains(callee),
        "test precondition: len is builtin"
    );

    let contracts = FxHashMap::default();
    let func_names = FxHashSet::default();
    assert!(!super::is_safe_non_sharing_callee(
        callee,
        &contracts,
        &builtins,
        &func_names
    ));
}

/// 8b: a non-builtin user function whose contract has `may_share == false` is
/// a safe non-sharing callee (returns true = `!may_share`).
#[test]
fn is_safe_non_sharing_callee_reads_may_share_false_for_user_fn() {
    let builtins = BuiltinOwnershipSets::empty();
    let callee = Name::from_raw(42);
    let mut contract = MemoryContract::conservative(0);
    contract.effects.may_share = false;
    let mut contracts = FxHashMap::default();
    contracts.insert(callee, contract);
    let func_names = FxHashSet::default();

    assert!(super::is_safe_non_sharing_callee(
        callee,
        &contracts,
        &builtins,
        &func_names
    ));
}

/// 8c: a non-builtin user function whose contract has `may_share == true` is
/// NOT a safe non-sharing callee (returns false). Matrix-clamp pair with 8b.
#[test]
fn is_safe_non_sharing_callee_reads_may_share_true_for_user_fn() {
    let builtins = BuiltinOwnershipSets::empty();
    let callee = Name::from_raw(43);
    let mut contract = MemoryContract::conservative(0);
    contract.effects.may_share = true;
    let mut contracts = FxHashMap::default();
    contracts.insert(callee, contract);
    let func_names = FxHashSet::default();

    assert!(!super::is_safe_non_sharing_callee(
        callee,
        &contracts,
        &builtins,
        &func_names
    ));
}

/// 8d: a non-builtin callee absent from BOTH contracts and `func_names` is a
/// legitimate FFI / external / DCE'd callee — conservative false, no panic.
#[test]
fn is_safe_non_sharing_callee_safe_fallback_for_external_callee() {
    let builtins = BuiltinOwnershipSets::empty();
    let callee = Name::from_raw(44);
    let contracts = FxHashMap::default();
    // callee is NOT in func_names → legitimately external, debug_assert holds.
    let func_names = FxHashSet::default();

    assert!(!super::is_safe_non_sharing_callee(
        callee,
        &contracts,
        &builtins,
        &func_names
    ));
}

/// 8e (debug build): a non-builtin callee IN `func_names` but MISSING from
/// contracts is an IC-1 pipeline-ordering violation — the `debug_assert` fires.
/// In release builds the `debug_assert` is stripped and the function returns
/// false silently (covered by the `cargo test --release` gate, where this
/// test is compiled WITHOUT the `should_panic` expectation; see the release
/// sibling below).
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "AIMS Invariant IC-1")]
fn is_safe_non_sharing_callee_debug_assert_on_pipeline_ordering_bug() {
    let builtins = BuiltinOwnershipSets::empty();
    let callee = Name::from_raw(45);
    let contracts = FxHashMap::default();
    // callee IS a known local function (in func_names) but its contract is
    // missing — the IC-1 invariant is violated → debug_assert fires.
    let mut func_names = FxHashSet::default();
    func_names.insert(callee);

    let _ = super::is_safe_non_sharing_callee(callee, &contracts, &builtins, &func_names);
}

/// 8e (release build): with `debug_assert!` stripped, the same IC-1-violating
/// input returns `false` silently rather than panicking. Compiled only when
/// `debug_assertions` is OFF (i.e. `cargo test --release`).
#[cfg(not(debug_assertions))]
#[test]
fn is_safe_non_sharing_callee_release_returns_false_silently_on_missing_contract() {
    let builtins = BuiltinOwnershipSets::empty();
    let callee = Name::from_raw(45);
    let contracts = FxHashMap::default();
    let mut func_names = FxHashSet::default();
    func_names.insert(callee);

    assert!(!super::is_safe_non_sharing_callee(
        callee,
        &contracts,
        &builtins,
        &func_names
    ));
}
