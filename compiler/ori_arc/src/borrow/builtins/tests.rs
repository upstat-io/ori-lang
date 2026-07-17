use super::*;

#[test]
fn borrowing_builtin_names_returns_correct_count() {
    let interner = StringInterner::default();
    let names = borrowing_builtin_names(&interner);
    // The interned set is derived from ori_registry::borrowing_method_names()
    // plus all-borrowed protocol builtins. Count must match registry + protocols.
    let registry_count = ori_registry::borrowing_method_names().len();
    let protocol_borrow_count = ProtocolBuiltin::ALL
        .iter()
        .filter(|pb| {
            pb.arg_ownership()
                .iter()
                .all(|o| *o == ProtocolArgOwnership::Borrowed)
        })
        .count();
    // Protocol names might overlap with registry names, so use >= not ==.
    // But protocol names (e.g., "__index") are prefixed with "__" and never
    // appear in regular builtin method names, so no overlap expected.
    assert_eq!(
        names.len(),
        registry_count + protocol_borrow_count,
        "interned set should be registry names + all-borrowed protocol builtins"
    );
}

#[test]
fn consuming_receiver_method_names_sorted() {
    for window in CONSUMING_RECEIVER_METHOD_NAMES.windows(2) {
        assert!(
            window[0] < window[1],
            "CONSUMING_RECEIVER_METHOD_NAMES not sorted: {:?} >= {:?}",
            window[0],
            window[1],
        );
    }
}

#[test]
fn consuming_receiver_method_names_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for &name in CONSUMING_RECEIVER_METHOD_NAMES {
        assert!(
            seen.insert(name),
            "duplicate in CONSUMING_RECEIVER_METHOD_NAMES: {name:?}",
        );
    }
}

#[test]
fn consuming_receiver_builtin_names_returns_correct_count() {
    let interner = StringInterner::default();
    let names = consuming_receiver_builtin_names(&interner);
    let mut expected = intern_name_set(CONSUMING_RECEIVER_METHOD_NAMES, &interner);
    expected.extend(persistent_list_runtime_methods().map(|method| interner.intern(method.name)));
    assert_eq!(names, expected);
}

#[test]
fn reverse_in_consuming() {
    // "reverse" is borrowing for Ordering (Ordering.reverse() is a pure read)
    // but consuming for List (COW semantics). The consuming-receiver override
    // in annotate_arg_ownership handles the list case.
    assert!(
        CONSUMING_RECEIVER_METHOD_NAMES.contains(&"reverse"),
        "\"reverse\" must be in CONSUMING — list.reverse() is COW consuming"
    );
}

#[test]
fn insert_in_consuming() {
    let interner = StringInterner::default();
    assert!(
        consuming_receiver_builtin_names(&interner).contains(&interner.intern("insert")),
        "\"insert\" must be in CONSUMING — list.insert() is COW consuming"
    );
}

#[test]
fn remove_in_consuming_receiver_only() {
    // "remove" consumes the receiver (COW) but only reads the key/element
    // for comparison — non-receiver args must be Borrowed to prevent leaks.
    assert!(
        CONSUMING_RECEIVER_ONLY_METHOD_NAMES.contains(&"remove"),
        "\"remove\" must be in CONSUMING_RECEIVER_ONLY — key is comparison-only"
    );
    let interner = StringInterner::default();
    assert!(
        consuming_receiver_builtin_names(&interner).contains(&interner.intern("remove")),
        "\"remove\" must be in CONSUMING_RECEIVER — list.remove() is COW consuming"
    );
}

#[test]
fn cow_methods_in_consuming() {
    let interner = StringInterner::default();
    let consuming = consuming_receiver_builtin_names(&interner);
    for &method in &[
        "add",
        "concat",
        "insert",
        "pop",
        "prepend",
        "push",
        "remove",
        "reverse",
        "sort",
        "sort_stable",
    ] {
        assert!(
            consuming.contains(&interner.intern(method)),
            "\"{method}\" must be a consuming-receiver builtin"
        );
    }
}

