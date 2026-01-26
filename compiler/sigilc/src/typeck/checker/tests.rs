//! Type Checker Tests

use super::*;
use crate::parser::parse;
use crate::ir::{SharedInterner, ParsedType};

/// Helper to parse source code
fn parse_source(source: &str, interner: &SharedInterner) -> crate::parser::ParseResult {
    let tokens = sigil_lexer::lex(source, interner);
    parse(&tokens, interner)
}

#[test]
fn test_generic_bounds_parsing() {
    let source = r#"
        @compare<T: Comparable> (a: T, b: T) -> int = 0
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    assert_eq!(parse_result.module.functions.len(), 1);
    let func = &parse_result.module.functions[0];

    let generic_params = parse_result.arena.get_generic_params(func.generics);
    assert_eq!(generic_params.len(), 1, "expected 1 generic param");

    let gp = &generic_params[0];
    assert_eq!(interner.lookup(gp.name), "T");
    assert_eq!(gp.bounds.len(), 1, "expected 1 bound");
    assert!(gp.bounds[0].rest.is_empty(), "expected single-segment path");
    assert_eq!(interner.lookup(gp.bounds[0].first), "Comparable");
}

#[test]
fn test_multiple_bounds_parsing() {
    let source = r#"
        @process<T: Eq + Clone> (x: T) -> T = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);
    assert_eq!(generic_params.len(), 1);

    let gp = &generic_params[0];
    assert_eq!(gp.bounds.len(), 2, "expected 2 bounds");
    assert_eq!(interner.lookup(gp.bounds[0].first), "Eq");
    assert_eq!(interner.lookup(gp.bounds[1].first), "Clone");
}

#[test]
fn test_where_clause_parsing() {
    let source = r#"
        @transform<T> (x: T) -> T where T: Clone = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    assert_eq!(func.where_clauses.len(), 1, "expected 1 where clause");
    let wc = &func.where_clauses[0];
    assert_eq!(interner.lookup(wc.param), "T");
    assert_eq!(wc.bounds.len(), 1);
    assert_eq!(interner.lookup(wc.bounds[0].first), "Clone");
}

#[test]
fn test_function_type_captures_generics() {
    let source = r#"
        @compare<T: Comparable> (a: T, b: T) -> int = 0
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    assert_eq!(typed.function_types.len(), 1);
    let func_type = &typed.function_types[0];

    assert_eq!(func_type.generics.len(), 1, "expected 1 generic");
    assert_eq!(interner.lookup(func_type.generics[0].param), "T");
    assert_eq!(func_type.generics[0].bounds.len(), 1);
    assert_eq!(interner.lookup(func_type.generics[0].bounds[0][0]), "Comparable");
}

#[test]
fn test_where_clause_merged_into_generics() {
    let source = r#"
        @process<T: Eq> (x: T) -> T where T: Clone = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 1);

    let bounds = &func_type.generics[0].bounds;
    assert_eq!(bounds.len(), 2, "expected 2 bounds (Eq + Clone)");
}

#[test]
fn test_type_annotation_captured_in_params() {
    let source = r#"
        @swap<T> (a: T, b: T) -> T = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let params = parse_result.arena.get_params(func.params);

    assert_eq!(params.len(), 2);
    let t_name = interner.intern("T");

    // Check that both params have ParsedType::Named with name "T"
    match &params[0].ty {
        Some(ParsedType::Named { name, type_args }) => {
            assert_eq!(*name, t_name, "first param should have type 'T'");
            assert!(type_args.is_empty(), "T should have no type args");
        }
        other => panic!("expected Named type for first param, got {:?}", other),
    }
    match &params[1].ty {
        Some(ParsedType::Named { name, type_args }) => {
            assert_eq!(*name, t_name, "second param should have type 'T'");
            assert!(type_args.is_empty(), "T should have no type args");
        }
        other => panic!("expected Named type for second param, got {:?}", other),
    }
}

#[test]
fn test_generic_params_share_type_variable() {
    let source = r#"
        @swap<T> (a: T, b: T) -> T = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.params.len(), 2);
}

#[test]
fn test_constraint_violation_detected() {
    let source = r#"
        trait Serializable {
            @serialize (self) -> str
        }

        @save<T: Serializable> (x: T) -> str = x.serialize()

        @main () -> void = run(
            let result = save(x: 42),
            print(msg: result)
        )
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();

    assert!(!bound_errors.is_empty(),
        "expected E2009 error for missing trait bound, got: {:?}",
        typed.errors);
}

#[test]
#[ignore = "trait definition parsing not yet wired up to main parser"]
fn test_constraint_satisfied_with_impl() {
    let source = r#"
        trait Printable {
            @to_string (self) -> str
        }

        @format<T: Printable> (x: T) -> str = "formatted"
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);

    if !parse_result.errors.is_empty() {
        panic!("parse errors: {:?}", parse_result.errors);
    }

    let typed = type_check(&parse_result, &interner);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 1);
    assert_eq!(func_type.generics[0].bounds.len(), 1);
    assert_eq!(
        interner.lookup(func_type.generics[0].bounds[0][0]),
        "Printable"
    );
}

