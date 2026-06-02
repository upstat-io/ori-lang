use super::*;
use crate::{Idx, ModuleChecker, ObjectSafetyViolation};
use ori_ir::{
    DerivedTrait, ExprArena, ExprId, Module, Name, ParsedType, ReprAttrKind, Span, StringInterner,
};

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
    let int_idx = resolve_parsed_type_simple(&mut checker, &int_parsed, &arena);
    assert_eq!(int_idx, Idx::INT);

    let bool_parsed = ParsedType::Primitive(ori_ir::TypeId::from_raw(2));
    let bool_idx = resolve_parsed_type_simple(&mut checker, &bool_parsed, &arena);
    assert_eq!(bool_idx, Idx::BOOL);
}

#[test]
fn resolve_self_type() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Self type should resolve to a placeholder during registration
    let self_parsed = ParsedType::SelfType;
    let self_idx = resolve_type_with_params(&mut checker, &self_parsed, &[], &arena);

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

// parsed_type_contains_self tests

#[test]
fn contains_self_direct() {
    let arena = ExprArena::new();
    assert!(parsed_type_contains_self(&arena, &ParsedType::SelfType));
}

#[test]
fn contains_self_primitive_is_false() {
    let arena = ExprArena::new();
    assert!(!parsed_type_contains_self(
        &arena,
        &ParsedType::Primitive(ori_ir::TypeId::INT)
    ));
}

#[test]
fn contains_self_in_list() {
    let mut arena = ExprArena::new();
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    assert!(parsed_type_contains_self(
        &arena,
        &ParsedType::List(self_id)
    ));
}

#[test]
fn contains_self_in_map_key() {
    let mut arena = ExprArena::new();
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    assert!(parsed_type_contains_self(
        &arena,
        &ParsedType::Map {
            key: self_id,
            value: int_id,
        }
    ));
}

#[test]
fn contains_self_in_map_value() {
    let mut arena = ExprArena::new();
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    assert!(parsed_type_contains_self(
        &arena,
        &ParsedType::Map {
            key: int_id,
            value: self_id,
        }
    ));
}

#[test]
fn contains_self_nested_function_return() {
    let mut arena = ExprArena::new();
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let params = arena.alloc_parsed_type_list(vec![int_id]);
    assert!(parsed_type_contains_self(
        &arena,
        &ParsedType::Function {
            params,
            ret: self_id,
        }
    ));
}

#[test]
fn contains_self_not_in_plain_function() {
    let mut arena = ExprArena::new();
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let bool_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::BOOL));
    let params = arena.alloc_parsed_type_list(vec![int_id]);
    assert!(!parsed_type_contains_self(
        &arena,
        &ParsedType::Function {
            params,
            ret: bool_id,
        }
    ));
}

// compute_object_safety_violations tests

/// Helper: create a simple Param.
fn make_param(name: Name, ty: Option<ParsedType>) -> ori_ir::Param {
    ori_ir::Param {
        name,
        pattern: None,
        ty,
        default: None,
        span: ori_ir::Span::DUMMY,
        is_variadic: false,
    }
}

#[test]
fn object_safe_trait_has_no_violations() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let method_name = interner.intern("to_str");
    let self_name = interner.intern("self");

    // @to_str (self) -> str
    let params = arena.alloc_params(vec![make_param(self_name, None)]);

    let trait_def = ori_ir::TraitDef {
        name: interner.intern("Printable"),
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits: vec![],
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: method_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(3)), // str
            where_clauses: vec![],
            span: ori_ir::Span::DUMMY,
        })],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    };

    // Create checker AFTER arena mutations
    let checker = ModuleChecker::new(&arena, &interner);
    let violations = compute_object_safety_violations(&checker, &trait_def, &arena);
    assert!(violations.is_empty(), "Printable should be object-safe");
}

#[test]
fn self_return_violates_object_safety() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let method_name = interner.intern("clone");
    let self_name = interner.intern("self");

    // @clone (self) -> Self
    let params = arena.alloc_params(vec![make_param(self_name, None)]);

    let trait_def = ori_ir::TraitDef {
        name: interner.intern("Clone"),
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits: vec![],
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: method_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: ParsedType::SelfType,
            where_clauses: vec![],
            span: ori_ir::Span::DUMMY,
        })],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    };

    let checker = ModuleChecker::new(&arena, &interner);
    let violations = compute_object_safety_violations(&checker, &trait_def, &arena);
    assert_eq!(violations.len(), 1);
    assert!(
        matches!(&violations[0], ObjectSafetyViolation::SelfReturn { method, .. } if *method == method_name)
    );
}

#[test]
fn self_param_violates_object_safety() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let method_name = interner.intern("equals");
    let self_name = interner.intern("self");
    let other_name = interner.intern("other");

    // @equals (self, other: Self) -> bool
    let params = arena.alloc_params(vec![
        make_param(self_name, None),
        make_param(other_name, Some(ParsedType::SelfType)),
    ]);

    let trait_def = ori_ir::TraitDef {
        name: interner.intern("Eq"),
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits: vec![],
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: method_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(2)), // bool
            where_clauses: vec![],
            span: ori_ir::Span::DUMMY,
        })],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    };

    let checker = ModuleChecker::new(&arena, &interner);
    let violations = compute_object_safety_violations(&checker, &trait_def, &arena);
    assert_eq!(violations.len(), 1);
    assert!(
        matches!(&violations[0], ObjectSafetyViolation::SelfParam { method, param, .. }
            if *method == method_name && *param == other_name)
    );
}