#[test]
fn runtime_list_mutation_ownership_is_registry_derived() {
    let interner = StringInterner::default();
    let receivers = consuming_receiver_builtin_names(&interner);
    let second_args = consuming_second_arg_builtin_names(&interner);
    let third_args = consuming_third_arg_builtin_names(&interner);

    for method in persistent_list_runtime_methods() {
        let name = interner.intern(method.name);
        assert!(receivers.contains(&name), "{}.receiver", method.name);
        assert_eq!(
            second_args.contains(&name),
            method
                .params
                .first()
                .is_some_and(|param| { param.ownership == ori_registry::Ownership::Owned }),
            "{}.param[0]",
            method.name,
        );
        assert_eq!(
            third_args.contains(&name),
            method
                .params
                .get(1)
                .is_some_and(|param| { param.ownership == ori_registry::Ownership::Owned }),
            "{}.param[1]",
            method.name,
        );
    }
}

#[test]
fn updated_in_consuming_and_third_arg() {
    // "updated" (IndexSet) consumes the receiver (COW, via the type-qualified
    // override) AND moves the inserted value (arg[2]); the key (arg[1])
    // stays borrowed.
    assert!(
        CONSUMING_RECEIVER_METHOD_NAMES.contains(&"updated"),
        "\"updated\" must be in CONSUMING_RECEIVER — list/map.updated() is COW consuming"
    );
    assert!(
        CONSUMING_THIRD_ARG_METHOD_NAMES.contains(&"updated"),
        "\"updated\" must be in CONSUMING_THIRD_ARG — the value is moved into the collection"
    );
}

#[test]
fn consuming_third_arg_method_names_sorted() {
    for window in CONSUMING_THIRD_ARG_METHOD_NAMES.windows(2) {
        assert!(
            window[0] < window[1],
            "CONSUMING_THIRD_ARG_METHOD_NAMES must stay sorted: {} >= {}",
            window[0],
            window[1]
        );
    }
}

#[test]
fn consuming_third_arg_builtin_names_match_registry_and_legacy_sources() {
    let interner = StringInterner::default();
    let names = consuming_third_arg_builtin_names(&interner);
    let mut expected = intern_name_set(CONSUMING_THIRD_ARG_METHOD_NAMES, &interner);
    expected.extend(
        persistent_list_runtime_methods()
            .filter(|method| {
                method
                    .params
                    .get(1)
                    .is_some_and(|param| param.ownership == ori_registry::Ownership::Owned)
            })
            .map(|method| interner.intern(method.name)),
    );
    assert_eq!(names, expected);
}

#[test]
fn consuming_second_arg_method_names_sorted() {
    for window in CONSUMING_SECOND_ARG_METHOD_NAMES.windows(2) {
        assert!(
            window[0] < window[1],
            "CONSUMING_SECOND_ARG_METHOD_NAMES not sorted: {:?} >= {:?}",
            window[0],
            window[1],
        );
    }
}

#[test]
fn consuming_second_arg_method_names_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for &name in CONSUMING_SECOND_ARG_METHOD_NAMES {
        assert!(
            seen.insert(name),
            "duplicate in CONSUMING_SECOND_ARG_METHOD_NAMES: {name:?}",
        );
    }
}

#[test]
fn consuming_second_arg_builtin_names_returns_correct_count() {
    let interner = StringInterner::default();
    let names = consuming_second_arg_builtin_names(&interner);
    let mut expected = intern_name_set(CONSUMING_SECOND_ARG_METHOD_NAMES, &interner);
    expected.extend(
        persistent_list_runtime_methods()
            .filter(|method| {
                method
                    .params
                    .first()
                    .is_some_and(|param| param.ownership == ori_registry::Ownership::Owned)
            })
            .map(|method| interner.intern(method.name)),
    );
    assert_eq!(names, expected);
}

