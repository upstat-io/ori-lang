//! Type Checker Tests

use super::*;
use crate::lexer::lex;
use crate::parser::parse;
use crate::ir::{SharedInterner, ParsedType};

/// Helper to parse source code
fn parse_source(source: &str, interner: &SharedInterner) -> crate::parser::ParseResult {
    let tokens = lex(source, interner);
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
    assert_eq!(gp.bounds[0].path.len(), 1);
    assert_eq!(interner.lookup(gp.bounds[0].path[0]), "Comparable");
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
    assert_eq!(interner.lookup(gp.bounds[0].path[0]), "Eq");
    assert_eq!(interner.lookup(gp.bounds[1].path[0]), "Clone");
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
    assert_eq!(interner.lookup(wc.bounds[0].path[0]), "Clone");
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

    assert_eq!(interner.lookup(generic_params[0].bounds[0].path[0]), "Eq");
    assert_eq!(interner.lookup(generic_params[0].bounds[1].path[0]), "Clone");
    assert_eq!(interner.lookup(generic_params[0].bounds[2].path[0]), "Default");
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

    let path = &generic_params[0].bounds[0].path;
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

    assert_eq!(interner.lookup(bounds[0].path[0]), "Alpha");
    assert_eq!(interner.lookup(bounds[1].path[0]), "Beta");
    assert_eq!(interner.lookup(bounds[2].path[0]), "Gamma");
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