#[test]
fn multiple_violations_in_single_trait() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let self_name = interner.intern("self");
    let clone_name = interner.intern("clone");
    let eq_name = interner.intern("equals");
    let other_name = interner.intern("other");

    // Method 1: @clone (self) -> Self (Rule 1 violation)
    let params1 = arena.alloc_params(vec![make_param(self_name, None)]);

    // Method 2: @equals (self, other: Self) -> bool (Rule 2 violation)
    let params2 = arena.alloc_params(vec![
        make_param(self_name, None),
        make_param(other_name, Some(ParsedType::SelfType)),
    ]);

    let trait_def = ori_ir::TraitDef {
        name: interner.intern("CloneEq"),
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits: vec![],
        items: vec![
            ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
                name: clone_name,
                generics: ori_ir::GenericParamRange::EMPTY,
                params: params1,
                return_ty: ParsedType::SelfType,
                where_clauses: vec![],
                span: ori_ir::Span::DUMMY,
            }),
            ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
                name: eq_name,
                generics: ori_ir::GenericParamRange::EMPTY,
                params: params2,
                return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(2)),
                where_clauses: vec![],
                span: ori_ir::Span::DUMMY,
            }),
        ],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    };

    let checker = ModuleChecker::new(&arena, &interner);
    let violations = compute_object_safety_violations(&checker, &trait_def, &arena);
    assert_eq!(violations.len(), 2, "Should detect both violations");
    assert!(matches!(
        &violations[0],
        ObjectSafetyViolation::SelfReturn { .. }
    ));
    assert!(matches!(
        &violations[1],
        ObjectSafetyViolation::SelfParam { .. }
    ));
}

#[test]
fn self_in_receiver_position_is_allowed() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let method_name = interner.intern("show");
    let self_name = interner.intern("self");

    // @show (self) -> str — self receiver has explicit Self type, still OK
    let params = arena.alloc_params(vec![make_param(self_name, Some(ParsedType::SelfType))]);

    let trait_def = ori_ir::TraitDef {
        name: interner.intern("Show"),
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits: vec![],
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: method_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(3)), // str
            where_clauses: vec![],
            span: ori_ir::Span::DUMMY,
        })],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    };

    let checker = ModuleChecker::new(&arena, &interner);
    let violations = compute_object_safety_violations(&checker, &trait_def, &arena);
    assert!(
        violations.is_empty(),
        "Self in receiver position should not violate object safety"
    );
}

// ── Super-trait inheritance (Phase B.1 — register_object_safety_violations) ──

/// Helper: build a `TraitDef` with one generic method `@<method_name><T> (self) -> T`.
/// The trait's direct items therefore violate object-safety Rule 3 (`GenericMethod`).
fn make_trait_with_generic_method(
    arena: &mut ExprArena,
    interner: &StringInterner,
    trait_name: Name,
    method_name: Name,
    super_traits: Vec<ori_ir::TraitBound>,
) -> ori_ir::TraitDef {
    let self_name = interner.intern("self");
    let t_param_name = interner.intern("T");
    let params = arena.alloc_params(vec![make_param(self_name, None)]);
    let method_generics = arena.alloc_generic_params(vec![ori_ir::GenericParam {
        name: t_param_name,
        bounds: vec![],
        default_type: None,
        is_const: false,
        const_type: None,
        default_value: None,
        span: ori_ir::Span::DUMMY,
    }]);
    ori_ir::TraitDef {
        name: trait_name,
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits,
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: method_name,
            generics: method_generics,
            params,
            // Return type is `int` here for simplicity — the GenericMethod
            // violation is triggered by the method's own generics being
            // non-empty, not by what the return type is.
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(0)),
            where_clauses: vec![],
            span: ori_ir::Span::DUMMY,
        })],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    }
}

/// Helper: build a non-generic trait with optional super-trait list. Direct
/// items are object-safe; violations only appear via inheritance from
/// `super_traits` after `register_object_safety_violations` runs.
fn make_trait_no_generic_method(
    arena: &mut ExprArena,
    interner: &StringInterner,
    trait_name: Name,
    method_name: Name,
    super_traits: Vec<ori_ir::TraitBound>,
) -> ori_ir::TraitDef {
    let self_name = interner.intern("self");
    let params = arena.alloc_params(vec![make_param(self_name, None)]);
    ori_ir::TraitDef {
        name: trait_name,
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits,
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: method_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(0)), // int
            where_clauses: vec![],
            span: ori_ir::Span::DUMMY,
        })],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    }
}

#[test]
fn super_trait_propagates_generic_method_parent_first() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let super_name = interner.intern("Super");
    let sub_name = interner.intern("Sub");
    let generic_method = interner.intern("generic");
    let ok_method = interner.intern("ok");

    let super_def =
        make_trait_with_generic_method(&mut arena, &interner, super_name, generic_method, vec![]);
    let super_bound = ori_ir::TraitBound {
        first: super_name,
        rest: vec![],
        span: ori_ir::Span::DUMMY,
    };
    let sub_def = make_trait_no_generic_method(
        &mut arena,
        &interner,
        sub_name,
        ok_method,
        vec![super_bound],
    );

    let module = Module {
        traits: vec![super_def, sub_def], // parent first
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_traits(&mut checker, &module);
    register_object_safety_violations(&mut checker, &module);

    let sub_entry = checker
        .trait_registry()
        .get_trait_by_name(sub_name)
        .expect("Sub registered");
    assert_eq!(
        sub_entry.object_safety_violations.len(),
        1,
        "Sub should inherit one GenericMethod violation from Super"
    );
    assert!(
        matches!(
            &sub_entry.object_safety_violations[0],
            ObjectSafetyViolation::GenericMethod { method, .. } if *method == generic_method
        ),
        "inherited violation must be the GenericMethod from Super.@generic<T>"
    );
}

