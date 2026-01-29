//! Tests for the trait registry.

use crate::registry::{ImplEntry, ImplMethodDef, TraitEntry, TraitMethodDef, TraitRegistry};
use ori_ir::{SharedInterner, Span, TypeId, Visibility};
use ori_types::Type;

fn make_span() -> Span {
    Span::new(0, 0)
}

#[test]
fn test_trait_registry_creation() {
    let registry = TraitRegistry::new();
    assert_eq!(registry.trait_count(), 0);
    assert_eq!(registry.impl_count(), 0);
}

#[test]
fn test_register_trait() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let printable = interner.intern("Printable");
    let to_string = interner.intern("to_string");

    let entry = TraitEntry {
        name: printable,
        span: make_span(),
        type_params: vec![],
        super_traits: vec![],
        methods: vec![TraitMethodDef {
            name: to_string,
            params: vec![],
            return_ty: TypeId::STR,
            has_default: false,
        }],
        assoc_types: vec![],
        visibility: Visibility::Public,
    };

    registry.register_trait(entry);

    assert!(registry.has_trait(printable));
    assert_eq!(registry.trait_count(), 1);

    let retrieved = registry.get_trait(printable).unwrap();
    assert_eq!(retrieved.methods.len(), 1);
    assert_eq!(retrieved.methods[0].name, to_string);
}

#[test]
fn test_register_inherent_impl() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let point = interner.intern("Point");
    let new_name = interner.intern("new");
    let point_type_id = registry.interner().named(point);

    let entry = ImplEntry {
        trait_name: None,
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: new_name,
            params: vec![TypeId::INT, TypeId::INT],
            return_ty: point_type_id,
        }],
        assoc_types: vec![],
    };

    registry.register_impl(entry).unwrap();
    assert_eq!(registry.impl_count(), 1);

    let lookup = registry.lookup_method(&Type::Named(point), new_name);
    assert!(lookup.is_some());
    assert!(lookup.unwrap().trait_name.is_none());
}

#[test]
fn test_register_trait_impl() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let printable = interner.intern("Printable");
    let to_string = interner.intern("to_string");
    let point = interner.intern("Point");

    // First register the trait
    let trait_entry = TraitEntry {
        name: printable,
        span: make_span(),
        type_params: vec![],
        super_traits: vec![],
        methods: vec![TraitMethodDef {
            name: to_string,
            params: vec![],
            return_ty: TypeId::STR,
            has_default: false,
        }],
        assoc_types: vec![],
        visibility: Visibility::Public,
    };
    registry.register_trait(trait_entry);

    // Then register the impl
    let impl_entry = ImplEntry {
        trait_name: Some(printable),
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: to_string,
            params: vec![],
            return_ty: TypeId::STR,
        }],
        assoc_types: vec![],
    };
    registry.register_impl(impl_entry).unwrap();

    assert!(registry.implements(&Type::Named(point), printable));

    let lookup = registry.lookup_method(&Type::Named(point), to_string);
    assert!(lookup.is_some());
    let lookup = lookup.unwrap();
    assert_eq!(lookup.trait_name, Some(printable));
    assert_eq!(lookup.return_ty, Type::Str);
}

#[test]
fn test_method_lookup_priority() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let point = interner.intern("Point");
    let describe = interner.intern("describe");

    // Register inherent impl
    let inherent_entry = ImplEntry {
        trait_name: None,
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: describe,
            params: vec![],
            return_ty: TypeId::STR,
        }],
        assoc_types: vec![],
    };
    registry.register_impl(inherent_entry).unwrap();

    // Lookup should find inherent method (no trait)
    let lookup = registry
        .lookup_method(&Type::Named(point), describe)
        .unwrap();
    assert!(lookup.trait_name.is_none());
}

#[test]
fn test_coherence_duplicate_trait_impl() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let printable = interner.intern("Printable");
    let to_string = interner.intern("to_string");
    let point = interner.intern("Point");

    // Register the trait
    let trait_entry = TraitEntry {
        name: printable,
        span: make_span(),
        type_params: vec![],
        super_traits: vec![],
        methods: vec![TraitMethodDef {
            name: to_string,
            params: vec![],
            return_ty: TypeId::STR,
            has_default: false,
        }],
        assoc_types: vec![],
        visibility: Visibility::Public,
    };
    registry.register_trait(trait_entry);

    // First impl should succeed
    let impl1 = ImplEntry {
        trait_name: Some(printable),
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: to_string,
            params: vec![],
            return_ty: TypeId::STR,
        }],
        assoc_types: vec![],
    };
    assert!(registry.register_impl(impl1).is_ok());

    // Second impl for same trait/type should fail
    let impl2 = ImplEntry {
        trait_name: Some(printable),
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: to_string,
            params: vec![],
            return_ty: TypeId::STR,
        }],
        assoc_types: vec![],
    };
    assert!(registry.register_impl(impl2).is_err());
}

#[test]
fn test_coherence_duplicate_inherent_method() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let point = interner.intern("Point");
    let describe = interner.intern("describe");

    // First inherent impl should succeed
    let impl1 = ImplEntry {
        trait_name: None,
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: describe,
            params: vec![],
            return_ty: TypeId::STR,
        }],
        assoc_types: vec![],
    };
    assert!(registry.register_impl(impl1).is_ok());

    // Second inherent impl with same method name should fail
    let impl2 = ImplEntry {
        trait_name: None,
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: describe,
            params: vec![],
            return_ty: TypeId::INT,
        }],
        assoc_types: vec![],
    };
    assert!(registry.register_impl(impl2).is_err());
}

#[test]
fn test_coherence_multiple_inherent_impls_different_methods() {
    let interner = SharedInterner::default();
    let mut registry = TraitRegistry::new();

    let point = interner.intern("Point");
    let method1 = interner.intern("method1");
    let method2 = interner.intern("method2");

    // First inherent impl
    let impl1 = ImplEntry {
        trait_name: None,
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: method1,
            params: vec![],
            return_ty: TypeId::INT,
        }],
        assoc_types: vec![],
    };
    assert!(registry.register_impl(impl1).is_ok());

    // Second inherent impl with different method should succeed (methods get merged)
    let impl2 = ImplEntry {
        trait_name: None,
        self_ty: Type::Named(point),
        span: make_span(),
        type_params: vec![],
        methods: vec![ImplMethodDef {
            name: method2,
            params: vec![],
            return_ty: TypeId::STR,
        }],
        assoc_types: vec![],
    };
    assert!(registry.register_impl(impl2).is_ok());

    // Both methods should be accessible
    assert!(registry
        .lookup_method(&Type::Named(point), method1)
        .is_some());
    assert!(registry
        .lookup_method(&Type::Named(point), method2)
        .is_some());
}
