use super::*;

#[test]
fn registry_resolves_every_function_exp_kind_to_a_named_pattern() {
    let registry = PatternRegistry::new();
    assert_eq!(registry.len(), FunctionExpKind::ALL.len());

    // Cross-crate exhaustiveness: every FunctionExpKind variant declared in
    // ori_ir must resolve to a registered Pattern with a non-empty name. A
    // variant added to ori_ir::FunctionExpKind but left unregistered here is
    // caught because `get`'s exhaustive match forces the registration AND this
    // test proves each one yields a usable pattern.
    for kind in FunctionExpKind::ALL {
        let pattern = registry.get(kind);
        assert!(
            !pattern.name().is_empty(),
            "{kind:?} must resolve to a pattern with a non-empty name"
        );
    }
}

#[test]
fn test_pattern_names() {
    let registry = PatternRegistry::new();

    assert_eq!(registry.get(FunctionExpKind::Recurse).name(), "recurse");
    assert_eq!(registry.get(FunctionExpKind::Parallel).name(), "parallel");
    assert_eq!(registry.get(FunctionExpKind::Timeout).name(), "timeout");
    assert_eq!(registry.get(FunctionExpKind::Print).name(), "print");
    assert_eq!(registry.get(FunctionExpKind::Panic).name(), "panic");
    assert_eq!(registry.get(FunctionExpKind::Todo).name(), "todo");
    assert_eq!(
        registry.get(FunctionExpKind::Unreachable).name(),
        "unreachable"
    );
    assert_eq!(registry.get(FunctionExpKind::Channel).name(), "channel");
}

#[test]
fn test_required_props() {
    let registry = PatternRegistry::new();

    let timeout = registry.get(FunctionExpKind::Timeout);
    assert!(timeout.required_props().contains(&"operation"));
    assert!(timeout.required_props().contains(&"after"));

    let print = registry.get(FunctionExpKind::Print);
    assert!(print.required_props().contains(&"msg"));

    // todo and unreachable have no required props (reason is optional)
    let todo = registry.get(FunctionExpKind::Todo);
    assert!(todo.required_props().is_empty());

    let unreachable = registry.get(FunctionExpKind::Unreachable);
    assert!(unreachable.required_props().is_empty());

    // channel requires buffer
    let channel = registry.get(FunctionExpKind::Channel);
    assert!(channel.required_props().contains(&"buffer"));
}

#[test]
fn kinds_iterator_yields_exactly_the_canonical_inventory() {
    let registry = PatternRegistry::new();
    let kinds: Vec<_> = registry.kinds().collect();
    // kinds() derives from FunctionExpKind::ALL — pin it to the canonical
    // inventory so neither side can drift without the other.
    assert_eq!(kinds.as_slice(), FunctionExpKind::ALL.as_slice());
}

#[test]
fn test_pattern_enum_is_copy() {
    // Compile-time assertion that Pattern implements Copy
    fn assert_copy<T: Copy>() {}
    assert_copy::<Pattern>();

    // Runtime verification: can use original after copy
    let registry = PatternRegistry::new();
    let pattern = registry.get(FunctionExpKind::Print);
    let copy = pattern;
    assert_eq!(pattern.name(), copy.name());
}

#[test]
fn test_pattern_enum_size() {
    // All inner patterns are ZSTs, so the enum should just be the discriminant
    assert_eq!(std::mem::size_of::<Pattern>(), 1);
}