#[test]
fn super_trait_propagates_generic_method_child_first() {
    // Order-independence: declaring Sub before Super must still produce the
    // inherited violation, because two-phase registration computes violations
    // AFTER all traits are registered.
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let super_name = interner.intern("Super");
    let sub_name = interner.intern("Sub");
    let generic_method = interner.intern("generic");
    let ok_method = interner.intern("ok");

    let super_bound = ori_ir::TraitBound {
        first: super_name,
        rest: vec![],
        span: ori_ir::Span::DUMMY,
    };
    let sub_def = make_trait_no_generic_method(
        &mut arena,
        &interner,
        sub_name,
        ok_method,
        vec![super_bound],
    );
    let super_def =
        make_trait_with_generic_method(&mut arena, &interner, super_name, generic_method, vec![]);

    let module = Module {
        traits: vec![sub_def, super_def], // child first
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_traits(&mut checker, &module);
    register_object_safety_violations(&mut checker, &module);

    let sub_entry = checker
        .trait_registry()
        .get_trait_by_name(sub_name)
        .expect("Sub registered");
    assert_eq!(
        sub_entry.object_safety_violations.len(),
        1,
        "child-first ordering must still inherit Super's GenericMethod"
    );
    assert!(matches!(
        &sub_entry.object_safety_violations[0],
        ObjectSafetyViolation::GenericMethod { method, .. } if *method == generic_method
    ));
}

#[test]
fn super_trait_propagates_through_multi_level_chain() {
    // C: B, B: A, A has generic method. C MUST inherit transitively.
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let a_name = interner.intern("A");
    let b_name = interner.intern("B");
    let c_name = interner.intern("C");
    let generic_method = interner.intern("generic");
    let ok_b = interner.intern("ok_b");
    let ok_c = interner.intern("ok_c");

    let a_def =
        make_trait_with_generic_method(&mut arena, &interner, a_name, generic_method, vec![]);
    let a_bound = ori_ir::TraitBound {
        first: a_name,
        rest: vec![],
        span: ori_ir::Span::DUMMY,
    };
    let b_def = make_trait_no_generic_method(&mut arena, &interner, b_name, ok_b, vec![a_bound]);
    let b_bound = ori_ir::TraitBound {
        first: b_name,
        rest: vec![],
        span: ori_ir::Span::DUMMY,
    };
    let c_def = make_trait_no_generic_method(&mut arena, &interner, c_name, ok_c, vec![b_bound]);

    let module = Module {
        traits: vec![a_def, b_def, c_def],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_traits(&mut checker, &module);
    register_object_safety_violations(&mut checker, &module);

    // Both B and C must inherit A's GenericMethod via the transitive DAG walk.
    let b_entry = checker
        .trait_registry()
        .get_trait_by_name(b_name)
        .expect("B registered");
    assert_eq!(
        b_entry.object_safety_violations.len(),
        1,
        "B inherits A's GenericMethod"
    );

    let c_entry = checker
        .trait_registry()
        .get_trait_by_name(c_name)
        .expect("C registered");
    assert_eq!(
        c_entry.object_safety_violations.len(),
        1,
        "C inherits A's GenericMethod transitively through B"
    );
}

#[test]
fn no_super_trait_means_no_inheritance() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let plain_name = interner.intern("Plain");
    let plain_method = interner.intern("plain");
    let plain_def =
        make_trait_no_generic_method(&mut arena, &interner, plain_name, plain_method, vec![]);

    let module = Module {
        traits: vec![plain_def],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_traits(&mut checker, &module);
    register_object_safety_violations(&mut checker, &module);

    let entry = checker
        .trait_registry()
        .get_trait_by_name(plain_name)
        .expect("Plain registered");
    assert!(
        entry.object_safety_violations.is_empty(),
        "trait with no super-traits and no own violations stays object-safe"
    );
}

// ── E2029: Derive Hashable without Eq ────────────────────────────────

#[test]
fn derive_hashable_without_eq_emits_error() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);

    let hashable = interner.intern("Hashable");
    let type_name = interner.intern("Point");
    let type_decl = ori_ir::TypeDecl {
        name: type_name,
        kind: ori_ir::TypeDeclKind::Struct(vec![]),
        generics: ori_ir::GenericParamRange::EMPTY,
        where_clauses: vec![],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
        derives: vec![hashable], // only Hashable, no Eq
        repr_attrs: vec![],
        target_attr: None,
        cfg_attr: None,
    };

    register_derived_impl(&mut checker, &type_decl, hashable);

    let errors = checker.errors();
    assert_eq!(errors.len(), 1, "expected exactly one error");
    assert_eq!(errors[0].code(), ori_diagnostic::ErrorCode::E2029);
}

#[test]
fn derive_eq_and_hashable_succeeds() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);

    let eq = interner.intern("Eq");
    let hashable = interner.intern("Hashable");
    let type_name = interner.intern("Point");
    let type_decl = ori_ir::TypeDecl {
        name: type_name,
        kind: ori_ir::TypeDeclKind::Struct(vec![]),
        generics: ori_ir::GenericParamRange::EMPTY,
        where_clauses: vec![],
        span: ori_ir::Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
        derives: vec![eq, hashable], // both Eq and Hashable
        repr_attrs: vec![],
        target_attr: None,
        cfg_attr: None,
    };

    // Register Eq first (as the derive processor would)
    register_derived_impl(&mut checker, &type_decl, eq);
    // Now register Hashable — should succeed
    register_derived_impl(&mut checker, &type_decl, hashable);

    let errors = checker.errors();
    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
}

// resolve_type_with_params — compound Self recursion tests

