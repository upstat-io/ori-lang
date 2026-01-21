// Comprehensive tests for the Sigil evaluator
//
// Test coverage:
// - Literal evaluation
// - Binary and unary operators
// - Function calls
// - Pattern evaluation (fold, map, filter, recurse)
// - Control flow (if, match)
// - Collections (list, tuple, struct)
// - Builtins
// - Method calls

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::eval::{run, value::Value};
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::types::check;
use test_case::test_case;

// ============================================================================
// Helper Functions
// ============================================================================

fn eval_source(source: &str) -> Result<Value, String> {
    let tokens = tokenize(source, "test.si")?;
    let module = parse(tokens, "test.si")?;
    let typed = check(module)?;
    run(typed)
}

fn eval_ok(source: &str) -> Value {
    eval_source(source).expect("evaluation should succeed")
}

fn eval_err(source: &str) -> String {
    eval_source(source).expect_err("evaluation should fail")
}

// ============================================================================
// Literal Evaluation Tests
// ============================================================================

#[test]
fn test_eval_int() {
    let result = eval_ok("@main () -> int = 42");
    assert_eq!(result, Value::Int(42));
}

#[test]
#[allow(clippy::approx_constant)] // Testing that source literal "3.14" evaluates correctly
fn test_eval_float() {
    let result = eval_ok("@main () -> float = 3.14");
    if let Value::Float(f) = result {
        assert!((f - 3.14).abs() < 0.001);
    } else {
        panic!("expected float");
    }
}