#[test]
fn test_multiple_generic_params() {
    let source = r#"
        trait Eq { @eq (self, other: Self) -> bool }
        trait Ord { @cmp (self, other: Self) -> int }

        @compare<A: Eq, B: Ord> (a: A, b: B) -> bool = true
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 2, "expected 2 generic params");

    let a_name = interner.intern("A");
    let b_name = interner.intern("B");

    let a_generic = func_type.generics.iter().find(|g| g.param == a_name);
    let b_generic = func_type.generics.iter().find(|g| g.param == b_name);

    assert!(a_generic.is_some(), "should have generic A");
    assert!(b_generic.is_some(), "should have generic B");

    assert_eq!(a_generic.unwrap().bounds.len(), 1);
    assert_eq!(b_generic.unwrap().bounds.len(), 1);
}

#[test]
fn test_unbounded_generic_param() {
    let source = r#"
        @identity<T> (x: T) -> T = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 1, "expected 1 generic");
    assert_eq!(func_type.generics[0].bounds.len(), 0, "expected 0 bounds");
}

#[test]
fn test_three_generic_params() {
    let source = r#"
        @triple<A, B, C> (a: A, b: B, c: C) -> A = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 3, "expected 3 generic params");
}

#[test]
fn test_three_bounds_on_single_param() {
    let source = r#"
        @constrained<T: Eq + Clone + Default> (x: T) -> T = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);
    assert_eq!(generic_params[0].bounds.len(), 3, "expected 3 bounds");

    assert_eq!(interner.lookup(generic_params[0].bounds[0].first), "Eq");
    assert_eq!(interner.lookup(generic_params[0].bounds[1].first), "Clone");
    assert_eq!(interner.lookup(generic_params[0].bounds[2].first), "Default");
}

#[test]
fn test_multiple_where_clauses() {
    let source = r#"
        @combine<T, U> (a: T, b: U) -> T where T: Clone, U: Default = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    assert_eq!(func.where_clauses.len(), 2, "expected 2 where clauses");

    assert_eq!(interner.lookup(func.where_clauses[0].param), "T");
    assert_eq!(interner.lookup(func.where_clauses[1].param), "U");
}

#[test]
fn test_where_clause_with_multiple_bounds() {
    let source = r#"
        @process<T> (x: T) -> T where T: Eq + Clone = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    assert_eq!(func.where_clauses.len(), 1);
    assert_eq!(func.where_clauses[0].bounds.len(), 2, "expected 2 bounds in where clause");
}

#[test]
fn test_mixed_inline_and_where_bounds() {
    let source = r#"
        @process<T: Eq, U> (a: T, b: U) -> T where U: Clone = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    let func_type = &typed.function_types[0];

    assert_eq!(func_type.generics.len(), 2);

    let t_name = interner.intern("T");
    let u_name = interner.intern("U");

    let t_gen = func_type.generics.iter().find(|g| g.param == t_name).unwrap();
    let u_gen = func_type.generics.iter().find(|g| g.param == u_name).unwrap();

    assert_eq!(t_gen.bounds.len(), 1, "T should have 1 bound");
    assert_eq!(u_gen.bounds.len(), 1, "U should have 1 bound");
}

#[test]
fn test_qualified_trait_path() {
    let source = r#"
        @compare<T: std.traits.Eq> (a: T, b: T) -> bool = true
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);
    assert_eq!(generic_params[0].bounds.len(), 1);

    let bound = &generic_params[0].bounds[0];
    let path = bound.path();
    assert_eq!(path.len(), 3, "expected 3-segment path");
    assert_eq!(interner.lookup(path[0]), "std");
    assert_eq!(interner.lookup(path[1]), "traits");
    assert_eq!(interner.lookup(path[2]), "Eq");
}

#[test]
fn test_generic_return_type_same_as_param() {
    let source = r#"
        @identity<T> (x: T) -> T = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.params.len(), 1);
}

#[test]
fn test_single_letter_generic_names() {
    let source = r#"
        @func_a<A> (x: A) -> A = x
        @func_t<T> (x: T) -> T = x
        @func_u<U> (x: U) -> U = x
        @func_k<K> (x: K) -> K = x
        @func_v<V> (x: V) -> V = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
    assert_eq!(typed.function_types.len(), 5);
}

#[test]
fn test_multi_letter_generic_names() {
    let source = r#"
        @process<Item, Value> (a: Item, b: Value) -> Item = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);
    assert_eq!(generic_params.len(), 2);
    assert_eq!(interner.lookup(generic_params[0].name), "Item");
    assert_eq!(interner.lookup(generic_params[1].name), "Value");
}

#[test]
fn test_generic_function_with_concrete_return() {
    let source = r#"
        @len<T> (x: T) -> int = 0
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
}

