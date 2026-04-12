//! Tests for AIMS builtin contracts.

use ori_ir::StringInterner;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;

use super::super::contract::MemoryContract;
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
fn seed_does_not_overwrite_existing() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();

    // Pre-insert a custom contract for "len".
    let len_name = interner.intern("len");
    let custom = MemoryContract::conservative(3);
    sigs.insert(len_name, custom.clone());

    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // Should not overwrite the existing entry.
    assert_eq!(sigs[&len_name].params.len(), 3);
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

// Negative pins — forbid the broken behavior that existed before the fix.

/// Negative pin: `IterDrop` must NOT have Borrowed access.
/// Before the fix (TPR-07-008), `IterDrop` was Borrowed, causing
/// a double-free on iterator cleanup.
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
/// The __index bug was caused by the "unknown callee -> all Owned"
/// fallthrough.
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
