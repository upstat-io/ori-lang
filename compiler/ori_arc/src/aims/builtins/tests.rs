//! Tests for AIMS builtin contracts.

use ori_ir::StringInterner;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;

use super::super::contract::{MemoryContract, ReturnAliasShape};
use super::super::lattice::{AccessClass, Cardinality, Consumption, Uniqueness};
use super::*;

fn setup() -> (StringInterner, BuiltinOwnershipSets) {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    (interner, builtins)
}

#[test]
fn seed_populates_borrowing_methods() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // Check a known borrowing method (e.g., "len")
    let len_name = interner.intern("len");
    if builtins.borrowing.contains(&len_name) {
        let contract = &sigs[&len_name];
        assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    }
}

#[test]
fn seed_cow_receiver_methods_base_is_borrowed() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // COW receiver methods are seeded as Borrowed (base contract).
    // `apply_consuming_overrides` overrides to Owned for List/Map/Set
    // receivers at call sites. This ensures string methods stay Borrowed.
    let push_name = interner.intern("push");
    if builtins.consuming_receiver.contains(&push_name) {
        let contract = &sigs[&push_name];
        assert_eq!(
            contract.params[0].access,
            AccessClass::Borrowed,
            "COW methods seeded as Borrowed; apply_consuming_overrides adds Owned for collections"
        );
    }
}

#[test]
fn seed_cow_add_base_is_borrowed() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // "add" is seeded as Borrowed (1 param). apply_consuming_overrides
    // adds Owned for List receivers at call sites.
    let add_name = interner.intern("add");
    if builtins.consuming_second_arg.contains(&add_name) {
        let contract = &sigs[&add_name];
        assert_eq!(contract.params.len(), 1);
        assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    }
}

#[test]
fn seed_cow_receiver_only_borrows_args() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // "remove" (map/set) consumes receiver, borrows key.
    let remove_name = interner.intern("remove");
    if builtins.consuming_receiver_only.contains(&remove_name) {
        let contract = &sigs[&remove_name];
        assert_eq!(contract.params[0].access, AccessClass::Owned);
        if contract.params.len() > 1 {
            assert_eq!(contract.params[1].access, AccessClass::Borrowed);
        }
    }
}

#[test]
fn effective_consuming_provenance_requires_specific_authority() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    assert!(effective_consuming_provenance(
        interner.intern("push"),
        0,
        crate::ir::ArgOwnership::Owned,
        false,
        &sigs,
        &interner,
    ));
    assert!(!effective_consuming_provenance(
        interner.intern("push"),
        0,
        crate::ir::ArgOwnership::Borrowed,
        false,
        &sigs,
        &interner,
    ));
    assert!(!effective_consuming_provenance(
        interner.intern("push"),
        0,
        crate::ir::ArgOwnership::Owned,
        true,
        &sigs,
        &interner,
    ));
    assert!(effective_consuming_provenance(
        interner.intern("remove"),
        0,
        crate::ir::ArgOwnership::Owned,
        false,
        &sigs,
        &interner,
    ));
    let registered_borrowed = interner.intern("registered_borrowed");
    sigs.insert(
        registered_borrowed,
        MemoryContract::all_borrowed(2, FipContract::Never),
    );
    assert!(!effective_consuming_provenance(
        registered_borrowed,
        0,
        crate::ir::ArgOwnership::Owned,
        false,
        &sigs,
        &interner,
    ));
}

#[test]
fn seed_sharing_methods_return_maybe_shared() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // "slice" returns MaybeShared (shares backing storage).
    let slice_name = interner.intern("slice");
    let contract = &sigs[&slice_name];
    assert_eq!(contract.return_info.uniqueness, Uniqueness::MaybeShared);
}

#[test]
fn fixed_capacity_conversions_are_direct_identity_transfers() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    for name in ["to_dynamic", "to_fixed"] {
        let contract = &sigs[&interner.intern(name)];
        assert_eq!(contract.params.len(), 1, "List.{name} receiver arity");
        assert_eq!(
            contract.params[0].access,
            AccessClass::Owned,
            "List.{name} transfers its receiver credit"
        );
        assert_eq!(contract.params[0].consumption, Consumption::Linear);
        assert!(contract.params[0].transfers_through_return);
        assert_eq!(
            contract.params[0].return_alias,
            Some(ReturnAliasShape::Direct)
        );
        assert!(
            !contract.return_info.returns_sharing_view,
            "List.{name} moves one credit; it does not mint a sharing-view credit"
        );
    }
}