#[test]
fn consuming_second_arg_subset_of_consuming_receiver() {
    let interner = StringInterner::default();
    let receivers = consuming_receiver_builtin_names(&interner);
    for method in consuming_second_arg_builtin_names(&interner) {
        assert!(
            receivers.contains(&method),
            "{method:?} is in CONSUMING_SECOND_ARG but not CONSUMING_RECEIVER — \
             a method can't consume arg[1] without also consuming the receiver"
        );
    }
}

// consuming_receiver_only tests

#[test]
fn consuming_receiver_only_method_names_sorted() {
    for window in CONSUMING_RECEIVER_ONLY_METHOD_NAMES.windows(2) {
        assert!(
            window[0] < window[1],
            "CONSUMING_RECEIVER_ONLY_METHOD_NAMES not sorted: {:?} >= {:?}",
            window[0],
            window[1],
        );
    }
}

#[test]
fn consuming_receiver_only_method_names_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for &name in CONSUMING_RECEIVER_ONLY_METHOD_NAMES {
        assert!(
            seen.insert(name),
            "duplicate in CONSUMING_RECEIVER_ONLY_METHOD_NAMES: {name:?}",
        );
    }
}

#[test]
fn consuming_receiver_only_builtin_names_returns_correct_count() {
    let interner = StringInterner::default();
    let names = consuming_receiver_only_builtin_names(&interner);
    assert_eq!(
        names.len(),
        CONSUMING_RECEIVER_ONLY_METHOD_NAMES.len(),
        "interned set should have same count as const array (no duplicates)"
    );
}

// all_cow_method_names tests

#[test]
fn all_cow_method_names_unions_both_sets() {
    let interner = StringInterner::default();
    let all = all_cow_method_names(&interner);
    let receiver = consuming_receiver_builtin_names(&interner);
    let receiver_only = consuming_receiver_only_builtin_names(&interner);

    // Every name from both sets must be in the union.
    for name in &receiver {
        assert!(
            all.contains(name),
            "all_cow_method_names missing consuming_receiver name"
        );
    }
    for name in &receiver_only {
        assert!(
            all.contains(name),
            "all_cow_method_names missing consuming_receiver_only name"
        );
    }

    // The union must not have more names than the two sets combined.
    // (Some names like "remove" appear in both, so the union may be smaller.)
    assert!(all.len() <= receiver.len() + receiver_only.len());
    // But it must be at least as large as the larger set.
    assert!(all.len() >= receiver.len());
    assert!(all.len() >= receiver_only.len());
}

#[test]
fn all_cow_method_names_includes_map_set_operations() {
    // Verify that map/set COW operations are included in the union.
    // These come from consuming_receiver_only and must not be missed when
    // building COW summaries for uniqueness analysis.
    let interner = StringInterner::default();
    let all = all_cow_method_names(&interner);

    for &method in &["difference", "intersection", "union", "remove"] {
        let name = interner.intern(method);
        assert!(
            all.contains(&name),
            "\"{method}\" must be in all_cow_method_names — \
             it's a COW operation whose result is always Unique"
        );
    }
}

#[test]
fn set_binary_ops_in_consuming_receiver_only() {
    // Set binary operations consume the receiver but only read the second set.
    for &method in &["union", "intersection", "difference"] {
        assert!(
            CONSUMING_RECEIVER_ONLY_METHOD_NAMES.contains(&method),
            "\"{method}\" must be in CONSUMING_RECEIVER_ONLY — \
             second set is read-only, not consumed"
        );
    }
}

// Registry sync tests — verify hardcoded lists match ori_registry definitions