#[test]
fn resolve_type_with_params_self_in_list() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build ParsedType::List(SelfType) — e.g., `-> [Self]` in a trait method
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let list_of_self = ParsedType::List(self_id);

    let mut checker = ModuleChecker::new(&arena, &interner);
    let result = resolve_type_with_params(&mut checker, &list_of_self, &[], &arena);

    // Outer type should be a List
    assert_eq!(
        checker.pool().tag(result),
        crate::Tag::List,
        "Should produce a List type"
    );

    // Inner element should be Named("Self"), NOT Idx::ERROR
    let elem = checker.pool().list_elem(result);
    assert_ne!(
        elem,
        Idx::ERROR,
        "Self inside List should not resolve to ERROR"
    );
    assert_eq!(
        checker.pool().tag(elem),
        crate::Tag::Named,
        "Self should become Named placeholder"
    );
}

#[test]
fn resolve_type_with_params_self_in_tuple() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build ParsedType::Tuple([SelfType, int]) — e.g., `-> (Self, int)`
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let elems = arena.alloc_parsed_type_list(vec![self_id, int_id]);
    let tuple_with_self = ParsedType::Tuple(elems);

    let mut checker = ModuleChecker::new(&arena, &interner);
    let result = resolve_type_with_params(&mut checker, &tuple_with_self, &[], &arena);

    assert_eq!(
        checker.pool().tag(result),
        crate::Tag::Tuple,
        "Should produce a Tuple type"
    );

    // First element (Self) should be Named, not ERROR
    let first = checker.pool().tuple_elem(result, 0);
    assert_ne!(
        first,
        Idx::ERROR,
        "Self inside Tuple should not resolve to ERROR"
    );
    assert_eq!(checker.pool().tag(first), crate::Tag::Named);

    // Second element (int) should be Int
    let second = checker.pool().tuple_elem(result, 1);
    assert_eq!(second, Idx::INT, "int element should resolve to Idx::INT");
}

#[test]
fn resolve_type_with_params_self_in_map_value() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build ParsedType::Map { key: str, value: SelfType } — e.g., `Map<str, Self>`
    let str_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::STR));
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let map_with_self = ParsedType::Map {
        key: str_id,
        value: self_id,
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    let result = resolve_type_with_params(&mut checker, &map_with_self, &[], &arena);

    assert_eq!(
        checker.pool().tag(result),
        crate::Tag::Map,
        "Should produce a Map type"
    );

    // Key should be str
    let key = checker.pool().map_key(result);
    assert_eq!(key, Idx::STR, "Map key should be str");

    // Value (Self) should be Named, not ERROR
    let value = checker.pool().map_value(result);
    assert_ne!(
        value,
        Idx::ERROR,
        "Self in Map value should not resolve to ERROR"
    );
    assert_eq!(checker.pool().tag(value), crate::Tag::Named);
}

#[test]
fn resolve_type_with_params_self_in_function_return() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build ParsedType::Function { params: [int], ret: SelfType }
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let params = arena.alloc_parsed_type_list(vec![int_id]);
    let fn_returning_self = ParsedType::Function {
        params,
        ret: self_id,
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    let result = resolve_type_with_params(&mut checker, &fn_returning_self, &[], &arena);

    assert_eq!(
        checker.pool().tag(result),
        crate::Tag::Function,
        "Should produce a Function type"
    );

    // Return type (Self) should be Named, not ERROR
    let ret = checker.pool().function_return(result);
    assert_ne!(
        ret,
        Idx::ERROR,
        "Self in function return should not resolve to ERROR"
    );
    assert_eq!(checker.pool().tag(ret), crate::Tag::Named);
}

#[test]
fn resolve_type_with_params_type_param_in_list() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build ParsedType::List(Named { name: "T", type_args: [] }) — e.g., `-> [T]`
    let t_name = interner.intern("T");
    let empty_args = arena.alloc_parsed_type_list(vec![]);
    let t_id = arena.alloc_parsed_type(ParsedType::Named {
        name: t_name,
        type_args: empty_args,
    });
    let list_of_t = ParsedType::List(t_id);

    let mut checker = ModuleChecker::new(&arena, &interner);
    let result = resolve_type_with_params(&mut checker, &list_of_t, &[t_name], &arena);

    assert_eq!(
        checker.pool().tag(result),
        crate::Tag::List,
        "Should produce a List type"
    );

    // Element (T) should be a Named type param
    let elem = checker.pool().list_elem(result);
    assert_ne!(
        elem,
        Idx::ERROR,
        "Type param inside List should not resolve to ERROR"
    );
    assert_eq!(checker.pool().tag(elem), crate::Tag::Named);
}

#[test]
fn resolve_type_with_params_nested_self_in_list_of_tuples() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Build ParsedType::List(Tuple([SelfType, int])) — e.g., `-> [(Self, int)]`
    let self_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let tuple_elems = arena.alloc_parsed_type_list(vec![self_id, int_id]);
    let tuple_id = arena.alloc_parsed_type(ParsedType::Tuple(tuple_elems));
    let list_of_tuple = ParsedType::List(tuple_id);

    let mut checker = ModuleChecker::new(&arena, &interner);
    let result = resolve_type_with_params(&mut checker, &list_of_tuple, &[], &arena);

    assert_eq!(
        checker.pool().tag(result),
        crate::Tag::List,
        "Outer type should be List"
    );

    // Inner element should be a Tuple
    let tuple = checker.pool().list_elem(result);
    assert_eq!(
        checker.pool().tag(tuple),
        crate::Tag::Tuple,
        "Inner should be Tuple"
    );

    // First tuple element (Self) should be Named, not ERROR
    let first = checker.pool().tuple_elem(tuple, 0);
    assert_ne!(first, Idx::ERROR, "Nested Self should not resolve to ERROR");
    assert_eq!(checker.pool().tag(first), crate::Tag::Named);
}

// --- Cross-crate sync enforcement ---

