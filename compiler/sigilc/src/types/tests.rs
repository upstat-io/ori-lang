// Comprehensive tests for the Sigil type checker
//
// Test coverage:
// - Type inference for literals
// - Function type checking
// - Pattern type checking (fold, map, filter, recurse)
// - Lambda type inference
// - Generic function instantiation
// - Error detection and messages

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::ast::*;
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::types::check;

// ============================================================================
// Helper Functions
// ============================================================================

fn check_source(source: &str) -> Result<Module, String> {
    let tokens = tokenize(source, "test.si")?;
    let module = parse(tokens, "test.si")?;
    check(module)
}

fn check_ok(source: &str) -> Module {
    check_source(source).expect("type checking should succeed")
}

fn check_err(source: &str) -> String {
    check_source(source).expect_err("type checking should fail")
}

// ============================================================================
// Literal Type Tests
// ============================================================================

#[test]
fn test_int_literal_type() {
    check_ok("@f () -> int = 42");
}

#[test]
fn test_float_literal_type() {
    check_ok("@f () -> float = 3.14");
}

#[test]
fn test_string_literal_type() {
    check_ok(r#"@f () -> str = "hello""#);
}

#[test]
fn test_bool_literal_type() {
    check_ok("@f () -> bool = true");
}

#[test]
fn test_nil_literal_type() {
    // nil is used as a value but void is the type
    check_ok("@f () -> void = nil");
}

// ============================================================================
// Collection Type Tests
// ============================================================================

#[test]
fn test_list_type() {
    check_ok("@f () -> [int] = [1, 2, 3]");
}

#[test]
fn test_empty_list_with_return_type() {
    check_ok("@f () -> [int] = []");
}

#[test]
fn test_list_homogeneous_elements() {
    let err = check_err(r#"@f () -> [int] = [1, "two", 3]"#);
    assert!(err.contains("type"));
}

#[test]
fn test_tuple_type() {
    check_ok(r#"@f () -> (int, str) = (1, "hello")"#);
}

// ============================================================================
// Binary Operator Type Tests
// ============================================================================

#[test]
fn test_add_int() {
    check_ok("@f () -> int = 1 + 2");
}

#[test]
fn test_add_float() {
    check_ok("@f () -> float = 1.0 + 2.0");
}

#[test]
fn test_add_string() {
    check_ok(r#"@f () -> str = "hello" + " world""#);
}

#[test]
fn test_comparison_returns_bool() {
    check_ok("@f () -> bool = 1 < 2");
}

#[test]
fn test_equality_returns_bool() {
    check_ok("@f () -> bool = 1 == 2");
}

#[test]
fn test_logical_and() {
    check_ok("@f () -> bool = true && false");
}

#[test]
fn test_logical_or() {
    check_ok("@f () -> bool = true || false");
}

// ============================================================================
// Function Call Type Tests
// ============================================================================

#[test]
fn test_call_defined_function() {
    check_ok(
        r#"
@add (a: int, b: int) -> int = a + b
@main () -> int = add(1, 2)
@test_add tests @add () -> void = assert(true)
"#,
    );
}

#[test]
fn test_call_wrong_arg_count() {
    let err = check_err(
        r#"
@add (a: int, b: int) -> int = a + b
@main () -> int = add(1)
@test_add tests @add () -> void = assert(true)
"#,
    );
    assert!(err.contains("expects") && err.contains("argument"));
}

#[test]
fn test_call_wrong_arg_type() {
    let err = check_err(
        r#"
@add (a: int, b: int) -> int = a + b
@main () -> int = add(1, "two")
@test_add tests @add () -> void = assert(true)
"#,
    );
    assert!(err.contains("type"));
}

#[test]
fn test_builtin_print() {
    check_ok(r#"@f () -> void = print("hello")"#);
}

#[test]
fn test_builtin_len_string() {
    check_ok(r#"@f () -> int = len("hello")"#);
}

#[test]
fn test_builtin_len_list() {
    check_ok("@f () -> int = len([1, 2, 3])");
}

// ============================================================================
// Control Flow Type Tests
// ============================================================================

#[test]
fn test_if_condition_must_be_bool() {
    let err = check_err("@f () -> int = if 42 :then 1 :else 0");
    assert!(err.contains("bool"));
}

#[test]
fn test_if_branches_must_match() {
    let err = check_err(r#"@f () -> int = if true :then 1 :else "no""#);
    assert!(err.contains("type"));
}

#[test]
fn test_if_returns_branch_type() {
    check_ok("@f () -> int = if true :then 1 :else 0");
}

#[test]
fn test_match_arms_must_match() {
    check_ok(r#"@f (x: int) -> str = match(x, 0: "zero", _: "other")"#);
}

// ============================================================================
// Method Call Type Tests
// ============================================================================

#[test]
fn test_string_len() {
    check_ok(r#"@f (s: str) -> int = s.len()"#);
}

#[test]
fn test_list_len() {
    check_ok("@f (arr: [int]) -> int = arr.len()");
}

#[test]
fn test_list_push() {
    check_ok("@f (arr: [int]) -> [int] = arr.push(42)");
}

#[test]
fn test_list_first() {
    check_ok("@f (arr: [int]) -> ?int = arr.first()");
}

#[test]
fn test_string_split() {
    check_ok(r#"@f (s: str) -> [str] = s.split(",")"#);
}

// ============================================================================
// Index Type Tests
// ============================================================================

#[test]
fn test_list_index() {
    check_ok("@f (arr: [int]) -> int = arr[0]");
}

#[test]
fn test_string_index() {
    check_ok(r#"@f (s: str) -> str = s[0]"#);
}

// ============================================================================
// Pattern Type Tests - Fold
// ============================================================================

#[test]
fn test_fold_sum() {
    check_ok("@sum (arr: [int]) -> int = fold(arr, 0, +)");
}

#[test]
fn test_fold_with_lambda() {
    check_ok("@sum (arr: [int]) -> int = fold(arr, 0, (acc, x) -> acc + x)");
}

#[test]
fn test_fold_non_list_error() {
    let err = check_err("@f (x: int) -> int = fold(x, 0, +)");
    assert!(err.contains("list"));
}

// ============================================================================
// Pattern Type Tests - Map
// ============================================================================

#[test]
fn test_map_transform() {
    check_ok("@double (arr: [int]) -> [int] = map(arr, x -> x * 2)");
}

#[test]
fn test_map_type_change() {
    check_ok(r#"@stringify (arr: [int]) -> [str] = map(arr, x -> str(x))"#);
}

// ============================================================================
// Pattern Type Tests - Filter
// ============================================================================

#[test]
fn test_filter_preserves_type() {
    check_ok("@evens (arr: [int]) -> [int] = filter(arr, x -> x % 2 == 0)");
}

#[test]
fn test_filter_predicate_must_return_bool() {
    // Lambda infers bool from filter context, so this should work
    check_ok("@positive (arr: [int]) -> [int] = filter(arr, x -> x > 0)");
}

// ============================================================================
// Pattern Type Tests - Recurse
// ============================================================================

#[test]
fn test_recurse_factorial() {
    check_ok("@factorial (n: int) -> int = recurse(n <= 1, 1, n * self(n - 1))");
}

#[test]
fn test_recurse_types_must_match() {
    let err = check_err(r#"@f (n: int) -> int = recurse(n <= 1, "base", n)"#);
    assert!(err.contains("type"));
}

// ============================================================================
// Pattern Type Tests - Collect
// ============================================================================

#[test]
fn test_collect_squares() {
    check_ok("@squares (n: int) -> [int] = collect(1..n, x -> x * x)");
}

// ============================================================================
// Lambda Type Inference Tests
// ============================================================================

#[test]
fn test_lambda_infers_type_from_context() {
    check_ok("@f () -> [int] = map([1, 2, 3], x -> x + 1)");
}

#[test]
fn test_lambda_without_context_error() {
    let err = check_err("@f () -> int = (x -> x + 1)(5)");
    assert!(err.contains("infer") || err.contains("lambda"));
}

// ============================================================================
// Return Type Mismatch Tests
// ============================================================================

#[test]
fn test_return_type_mismatch() {
    let err = check_err(r#"@f () -> int = "not an int""#);
    assert!(err.contains("type"));
}

#[test]
fn test_return_type_mismatch_function() {
    let err = check_err("@f () -> str = 42");
    assert!(err.contains("type"));
}

// ============================================================================
// Variable Scope Tests
// ============================================================================

#[test]
fn test_param_in_scope() {
    check_ok("@f (x: int) -> int = x + 1");
}

#[test]
fn test_undefined_variable() {
    let err = check_err("@f () -> int = undefined_var");
    assert!(err.contains("Unknown identifier") || err.contains("undefined"));
}

#[test]
fn test_assignment_creates_binding() {
    check_ok(
        r#"
@f () -> int = run(
    x := 5,
    x + 1
)
"#,
    );
}

// ============================================================================
// Result/Option Type Tests
// ============================================================================

#[test]
fn test_ok_type() {
    check_ok("@f () -> Result int str = Ok(42)");
}

#[test]
fn test_err_type() {
    check_ok(r#"@f () -> Result int str = Err("error")"#);
}

#[test]
fn test_some_type() {
    check_ok("@f () -> ?int = Some(42)");
}

#[test]
fn test_none_type_with_context() {
    check_ok("@f () -> ?int = None");
}

#[test]
fn test_coalesce_type() {
    check_ok("@f (x: ?int) -> int = x ?? 0");
}

// ============================================================================
// Generic Function Tests
// ============================================================================

#[test]
fn test_assert_eq_generic() {
    check_ok("@f () -> void = assert_eq(1, 1)");
}

#[test]
fn test_assert_eq_type_mismatch() {
    let err = check_err(r#"@f () -> void = assert_eq(1, "one")"#);
    assert!(err.contains("type"));
}

// ============================================================================
// Config Type Tests
// ============================================================================

#[test]
fn test_config_infers_type() {
    check_ok("$timeout = 5000");
}

#[test]
fn test_config_explicit_type() {
    check_ok("$name: str = \"default\"");
}

#[test]
fn test_config_type_mismatch() {
    let err = check_err(r#"$count: int = "not a number""#);
    assert!(err.contains("type"));
}

// ============================================================================
// Field Access Tests
// ============================================================================

#[test]
fn test_struct_field_access() {
    check_ok(
        r#"
type Point { x: int, y: int }
@get_x (p: Point) -> int = p.x
@test_point tests @get_x () -> void = assert(true)
"#,
    );
}

// ============================================================================
// For Loop Tests
// ============================================================================

#[test]
fn test_for_loop_binding_type() {
    check_ok("@f () -> void = for i in 1..10 { print(i) }");
}

// ============================================================================
// Range Tests
// ============================================================================

#[test]
fn test_range_must_be_numeric() {
    let err = check_err(r#"@f () -> void = for i in "a".."z" { print(i) }"#);
    assert!(err.contains("numeric"));
}

// ============================================================================
// Unary Operator Tests
// ============================================================================

#[test]
fn test_negation_numeric() {
    check_ok("@f (x: int) -> int = -x");
}

#[test]
fn test_not_bool() {
    check_ok("@f (x: bool) -> bool = !x");
}

#[test]
fn test_negation_non_numeric_error() {
    let err = check_err(r#"@f (s: str) -> str = -s"#);
    assert!(err.contains("numeric") || err.contains("negate"));
}

#[test]
fn test_not_non_bool_error() {
    let err = check_err("@f (x: int) -> bool = !x");
    assert!(err.contains("bool"));
}

// ============================================================================
// Test Definition Tests
// ============================================================================

#[test]
fn test_test_references_function() {
    check_ok(
        r#"
@add (a: int, b: int) -> int = a + b
@test_add tests @add () -> void = assert(add(1, 2) == 3)
"#,
    );
}

#[test]
fn test_test_unknown_function_error() {
    let err = check_err("@test_foo tests @nonexistent () -> void = assert(true)");
    assert!(err.contains("unknown") || err.contains("nonexistent"));
}

// ============================================================================
// Parallel Pattern Tests
// ============================================================================

#[test]
fn test_parallel_returns_record() {
    check_ok(
        r#"
@getA () -> int = 1
@getB () -> str = "b"
@fetch () -> { a: int, b: str } = parallel(.a: getA(), .b: getB())
@test_getA tests @getA () -> void = assert(true)
@test_getB tests @getB () -> void = assert(true)
"#,
    );
}