/// Every method in `CONSUMING_RECEIVER_METHOD_NAMES` must exist as a method on
/// at least one collection type (List, Map, or Set) in the registry, OR be an
/// operator trait method dispatched via the operator system.
///
/// Catches stale entries: if a method is renamed or removed from the registry
/// but left in this list, the ARC pipeline would silently produce wrong ownership
/// annotations (consuming semantics for a non-existent method = no-op, but the
/// borrowing set would also be wrong).
#[test]
fn consuming_receiver_methods_exist_in_registry() {
    use ori_registry::TypeTag;

    let collection_types = [TypeTag::List, TypeTag::Map, TypeTag::Set];

    // Operator trait methods dispatched via the operator system, not registered
    // as direct type methods in the registry. "add" is the Add trait method
    // called when using `+` on lists (desugars to `list1.add(list2)`).
    let operator_trait_methods: &[&str] = &["add"];

    for &method in CONSUMING_RECEIVER_METHOD_NAMES {
        if operator_trait_methods.contains(&method) {
            continue;
        }
        let found = collection_types
            .iter()
            .any(|&tag| ori_registry::has_method(tag, method));
        assert!(
            found,
            "CONSUMING_RECEIVER_METHOD_NAMES contains \"{method}\" but it does not \
             exist as a method on List, Map, or Set in ori_registry. \
             Was it renamed or removed?"
        );
    }
}

/// Every method in `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` must exist as a method
/// on Map or Set in the registry.
///
/// These are Map/Set COW methods where only the receiver is consumed. If a method
/// remains in the borrow catalog, the type-qualified authority
/// would publish an ownership transfer for a method the runtime doesn't
/// recognize.
#[test]
fn consuming_receiver_only_methods_exist_in_registry() {
    use ori_registry::TypeTag;

    let map_set_types = [TypeTag::Map, TypeTag::Set];

    for &method in CONSUMING_RECEIVER_ONLY_METHOD_NAMES {
        let found = map_set_types
            .iter()
            .any(|&tag| ori_registry::has_method(tag, method));
        assert!(
            found,
            "CONSUMING_RECEIVER_ONLY_METHOD_NAMES contains \"{method}\" but it does not \
             exist as a method on Map or Set in ori_registry. \
             Was it renamed or removed?"
        );
    }
}

/// Every method in `SHARING_METHOD_NAMES` must exist in the registry.
///
/// Sharing methods return values that reference the receiver's heap data
/// (slices, substrings). If a method is renamed or removed from the registry
/// but remains in the borrow catalog, uniqueness analysis would incorrectly mark it as producing
/// `MaybeShared` results — benign (conservative) but misleading.
#[test]
fn sharing_methods_exist_in_registry() {
    use ori_registry::TypeTag;

    let collection_types = [TypeTag::List, TypeTag::Str];

    for &method in SHARING_METHOD_NAMES {
        let found = collection_types
            .iter()
            .any(|&tag| ori_registry::has_method(tag, method));
        assert!(
            found,
            "SHARING_METHOD_NAMES contains \"{method}\" but it does not exist \
             as a method on List or Str in ori_registry. Was it renamed or removed?"
        );
    }
}

/// Verify `BuiltinOwnershipSets.protocol` maps each protocol builtin
/// name to its correct per-arg ownership array. This is the data
/// that `annotate_arg_ownership()` consumes at call sites.
#[test]
fn ownership_sets_protocol_map_matches_all_builtins() {
    let interner = StringInterner::default();
    let sets = BuiltinOwnershipSets::new(&interner);
    for &pb in ProtocolBuiltin::ALL {
        let name = interner.intern(pb.name());
        let ownership = sets.protocol.get(&name).unwrap_or_else(|| {
            panic!("protocol {pb:?} missing from BuiltinOwnershipSets.protocol")
        });
        assert_eq!(
            *ownership,
            pb.arg_ownership(),
            "BuiltinOwnershipSets.protocol[{pb:?}] doesn't match ProtocolBuiltin.arg_ownership()"
        );
    }
    assert_eq!(
        sets.protocol.len(),
        ProtocolBuiltin::ALL.len(),
        "BuiltinOwnershipSets.protocol has extra entries beyond ProtocolBuiltin::ALL"
    );
}