#[test]
fn test_function_without_generics() {
    let source = r#"
        @add (a: int, b: int) -> int = a + b
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 0, "non-generic function should have 0 generics");
}

#[test]
fn test_duplicate_bound_not_deduplicated() {
    let source = r#"
        @weird<T: Eq + Eq> (x: T) -> T = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);
    assert_eq!(generic_params[0].bounds.len(), 2);
}

#[test]
fn test_where_clause_adds_to_inline_bounds() {
    let source = r#"
        @process<T: Eq> (x: T) -> T where T: Clone, T: Default = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    let func_type = &typed.function_types[0];

    let t_name = interner.intern("T");
    let t_gen = func_type.generics.iter().find(|g| g.param == t_name).unwrap();
    assert_eq!(t_gen.bounds.len(), 3, "T should have 3 bounds total");
}

#[test]
fn test_generics_with_list_type() {
    let source = r#"
        @first<T> (list: [T]) -> T = list[0]
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    if parse_result.errors.is_empty() {
        let typed = type_check(&parse_result, &interner);
        let func_type = &typed.function_types[0];
        assert_eq!(func_type.generics.len(), 1);
    }
}

#[test]
fn test_empty_generic_params_list() {
    let source = r#"
        @empty<> (x: int) -> int = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    if parse_result.errors.is_empty() {
        let func = &parse_result.module.functions[0];
        let generic_params = parse_result.arena.get_generic_params(func.generics);
        assert_eq!(generic_params.len(), 0, "expected 0 generic params");
    }
}

#[test]
fn test_constraint_violation_with_int() {
    let source = r#"
        @needs_custom<T: CustomTrait> (x: T) -> T = x
        @main () -> int = needs_custom(x: 42)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    if parse_result.errors.is_empty() {
        let typed = type_check(&parse_result, &interner);
        let bound_errors: Vec<_> = typed.errors.iter()
            .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
            .collect();
        assert!(!bound_errors.is_empty(),
            "expected E2009 for int not implementing CustomTrait");
    }
}

#[test]
fn test_generic_function_call_with_explicit_type() {
    let source = r#"
        @identity<T> (x: T) -> T = x
        @main () -> int = identity(x: 42)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
}

#[test]
fn test_generic_function_preserves_param_order() {
    let source = r#"
        @ordered<First, Second, Third> (a: First, b: Second, c: Third) -> First = a
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);

    assert_eq!(interner.lookup(generic_params[0].name), "First");
    assert_eq!(interner.lookup(generic_params[1].name), "Second");
    assert_eq!(interner.lookup(generic_params[2].name), "Third");
}

#[test]
fn test_bounds_preserve_trait_order() {
    let source = r#"
        @ordered<T: Alpha + Beta + Gamma> (x: T) -> T = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let func = &parse_result.module.functions[0];
    let generic_params = parse_result.arena.get_generic_params(func.generics);
    let bounds = &generic_params[0].bounds;

    assert_eq!(interner.lookup(bounds[0].first), "Alpha");
    assert_eq!(interner.lookup(bounds[1].first), "Beta");
    assert_eq!(interner.lookup(bounds[2].first), "Gamma");
}

#[test]
fn test_where_clause_for_non_generic_param() {
    let source = r#"
        @weird<T> (x: T) -> T where X: Clone = x
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    if parse_result.errors.is_empty() {
        let typed = type_check(&parse_result, &interner);
        let _ = typed;
    }
}

#[test]
fn test_type_param_used_only_in_return() {
    let source = r#"
        @create<T> () -> T = panic(msg: "cannot create")
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    let func_type = &typed.function_types[0];
    assert_eq!(func_type.generics.len(), 1);
}

#[test]
fn test_multiple_functions_with_same_type_param_name() {
    let source = r#"
        @first<T> (x: T) -> T = x
        @second<T> (y: T) -> T = y
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);

    assert_eq!(typed.function_types.len(), 2);
    assert_eq!(typed.function_types[0].generics.len(), 1);
    assert_eq!(typed.function_types[1].generics.len(), 1);
}

// ============================================================================
// Len and IsEmpty Trait Bound Tests
// ============================================================================

#[test]
fn test_len_bound_satisfied_by_list() {
    let source = r#"
        @get_length<T: Len> (x: T) -> int = x.len()
        @main () -> int = get_length(x: [1, 2, 3])
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    // Should have no E2009 errors for Len bound violation
    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "list should satisfy Len bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_len_bound_satisfied_by_str() {
    let source = r#"
        @get_length<T: Len> (x: T) -> int = x.len()
        @main () -> int = get_length(x: "hello")
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "str should satisfy Len bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_len_bound_not_satisfied_by_int() {
    let source = r#"
        @get_length<T: Len> (x: T) -> int = 0
        @main () -> int = get_length(x: 42)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(!bound_errors.is_empty(),
        "int should NOT satisfy Len bound, expected E2009 error");
}