#[test]
fn indexed_cow_result_is_born_unique_and_block_local() {
    let interner = StringInterner::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let contract = &sigs[&interner.intern("updated")];
    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
    assert_eq!(contract.return_info.locality, Locality::BlockLocal);
    assert!(contract.return_info.preserves_freshness);
}

#[test]
fn yield_finalizer_result_is_born_unique_and_fresh() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let contract = &sigs[&interner.intern("ori_list_take")];
    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
    assert_eq!(contract.return_info.locality, Locality::BlockLocal);
    assert!(contract.return_info.preserves_freshness);
    assert!(contract.return_info.returns_fresh_self_alloc);
}

#[test]
fn seed_does_not_overwrite_existing() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();

    // Pre-insert a custom contract for "len".
    let len_name = interner.intern("len");
    let custom = MemoryContract::conservative(3);
    sigs.insert(len_name, custom);

    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // Should not overwrite the existing entry.
    assert_eq!(sigs[&len_name].params.len(), 3);
}

#[test]
fn iter_map_contract_matches_five_argument_runtime_abi() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let contract = &sigs[&interner.intern("ori_iter_map")];
    assert_eq!(contract.params.len(), 5);
    assert_eq!(contract.params[0].access, AccessClass::Owned);
    assert!(contract.params[1..]
        .iter()
        .all(|param| param.access == AccessClass::Borrowed));
}

#[test]
fn seed_covers_all_builtin_sets() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // Every borrowing builtin should have a contract.
    for &name in &builtins.borrowing {
        assert!(
            sigs.contains_key(&name),
            "missing contract for borrowing builtin"
        );
    }

    // Every consuming receiver builtin should have a contract.
    for &name in &builtins.consuming_receiver {
        assert!(
            sigs.contains_key(&name),
            "missing contract for COW receiver builtin"
        );
    }

    // Every consuming receiver-only builtin should have a contract.
    for &name in &builtins.consuming_receiver_only {
        assert!(
            sigs.contains_key(&name),
            "missing contract for COW receiver-only builtin"
        );
    }

    // Every protocol builtin should have a contract.
    for &name in builtins.protocol.keys() {
        assert!(
            sigs.contains_key(&name),
            "missing contract for protocol builtin"
        );
    }
}

#[test]
fn seed_updated_contract_borrows_key_and_moves_value() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // `updated` borrows its receiver/key and transfers its value.
    let updated_name = interner.intern("updated");
    assert!(builtins.consuming_third_arg.contains(&updated_name));
    let contract = &sigs[&updated_name];
    assert_eq!(
        contract.params.len(),
        3,
        "updated is 3-param (self, key, value)"
    );
    assert_eq!(
        contract.params[0].access,
        AccessClass::Borrowed,
        "receiver base"
    );
    assert_eq!(
        contract.params[1].access,
        AccessClass::Borrowed,
        "key borrowed"
    );
    assert_eq!(contract.params[2].access, AccessClass::Owned, "value moved");
    assert_eq!(contract.params[2].consumption, Consumption::Linear);
    assert_eq!(contract.params[2].cardinality, Cardinality::Once);
    assert_eq!(
        contract.return_info.uniqueness,
        Uniqueness::Unique,
        "COW result"
    );
}

#[test]
fn seed_updated_not_overwritten_by_consuming_receiver_base() {
    // "updated" is ALSO in consuming_receiver; the 3-param indexed-update
    // contract must be seeded FIRST so the 1-param borrowing base does not
    // erase the value-param ownership transfer.
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let updated_name = interner.intern("updated");
    assert!(builtins.consuming_receiver.contains(&updated_name));
    assert_eq!(sigs[&updated_name].params.len(), 3);
}

// Protocol builtin consumer-level tests — verify that seed_builtin_contracts
// produces MemoryContract with correct field values, not just "entry exists."

/// Verify Index protocol has two Borrowed/Dead/Once params.
#[test]
fn protocol_contract_index_has_two_borrowed_params() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("__index");
    let contract = &sigs[&name];
    assert_eq!(contract.params.len(), 2);
    for (i, param) in contract.params.iter().enumerate() {
        assert_eq!(
            param.access,
            AccessClass::Borrowed,
            "Index param {i} access"
        );
        assert_eq!(
            param.consumption,
            Consumption::Dead,
            "Index param {i} consumption"
        );
        assert_eq!(
            param.cardinality,
            Cardinality::Once,
            "Index param {i} cardinality"
        );
    }
}

