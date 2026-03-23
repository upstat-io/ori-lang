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
    assert_eq!(
        names.len(),
        CONSUMING_RECEIVER_METHOD_NAMES.len(),
        "interned set should have same count as const array (no duplicates)"
    );
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
    // "insert" is COW consuming for all collection types (list, map, set).
    // All args are owned (key/value/elem transferred to collection).
    assert!(
        CONSUMING_RECEIVER_METHOD_NAMES.contains(&"insert"),
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
    assert!(
        CONSUMING_RECEIVER_METHOD_NAMES.contains(&"remove"),
        "\"remove\" must be in CONSUMING_RECEIVER — list.remove() is COW consuming"
    );
}

#[test]
fn cow_methods_in_consuming() {
    for &method in &[
        "add",
        "concat",
        "insert",
        "pop",
        "push",
        "remove",
        "reverse",
        "sort",
        "sort_stable",
    ] {
        assert!(
            CONSUMING_RECEIVER_METHOD_NAMES.contains(&method),
            "\"{method}\" must be in CONSUMING_RECEIVER_METHOD_NAMES"
        );
    }
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
    assert_eq!(
        names.len(),
        CONSUMING_SECOND_ARG_METHOD_NAMES.len(),
        "interned set should have same count as const array (no duplicates)"
    );
}

#[test]
fn consuming_second_arg_subset_of_consuming_receiver() {
    for &method in CONSUMING_SECOND_ARG_METHOD_NAMES {
        assert!(
            CONSUMING_RECEIVER_METHOD_NAMES.contains(&method),
            "\"{method}\" is in CONSUMING_SECOND_ARG but not CONSUMING_RECEIVER — \
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
/// is removed from the registry but left here, `compute_arg_ownership` would
/// produce `[Owned, Borrowed]` for a method the runtime doesn't recognize — a
/// potential RC imbalance.
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
/// but left here, uniqueness analysis would incorrectly mark it as producing
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