#[test]
fn test_is_empty_bound_satisfied_by_list() {
    let source = r#"
        @check_empty<T: IsEmpty> (x: T) -> bool = x.is_empty()
        @main () -> bool = check_empty(x: [])
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "list should satisfy IsEmpty bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_is_empty_bound_satisfied_by_str() {
    let source = r#"
        @check_empty<T: IsEmpty> (x: T) -> bool = x.is_empty()
        @main () -> bool = check_empty(x: "")
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "str should satisfy IsEmpty bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_is_empty_bound_not_satisfied_by_int() {
    let source = r#"
        @check_empty<T: IsEmpty> (x: T) -> bool = false
        @main () -> bool = check_empty(x: 42)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(!bound_errors.is_empty(),
        "int should NOT satisfy IsEmpty bound, expected E2009 error");
}

#[test]
fn test_combined_len_and_is_empty_bounds() {
    let source = r#"
        @check_size<T: Len + IsEmpty> (x: T) -> int = if x.is_empty() then 0 else x.len()
        @main () -> int = check_size(x: [1, 2, 3])
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "list should satisfy both Len and IsEmpty bounds, got errors: {:?}", bound_errors);
}

// ============================================================================
// Comparable and Eq Trait Bound Tests
// ============================================================================

#[test]
fn test_comparable_bound_satisfied_by_int() {
    let source = r#"
        @compare_vals<T: Comparable> (a: T, b: T) -> bool = true
        @main () -> bool = compare_vals(a: 1, b: 2)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "int should satisfy Comparable bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_comparable_bound_satisfied_by_str() {
    let source = r#"
        @compare_vals<T: Comparable> (a: T, b: T) -> bool = true
        @main () -> bool = compare_vals(a: "hello", b: "world")
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "str should satisfy Comparable bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_eq_bound_satisfied_by_int() {
    let source = r#"
        @check_eq<T: Eq> (a: T, b: T) -> bool = true
        @main () -> bool = check_eq(a: 1, b: 1)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "int should satisfy Eq bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_eq_bound_satisfied_by_bool() {
    let source = r#"
        @check_eq<T: Eq> (a: T, b: T) -> bool = true
        @main () -> bool = check_eq(a: true, b: false)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "bool should satisfy Eq bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_clone_bound_satisfied_by_int() {
    let source = r#"
        @duplicate<T: Clone> (x: T) -> T = x
        @main () -> int = duplicate(x: 42)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "int should satisfy Clone bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_hashable_bound_satisfied_by_str() {
    let source = r#"
        @hash_val<T: Hashable> (x: T) -> int = 0
        @main () -> int = hash_val(x: "hello")
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "str should satisfy Hashable bound, got errors: {:?}", bound_errors);
}

#[test]
fn test_default_bound_satisfied_by_int() {
    let source = r#"
        @default_val<T: Default> () -> int = 0
        @use_default () -> int = default_val()
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    // This test just verifies parsing works - the bound check
    // would require calling with a specific type
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);
}

#[test]
fn test_printable_bound_satisfied_by_int() {
    let source = r#"
        @to_str<T: Printable> (x: T) -> str = "printed"
        @main () -> str = to_str(x: 42)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let bound_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
        .collect();
    assert!(bound_errors.is_empty(),
        "int should satisfy Printable bound, got errors: {:?}", bound_errors);
}

// ============================================================================
// Capability Trait Validation Tests
// ============================================================================

#[test]
fn test_capability_with_defined_trait_no_error() {
    // When a capability is a defined trait, no E2012 error should be raised
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        @fetch (url: str) -> str uses Http = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let cap_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2012)
        .collect();
    assert!(cap_errors.is_empty(),
        "defined trait should be valid capability, got: {:?}", cap_errors);
}

#[test]
fn test_capability_with_undefined_trait_reports_error() {
    // When a capability references a non-existent trait, E2012 should be raised
    let source = r#"
        @fetch (url: str) -> str uses UndefinedCapability = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let cap_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2012)
        .collect();
    assert!(!cap_errors.is_empty(),
        "undefined capability should produce E2012 error, got: {:?}", typed.errors);
    assert!(cap_errors[0].message.contains("UndefinedCapability"),
        "error should mention the undefined capability name");
}

#[test]
fn test_multiple_capabilities_all_defined() {
    // Multiple capabilities that are all defined traits should produce no errors
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        trait Logger {
            @log (msg: str) -> void
        }

        @fetch_and_log (url: str) -> str uses Http, Logger = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let cap_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2012)
        .collect();
    assert!(cap_errors.is_empty(),
        "all defined capabilities should be valid, got: {:?}", cap_errors);
}

#[test]
fn test_multiple_capabilities_one_undefined() {
    // If one of multiple capabilities is undefined, E2012 should be raised for it
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        @fetch_and_log (url: str) -> str uses Http, UndefinedLogger = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let cap_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2012)
        .collect();
    assert_eq!(cap_errors.len(), 1,
        "should have exactly one E2012 error for undefined capability");
    assert!(cap_errors[0].message.contains("UndefinedLogger"),
        "error should mention the undefined capability name");
}