#[test]
fn all_derived_traits_have_type_signatures() {
    // Verifies that build_derived_methods() produces a valid method
    // for every DerivedTrait variant. Catches: "added trait to enum
    // but forgot to register its signature in the type checker."
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);

    for &trait_kind in DerivedTrait::ALL {
        let type_name = interner.intern("TestType");
        let self_type = checker.pool_mut().named(type_name);

        let methods = build_derived_methods(&mut checker, trait_kind, self_type, Span::DUMMY);

        assert!(
            !methods.is_empty(),
            "DerivedTrait::{:?} (trait '{}') produced no methods in type checker",
            trait_kind,
            trait_kind.trait_name()
        );

        // Verify the method name matches
        let method_name = interner.intern(trait_kind.method_name());
        assert!(
            methods.contains_key(&method_name),
            "DerivedTrait::{trait_kind:?} registered method name doesn't match method_name()",
        );
    }
}

#[test]
fn all_derived_traits_have_well_known_names() {
    // Verifies that every DerivedTrait has a corresponding pre-interned name
    // in WellKnownNames. The exhaustive match forces a compile error if a new
    // variant is added without mapping it to a WellKnownNames field.
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);
    let wk = checker.well_known();

    for &trait_kind in DerivedTrait::ALL {
        let trait_name = interner.intern(trait_kind.trait_name());

        // Verify each DerivedTrait's name is registered in the trait satisfaction
        // bitfield. Adding a DerivedTrait without a trait_bits entry means it
        // won't participate in satisfaction checks.
        assert!(
            wk.has_trait_bit(trait_name),
            "DerivedTrait::{:?} trait_name '{}' has no bit in trait satisfaction system",
            trait_kind,
            trait_kind.trait_name()
        );
    }
}

// #repr attribute validation

/// Helper: create a Module with a single type declaration for repr validation tests.
fn make_module_with_repr(
    interner: &StringInterner,
    name: &str,
    kind: ori_ir::TypeDeclKind,
    repr_attrs: Vec<ReprAttrKind>,
) -> Module {
    let mut module = Module::default();
    module.types.push(ori_ir::TypeDecl {
        name: interner.intern(name),
        kind,
        generics: ori_ir::GenericParamRange::EMPTY,
        where_clauses: vec![],
        span: Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
        derives: vec![],
        repr_attrs,
        target_attr: None,
        cfg_attr: None,
    });
    module
}

/// Explicit `#repr("transparent")` on newtypes must be rejected with E2041.
///
/// The spec says `#repr` applies only to struct types. Newtypes are implicitly
/// transparent. Explicit `#repr("transparent")` on a newtype is an error, not
/// a redundant no-op.
#[test]
fn repr_transparent_on_newtype_rejected() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let module = make_module_with_repr(
        &interner,
        "UserId",
        ori_ir::TypeDeclKind::Newtype(ParsedType::Primitive(ori_ir::TypeId::from_raw(0))),
        vec![ReprAttrKind::Transparent],
    );
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    assert_eq!(
        errors.len(),
        1,
        "expected E2041 for transparent on newtype, got: {errors:?}"
    );
    assert_eq!(errors[0].code(), ori_diagnostic::ErrorCode::E2041);
}

/// Duplicate same-kind `#repr` attributes must be rejected with E2041.
///
/// The spec only permits `c + aligned` as a valid multi-attribute combination.
/// Same-kind duplicates like `#repr("c") #repr("c")` are invalid.
#[test]
fn repr_duplicate_c_rejected() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let module = make_module_with_repr(
        &interner,
        "DupC",
        ori_ir::TypeDeclKind::Struct(vec![ori_ir::StructField {
            name: interner.intern("x"),
            ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(0)),
            span: Span::DUMMY,
        }]),
        vec![ReprAttrKind::C, ReprAttrKind::C],
    );
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    assert_eq!(
        errors.len(),
        1,
        "expected E2041 for duplicate #repr(\"c\"), got: {errors:?}"
    );
    assert_eq!(errors[0].code(), ori_diagnostic::ErrorCode::E2041);
}

/// Duplicate `#repr("aligned", N)` with different N must be rejected.
#[test]
fn repr_duplicate_aligned_rejected() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let module = make_module_with_repr(
        &interner,
        "DupAligned",
        ori_ir::TypeDeclKind::Struct(vec![ori_ir::StructField {
            name: interner.intern("x"),
            ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(0)),
            span: Span::DUMMY,
        }]),
        vec![ReprAttrKind::Aligned(8), ReprAttrKind::Aligned(16)],
    );
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    assert_eq!(
        errors.len(),
        1,
        "expected E2041 for duplicate #repr(\"aligned\"), got: {errors:?}"
    );
    assert_eq!(errors[0].code(), ori_diagnostic::ErrorCode::E2041);
}

/// semantic pin: the ONLY valid multi-attribute case is `c + aligned(N)`.
#[test]
fn repr_c_plus_aligned_still_valid() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let module = make_module_with_repr(
        &interner,
        "CAligned",
        ori_ir::TypeDeclKind::Struct(vec![ori_ir::StructField {
            name: interner.intern("x"),
            ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(0)),
            span: Span::DUMMY,
        }]),
        vec![ReprAttrKind::C, ReprAttrKind::Aligned(16)],
    );
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    assert!(
        errors.is_empty(),
        "c + aligned should be valid, got errors: {errors:?}"
    );
}

// ── E2049: Value + Drop mutual exclusion ───────────────────────────────

