use super::*;
use ori_ir::{ExprArena, StringInterner};

#[test]
fn register_builtin_ordering() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    register_builtin_types(&mut checker);

    // Verify Ordering is registered
    let ordering_name = interner.intern("Ordering");
    let entry = checker.type_registry().get_by_name(ordering_name);
    assert!(entry.is_some(), "Ordering should be registered");

    let entry = entry.unwrap();
    assert!(matches!(entry.kind, crate::TypeKind::Enum { ref variants } if variants.len() == 3));

    // The registered idx must match the pre-interned Idx::ORDERING primitive.
    // Without this, return type annotations (-> Ordering) resolve to Idx::ORDERING
    // but variant constructors (Less, Equal, Greater) return the Named idx, causing
    // unification failures.
    assert_eq!(entry.idx, Idx::ORDERING);
}

#[test]
fn ordering_variant_returns_pre_interned_idx() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    register_builtin_types(&mut checker);

    // When looking up a variant like `Less`, the returned type_entry.idx must
    // be Idx::ORDERING so that it unifies with return type annotations.
    let less_name = interner.intern("Less");
    let (type_entry, variant_def) = checker
        .type_registry()
        .lookup_variant_def(less_name)
        .expect("Less variant should be registered");

    assert_eq!(type_entry.idx, Idx::ORDERING);
    assert!(matches!(variant_def.fields, crate::VariantFields::Unit));
}

#[test]
fn ordering_lookup_by_pre_interned_idx() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    register_builtin_types(&mut checker);

    // Looking up by Idx::ORDERING must find the enum with 3 variants.
    // This is the idx that return type annotations resolve to.
    let entry = checker
        .type_registry()
        .get_by_idx(Idx::ORDERING)
        .expect("Ordering should be findable by Idx::ORDERING");

    assert!(
        matches!(entry.kind, crate::TypeKind::Enum { ref variants } if variants.len() == 3),
        "Idx::ORDERING should map to an enum with 3 variants"
    );
}

#[test]
fn empty_module_registration() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);
    let module = Module::default();

    // These should not panic
    register_user_types(&mut checker, &module);
    register_traits(&mut checker, &module);
    register_impls(&mut checker, &module);
    register_derived_impls(&mut checker, &module);
    register_consts(&mut checker, &module);
}

#[test]
fn trait_registry_integration() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);

    // After initialization, trait registry should be empty
    assert_eq!(checker.trait_registry().trait_count(), 0);
    assert_eq!(checker.trait_registry().impl_count(), 0);
}

#[test]
fn resolve_primitive_types() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Test primitive type resolution
    let int_parsed = ParsedType::Primitive(ori_ir::TypeId::from_raw(0));
    let int_idx = resolve_parsed_type_simple(&mut checker, &int_parsed);
    assert_eq!(int_idx, Idx::INT);

    let bool_parsed = ParsedType::Primitive(ori_ir::TypeId::from_raw(2));
    let bool_idx = resolve_parsed_type_simple(&mut checker, &bool_parsed);
    assert_eq!(bool_idx, Idx::BOOL);
}

#[test]
fn resolve_self_type() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Self type should resolve to a placeholder during registration
    let self_parsed = ParsedType::SelfType;
    let self_idx = resolve_type_with_params(&mut checker, &self_parsed, &[]);

    // Should be a named type (placeholder for Self)
    assert_eq!(checker.pool().tag(self_idx), crate::Tag::Named);
}

#[test]
fn resolve_type_with_self_substitution() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Create a concrete self type (e.g., Point)
    let point_name = interner.intern("Point");
    let self_type = checker.pool_mut().named(point_name);

    // Self should be substituted with Point
    let self_parsed = ParsedType::SelfType;
    let resolved = resolve_type_with_self(&mut checker, &self_parsed, &[], self_type);

    assert_eq!(resolved, self_type);
}