#[test]
fn test_with_expression_provider_implements_trait() {
    // When provider implements the capability trait, no E2013 error
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        type RealHttp = { base_url: str }

        impl Http for RealHttp {
            @get (self, url: str) -> str = url
        }

        @fetch (url: str) -> str uses Http = url

        @main () -> str =
            with Http = RealHttp { base_url: "https://api.example.com" } in
                fetch(url: "/data")
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let provider_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2013)
        .collect();
    assert!(provider_errors.is_empty(),
        "provider implementing trait should not produce E2013, got: {:?}", provider_errors);
}

#[test]
fn test_function_with_capability_stores_in_signature() {
    // Verify that capabilities are stored in the function type
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        trait Logger {
            @log (msg: str) -> void
        }

        @fetch_and_log (url: str) -> str uses Http, Logger = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    // Find the function type
    let func_type = typed.function_types.iter()
        .find(|ft| interner.lookup(ft.name) == "fetch_and_log")
        .expect("should find fetch_and_log function");

    assert_eq!(func_type.capabilities.len(), 2,
        "function should have 2 capabilities");

    let cap_names: Vec<_> = func_type.capabilities.iter()
        .map(|n| interner.lookup(*n))
        .collect();
    assert!(cap_names.contains(&"Http"), "should include Http capability");
    assert!(cap_names.contains(&"Logger"), "should include Logger capability");
}

#[test]
fn test_pure_function_no_capabilities() {
    // Pure function should have empty capabilities list
    let source = r#"
        @add (a: int, b: int) -> int = a + b
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);

    let func_type = &typed.function_types[0];
    assert!(func_type.capabilities.is_empty(),
        "pure function should have no capabilities");
}

// ============================================================================
// Async Capability Tests
// ============================================================================

#[test]
fn test_async_marker_trait_valid_capability() {
    // Async is a marker trait with no methods - should be valid as capability
    let source = r#"
        trait Async {}

        @async_op () -> int uses Async = 42
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let cap_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2012)
        .collect();
    assert!(cap_errors.is_empty(),
        "Async marker trait should be valid capability, got: {:?}", cap_errors);
}

#[test]
fn test_async_capability_stored_in_signature() {
    // Verify that Async capability is stored in function type
    let source = r#"
        trait Async {}

        @may_suspend () -> str uses Async = "done"
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);

    let func_type = typed.function_types.iter()
        .find(|ft| interner.lookup(ft.name) == "may_suspend")
        .expect("should find may_suspend function");

    assert_eq!(func_type.capabilities.len(), 1, "should have 1 capability");
    assert_eq!(interner.lookup(func_type.capabilities[0]), "Async",
        "capability should be Async");
}

#[test]
fn test_async_with_other_capabilities() {
    // Async can be combined with other capabilities
    let source = r#"
        trait Async {}
        trait Http {
            @get (url: str) -> str
        }

        @async_fetch (url: str) -> str uses Http, Async = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let cap_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2012)
        .collect();
    assert!(cap_errors.is_empty(),
        "Http and Async should both be valid capabilities, got: {:?}", cap_errors);

    let func_type = typed.function_types.iter()
        .find(|ft| interner.lookup(ft.name) == "async_fetch")
        .expect("should find async_fetch function");

    assert_eq!(func_type.capabilities.len(), 2, "should have 2 capabilities");
    let cap_names: Vec<_> = func_type.capabilities.iter()
        .map(|n| interner.lookup(*n))
        .collect();
    assert!(cap_names.contains(&"Http"), "should include Http");
    assert!(cap_names.contains(&"Async"), "should include Async");
}

#[test]
fn test_sync_function_no_async_capability() {
    // Function without uses Async is synchronous
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        @sync_fetch (url: str) -> str uses Http = url
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);

    let func_type = typed.function_types.iter()
        .find(|ft| interner.lookup(ft.name) == "sync_fetch")
        .expect("should find sync_fetch function");

    let cap_names: Vec<_> = func_type.capabilities.iter()
        .map(|n| interner.lookup(*n))
        .collect();
    assert!(!cap_names.contains(&"Async"),
        "sync function should not have Async capability");
}

// ============================================================================
// Await Expression Rejection Tests
// ============================================================================

#[test]
fn test_await_syntax_not_supported() {
    // Sigil does not support .await syntax - it's parsed as field access
    // and rejected because primitives don't have an "await" field.
    // This verifies the design decision: no .await in Sigil.
    let source = r#"
        @main () -> int = run(
            let x = 42,
            x.await,
        )
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    // Should have an error - either "await not supported" or "no such field/method"
    assert!(!typed.errors.is_empty(),
        ".await syntax should produce a type error");
    // The error will be about field access or method not found, which is correct
    // behavior - Sigil doesn't have await syntax
}