/// Helper: build a minimal `impl Point: Drop { @drop (self) -> void = ... }`
/// `ImplDef` over a struct named `Point`. The body `ExprId` is `INVALID`
/// (Phase 4 body-check skipped); only the `trait_path`/`self_path` are
/// load-bearing for the conflict-detection wire-up.
fn make_drop_impl_for(
    arena: &mut ExprArena,
    interner: &StringInterner,
    type_name: Name,
) -> ori_ir::ImplDef {
    let drop_name = interner.intern("Drop");
    let drop_method = interner.intern("drop");
    let self_name = interner.intern("self");
    let params = arena.alloc_params(vec![make_param(self_name, None)]);
    // Idx::UNIT corresponds to TypeId::from_raw(6) per §TY-5.
    let unit_primitive = ParsedType::Primitive(ori_ir::TypeId::from_raw(6));
    ori_ir::ImplDef {
        generics: ori_ir::GenericParamRange::EMPTY,
        trait_path: Some(vec![drop_name]),
        trait_type_args: ori_ir::ParsedTypeRange::EMPTY,
        self_path: vec![type_name],
        self_ty: ParsedType::Named {
            name: type_name,
            type_args: ori_ir::ParsedTypeRange::EMPTY,
        },
        where_clauses: vec![],
        methods: vec![ori_ir::ImplMethod {
            name: drop_method,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: unit_primitive,
            capabilities: vec![],
            where_clauses: vec![],
            body: ExprId::INVALID,
            span: Span::DUMMY,
        }],
        assoc_types: vec![],
        span: Span::DUMMY,
        target_attr: None,
        cfg_attr: None,
    }
}

/// Helper: build a minimal `trait Drop { @drop (self) -> void }` `TraitDef`.
/// Sufficient for `register_traits` + `register_impl` to wire Drop's
/// trait `Idx`; method signature shape is not consumed by the conflict
/// checks.
fn make_drop_trait(arena: &mut ExprArena, interner: &StringInterner) -> ori_ir::TraitDef {
    let drop_method = interner.intern("drop");
    let self_name = interner.intern("self");
    let params = arena.alloc_params(vec![make_param(self_name, None)]);
    let unit_primitive = ParsedType::Primitive(ori_ir::TypeId::from_raw(6));
    ori_ir::TraitDef {
        name: interner.intern("Drop"),
        generics: ori_ir::GenericParamRange::EMPTY,
        super_traits: vec![],
        items: vec![ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
            name: drop_method,
            generics: ori_ir::GenericParamRange::EMPTY,
            params,
            return_ty: unit_primitive,
            where_clauses: vec![],
            span: Span::DUMMY,
        })],
        span: Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
    }
}

/// Helper: build a `type T = { x: int }` struct decl carrying the named
/// derives (e.g., `["Value", "Eq"]`).
fn make_struct_decl_with_derives(
    interner: &StringInterner,
    type_name: &str,
    derive_names: &[&str],
) -> ori_ir::TypeDecl {
    let derives: Vec<Name> = derive_names.iter().map(|n| interner.intern(n)).collect();
    ori_ir::TypeDecl {
        name: interner.intern(type_name),
        kind: ori_ir::TypeDeclKind::Struct(vec![ori_ir::StructField {
            name: interner.intern("x"),
            ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(0)), // int
            span: Span::DUMMY,
        }]),
        generics: ori_ir::GenericParamRange::EMPTY,
        where_clauses: vec![],
        span: Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
        derives,
        repr_attrs: vec![],
        target_attr: None,
        cfg_attr: None,
    }
}

/// Surface 1 — `register_user_types` detects an existing `impl T: Drop`
/// and emits E2049 with span at the type declaration.
///
/// Order: Drop impl registered FIRST, then `type T: Value, ...` declared.
/// Second registration (the type-decl) is the one rejected.
#[test]
fn value_type_decl_with_drop_impl_emits_e2049_at_user_types_surface() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let point_name = interner.intern("Point");
    // Drop trait must exist for impl registration to wire correctly.
    let drop_trait = make_drop_trait(&mut arena, &interner);
    // Drop impl over Point — Point must be in the pool already, which
    // `register_impls` handles via `pool_mut().named(...)`.
    let drop_impl = make_drop_impl_for(&mut arena, &interner, point_name);
    // Type decl with `derives = ["Value"]` (Surface 1 trigger).
    let point_decl = make_struct_decl_with_derives(&interner, "Point", &["Value"]);

    // Order: Drop impl FIRST (parent already has type registered as Named
    // via pool); then `type Point: Value, ...` lands second AND collides.
    let module = Module {
        traits: vec![drop_trait],
        impls: vec![drop_impl],
        types: vec![point_decl],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);
    register_traits(&mut checker, &module);
    register_impls(&mut checker, &module);
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    let e2049_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2049)
        .count();
    assert_eq!(
        e2049_count, 1,
        "expected exactly one E2049 at Surface 1, got errors: {errors:?}"
    );
}

/// Surface 2 — `register_impl` detects an existing `Value` marker on the
/// type and emits E2049 with span at the impl declaration.
///
/// Order: `type Point: Value, ...` declared FIRST, then `impl Point: Drop`
/// registered SECOND. The impl registration is the one rejected.
#[test]
fn drop_impl_for_value_type_emits_e2049_at_register_impl_surface() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let point_name = interner.intern("Point");
    let drop_trait = make_drop_trait(&mut arena, &interner);
    let drop_impl = make_drop_impl_for(&mut arena, &interner, point_name);
    let point_decl = make_struct_decl_with_derives(&interner, "Point", &["Value"]);

    // Order: type decl FIRST (Surface 1 records the marker), then impl
    // SECOND (Surface 2 reads it + emits).
    let module = Module {
        types: vec![point_decl],
        traits: vec![drop_trait],
        impls: vec![drop_impl],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);
    register_user_types(&mut checker, &module);
    register_traits(&mut checker, &module);
    register_impls(&mut checker, &module);

    let errors = checker.errors();
    let e2049_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2049)
        .count();
    assert_eq!(
        e2049_count, 1,
        "expected exactly one E2049 at Surface 2, got errors: {errors:?}"
    );
}

