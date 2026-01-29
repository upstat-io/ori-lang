//! Tests for function type inference (parameters, calls, lambdas).

use super::check_source;
use ori_ir::TypeId;

#[test]
fn test_function_with_typed_params() {
    let (_, typed) = check_source("@add (a: int, b: int) -> int = a + b");

    assert!(!typed.has_errors());
    assert_eq!(typed.function_types.len(), 1);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.params.len(), 2);
    assert_eq!(func_type.params[0], TypeId::INT);
    assert_eq!(func_type.params[1], TypeId::INT);
    assert_eq!(func_type.return_type, TypeId::INT);
}

#[test]
fn test_function_call_type_inference() {
    let (_, typed) = check_source("@double (x: int) -> int = x * 2");

    assert!(!typed.has_errors());
    assert_eq!(typed.function_types.len(), 1);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.return_type, TypeId::INT);
}

#[test]
fn test_lambda_with_typed_param() {
    let (_, typed) = check_source("@test () = (x: int) -> x + 1");

    assert!(!typed.has_errors());
}

#[test]
fn test_type_mismatch_error() {
    let (_, typed) = check_source("@test () -> int = if 42 then 1 else 2");

    assert!(typed.has_errors());
    assert!(
        typed.errors[0].message.contains("type mismatch")
            || typed.errors[0].message.contains("expected")
    );
}

#[test]
fn test_let_type_annotation_mismatch() {
    // Simpler test - just the let binding
    let source = r#"@main () -> void = let x: int = "hello""#;
    let (parsed, typed) = check_source(source);

    eprintln!("Parse errors: {:?}", parsed.errors);
    eprintln!("Type errors: {:?}", typed.errors);
    assert!(
        typed.has_errors(),
        "Should have type error for let x: int = \"hello\""
    );
}

#[test]
fn test_let_type_annotation_mismatch_in_run() {
    // Test inside run pattern - with void return
    let source = r#"@main () -> void = run(let x: int = "hello", ())"#;
    let (parsed, typed) = check_source(source);

    eprintln!("Parse errors: {:?}", parsed.errors);
    eprintln!("Type errors: {:?}", typed.errors);
    // Should catch the int/str mismatch, not just return type
    let has_int_str_error = typed
        .errors
        .iter()
        .any(|e| e.message.contains("int") && e.message.contains("str"));
    assert!(
        has_int_str_error,
        "Should have type error for let x: int = \"hello\" inside run"
    );
}

#[test]
fn test_typed_module_salsa_traits() {
    use std::collections::HashSet;

    let (_, typed1) = check_source("@main () -> int = 42");
    let (_, typed2) = check_source("@main () -> int = 42");
    let (_, typed3) = check_source("@main () -> bool = true");

    assert_eq!(typed1, typed2);
    assert_ne!(typed1, typed3);

    let mut set = HashSet::new();
    set.insert(typed1.clone());
    set.insert(typed2);
    set.insert(typed3);
    assert_eq!(set.len(), 2);
}