/// Verify `IterDrop` protocol has one Owned/Linear/Once param.
#[test]
fn protocol_contract_iter_drop_has_owned_linear_param() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("ori_iter_drop");
    let contract = &sigs[&name];
    assert_eq!(contract.params.len(), 1);
    assert_eq!(contract.params[0].access, AccessClass::Owned);
    assert_eq!(contract.params[0].consumption, Consumption::Linear);
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
}

/// Verify `IterNext` protocol has Owned first param, Borrowed second param.
#[test]
fn protocol_contract_iter_next_owned_then_borrowed() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("__iter_next");
    let contract = &sigs[&name];
    assert_eq!(contract.params.len(), 2);
    assert_eq!(contract.params[0].access, AccessClass::Owned);
    assert_eq!(contract.params[0].consumption, Consumption::Linear);
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
    assert_eq!(contract.params[1].access, AccessClass::Borrowed);
    assert_eq!(contract.params[1].consumption, Consumption::Dead);
    assert_eq!(contract.params[1].cardinality, Cardinality::Once);
}

/// Verify `CollectSet` protocol has one Owned/Linear/Once param.
#[test]
fn protocol_contract_collect_set_owned_linear() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("__collect_set");
    let contract = &sigs[&name];
    assert_eq!(contract.params.len(), 1);
    assert_eq!(contract.params[0].access, AccessClass::Owned);
    assert_eq!(contract.params[0].consumption, Consumption::Linear);
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
}

/// Verify Iter protocol has one Borrowed/Dead/Once param.
#[test]
fn protocol_contract_iter_borrowed_param() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("iter");
    let contract = &sigs[&name];
    assert_eq!(contract.params.len(), 1);
    assert_eq!(contract.params[0].access, AccessClass::Borrowed);
    assert_eq!(contract.params[0].consumption, Consumption::Dead);
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
}

// Negative ownership constraints.

/// Negative pin: `IterDrop` must NOT have Borrowed access.
/// Borrowed access would emit a second scope-exit decrement during cleanup.
#[test]
fn protocol_contract_iter_drop_forbids_borrowed() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("ori_iter_drop");
    let contract = &sigs[&name];
    assert_ne!(
        contract.params[0].access,
        AccessClass::Borrowed,
        "IterDrop MUST NOT be Borrowed — Borrowed ownership causes \
         a second scope-exit RcDec (double-free)"
    );
}

/// Negative pin: Index must NOT have Owned access on arg 0.
/// Owned access would consume the receiver during a lookup.
#[test]
fn protocol_contract_index_forbids_owned_receiver() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    let name = interner.intern("__index");
    let contract = &sigs[&name];
    assert_ne!(
        contract.params[0].access,
        AccessClass::Owned,
        "Index receiver MUST NOT be Owned — Owned receiver causes \
         the collection to be consumed on index lookup"
    );
}

// Consistency pin — verify contracts match arg_ownership() for all builtins.

/// Consistency pin: contract access class matches `arg_ownership()` for all
/// protocol builtins. Catches drift between the ownership constant and the
/// contract seeding path.
#[test]
fn protocol_contract_access_consistent_with_arg_ownership() {
    use ori_ir::builtin_constants::protocol::{ProtocolArgOwnership, ProtocolBuiltin};

    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    for &pb in ProtocolBuiltin::ALL {
        let name = interner.intern(pb.name());
        let contract = &sigs[&name];
        assert_eq!(
            contract.params.len(),
            pb.arg_ownership().len(),
            "{pb:?}: param count mismatch"
        );
        for (i, arg_own) in pb.arg_ownership().iter().enumerate() {
            let expected = match arg_own {
                ProtocolArgOwnership::Borrowed => AccessClass::Borrowed,
                ProtocolArgOwnership::Owned => AccessClass::Owned,
            };
            let actual = contract.params[i].access;
            assert_eq!(
                actual, expected,
                "{pb:?} arg {i}: contract {actual:?} != expected {expected:?}"
            );
        }
    }
}