/// Positive — clean Value type (no Drop impl) registers without error
/// AND populates an empty `UserBurdenSpec`.
#[test]
fn value_type_without_drop_impl_registers_empty_user_burden_spec() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let point_decl = make_struct_decl_with_derives(&interner, "Point", &["Value"]);
    let module = Module {
        types: vec![point_decl],
        ..Module::default()
    };

    register_builtin_types(&mut checker);
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    let e2049_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2049)
        .count();
    assert_eq!(
        e2049_count, 0,
        "Value-only type must NOT emit E2049, got errors: {errors:?}"
    );

    // Verify the marker was recorded.
    let point_name = interner.intern("Point");
    let point_idx = checker.type_registry().get_by_name(point_name).unwrap().idx;
    assert!(
        checker.type_registry().carries_value_marker(point_idx),
        "Value-marker registration must record the type in the value_marker_types set"
    );

    // Verify an empty UserBurdenSpec was registered (Value types have no
    // heap, no owned fields, no variant payloads).
    let burden = checker.type_registry().burden(point_idx);
    assert!(
        burden.is_some(),
        "Value type must register an empty UserBurdenSpec (vs leaving burden=None)"
    );
    let burden = burden.unwrap();
    assert!(
        !burden.self_heap_alloc,
        "Value spec self_heap_alloc must be false"
    );
    assert!(
        burden.owned_fields.is_empty(),
        "Value spec owned_fields must be empty"
    );
    assert!(
        burden.borrowed_fields.is_empty(),
        "Value spec borrowed_fields must be empty"
    );
    assert!(
        burden.variant_burdens.is_empty(),
        "Value spec variant_burdens must be empty"
    );
    assert!(
        burden.element_burden.is_none(),
        "Value spec element_burden must be None"
    );
    assert!(
        burden.compiled_drop.is_none(),
        "Value spec compiled_drop must be None"
    );
    assert!(
        burden.user_drop.is_none(),
        "Value spec user_drop must be None"
    );
}

/// Helper: build a struct decl carrying the named derives with an explicit
/// field list `(field_name, field_type)`. Mirrors `make_struct_decl_with_derives`
/// but lets the caller pin field shapes for the transitive-Value field-walk pins.
fn make_struct_decl_with_fields(
    interner: &StringInterner,
    type_name: &str,
    derive_names: &[&str],
    fields: Vec<(&str, ParsedType)>,
) -> ori_ir::TypeDecl {
    let derives: Vec<Name> = derive_names.iter().map(|n| interner.intern(n)).collect();
    let struct_fields: Vec<ori_ir::StructField> = fields
        .into_iter()
        .map(|(fname, fty)| ori_ir::StructField {
            name: interner.intern(fname),
            ty: fty,
            span: Span::DUMMY,
        })
        .collect();
    ori_ir::TypeDecl {
        name: interner.intern(type_name),
        kind: ori_ir::TypeDeclKind::Struct(struct_fields),
        generics: ori_ir::GenericParamRange::EMPTY,
        where_clauses: vec![],
        span: Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
        derives,
        repr_attrs: vec![],
        target_attr: None,
        cfg_attr: None,
    }
}

/// Helper: `ParsedType::Named { name, type_args: [] }` — a nullary nominal
/// reference (e.g. a field `inner: Pt`). Allocates the empty type-arg list.
fn named_no_args(arena: &mut ExprArena, interner: &StringInterner, name: &str) -> ParsedType {
    let empty_args = arena.alloc_parsed_type_list(vec![]);
    ParsedType::Named {
        name: interner.intern(name),
        type_args: empty_args,
    }
}

/// Positive (field-walk) — every field of a `Value`-marked type is itself
/// `Value` (two `float` fields): registers the empty `UserBurdenSpec` with
/// NO field-constraint diagnostic (E2032) and NO E2049.
#[test]
fn value_type_all_value_fields_registers_empty_burden() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // type Pt: Value = { x: float, y: float }
    let pt_decl = make_struct_decl_with_fields(
        &interner,
        "Pt",
        &["Value"],
        vec![
            ("x", ParsedType::Primitive(ori_ir::TypeId::from_raw(1))), // float
            ("y", ParsedType::Primitive(ori_ir::TypeId::from_raw(1))), // float
        ],
    );
    let module = Module {
        types: vec![pt_decl],
        ..Module::default()
    };

    register_builtin_types(&mut checker);
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    let field_constraint_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2032)
        .count();
    assert_eq!(
        field_constraint_count, 0,
        "all-Value-fields type must NOT emit the field-constraint diagnostic, got: {errors:?}"
    );

    // Empty UserBurdenSpec still registered (the sound empty-burden path).
    let pt_idx = checker
        .type_registry()
        .get_by_name(interner.intern("Pt"))
        .unwrap()
        .idx;
    let burden = checker.type_registry().burden(pt_idx);
    assert!(
        burden.is_some(),
        "all-Value-fields type must register the empty UserBurdenSpec"
    );
    assert!(
        !burden.unwrap().self_heap_alloc,
        "all-Value-fields Value spec self_heap_alloc must be false"
    );
}

/// Negative (field-walk) — a `Value`-marked type with a `str` field is
/// rejected with the field-constraint diagnostic (E2032) naming the
/// offending `name: str` field. The empty `UserBurdenSpec` for a heap-bearing
/// field would be the latent silent-RC-drop; the validator forbids it.
#[test]
fn value_type_with_str_field_rejected() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // type Bad: Value = { name: str }
    let bad_decl = make_struct_decl_with_fields(
        &interner,
        "Bad",
        &["Value"],
        vec![("name", ParsedType::Primitive(ori_ir::TypeId::from_raw(3)))], // str
    );
    let module = Module {
        types: vec![bad_decl],
        ..Module::default()
    };

    register_builtin_types(&mut checker);
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    let field_constraint: Vec<_> = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2032)
        .collect();
    assert_eq!(
        field_constraint.len(),
        1,
        "Value type with a `str` field must emit exactly one field-constraint diagnostic, got: {errors:?}"
    );
    // The diagnostic names the offending field.
    let name_field = interner.intern("name");
    let names_field = matches!(
        &field_constraint[0].kind,
        crate::TypeErrorKind::FieldMissingTraitInDerive { field_name, .. } if *field_name == name_field
    );
    assert!(
        names_field,
        "field-constraint diagnostic must name the offending `name` field, got: {:?}",
        field_constraint[0].kind
    );
}

