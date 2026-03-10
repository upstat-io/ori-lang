//! Tests for AIMS builtin contracts.

use ori_ir::StringInterner;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;

use super::super::contract::MemoryContract;
use super::super::lattice::{AccessClass, Consumption, Uniqueness};
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
fn seed_populates_cow_receiver_methods() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // "push" is a COW receiver method.
    let push_name = interner.intern("push");
    if builtins.consuming_receiver.contains(&push_name) {
        let contract = &sigs[&push_name];
        assert_eq!(contract.params[0].access, AccessClass::Owned);
        assert_eq!(contract.params[0].consumption, Consumption::Linear);
        assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
    }
}

#[test]
fn seed_cow_add_consumes_second_arg() {
    let (interner, builtins) = setup();
    let mut sigs = FxHashMap::default();
    seed_builtin_contracts(&mut sigs, &builtins, &interner);

    // "add" consumes both receiver and second arg.
    let add_name = interner.intern("add");
    if builtins.consuming_second_arg.contains(&add_name) {
        let contract = &sigs[&add_name];
        assert!(contract.params.len() >= 2);
        assert_eq!(contract.params[0].access, AccessClass::Owned);
        assert_eq!(contract.params[1].access, AccessClass::Owned);
        assert_eq!(contract.params[1].consumption, Consumption::Linear);
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
