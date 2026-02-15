use super::*;
use ori_ir::{DerivedMethodInfo, DerivedTrait, ExprArena, SharedInterner};

fn dummy_name() -> Name {
    let interner = SharedInterner::default();
    interner.intern("dummy")
}

fn dummy_arena() -> SharedArena {
    SharedArena::new(ExprArena::new())
}

fn dummy_captures() -> Arc<FxHashMap<Name, Value>> {
    Arc::new(FxHashMap::default())
}

#[test]
fn test_register_and_lookup() {
    let interner = SharedInterner::default();
    let mut registry = UserMethodRegistry::new();
    let method = UserMethod::new(vec![dummy_name()], dummy_captures(), dummy_arena());

    let point = interner.intern("Point");
    let distance = interner.intern("distance");
    let other = interner.intern("other");
    let other_type = interner.intern("Other");

    registry.register(point, distance, method);

    assert!(registry.has_method(point, distance));
    assert!(!registry.has_method(point, other));
    assert!(!registry.has_method(other_type, distance));

    let found = registry.lookup(point, distance);
    assert!(found.is_some());
}

#[test]
fn test_empty_registry() {
    let interner = SharedInterner::default();
    let registry = UserMethodRegistry::new();
    let point = interner.intern("Point");
    let distance = interner.intern("distance");

    assert!(!registry.has_method(point, distance));
    assert!(registry.lookup(point, distance).is_none());
}

#[test]
fn test_derived_trait_from_name() {
    assert_eq!(DerivedTrait::from_name("Eq"), Some(DerivedTrait::Eq));
    assert_eq!(DerivedTrait::from_name("Clone"), Some(DerivedTrait::Clone));
    assert_eq!(
        DerivedTrait::from_name("Hashable"),
        Some(DerivedTrait::Hashable)
    );
    assert_eq!(
        DerivedTrait::from_name("Printable"),
        Some(DerivedTrait::Printable)
    );
    assert_eq!(
        DerivedTrait::from_name("Default"),
        Some(DerivedTrait::Default)
    );
    assert_eq!(DerivedTrait::from_name("Unknown"), None);
}

#[test]
fn test_derived_trait_method_name() {
    assert_eq!(DerivedTrait::Eq.method_name(), "eq");
    assert_eq!(DerivedTrait::Clone.method_name(), "clone");
    assert_eq!(DerivedTrait::Hashable.method_name(), "hash");
    assert_eq!(DerivedTrait::Printable.method_name(), "to_string");
    assert_eq!(DerivedTrait::Default.method_name(), "default");
}

#[test]
fn test_register_and_lookup_derived() {
    let interner = SharedInterner::default();
    let mut registry = UserMethodRegistry::new();

    let point = interner.intern("Point");
    let eq = interner.intern("eq");
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");
    let info = DerivedMethodInfo::new(DerivedTrait::Eq, vec![x_name, y_name]);

    registry.register_derived(point, eq, info);

    assert!(registry.has_method(point, eq));
    assert!(registry.lookup_derived(point, eq).is_some());
    assert!(registry.lookup(point, eq).is_none()); // not a user method

    let found = registry.lookup_derived(point, eq).unwrap();
    assert_eq!(found.trait_kind, DerivedTrait::Eq);
    assert_eq!(found.field_names.len(), 2);
}

#[test]
fn test_lookup_any() {
    let interner = SharedInterner::default();
    let mut registry = UserMethodRegistry::new();

    let point = interner.intern("Point");
    let distance = interner.intern("distance");
    let eq = interner.intern("eq");
    let nonexistent = interner.intern("nonexistent");

    // Register a user method
    let method = UserMethod::new(vec![dummy_name()], dummy_captures(), dummy_arena());
    registry.register(point, distance, method);

    // Register a derived method
    let x_name = interner.intern("x");
    let info = DerivedMethodInfo::new(DerivedTrait::Eq, vec![x_name]);
    registry.register_derived(point, eq, info);

    // Lookup user method via lookup_any
    if let Some(MethodEntry::User(_)) = registry.lookup_any(point, distance) {
        // ok
    } else {
        panic!("Expected User method entry");
    }

    // Lookup derived method via lookup_any
    if let Some(MethodEntry::Derived(info)) = registry.lookup_any(point, eq) {
        assert_eq!(info.trait_kind, DerivedTrait::Eq);
    } else {
        panic!("Expected Derived method entry");
    }

    // Lookup non-existent method
    assert!(registry.lookup_any(point, nonexistent).is_none());
}

#[test]
fn test_merge_registries() {
    let interner = SharedInterner::default();
    let mut registry1 = UserMethodRegistry::new();
    let mut registry2 = UserMethodRegistry::new();

    let point = interner.intern("Point");
    let distance = interner.intern("distance");
    let clone_name = interner.intern("clone");

    // Register in first registry
    let method = UserMethod::new(vec![dummy_name()], dummy_captures(), dummy_arena());
    registry1.register(point, distance, method);

    // Register derived in second registry
    let x_name = interner.intern("x");
    let info = DerivedMethodInfo::new(DerivedTrait::Clone, vec![x_name]);
    registry2.register_derived(point, clone_name, info);

    // Merge
    registry1.merge(registry2);

    // Both should be present
    assert!(registry1.has_method(point, distance));
    assert!(registry1.has_method(point, clone_name));
}
