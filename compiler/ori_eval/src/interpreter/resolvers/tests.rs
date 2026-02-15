use super::*;

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
