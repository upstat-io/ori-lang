use super::*;

#[test]
fn test_registry_has_all_patterns() {
    let registry = PatternRegistry::new();
    assert_eq!(registry.len(), 15);

    // Verify each pattern is accessible (all FunctionExpKind variants are covered)
    let _ = registry.get(FunctionExpKind::Recurse);
    let _ = registry.get(FunctionExpKind::Parallel);
    let _ = registry.get(FunctionExpKind::Spawn);
    let _ = registry.get(FunctionExpKind::Timeout);
    let _ = registry.get(FunctionExpKind::Cache);
    let _ = registry.get(FunctionExpKind::With);
    let _ = registry.get(FunctionExpKind::Print);
    let _ = registry.get(FunctionExpKind::Panic);
    let _ = registry.get(FunctionExpKind::Catch);
    let _ = registry.get(FunctionExpKind::Todo);
    let _ = registry.get(FunctionExpKind::Unreachable);
    let _ = registry.get(FunctionExpKind::Channel);
    let _ = registry.get(FunctionExpKind::ChannelIn);
    let _ = registry.get(FunctionExpKind::ChannelOut);
    let _ = registry.get(FunctionExpKind::ChannelAll);
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
fn test_kinds_iterator() {
    let registry = PatternRegistry::new();
    let kinds: Vec<_> = registry.kinds().collect();
    assert_eq!(kinds.len(), 15);
    assert!(kinds.contains(&FunctionExpKind::Recurse));
    assert!(kinds.contains(&FunctionExpKind::Parallel));
    assert!(kinds.contains(&FunctionExpKind::Spawn));
    assert!(kinds.contains(&FunctionExpKind::Timeout));
    assert!(kinds.contains(&FunctionExpKind::Cache));
    assert!(kinds.contains(&FunctionExpKind::With));
    assert!(kinds.contains(&FunctionExpKind::Print));
    assert!(kinds.contains(&FunctionExpKind::Panic));
    assert!(kinds.contains(&FunctionExpKind::Catch));
    assert!(kinds.contains(&FunctionExpKind::Todo));
    assert!(kinds.contains(&FunctionExpKind::Unreachable));
    assert!(kinds.contains(&FunctionExpKind::Channel));
    assert!(kinds.contains(&FunctionExpKind::ChannelIn));
    assert!(kinds.contains(&FunctionExpKind::ChannelOut));
    assert!(kinds.contains(&FunctionExpKind::ChannelAll));
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