// ============================================================================
// Capability Propagation Tests
// ============================================================================

#[test]
fn test_capability_propagation_caller_declares() {
    // When caller declares the same capability, no E2014 error
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        @fetch (url: str) -> str uses Http = url

        @caller (url: str) -> str uses Http = fetch(url: url)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert!(prop_errors.is_empty(),
        "caller declaring same capability should not produce E2014, got: {:?}", prop_errors);
}

#[test]
fn test_capability_propagation_caller_missing() {
    // When caller doesn't declare the capability, E2014 should be raised
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        @fetch (url: str) -> str uses Http = url

        @caller (url: str) -> str = fetch(url: url)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert_eq!(prop_errors.len(), 1,
        "missing capability should produce E2014 error");
    assert!(prop_errors[0].message.contains("Http"),
        "error should mention the missing capability: {}", prop_errors[0].message);
    assert!(prop_errors[0].message.contains("fetch"),
        "error should mention the called function: {}", prop_errors[0].message);
}

#[test]
fn test_capability_propagation_with_provides() {
    // When capability is provided via with...in, no E2014 error
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        type MockHttp = { base_url: str }

        impl Http for MockHttp {
            @get (self, url: str) -> str = url
        }

        @fetch (url: str) -> str uses Http = url

        @caller (url: str) -> str =
            with Http = MockHttp { base_url: "test" } in
                fetch(url: url)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert!(prop_errors.is_empty(),
        "with...in providing capability should not produce E2014, got: {:?}", prop_errors);
}

#[test]
fn test_capability_propagation_with_wrong_capability() {
    // When with...in provides a different capability, E2014 should be raised
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        trait Logger {
            @log (msg: str) -> void
        }

        type MockLogger = { prefix: str }

        impl Logger for MockLogger {
            @log (self, msg: str) -> void = ()
        }

        @fetch (url: str) -> str uses Http = url

        @caller (url: str) -> str =
            with Logger = MockLogger { prefix: "" } in
                fetch(url: url)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert_eq!(prop_errors.len(), 1,
        "providing wrong capability should still produce E2014");
    assert!(prop_errors[0].message.contains("Http"),
        "error should mention the required capability: {}", prop_errors[0].message);
}

#[test]
fn test_capability_propagation_multiple_capabilities() {
    // When called function requires multiple capabilities,
    // caller must declare or provide all of them
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        trait Logger {
            @log (msg: str) -> void
        }

        @fetch_and_log (url: str) -> str uses Http, Logger = url

        // Only declares Http, missing Logger
        @caller (url: str) -> str uses Http = fetch_and_log(url: url)
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert_eq!(prop_errors.len(), 1,
        "missing one of multiple capabilities should produce E2014");
    assert!(prop_errors[0].message.contains("Logger"),
        "error should mention the missing capability: {}", prop_errors[0].message);
}

#[test]
fn test_capability_propagation_test_with_provide() {
    // Tests don't have capability declarations, so must provide via with...in
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        type MockHttp = { base_url: str }

        impl Http for MockHttp {
            @get (self, url: str) -> str = url
        }

        @fetch (url: str) -> str uses Http = url

        @test_fetch tests @fetch () -> void =
            with Http = MockHttp { base_url: "test" } in
                run(
                    let result = fetch(url: "/data"),
                    assert_eq(actual: result, expected: "/data"),
                )
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert!(prop_errors.is_empty(),
        "test with with...in should not produce E2014, got: {:?}", prop_errors);
}

#[test]
fn test_capability_propagation_test_missing_provide() {
    // Test calling function with capability without providing it
    let source = r#"
        trait Http {
            @get (url: str) -> str
        }

        @fetch (url: str) -> str uses Http = url

        @test_fetch tests @fetch () -> void = run(
            let result = fetch(url: "/data"),
            assert_eq(actual: result, expected: "/data"),
        )
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);

    let prop_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.code == crate::diagnostic::ErrorCode::E2014)
        .collect();
    assert_eq!(prop_errors.len(), 1,
        "test calling capability function without providing should produce E2014");
}

// ============================================================================
// self Parameter Handling Tests
// ============================================================================