/// Protocol builtins with all-borrowed args must be in `borrowing_builtin_names()`,
/// and protocol builtins with any Owned args must NOT be.
///
/// Verifies the dynamic protocol builtin integration in `borrowing_builtin_names()`.
/// If a new protocol is added with all-borrowed semantics but the function doesn't
/// include it, borrow inference would treat it as unknown (all-Owned), causing RC
/// leaks.
#[test]
fn protocol_builtins_borrowing_sync() {
    let interner = StringInterner::default();
    let names = borrowing_builtin_names(&interner);
    for &pb in ProtocolBuiltin::ALL {
        let all_borrowed = pb
            .arg_ownership()
            .iter()
            .all(|o| *o == ProtocolArgOwnership::Borrowed);
        let interned = interner.intern(pb.name());
        let in_borrowing = names.contains(&interned);
        assert_eq!(
            all_borrowed,
            in_borrowing,
            "ProtocolBuiltin::{:?} (name={}) has all_borrowed={} but \
             in borrowing_builtin_names={}",
            pb,
            pb.name(),
            all_borrowed,
            in_borrowing,
        );
    }
}

#[test]
fn type_qualified_collection_consuming_positions_are_exact() {
    use ori_registry::TypeTag;

    let interner = StringInterner::default();
    let sets = BuiltinOwnershipSets::new(&interner);
    let positions = |method, tags: &[Option<TypeTag>]| {
        sets.type_qualified_consuming_positions(interner.intern(method), tags)
    };

    assert_eq!(
        positions(
            "insert",
            &[Some(TypeTag::List), Some(TypeTag::Int), Some(TypeTag::Str)]
        )
        .as_slice(),
        &[0, 2]
    );
    assert_eq!(
        positions(
            "insert",
            &[Some(TypeTag::Map), Some(TypeTag::Int), Some(TypeTag::Str)]
        )
        .as_slice(),
        &[0]
    );
    assert_eq!(
        positions(
            "insert",
            &[Some(TypeTag::Set), Some(TypeTag::Str), Some(TypeTag::Str)]
        )
        .as_slice(),
        &[0]
    );
    assert_eq!(
        positions("concat", &[Some(TypeTag::List), Some(TypeTag::List)]).as_slice(),
        &[0, 1]
    );
    assert_eq!(
        positions("difference", &[Some(TypeTag::Set), Some(TypeTag::Set)]).as_slice(),
        &[0]
    );
    assert!(positions("concat", &[Some(TypeTag::Str), Some(TypeTag::Str)]).is_empty());
    assert!(positions("pop", &[Some(TypeTag::List)]).is_empty());
}

#[test]
fn type_qualified_iterator_positions_require_iterator_operands() {
    use ori_registry::TypeTag;

    let interner = StringInterner::default();
    let sets = BuiltinOwnershipSets::new(&interner);
    let positions = |method, tags: &[Option<TypeTag>]| {
        sets.type_qualified_consuming_positions(interner.intern(method), tags)
    };

    assert_eq!(
        positions("map", &[Some(TypeTag::Iterator), Some(TypeTag::Function)]).as_slice(),
        &[0]
    );
    assert_eq!(
        positions(
            "zip",
            &[Some(TypeTag::Iterator), Some(TypeTag::DoubleEndedIterator)]
        )
        .as_slice(),
        &[0, 1]
    );
    assert_eq!(
        positions(
            "chain",
            &[Some(TypeTag::DoubleEndedIterator), Some(TypeTag::Iterator)]
        )
        .as_slice(),
        &[0, 1]
    );
    assert_eq!(
        positions("zip", &[Some(TypeTag::Iterator), Some(TypeTag::List)]).as_slice(),
        &[0]
    );
    assert!(positions("zip", &[None, Some(TypeTag::Iterator)]).is_empty());
}