/// Pins `ori_panic`'s RL-2 ownership transfer for its message parameter.
#[test]
fn seed_ori_panic_message_param_owned_transfer() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let ori_panic = interner.intern("ori_panic");
    let contract = sigs
        .get(&ori_panic)
        .unwrap_or_else(|| panic!("ori_panic must carry a seeded contract"));
    assert_eq!(
        contract.params.len(),
        1,
        "ori_panic takes one message param"
    );
    assert_eq!(
        contract.params[0].access,
        AccessClass::Owned,
        "the panic message transfers to the panic machinery"
    );
    assert_eq!(contract.params[0].consumption, Consumption::Linear);
    // INVARIANT: The seed narrows parameter ownership without narrowing panic effects.
    assert!(
        contract.effects.may_throw,
        "ori_panic raises an exception — may_throw must stay true"
    );
    assert!(
        contract.effects.may_deallocate,
        "ori_panic releases the message — may_deallocate must stay true"
    );
}

/// Pins `__ori_inject_trace`'s RL-34 forwarder-identity transfer.
#[test]
fn seed_ori_inject_trace_forwarder_identity_transfer() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    let ori_inject_trace = interner.intern("__ori_inject_trace");
    let contract = sigs
        .get(&ori_inject_trace)
        .unwrap_or_else(|| panic!("__ori_inject_trace must carry a seeded contract"));
    assert_eq!(
        contract.params.len(),
        1,
        "__ori_inject_trace takes one Error receiver param"
    );
    assert_eq!(
        contract.params[0].access,
        AccessClass::Owned,
        "the receiver Error is consumed"
    );
    assert_eq!(contract.params[0].consumption, Consumption::Linear);
    assert!(
        contract.params[0].transfers_through_return,
        "the receiver's refs transfer through the by-value return (RL-34)"
    );
    assert_eq!(
        contract.params[0].return_alias,
        Some(ReturnAliasShape::Direct),
        "the returned Error carries the transferred refs directly, not wrapped"
    );
    // Effects stay narrowed to allocate+deallocate only — the runtime
    // COW-pushes onto error.trace (may realloc a new buffer + free the old);
    // a default EffectSummary would under-approximate the callee's effects.
    assert!(
        contract.effects.may_allocate,
        "the COW-push may allocate a new trace buffer"
    );
    assert!(
        contract.effects.may_deallocate,
        "the COW-push may free the old trace buffer"
    );
}

// Sharing-view CREDIT seeds (returns_sharing_view; Spec: Annex E §AIMS §12).

/// Positive pin: every seamless-slice view producer carries the typed
/// sharing-view CREDIT on its return contract, with a borrowed (non-consuming)
/// receiver and a sharing effect that invalidates the receiver's pre-call
/// uniqueness — the READ + CREDIT boundary pair.
#[test]
fn sharing_view_builtins_carry_returns_sharing_view_credit() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    for name in [
        "slice",
        "substring",
        "ori_list_slice_take",
        "ori_list_slice_drop",
    ] {
        let contract = &sigs[&interner.intern(name)];
        assert!(
            contract.return_info.returns_sharing_view,
            "{name} must carry the sharing-view CREDIT"
        );
        assert_eq!(
            contract.return_info.uniqueness,
            Uniqueness::MaybeShared,
            "{name} view result shares the receiver's backing"
        );
        assert!(
            !contract.return_info.returns_fresh_self_alloc,
            "{name} view is never a fresh self-alloc"
        );
        assert_eq!(
            contract.params[0].access,
            AccessClass::Borrowed,
            "{name} receiver is borrowed (READ), the CREDIT rides the return"
        );
        assert!(
            contract.params[0].may_share,
            "{name} receiver contract must publish the retained backing owner"
        );
        assert!(
            contract.effects.may_share,
            "{name} must invalidate Unique on its borrowed receiver"
        );
    }
}

/// Negative pin: non-view builtins mint no sharing-view CREDIT — a COW
/// producer (`concat`), a retaining accessor (`first`), and the
/// collision-prone bare surface names `take` / `drop` (a user
/// `Drop::drop` shares the name; never seeded) all stay CREDIT-free.
#[test]
fn non_sharing_builtins_carry_no_sharing_view_credit() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);
    for name in ["concat", "first", "iter", "ori_list_take", "take", "drop"] {
        if let Some(contract) = sigs.get(&interner.intern(name)) {
            assert!(
                !contract.return_info.returns_sharing_view,
                "{name} is not a sharing-view producer"
            );
        }
    }
}