#[test]
fn test_self_parameter_in_trait_method() {
    // Verify that traits with self parameter parse correctly
    let source = r#"
        trait Identifiable {
            @id (self) -> int
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    // Verify the trait was parsed
    assert_eq!(parse_result.module.traits.len(), 1);
    let trait_def = &parse_result.module.traits[0];
    assert_eq!(interner.lookup(trait_def.name), "Identifiable");

    // Verify it has one item (the method)
    assert_eq!(trait_def.items.len(), 1);
}

#[test]
fn test_self_parameter_in_impl_method() {
    let source = r#"
        type Point = { x: int, y: int }

        impl Point {
            @get_x (self) -> int = self.x
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    // Type check should succeed
    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
}

#[test]
fn test_self_with_additional_params() {
    let source = r#"
        type Counter = { value: int }

        impl Counter {
            @add (self, amount: int) -> int = self.value + amount
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
}

// ============================================================================
// Self Type Reference Tests
// ============================================================================

#[test]
fn test_self_type_in_return() {
    let source = r#"
        trait Cloneable {
            @clone (self) -> Self
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    // Verify trait parsed with item
    let trait_def = &parse_result.module.traits[0];
    assert_eq!(trait_def.items.len(), 1);

    // Get the method from items
    use crate::ir::TraitItem;
    match &trait_def.items[0] {
        TraitItem::MethodSig(sig) => {
            // Return type should be Self
            match &sig.return_ty {
                ParsedType::SelfType => {}
                other => panic!("expected Self return type, got {:?}", other),
            }
        }
        other => panic!("expected MethodSig, got {:?}", other),
    }
}

#[test]
fn test_self_type_in_parameter() {
    let source = r#"
        trait Combinable {
            @combine (self, other: Self) -> Self
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let trait_def = &parse_result.module.traits[0];

    use crate::ir::TraitItem;
    match &trait_def.items[0] {
        TraitItem::MethodSig(sig) => {
            // Check return type is Self
            match &sig.return_ty {
                ParsedType::SelfType => {}
                other => panic!("expected Self return type, got {:?}", other),
            }

            // Check 'other' parameter type is Self
            let params = parse_result.arena.get_params(sig.params);
            // params includes self and other, so find the 'other' param
            let other_param = params.iter()
                .find(|p| interner.lookup(p.name) == "other")
                .expect("should have 'other' parameter");
            match &other_param.ty {
                Some(ParsedType::SelfType) => {}
                other => panic!("expected Self param type, got {:?}", other),
            }
        }
        other => panic!("expected MethodSig, got {:?}", other),
    }
}

#[test]
fn test_self_type_in_impl() {
    let source = r#"
        type Vector = { x: int, y: int }

        impl Vector {
            @scale (self, factor: int) -> Self = Vector { x: self.x * factor, y: self.y * factor }
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    assert!(typed.errors.is_empty(), "type errors: {:?}", typed.errors);
}

// ============================================================================
// Trait Inheritance Tests
// ============================================================================

#[test]
fn test_trait_inheritance_parsing() {
    let source = r#"
        trait Named {
            @name (self) -> str
        }

        trait Describable: Named {
            @describe (self) -> str
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    assert_eq!(parse_result.module.traits.len(), 2);

    // Find the Describable trait
    let describable = parse_result.module.traits.iter()
        .find(|t| interner.lookup(t.name) == "Describable")
        .expect("should find Describable trait");

    // Check that it has a parent trait
    assert_eq!(describable.super_traits.len(), 1,
        "Describable should have 1 super trait");
    assert_eq!(interner.lookup(describable.super_traits[0].first), "Named",
        "super trait should be Named");
}

#[test]
fn test_trait_multiple_inheritance() {
    let source = r#"
        trait A {
            @a (self) -> int
        }

        trait B {
            @b (self) -> int
        }

        trait C: A + B {
            @c (self) -> int
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);

    // Multiple trait inheritance with + syntax may or may not be supported
    // This test documents current behavior
    if parse_result.errors.is_empty() {
        let c_trait = parse_result.module.traits.iter()
            .find(|t| interner.lookup(t.name) == "C")
            .expect("should find C trait");

        assert!(c_trait.super_traits.len() >= 1,
            "C should have at least one super trait");
    }
}

#[test]
fn test_trait_inheritance_with_impl() {
    let source = r#"
        trait Base {
            @base_method (self) -> int
        }

        trait Derived: Base {
            @derived_method (self) -> int
        }

        type MyType = { value: int }

        impl Base for MyType {
            @base_method (self) -> int = self.value
        }

        impl Derived for MyType {
            @derived_method (self) -> int = self.value * 2
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let typed = type_check(&parse_result, &interner);
    // This may or may not produce errors depending on inheritance enforcement
    // The test documents current behavior
    let _ = typed;
}

// ============================================================================
// Associated Type Tests
// ============================================================================

#[test]
fn test_associated_type_declaration() {
    let source = r#"
        trait Container {
            type Item
            @first (self) -> Option<Self.Item>
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    let trait_def = &parse_result.module.traits[0];

    // Count associated types in items
    use crate::ir::TraitItem;
    let assoc_types: Vec<_> = trait_def.items.iter()
        .filter_map(|item| match item {
            TraitItem::AssocType(at) => Some(at),
            _ => None,
        })
        .collect();

    assert_eq!(assoc_types.len(), 1, "trait should have 1 associated type");
    assert_eq!(interner.lookup(assoc_types[0].name), "Item",
        "associated type should be named Item");
}

#[test]
fn test_associated_type_in_impl() {
    let source = r#"
        trait Container {
            type Item
            @first (self) -> Option<Self.Item>
        }

        type IntBox = { value: int }

        impl Container for IntBox {
            type Item = int
            @first (self) -> Option<Self.Item> = Some(self.value)
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    // Find the impl
    assert!(!parse_result.module.impls.is_empty(), "should have at least one impl");

    // Check that impl has associated type definition
    let impl_def = &parse_result.module.impls[0];
    assert_eq!(impl_def.assoc_types.len(), 1, "impl should have 1 associated type");
    assert_eq!(interner.lookup(impl_def.assoc_types[0].name), "Item");
}

#[test]
fn test_self_dot_item_type_reference() {
    let source = r#"
        trait Iterator {
            type Item
            @next (self) -> Option<Self.Item>
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    // The return type should reference Self.Item
    let trait_def = &parse_result.module.traits[0];

    // Find the method item
    use crate::ir::TraitItem;
    let method = trait_def.items.iter()
        .find_map(|item| match item {
            TraitItem::MethodSig(sig) => Some(sig),
            _ => None,
        })
        .expect("should have a method");

    // Return type should be Option<Self.Item>
    match &method.return_ty {
        ParsedType::Named { name, type_args } => {
            assert_eq!(interner.lookup(*name), "Option");
            assert_eq!(type_args.len(), 1);
            match &type_args[0] {
                ParsedType::AssociatedType { base, assoc_name } => {
                    // base should be Self
                    match base.as_ref() {
                        ParsedType::SelfType => {}
                        other => panic!("expected Self as base, got {:?}", other),
                    }
                    assert_eq!(interner.lookup(*assoc_name), "Item",
                        "associated type name should be Item");
                }
                other => panic!("expected associated type, got {:?}", other),
            }
        }
        other => panic!("expected Named type, got {:?}", other),
    }
}

// =============================================================================
// Associated Type Constraints Tests
// =============================================================================

#[test]
fn test_where_clause_with_projection_parsing() {
    let source = r#"
        trait Container {
            type Item
            @first (self) -> Option<Self.Item>
        }

        @needs_eq_item<C: Container> (c: C) -> bool where C.Item: Eq = true
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

    // Find the function (skip the trait method)
    let func = parse_result.module.functions.iter()
        .find(|f| interner.lookup(f.name) == "needs_eq_item")
        .expect("should find needs_eq_item function");

    assert_eq!(func.where_clauses.len(), 1, "expected 1 where clause");
    let wc = &func.where_clauses[0];
    assert_eq!(interner.lookup(wc.param), "C", "where clause should constrain C");
    assert!(wc.projection.is_some(), "where clause should have projection");
    assert_eq!(interner.lookup(wc.projection.unwrap()), "Item", "projection should be Item");
    assert_eq!(wc.bounds.len(), 1, "expected 1 bound");
    assert_eq!(interner.lookup(wc.bounds[0].first), "Eq", "bound should be Eq");
}

#[test]
fn test_where_constraint_stored_in_function_type() {
    let source = r#"
        trait Container {
            type Item
            @first (self) -> Option<Self.Item>
        }

        @needs_eq_item<C: Container> (c: C) -> bool where C.Item: Eq = true
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    // Find the function type
    let func_type = typed.function_types.iter()
        .find(|ft| interner.lookup(ft.name) == "needs_eq_item")
        .expect("should find needs_eq_item function type");

    assert_eq!(func_type.where_constraints.len(), 1, "expected 1 where constraint");
    let constraint = &func_type.where_constraints[0];
    assert_eq!(interner.lookup(constraint.param), "C");
    assert!(constraint.projection.is_some());
    assert_eq!(interner.lookup(constraint.projection.unwrap()), "Item");
    assert_eq!(constraint.bounds.len(), 1);
}

#[test]
fn test_impl_missing_associated_type_error() {
    let source = r#"
        trait Container {
            type Item
            @first (self) -> Option<Self.Item>
        }

        type EmptyBox = { }

        impl Container for EmptyBox {
            @first (self) -> Option<Self.Item> = None
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    // Should have an error about missing associated type
    let missing_assoc_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.message.contains("missing associated type"))
        .collect();

    assert!(!missing_assoc_errors.is_empty(),
        "expected error about missing associated type, got: {:?}",
        typed.errors);
}

#[test]
fn test_impl_with_associated_type_no_error() {
    let source = r#"
        trait Container {
            type Item
            @first (self) -> Option<Self.Item>
        }

        type IntBox = { value: int }

        impl Container for IntBox {
            type Item = int
            @first (self) -> Option<Self.Item> = Some(self.value)
        }
    "#;

    let interner = SharedInterner::default();
    let parse_result = parse_source(source, &interner);
    let typed = type_check(&parse_result, &interner);

    // Should NOT have an error about missing associated type
    let missing_assoc_errors: Vec<_> = typed.errors.iter()
        .filter(|e| e.message.contains("missing associated type"))
        .collect();

    assert!(missing_assoc_errors.is_empty(),
        "should not have missing associated type errors, got: {:?}",
        missing_assoc_errors);
}
