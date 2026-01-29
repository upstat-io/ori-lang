//! Tests for literal type inference (integers, lists, tuples, etc.).

use super::{check_source, check_source_with_interner};
use ori_ir::TypeId;
use ori_types::TypeData;

#[test]
fn test_literal_types() {
    let (parsed, typed) = check_source("@main () -> int = 42");

    assert!(!typed.has_errors());
    assert_eq!(typed.function_types.len(), 1);

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::INT);
}

#[test]
fn test_list_type() {
    let result = check_source_with_interner("@test () = [1, 2, 3]");

    assert!(!result.typed.has_errors());

    let func = &result.parsed.module.functions[0];
    let body_type = result.typed.expr_types[func.body.index()];
    // Verify it's a List(Int) by looking up in the shared interner
    let type_data = result.type_interner.lookup(body_type);
    match type_data {
        TypeData::List(elem_id) => {
            assert_eq!(elem_id, TypeId::INT, "List element should be int");
        }
        _ => panic!("Expected List type, got {type_data:?}"),
    }
}

#[test]
fn test_tuple_type() {
    let result = check_source_with_interner("@test () = (1, true, \"hello\")");

    assert!(!result.typed.has_errors());

    let func = &result.parsed.module.functions[0];
    let body_type = result.typed.expr_types[func.body.index()];
    // Verify it's a Tuple(Int, Bool, Str) by looking up in the shared interner
    let type_data = result.type_interner.lookup(body_type);
    match type_data {
        TypeData::Tuple(elem_ids) => {
            assert_eq!(elem_ids.len(), 3, "Tuple should have 3 elements");
            assert_eq!(elem_ids[0], TypeId::INT, "First element should be int");
            assert_eq!(elem_ids[1], TypeId::BOOL, "Second element should be bool");
            assert_eq!(elem_ids[2], TypeId::STR, "Third element should be str");
        }
        _ => panic!("Expected Tuple type, got {type_data:?}"),
    }
}

#[test]
fn test_binary_arithmetic() {
    let (parsed, typed) = check_source("@add () -> int = 1 + 2");

    assert!(!typed.has_errors());

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::INT);
}

#[test]
fn test_comparison() {
    let (parsed, typed) = check_source("@cmp () -> bool = 1 < 2");

    assert!(!typed.has_errors());

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::BOOL);
}

#[test]
fn test_if_expression() {
    let (parsed, typed) = check_source("@test () -> int = if true then 1 else 2");

    assert!(!typed.has_errors());

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::INT);
}

#[test]
fn test_nested_if_type() {
    let (_, typed) = check_source(
        r"
            @test (x: int) -> int =
                if x > 0 then
                    if x > 10 then 100 else 10
                else
                    0
        ",
    );

    assert!(!typed.has_errors());
}

#[test]
fn test_run_pattern_type() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let x: int = 1,
                let y: int = 2,
                x + y
            )
        ",
    );

    assert!(!typed.has_errors());
}
