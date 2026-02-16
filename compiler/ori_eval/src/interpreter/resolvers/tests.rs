use super::*;

// Iterator method consistency

/// Verify `all_iterator_variants()` is exhaustive: every variant that passes
/// `is_iterator_method()` must appear in the list, and vice versa.
///
/// This catches the case where a new `Iter*` variant is added to the enum
/// and to `is_iterator_method()` but not to `all_iterator_variants()` (which
/// is the source of truth for resolver/dispatcher sync tests).
#[test]
fn iterator_variant_list_matches_predicate() {
    let list: Vec<CollectionMethod> = CollectionMethod::all_iterator_variants()
        .iter()
        .map(|&(_, method)| method)
        .collect();

    // Every listed variant must pass the predicate
    for method in &list {
        assert!(
            method.is_iterator_method(),
            "{method:?} is in all_iterator_variants() but is_iterator_method() returns false"
        );
    }

    // Count check: the predicate should match exactly the listed variants.
    // If a new Iter* variant is added to is_iterator_method() but not to
    // all_iterator_variants(), this will fail.
    let all_variants = [
        CollectionMethod::Map,
        CollectionMethod::Filter,
        CollectionMethod::Fold,
        CollectionMethod::Find,
        CollectionMethod::Collect,
        CollectionMethod::MapEntries,
        CollectionMethod::FilterEntries,
        CollectionMethod::Any,
        CollectionMethod::All,
        CollectionMethod::IterNext,
        CollectionMethod::IterMap,
        CollectionMethod::IterFilter,
        CollectionMethod::IterTake,
        CollectionMethod::IterSkip,
        CollectionMethod::IterEnumerate,
        CollectionMethod::IterZip,
        CollectionMethod::IterChain,
        CollectionMethod::IterFlatten,
        CollectionMethod::IterFlatMap,
        CollectionMethod::IterCycle,
        CollectionMethod::IterNextBack,
        CollectionMethod::IterRev,
        CollectionMethod::IterLast,
        CollectionMethod::IterRFind,
        CollectionMethod::IterRFold,
        CollectionMethod::IterFold,
        CollectionMethod::IterCount,
        CollectionMethod::IterFind,
        CollectionMethod::IterAny,
        CollectionMethod::IterAll,
        CollectionMethod::IterForEach,
        CollectionMethod::IterCollect,
    ];
    let predicate_count = all_variants
        .iter()
        .filter(|m| m.is_iterator_method())
        .count();
    assert_eq!(
        list.len(),
        predicate_count,
        "all_iterator_variants() has {} entries but is_iterator_method() matches {} variants \
         — a new Iter* variant was likely added to the enum without updating all_iterator_variants()",
        list.len(),
        predicate_count,
    );
}

/// Verify that iterator method names in `all_iterator_variants()` are unique.
#[test]
fn iterator_variant_names_unique() {
    let mut seen = std::collections::HashSet::new();
    for &(name, _) in CollectionMethod::all_iterator_variants() {
        assert!(
            seen.insert(name),
            "duplicate method name '{name}' in all_iterator_variants()"
        );
    }
}

#[test]
fn test_collection_method_from_name() {
    assert_eq!(
        CollectionMethod::from_name("map"),
        Some(CollectionMethod::Map)
    );
    assert_eq!(
        CollectionMethod::from_name("filter"),
        Some(CollectionMethod::Filter)
    );
    assert_eq!(
        CollectionMethod::from_name("fold"),
        Some(CollectionMethod::Fold)
    );
    assert_eq!(
        CollectionMethod::from_name("find"),
        Some(CollectionMethod::Find)
    );
    assert_eq!(
        CollectionMethod::from_name("collect"),
        Some(CollectionMethod::Collect)
    );
    assert_eq!(
        CollectionMethod::from_name("any"),
        Some(CollectionMethod::Any)
    );
    assert_eq!(
        CollectionMethod::from_name("all"),
        Some(CollectionMethod::All)
    );
    assert_eq!(CollectionMethod::from_name("unknown"), None);
}

#[test]
fn test_dispatcher_priority_ordering() {
    use crate::SharedMutableRegistry;
    use crate::UserMethodRegistry;
    use ori_ir::SharedInterner;

    let interner = SharedInterner::default();
    let registry = SharedMutableRegistry::new(UserMethodRegistry::new());

    // Create resolvers in wrong order
    let resolvers = vec![
        MethodResolverKind::Builtin(BuiltinMethodResolver::new(&interner)), // priority 2
        MethodResolverKind::UserRegistry(UserRegistryResolver::new(registry)), // priority 0
        MethodResolverKind::Collection(CollectionMethodResolver::new(&interner)), // priority 1
    ];

    let dispatcher = MethodDispatcher::new(resolvers);

    // Verify they're sorted by priority (0, 1, 2)
    assert_eq!(dispatcher.resolvers[0].priority(), 0);
    assert_eq!(dispatcher.resolvers[1].priority(), 1);
    assert_eq!(dispatcher.resolvers[2].priority(), 2);
    assert_eq!(dispatcher.resolvers[0].name(), "UserRegistryResolver");
    assert_eq!(dispatcher.resolvers[1].name(), "CollectionMethodResolver");
    assert_eq!(dispatcher.resolvers[2].name(), "BuiltinMethodResolver");
}

#[test]
fn test_resolver_kind_dispatch() {
    use ori_ir::SharedInterner;

    let interner = SharedInterner::default();

    // Test that MethodResolverKind correctly dispatches to underlying resolvers
    let builtin = MethodResolverKind::Builtin(BuiltinMethodResolver::new(&interner));
    assert_eq!(builtin.priority(), 2);
    assert_eq!(builtin.name(), "BuiltinMethodResolver");

    let collection = MethodResolverKind::Collection(CollectionMethodResolver::new(&interner));
    assert_eq!(collection.priority(), 1);
    assert_eq!(collection.name(), "CollectionMethodResolver");
}