/// Transitive (field-walk) — a `Value`-marked type whose field is another
/// `Value`-marked struct (`inner: Pt` where `Pt: Value`) is accepted via the
/// transitive Value-membership lookup (the field's resolved Idx carries the
/// Value marker). Pt is declared FIRST so its marker is recorded before Outer.
#[test]
fn value_type_with_nested_value_struct_field_accepts() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // type Pt: Value = { x: float, y: float }
    let pt_decl = make_struct_decl_with_fields(
        &interner,
        "Pt",
        &["Value"],
        vec![
            ("x", ParsedType::Primitive(ori_ir::TypeId::from_raw(1))),
            ("y", ParsedType::Primitive(ori_ir::TypeId::from_raw(1))),
        ],
    );
    // type Outer: Value = { inner: Pt }
    let inner_ty = named_no_args(&mut arena, &interner, "Pt");
    let outer_decl =
        make_struct_decl_with_fields(&interner, "Outer", &["Value"], vec![("inner", inner_ty)]);

    // Declaration order: Pt FIRST (marker recorded), then Outer.
    let module = Module {
        types: vec![pt_decl, outer_decl],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);
    register_user_types(&mut checker, &module);

    let errors = checker.errors();
    let field_constraint_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2032)
        .count();
    assert_eq!(
        field_constraint_count, 0,
        "Value type with a nested Value-struct field must be accepted transitively, got: {errors:?}"
    );

    // Outer still gets its empty UserBurdenSpec.
    let outer_idx = checker
        .type_registry()
        .get_by_name(interner.intern("Outer"))
        .unwrap()
        .idx;
    assert!(
        checker.type_registry().burden(outer_idx).is_some(),
        "transitively-Value Outer must register the empty UserBurdenSpec"
    );
}

/// Cycle (field-walk) — a `Value`-marked type whose field-walk transitively
/// revisits the same type `Idx` (a self-referential Value struct) terminates
/// via the visited-set memo: no infinite recursion, no stack overflow. The
/// self-reference resolves as Value-in-progress (treated as Value), so the
/// walk accepts without the field-constraint diagnostic.
#[test]
fn value_type_with_recursive_value_field_terminates() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // type SelfRef: Value = { me: SelfRef, x: float }
    // A field referencing the declaring type itself — the cycle the
    // visited-set memo must short-circuit. `x: float` keeps a genuine
    // Value field alongside the self-reference.
    let self_ty = named_no_args(&mut arena, &interner, "SelfRef");
    let self_decl = make_struct_decl_with_fields(
        &interner,
        "SelfRef",
        &["Value"],
        vec![
            ("me", self_ty),
            ("x", ParsedType::Primitive(ori_ir::TypeId::from_raw(1))),
        ],
    );
    let module = Module {
        types: vec![self_decl],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);
    // Must terminate — a non-cycle-aware walk would recurse forever on `me`.
    register_user_types(&mut checker, &module);

    // The walk terminated (we reached here). The self-referential field
    // resolves as Value-in-progress, so no field-constraint diagnostic fires.
    let errors = checker.errors();
    let field_constraint_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2032)
        .count();
    assert_eq!(
        field_constraint_count, 0,
        "recursive Value field must terminate via the visited-set memo with no field-constraint diagnostic, got: {errors:?}"
    );
}

/// Negative — `impl T: Drop` on a NON-Value type registers `user_drop`
/// AND emits no E2049 (the conflict requires BOTH markers).
#[test]
fn drop_impl_for_non_value_type_registers_user_drop_no_e2049() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();
    let point_name = interner.intern("Point");
    let drop_trait = make_drop_trait(&mut arena, &interner);
    let drop_impl = make_drop_impl_for(&mut arena, &interner, point_name);
    // Type decl WITHOUT Value in derives.
    let point_decl = make_struct_decl_with_derives(&interner, "Point", &[]);

    let module = Module {
        types: vec![point_decl],
        traits: vec![drop_trait],
        impls: vec![drop_impl],
        ..Module::default()
    };

    let mut checker = ModuleChecker::new(&arena, &interner);
    register_builtin_types(&mut checker);
    register_user_types(&mut checker, &module);
    register_traits(&mut checker, &module);
    register_impls(&mut checker, &module);

    let errors = checker.errors();
    let e2049_count = errors
        .iter()
        .filter(|e| e.code() == ori_diagnostic::ErrorCode::E2049)
        .count();
    assert_eq!(
        e2049_count, 0,
        "Drop without Value must NOT emit E2049, got errors: {errors:?}"
    );

    // Verify Point does not carry the Value marker.
    let point_idx = checker.type_registry().get_by_name(point_name).unwrap().idx;
    assert!(
        !checker.type_registry().carries_value_marker(point_idx),
        "Non-Value type must NOT carry Value marker"
    );

    // Verify the Drop impl was registered (user_drop populated).
    let burden = checker.type_registry().burden(point_idx);
    assert!(
        burden.is_some(),
        "Drop type's burden must be populated by populate_drop_burden_if_applicable"
    );
    let burden = burden.unwrap();
    assert!(
        burden.user_drop.is_some(),
        "Drop type's user_drop must be set by populate_drop_burden_if_applicable"
    );
}