#[test]
fn test_eval_string() {
    let result = eval_ok(r#"@main () -> str = "hello""#);
    assert_eq!(result, Value::String("hello".to_string()));
}

#[test]
fn test_eval_bool_true() {
    let result = eval_ok("@main () -> bool = true");
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_eval_bool_false() {
    let result = eval_ok("@main () -> bool = false");
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_eval_nil() {
    let result = eval_ok("@main () -> void = nil");
    assert_eq!(result, Value::Nil);
}

// ============================================================================
// Binary Operator Tests
// ============================================================================

#[test_case("1 + 2" => Value::Int(3); "add int")]
#[test_case("5 - 3" => Value::Int(2); "sub int")]
#[test_case("2 * 3" => Value::Int(6); "mul int")]
#[test_case("6 / 2" => Value::Int(3); "div int")]
#[test_case("7 % 3" => Value::Int(1); "mod int")]
fn test_arithmetic_op(expr: &str) -> Value {
    eval_ok(&format!("@main () -> int = {}", expr))
}

#[test_case("1 == 1" => Value::Bool(true); "eq true")]
#[test_case("1 == 2" => Value::Bool(false); "eq false")]
#[test_case("1 != 2" => Value::Bool(true); "neq true")]
#[test_case("1 < 2" => Value::Bool(true); "lt true")]
#[test_case("2 <= 2" => Value::Bool(true); "lte true")]
#[test_case("3 > 2" => Value::Bool(true); "gt true")]
#[test_case("2 >= 2" => Value::Bool(true); "gte true")]
fn test_comparison_op(expr: &str) -> Value {
    eval_ok(&format!("@main () -> bool = {}", expr))
}

#[test_case("true && true" => Value::Bool(true); "and tt")]
#[test_case("true && false" => Value::Bool(false); "and tf")]
#[test_case("true || false" => Value::Bool(true); "or tf")]
#[test_case("false || false" => Value::Bool(false); "or ff")]
fn test_logical_op(expr: &str) -> Value {
    eval_ok(&format!("@main () -> bool = {}", expr))
}

#[test]
fn test_string_concat() {
    let result = eval_ok(r#"@main () -> str = "hello" + " world""#);
    assert_eq!(result, Value::String("hello world".to_string()));
}

#[test]
fn test_list_concat() {
    let result = eval_ok("@main () -> [int] = [1, 2] + [3, 4]");
    assert_eq!(
        result,
        Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4)
        ])
    );
}

// ============================================================================
// Unary Operator Tests
// ============================================================================

#[test]
fn test_negate_int() {
    let result = eval_ok("@main () -> int = -42");
    assert_eq!(result, Value::Int(-42));
}

#[test]
fn test_not_bool() {
    let result = eval_ok("@main () -> bool = !true");
    assert_eq!(result, Value::Bool(false));
}

// ============================================================================
// Collection Tests
// ============================================================================

#[test]
fn test_list_literal() {
    let result = eval_ok("@main () -> [int] = [1, 2, 3]");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
}

#[test]
fn test_empty_list() {
    let result = eval_ok("@main () -> [int] = []");
    assert_eq!(result, Value::List(vec![]));
}

#[test]
fn test_tuple_literal() {
    let result = eval_ok(r#"@main () -> (int, str) = (1, "hello")"#);
    assert_eq!(
        result,
        Value::Tuple(vec![Value::Int(1), Value::String("hello".to_string())])
    );
}

#[test]
fn test_list_index() {
    let result = eval_ok("@main () -> int = [10, 20, 30][1]");
    assert_eq!(result, Value::Int(20));
}

// ============================================================================
// Function Call Tests
// ============================================================================

#[test]
fn test_call_function() {
    let result = eval_ok(
        r#"
@add (a: int, b: int) -> int = a + b
@main () -> int = add(3, 4)
@test_add tests @add () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(7));
}

#[test]
fn test_recursive_function() {
    let result = eval_ok(
        r#"
@factorial (n: int) -> int = if n <= 1 :then 1 :else n * factorial(n - 1)
@main () -> int = factorial(5)
@test_factorial tests @factorial () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(120));
}

// ============================================================================
// Control Flow Tests
// ============================================================================

#[test]
fn test_if_then_else_true() {
    let result = eval_ok("@main () -> int = if true :then 1 :else 0");
    assert_eq!(result, Value::Int(1));
}

#[test]
fn test_if_then_else_false() {
    let result = eval_ok("@main () -> int = if false :then 1 :else 0");
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_nested_if() {
    let result = eval_ok("@main () -> int = if true :then if false :then 1 :else 2 :else 3");
    assert_eq!(result, Value::Int(2));
}

#[test]
fn test_match_literal() {
    let result = eval_ok(r#"@main () -> str = match(1, 0: "zero", 1: "one", _: "other")"#);
    assert_eq!(result, Value::String("one".to_string()));
}

#[test]
fn test_match_wildcard() {
    let result = eval_ok(r#"@main () -> str = match(99, 0: "zero", _: "other")"#);
    assert_eq!(result, Value::String("other".to_string()));
}

// ============================================================================
// Pattern Tests - Fold
// ============================================================================

#[test]
fn test_fold_sum() {
    let result = eval_ok("@main () -> int = fold([1, 2, 3, 4], 0, +)");
    assert_eq!(result, Value::Int(10));
}

#[test]
fn test_fold_product() {
    let result = eval_ok("@main () -> int = fold([1, 2, 3, 4], 1, *)");
    assert_eq!(result, Value::Int(24));
}

#[test]
fn test_fold_with_lambda() {
    let result = eval_ok("@main () -> int = fold([1, 2, 3], 0, (acc, x) -> acc + x * 2)");
    assert_eq!(result, Value::Int(12)); // 0 + 2 + 4 + 6
}

// ============================================================================
// Pattern Tests - Map
// ============================================================================

#[test]
fn test_map_double() {
    let result = eval_ok("@main () -> [int] = map([1, 2, 3], x -> x * 2)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(2), Value::Int(4), Value::Int(6)])
    );
}

#[test]
fn test_map_to_string() {
    let result = eval_ok("@main () -> [str] = map([1, 2, 3], x -> str(x))");
    assert_eq!(
        result,
        Value::List(vec![
            Value::String("1".to_string()),
            Value::String("2".to_string()),
            Value::String("3".to_string())
        ])
    );
}

// ============================================================================
// Pattern Tests - Filter
// ============================================================================

#[test]
fn test_filter_evens() {
    let result = eval_ok("@main () -> [int] = filter([1, 2, 3, 4, 5, 6], x -> x % 2 == 0)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(2), Value::Int(4), Value::Int(6)])
    );
}

#[test]
fn test_filter_positive() {
    let result = eval_ok("@main () -> [int] = filter([-1, 0, 1, 2], x -> x > 0)");
    assert_eq!(result, Value::List(vec![Value::Int(1), Value::Int(2)]));
}

// ============================================================================
// Pattern Tests - Recurse
// ============================================================================

#[test]
fn test_recurse_simple() {
    // Test recurse with a function that properly uses the parameter
    let result = eval_ok(
        r#"
@fact (n: int) -> int = recurse(n <= 1, 1, n * self(n - 1))
@main () -> int = fact(5)
@test_fact tests @fact () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(120));
}

#[test]
fn test_recurse_with_function() {
    let result = eval_ok(
        r#"
@factorial (n: int) -> int = recurse(n <= 1, 1, n * self(n - 1))
@main () -> int = factorial(6)
@test_factorial tests @factorial () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(720));
}

#[test]
fn test_recurse_memoized_fibonacci() {
    let result = eval_ok(
        r#"
@fib (n: int) -> int = recurse(n <= 1, n, self(n - 1) + self(n - 2), true)
@main () -> int = fib(10)
@test_fib tests @fib () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(55));
}

// ============================================================================
// Pattern Tests - Collect
// ============================================================================

#[test]
fn test_collect_squares() {
    let result = eval_ok("@main () -> [int] = collect(1..5, x -> x * x)");
    assert_eq!(
        result,
        Value::List(vec![
            Value::Int(1),
            Value::Int(4),
            Value::Int(9),
            Value::Int(16)
        ])
    );
}

// ============================================================================
// Builtin Function Tests
// ============================================================================

#[test]
fn test_builtin_len_list() {
    let result = eval_ok("@main () -> int = len([1, 2, 3])");
    assert_eq!(result, Value::Int(3));
}

#[test]
fn test_builtin_len_string() {
    let result = eval_ok(r#"@main () -> int = len("hello")"#);
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_builtin_str() {
    let result = eval_ok("@main () -> str = str(42)");
    assert_eq!(result, Value::String("42".to_string()));
}

#[test]
fn test_builtin_int() {
    let result = eval_ok(r#"@main () -> int = int("42")"#);
    assert_eq!(result, Value::Int(42));
}

// ============================================================================
// Method Call Tests
// ============================================================================

#[test]
fn test_string_upper() {
    let result = eval_ok(r#"@main () -> str = "hello".upper()"#);
    assert_eq!(result, Value::String("HELLO".to_string()));
}

#[test]
fn test_string_lower() {
    let result = eval_ok(r#"@main () -> str = "HELLO".lower()"#);
    assert_eq!(result, Value::String("hello".to_string()));
}

#[test]
fn test_string_split() {
    let result = eval_ok(r#"@main () -> [str] = "a,b,c".split(",")"#);
    assert_eq!(
        result,
        Value::List(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
            Value::String("c".to_string())
        ])
    );
}

#[test]
fn test_list_push() {
    let result = eval_ok("@main () -> [int] = [1, 2].push(3)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
}

#[test]
fn test_list_first() {
    let result = eval_ok("@main () -> ?int = [1, 2, 3].first()");
    assert_eq!(result, Value::Some(Box::new(Value::Int(1))));
}

#[test]
fn test_list_first_empty() {
    // Empty list needs type context from return type
    let result = eval_ok(
        r#"
@empty () -> [int] = []
@main () -> ?int = empty().first()
@test_empty tests @empty () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::None_);
}

#[test]
fn test_list_join() {
    let result = eval_ok(r#"@main () -> str = [1, 2, 3].join(",")"#);
    assert_eq!(result, Value::String("1,2,3".to_string()));
}

// ============================================================================
// Assignment Tests
// ============================================================================

#[test]
fn test_let_in_block() {
    let result = eval_ok(
        r#"
@main () -> int = run(
    let x = 5,
    let y = x + 3,
    y * 2
)
"#,
    );
    assert_eq!(result, Value::Int(16));
}

// ============================================================================
// Config Tests
// ============================================================================

#[test]
fn test_config_usage() {
    let result = eval_ok(
        r#"
$multiplier = 10
@main () -> int = $multiplier * 5
"#,
    );
    assert_eq!(result, Value::Int(50));
}

// ============================================================================
// Range Tests
// ============================================================================

#[test]
fn test_range_collect() {
    let result = eval_ok("@main () -> [int] = collect(1..4, x -> x)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
}

// ============================================================================
// Result Type Tests
// ============================================================================

#[test]
fn test_ok_value() {
    let result = eval_ok("@main () -> Result<int, str> = Ok(42)");
    assert_eq!(result, Value::Ok(Box::new(Value::Int(42))));
}

#[test]
fn test_err_value() {
    let result = eval_ok(r#"@main () -> Result<int, str> = Err("error")"#);
    assert_eq!(
        result,
        Value::Err(Box::new(Value::String("error".to_string())))
    );
}

// ============================================================================
// Option Type Tests
// ============================================================================

#[test]
fn test_some_value() {
    let result = eval_ok("@main () -> ?int = Some(42)");
    assert_eq!(result, Value::Some(Box::new(Value::Int(42))));
}

#[test]
fn test_none_value() {
    let result = eval_ok("@main () -> ?int = None");
    assert_eq!(result, Value::None_);
}

#[test]
fn test_coalesce_some() {
    let result = eval_ok("@main () -> int = Some(42) ?? 0");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_coalesce_none() {
    // Use a function that returns None to provide type context
    let result = eval_ok(
        r#"
@get_none () -> ?int = None
@main () -> int = get_none() ?? 0
@test_get_none tests @get_none () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(0));
}

// ============================================================================
// Error Tests
// ============================================================================

#[test]
fn test_division_by_zero() {
    let err = eval_err("@main () -> int = 1 / 0");
    assert!(err.contains("zero") || err.contains("Division"));
}

#[test]
fn test_assert_failure() {
    let err = eval_err("@main () -> void = assert(false)");
    assert!(err.contains("Assertion") || err.contains("failed"));
}

#[test]
fn test_assert_eq_failure() {
    let err = eval_err("@main () -> void = assert_eq(1, 2)");
    assert!(err.contains("Assertion") || err.contains("!="));
}

// ============================================================================
// Pattern Tests - Additional Fold Variations
// ============================================================================

#[test]
fn test_fold_find_max() {
    let result = eval_ok("@main () -> int = fold([3, 1, 4, 1, 5, 9], 0, (acc, x) -> if x > acc :then x :else acc)");
    assert_eq!(result, Value::Int(9));
}

#[test]
fn test_fold_string_concat() {
    let result = eval_ok(r#"@main () -> str = fold(["a", "b", "c"], "", (acc, s) -> acc + s)"#);
    assert_eq!(result, Value::String("abc".to_string()));
}

#[test]
fn test_fold_named_syntax() {
    let result = eval_ok("@main () -> int = fold(.over: [1, 2, 3], .init: 10, .op: +)");
    assert_eq!(result, Value::Int(16));
}

// ============================================================================
// Pattern Tests - Additional Map Variations
// ============================================================================

#[test]
fn test_map_named_syntax() {
    let result = eval_ok("@main () -> [int] = map(.over: [1, 2, 3], .transform: x -> x * 10)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)])
    );
}

#[test]
fn test_map_range() {
    let result = eval_ok("@main () -> [int] = map(1..4, x -> x * x)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(1), Value::Int(4), Value::Int(9)])
    );
}

// ============================================================================
// Pattern Tests - Additional Filter Variations
// ============================================================================

#[test]
fn test_filter_named_syntax() {
    let result = eval_ok("@main () -> [int] = filter(.over: [1, 2, 3, 4, 5], .predicate: x -> x > 2)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(3), Value::Int(4), Value::Int(5)])
    );
}

#[test]
fn test_filter_empty_result() {
    let result = eval_ok("@main () -> [int] = filter([1, 2, 3], x -> x > 100)");
    assert_eq!(result, Value::List(vec![]));
}

// ============================================================================
// Pattern Tests - Additional Recurse Variations
// ============================================================================

#[test]
fn test_recurse_named_syntax() {
    let result = eval_ok(
        r#"
@fib (n: int) -> int = recurse(.cond: n <= 1, .base: n, .step: self(n - 1) + self(n - 2), .memo: true)
@main () -> int = fib(10)
@test_fib tests @fib () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(55));
}

#[test]
fn test_recurse_countdown() {
    let result = eval_ok(
        r#"
@countdown (n: int) -> int = recurse(n <= 0, 0, self(n - 1) + 1)
@main () -> int = countdown(5)
@test_countdown tests @countdown () -> void = assert(true)
"#,
    );
    assert_eq!(result, Value::Int(5));
}

// ============================================================================
// Pattern Tests - Additional Collect Variations
// ============================================================================

#[test]
fn test_collect_cubes() {
    let result = eval_ok("@main () -> [int] = collect(1..4, x -> x * x * x)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(1), Value::Int(8), Value::Int(27)])
    );
}

#[test]
fn test_collect_named_syntax() {
    let result = eval_ok("@main () -> [int] = collect(.range: 1..5, .transform: x -> x + 10)");
    assert_eq!(
        result,
        Value::List(vec![Value::Int(11), Value::Int(12), Value::Int(13), Value::Int(14)])
    );
}

// ============================================================================
// Pattern Tests - Parallel
// ============================================================================

#[test]
fn test_parallel_basic() {
    let result = eval_ok("@main () -> {a: int, b: int} = parallel(.a: 1 + 1, .b: 2 + 2)");
    if let Value::Struct { fields, .. } = result {
        assert_eq!(fields.get("a"), Some(&Value::Int(2)));
        assert_eq!(fields.get("b"), Some(&Value::Int(4)));
    } else {
        panic!("Expected struct result from parallel");
    }
}

#[test]
fn test_parallel_different_types() {
    let result = eval_ok(r#"@main () -> {n: int, s: str} = parallel(.n: 42, .s: "hello")"#);
    if let Value::Struct { fields, .. } = result {
        assert_eq!(fields.get("n"), Some(&Value::Int(42)));
        assert_eq!(fields.get("s"), Some(&Value::String("hello".to_string())));
    } else {
        panic!("Expected struct result from parallel");
    }
}

#[test]
fn test_parallel_computations() {
    let result = eval_ok(
        r#"
@main () -> {sum: int, product: int} = parallel(
    .sum: fold([1, 2, 3], 0, +),
    .product: fold([1, 2, 3], 1, *)
)
"#,
    );
    if let Value::Struct { fields, .. } = result {
        assert_eq!(fields.get("sum"), Some(&Value::Int(6)));
        assert_eq!(fields.get("product"), Some(&Value::Int(6)));
    } else {
        panic!("Expected struct result from parallel");
    }
}
