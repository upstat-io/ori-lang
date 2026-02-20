use super::*;
use crate::{Idx, ModuleChecker, ObjectSafetyViolation};
use ori_ir::{DerivedTrait, ExprArena, Module, Name, ParsedType, Span, StringInterner};

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

// ============================================================================
// parsed_type_contains_self tests
// ============================================================================

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

// ============================================================================
// compute_object_safety_violations tests
// ============================================================================

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
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(3)), // str
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
            params,
            return_ty: ParsedType::SelfType,
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
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(2)), // bool
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
                params: params1,
                return_ty: ParsedType::SelfType,
                span: ori_ir::Span::DUMMY,
            }),
            ori_ir::TraitItem::MethodSig(ori_ir::TraitMethodSig {
                name: eq_name,
                params: params2,
                return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(2)),
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
            params,
            return_ty: ParsedType::Primitive(ori_ir::TypeId::from_raw(3)), // str
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
    };

    // Register Eq first (as the derive processor would)
    register_derived_impl(&mut checker, &type_decl, eq);
    // Now register Hashable — should succeed
    register_derived_impl(&mut checker, &type_decl, hashable);

    let errors = checker.errors();
    assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
}

// ============================================================================
// resolve_type_with_params — compound Self recursion tests
// ============================================================================

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

// --- Cross-crate sync enforcement (Section 05.1, Tests 2 and 3) ---

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
        let trait_name = interner.intern(trait_kind.trait_name());
        let type_name = interner.intern("TestType");
        let self_type = checker.pool_mut().named(type_name);

        let methods = build_derived_methods(&mut checker, trait_name, self_type, Span::DUMMY);

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
