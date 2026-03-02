use super::*;

#[test]
fn borrowing_method_names_sorted() {
    for window in BORROWING_METHOD_NAMES.windows(2) {
        assert!(
            window[0] < window[1],
            "BORROWING_METHOD_NAMES not sorted: {:?} >= {:?}",
            window[0],
            window[1],
        );
    }
}

#[test]
fn borrowing_method_names_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for &name in BORROWING_METHOD_NAMES {
        assert!(
            seen.insert(name),
            "duplicate in BORROWING_METHOD_NAMES: {name:?}",
        );
    }
}

#[test]
fn borrowing_builtin_names_returns_correct_count() {
    let interner = StringInterner::default();
    let names = borrowing_builtin_names(&interner);
    assert_eq!(
        names.len(),
        BORROWING_METHOD_NAMES.len(),
        "interned set should have same count as const array (no duplicates)"
    );
}

#[test]
fn iter_excluded() {
    assert!(
        !BORROWING_METHOD_NAMES.contains(&"iter"),
        "\"iter\" must not be in BORROWING_METHOD_NAMES — .iter() creates dependent values \
         (the iterator references the receiver's data). With borrowing, the ARC pipeline \
         would dec the list data before the iterator is consumed → use-after-free. \
         Instead, iter uses Owned semantics (default) and the runtime handles cleanup: \
         IterState::List stores the data pointer and drops it via ori_buffer_rc_dec."
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
fn push_not_in_borrowing() {
    assert!(
        !BORROWING_METHOD_NAMES.contains(&"push"),
        "\"push\" must not be in BORROWING — it's list-only and COW consuming"
    );
}

#[test]
fn pop_not_in_borrowing() {
    assert!(
        !BORROWING_METHOD_NAMES.contains(&"pop"),
        "\"pop\" must not be in BORROWING — it's list-only and COW consuming"
    );
}

#[test]
fn add_not_in_borrowing() {
    assert!(
        !BORROWING_METHOD_NAMES.contains(&"add"),
        "\"add\" must not be in BORROWING — it's list-only and COW consuming"
    );
}

#[test]
fn reverse_in_both_borrowing_and_consuming() {
    // "reverse" is borrowing for Ordering (Ordering.reverse() is a pure read)
    // but consuming for List (COW semantics). The consuming-receiver override
    // in annotate_arg_ownership handles the list case.
    assert!(
        BORROWING_METHOD_NAMES.contains(&"reverse"),
        "\"reverse\" must be in BORROWING — Ordering.reverse() borrows"
    );
    assert!(
        CONSUMING_RECEIVER_METHOD_NAMES.contains(&"reverse"),
        "\"reverse\" must be in CONSUMING — list.reverse() is COW consuming"
    );
}

#[test]
fn insert_not_in_borrowing() {
    // "insert" is COW consuming for all collection types (list, map, set).
    // All args are owned (key/value/elem transferred to collection).
    assert!(
        !BORROWING_METHOD_NAMES.contains(&"insert"),
        "\"insert\" must NOT be in BORROWING — COW consuming for all types"
    );
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
        !BORROWING_METHOD_NAMES.contains(&"remove"),
        "\"remove\" must NOT be in BORROWING — COW consuming for all types"
    );
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

#[test]
fn consuming_receiver_only_not_in_borrowing() {
    // These methods are COW consuming for all types — they must NOT be in
    // BORROWING, which would make all args (including the receiver) borrowed.
    for &method in CONSUMING_RECEIVER_ONLY_METHOD_NAMES {
        assert!(
            !BORROWING_METHOD_NAMES.contains(&method),
            "\"{method}\" is in CONSUMING_RECEIVER_ONLY but also in BORROWING — \
             a COW-consuming method cannot borrow its receiver"
        );
    }
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
