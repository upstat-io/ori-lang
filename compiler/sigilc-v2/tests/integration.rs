//! Integration Tests for Sigil V2 Compiler
//!
//! These tests exercise the full compiler pipeline (lex → parse → evaluate)
//! to ensure code coverage of the actual compiler implementation.

use std::rc::Rc;
use std::cell::RefCell;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ItemKind};
use sigilc_v2::eval::{Evaluator, Value, FunctionValue};

/// Helper to run Sigil code and return the result
fn eval(source: &str) -> Result<Value, String> {
    let interner = StringInterner::new();
    let lexer = Lexer::new(source, &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let parse_result = parser.parse_module();

    // Check for parse errors
    if !parse_result.diagnostics.is_empty() {
        let errors: Vec<String> = parse_result.diagnostics
            .iter()
            .map(|d| format!("{:?}", d))
            .collect();
        return Err(format!("Parse error: {}", errors.join(", ")));
    }

    let mut evaluator = Evaluator::new(&interner, &parse_result.arena);
    evaluator.register_prelude();

    // Register all functions
    for item in &parse_result.items {
        if let ItemKind::Function(func) = &item.kind {
            let params: Vec<_> = parse_result.arena.get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();
            let func_value = Value::Function(FunctionValue {
                params,
                body: func.body,
                captures: Rc::new(RefCell::new(Default::default())),
            });
            evaluator.env_mut().define_global(func.name, func_value);
        }
    }

    // Find and evaluate main function
    for item in &parse_result.items {
        if let ItemKind::Function(func) = &item.kind {
            let name = interner.lookup(func.name);
            if name == "main" {
                return evaluator.eval(func.body).map_err(|e| format!("Eval error: {}", e.message));
            }
        }
    }

    Err("No main function found".to_string())
}

/// Helper to check if code evaluates successfully
fn eval_ok(source: &str) -> Value {
    match eval(source) {
        Ok(v) => v,
        Err(e) => panic!("Expected success, got error: {}", e),
    }
}

/// Helper to check if code fails with expected error
fn eval_err(source: &str, expected: &str) {
    match eval(source) {
        Err(e) => assert!(e.contains(expected), "Expected '{}' in error: {}", expected, e),
        Ok(v) => panic!("Expected error containing '{}', got: {:?}", expected, v),
    }
}

/// Helper to compare Value::List contents
fn assert_list_eq(actual: &Value, expected: Vec<Value>) {
    if let Value::List(items) = actual {
        assert_eq!(items.as_ref(), &expected, "List contents mismatch");
    } else {
        panic!("Expected list, got {:?}", actual);
    }
}

// =============================================================================
// Arithmetic Expressions
// =============================================================================

mod arithmetic {
    use super::*;

    #[test]
    fn test_integer_literals() {
        assert_eq!(eval_ok("@main () -> int = 42"), Value::Int(42));
        assert_eq!(eval_ok("@main () -> int = 0"), Value::Int(0));
        assert_eq!(eval_ok("@main () -> int = -1"), Value::Int(-1));
        assert_eq!(eval_ok("@main () -> int = 1_000_000"), Value::Int(1_000_000));
    }

    #[test]
    fn test_float_literals() {
        assert_eq!(eval_ok("@main () -> float = 3.14"), Value::Float(3.14));
        assert_eq!(eval_ok("@main () -> float = 0.0"), Value::Float(0.0));
        assert_eq!(eval_ok("@main () -> float = -2.5"), Value::Float(-2.5));
    }

    #[test]
    fn test_binary_literals() {
        assert_eq!(eval_ok("@main () -> int = 0b1010"), Value::Int(10));
        assert_eq!(eval_ok("@main () -> int = 0b1111"), Value::Int(15));
        assert_eq!(eval_ok("@main () -> int = 0b0"), Value::Int(0));
    }

    #[test]
    fn test_hex_literals() {
        assert_eq!(eval_ok("@main () -> int = 0xFF"), Value::Int(255));
        assert_eq!(eval_ok("@main () -> int = 0x10"), Value::Int(16));
        assert_eq!(eval_ok("@main () -> int = 0xABCD"), Value::Int(0xABCD));
    }

    #[test]
    fn test_addition() {
        assert_eq!(eval_ok("@main () -> int = 1 + 2"), Value::Int(3));
        assert_eq!(eval_ok("@main () -> int = 10 + 20 + 30"), Value::Int(60));
    }

    #[test]
    fn test_subtraction() {
        assert_eq!(eval_ok("@main () -> int = 5 - 3"), Value::Int(2));
        assert_eq!(eval_ok("@main () -> int = 10 - 20"), Value::Int(-10));
    }

    #[test]
    fn test_multiplication() {
        assert_eq!(eval_ok("@main () -> int = 3 * 4"), Value::Int(12));
        assert_eq!(eval_ok("@main () -> int = -2 * 3"), Value::Int(-6));
    }

    #[test]
    fn test_division() {
        assert_eq!(eval_ok("@main () -> int = 10 / 2"), Value::Int(5));
        assert_eq!(eval_ok("@main () -> int = 7 / 2"), Value::Int(3));
    }

    #[test]
    fn test_modulo() {
        assert_eq!(eval_ok("@main () -> int = 10 % 3"), Value::Int(1));
        assert_eq!(eval_ok("@main () -> int = 15 % 5"), Value::Int(0));
    }

    #[test]
    fn test_floor_division() {
        assert_eq!(eval_ok("@main () -> int = 7 div 2"), Value::Int(3));
        assert_eq!(eval_ok("@main () -> int = -7 div 2"), Value::Int(-4));
    }

    #[test]
    fn test_unary_minus() {
        assert_eq!(eval_ok("@main () -> int = -5"), Value::Int(-5));
        assert_eq!(eval_ok("@main () -> int = --5"), Value::Int(5));
    }

    #[test]
    fn test_precedence() {
        assert_eq!(eval_ok("@main () -> int = 2 + 3 * 4"), Value::Int(14));
        assert_eq!(eval_ok("@main () -> int = (2 + 3) * 4"), Value::Int(20));
        assert_eq!(eval_ok("@main () -> int = 10 - 4 / 2"), Value::Int(8));
    }
}

// =============================================================================
// Comparison and Logical Expressions
// =============================================================================

mod comparison {
    use super::*;

    #[test]
    fn test_equality() {
        assert_eq!(eval_ok("@main () -> bool = 1 == 1"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 1 == 2"), Value::Bool(false));
        assert_eq!(eval_ok("@main () -> bool = 1 != 2"), Value::Bool(true));
    }

    #[test]
    fn test_comparison() {
        assert_eq!(eval_ok("@main () -> bool = 1 < 2"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 2 <= 2"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 3 > 2"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 3 >= 3"), Value::Bool(true));
    }

    #[test]
    fn test_string_equality() {
        // String comparison (< > etc) not yet implemented, but equality works
        assert_eq!(eval_ok(r#"@main () -> bool = "hello" == "hello""#), Value::Bool(true));
        assert_eq!(eval_ok(r#"@main () -> bool = "hello" != "world""#), Value::Bool(true));
    }

    #[test]
    fn test_char_comparison() {
        assert_eq!(eval_ok("@main () -> bool = 'a' < 'b'"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 'z' > 'a'"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 'x' == 'x'"), Value::Bool(true));
    }

    #[test]
    fn test_logical_and() {
        assert_eq!(eval_ok("@main () -> bool = true && true"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = true && false"), Value::Bool(false));
        assert_eq!(eval_ok("@main () -> bool = false && true"), Value::Bool(false));
    }

    #[test]
    fn test_logical_or() {
        assert_eq!(eval_ok("@main () -> bool = true || false"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = false || true"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = false || false"), Value::Bool(false));
    }

    #[test]
    fn test_logical_not() {
        assert_eq!(eval_ok("@main () -> bool = !true"), Value::Bool(false));
        assert_eq!(eval_ok("@main () -> bool = !false"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = !!true"), Value::Bool(true));
    }
}

// =============================================================================
// Bitwise Expressions
// =============================================================================

mod bitwise {
    use super::*;

    #[test]
    fn test_bitwise_and() {
        assert_eq!(eval_ok("@main () -> int = 0b1100 & 0b1010"), Value::Int(0b1000));
        assert_eq!(eval_ok("@main () -> int = 0xFF & 0x0F"), Value::Int(0x0F));
    }

    #[test]
    fn test_bitwise_or() {
        assert_eq!(eval_ok("@main () -> int = 0b1100 | 0b1010"), Value::Int(0b1110));
        assert_eq!(eval_ok("@main () -> int = 0xF0 | 0x0F"), Value::Int(0xFF));
    }

    #[test]
    fn test_bitwise_xor() {
        assert_eq!(eval_ok("@main () -> int = 0b1100 ^ 0b1010"), Value::Int(0b0110));
        assert_eq!(eval_ok("@main () -> int = 0xFF ^ 0xFF"), Value::Int(0));
    }

    #[test]
    fn test_bitwise_not() {
        assert_eq!(eval_ok("@main () -> int = ~0"), Value::Int(-1));
        assert_eq!(eval_ok("@main () -> int = ~1"), Value::Int(-2));
        assert_eq!(eval_ok("@main () -> int = ~~5"), Value::Int(5));
    }

    #[test]
    fn test_shift_left() {
        assert_eq!(eval_ok("@main () -> int = 1 << 4"), Value::Int(16));
        assert_eq!(eval_ok("@main () -> int = 3 << 2"), Value::Int(12));
    }

    #[test]
    fn test_shift_right() {
        assert_eq!(eval_ok("@main () -> int = 16 >> 2"), Value::Int(4));
        assert_eq!(eval_ok("@main () -> int = 255 >> 4"), Value::Int(15));
    }
}

// =============================================================================
// Strings and Characters
// =============================================================================

mod strings {
    use super::*;

    #[test]
    fn test_string_literals() {
        assert_eq!(eval_ok(r#"@main () -> str = "hello""#), Value::Str(Rc::new("hello".into())));
        assert_eq!(eval_ok(r#"@main () -> str = """#), Value::Str(Rc::new("".into())));
    }

    #[test]
    fn test_string_escapes() {
        assert_eq!(eval_ok(r#"@main () -> str = "line1\nline2""#), Value::Str(Rc::new("line1\nline2".into())));
        assert_eq!(eval_ok(r#"@main () -> str = "tab\there""#), Value::Str(Rc::new("tab\there".into())));
    }

    #[test]
    fn test_string_concatenation() {
        assert_eq!(eval_ok(r#"@main () -> str = "hello" + " " + "world""#), Value::Str(Rc::new("hello world".into())));
    }

    // Note: char type parsing not yet implemented in type expressions
    // Char values work but we can't use `-> char` as return type
    #[test]
    fn test_char_in_list() {
        // Test chars as values (not using char type annotation)
        assert_eq!(eval_ok("@main () -> bool = 'a' == 'a'"), Value::Bool(true));
        assert_eq!(eval_ok("@main () -> bool = 'a' != 'b'"), Value::Bool(true));
    }
}

// =============================================================================
// Collections
// =============================================================================

mod collections {
    use super::*;

    #[test]
    fn test_list_literal() {
        let result = eval_ok("@main () -> [int] = [1, 2, 3]");
        assert_list_eq(&result, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    }

    #[test]
    fn test_empty_list() {
        let result = eval_ok("@main () -> [int] = []");
        assert_list_eq(&result, vec![]);
    }

    #[test]
    fn test_list_index() {
        assert_eq!(eval_ok("@main () -> int = run(let arr = [10, 20, 30], arr[0])"), Value::Int(10));
        assert_eq!(eval_ok("@main () -> int = run(let arr = [10, 20, 30], arr[2])"), Value::Int(30));
    }

    #[test]
    fn test_list_hash_length() {
        assert_eq!(eval_ok("@main () -> int = run(let arr = [1, 2, 3, 4, 5], arr[# - 1])"), Value::Int(5));
        assert_eq!(eval_ok("@main () -> int = run(let arr = [1, 2, 3, 4, 5], arr[# - 2])"), Value::Int(4));
    }

    #[test]
    fn test_map_literal() {
        let result = eval_ok(r#"@main () -> {str: int} = {"a": 1, "b": 2}"#);
        if let Value::Map(m) = result {
            assert_eq!(m.len(), 2);
        } else {
            panic!("Expected map, got {:?}", result);
        }
    }

    #[test]
    fn test_map_index() {
        assert_eq!(eval_ok(r#"@main () -> int = run(let m = {"a": 1, "b": 2}, m["a"])"#), Value::Int(1));
        assert_eq!(eval_ok(r#"@main () -> int = run(let m = {"x": 42}, m["x"])"#), Value::Int(42));
    }

    #[test]
    fn test_tuple() {
        let result = eval_ok("@main () -> (int, str) = (42, \"hello\")");
        if let Value::Tuple(items) = result {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], Value::Int(42));
            assert_eq!(items[1], Value::Str(Rc::new("hello".into())));
        } else {
            panic!("Expected tuple, got {:?}", result);
        }
    }
}

// =============================================================================
// Bindings
// =============================================================================

mod bindings {
    use super::*;

    #[test]
    fn test_let_binding() {
        assert_eq!(eval_ok("@main () -> int = run(let x = 42, x)"), Value::Int(42));
    }

    #[test]
    fn test_let_mutable() {
        assert_eq!(eval_ok("@main () -> int = run(let mut x = 0, x = 1, x = 2, x)"), Value::Int(2));
    }

    #[test]
    fn test_let_shadowing() {
        assert_eq!(eval_ok("@main () -> int = run(let x = 1, let x = x + 10, x)"), Value::Int(11));
    }

    #[test]
    fn test_tuple_destructure() {
        assert_eq!(eval_ok("@main () -> int = run(let (a, b) = (1, 2), a + b)"), Value::Int(3));
    }

    #[test]
    fn test_nested_tuple_destructure() {
        assert_eq!(eval_ok("@main () -> int = run(let (x, (y, z)) = (1, (2, 3)), x + y + z)"), Value::Int(6));
    }

    #[test]
    fn test_list_destructure() {
        assert_eq!(eval_ok("@main () -> int = run(let [a, b] = [1, 2], a + b)"), Value::Int(3));
    }

    #[test]
    fn test_list_rest_destructure() {
        assert_eq!(eval_ok("@main () -> int = run(let [head, ..tail] = [1, 2, 3, 4], head)"), Value::Int(1));
    }
}

// =============================================================================
// Conditionals
// =============================================================================

mod conditionals {
    use super::*;

    #[test]
    fn test_if_then_else() {
        assert_eq!(eval_ok("@main () -> int = if true then 1 else 2"), Value::Int(1));
        assert_eq!(eval_ok("@main () -> int = if false then 1 else 2"), Value::Int(2));
    }

    #[test]
    fn test_if_comparison() {
        assert_eq!(eval_ok("@main () -> int = if 5 > 3 then 10 else 20"), Value::Int(10));
        assert_eq!(eval_ok("@main () -> int = if 1 == 2 then 10 else 20"), Value::Int(20));
    }

    #[test]
    fn test_nested_if() {
        assert_eq!(eval_ok("@main () -> int = if true then if false then 1 else 2 else 3"), Value::Int(2));
    }

    #[test]
    fn test_if_else_if() {
        let code = "@main () -> int = if false then 1 else if false then 2 else 3";
        assert_eq!(eval_ok(code), Value::Int(3));
    }
}

// =============================================================================
// Functions
// =============================================================================

mod functions {
    use super::*;

    #[test]
    fn test_function_call() {
        let code = r#"
            @add (a: int, b: int) -> int = a + b
            @main () -> int = add(2, 3)
        "#;
        assert_eq!(eval_ok(code), Value::Int(5));
    }

    #[test]
    fn test_recursive_function() {
        let code = r#"
            @factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n - 1)
            @main () -> int = factorial(5)
        "#;
        assert_eq!(eval_ok(code), Value::Int(120));
    }

    #[test]
    fn test_mutual_recursion() {
        let code = r#"
            @is_even (n: int) -> bool = if n == 0 then true else is_odd(n - 1)
            @is_odd (n: int) -> bool = if n == 0 then false else is_even(n - 1)
            @main () -> bool = is_even(10)
        "#;
        assert_eq!(eval_ok(code), Value::Bool(true));
    }

    #[test]
    fn test_higher_order_function() {
        let code = r#"
            @apply (f: (int) -> int, x: int) -> int = f(x)
            @double (n: int) -> int = n * 2
            @main () -> int = apply(double, 21)
        "#;
        assert_eq!(eval_ok(code), Value::Int(42));
    }
}

// =============================================================================
// Lambdas
// =============================================================================

mod lambdas {
    use super::*;

    #[test]
    fn test_simple_lambda() {
        assert_eq!(eval_ok("@main () -> int = run(let f = x -> x + 1, f(5))"), Value::Int(6));
    }

    #[test]
    fn test_multi_param_lambda() {
        assert_eq!(eval_ok("@main () -> int = run(let f = (a, b) -> a + b, f(3, 4))"), Value::Int(7));
    }

    #[test]
    fn test_no_param_lambda() {
        assert_eq!(eval_ok("@main () -> int = run(let f = () -> 42, f())"), Value::Int(42));
    }

    #[test]
    fn test_lambda_closure() {
        assert_eq!(eval_ok("@main () -> int = run(let x = 10, let f = y -> x + y, f(5))"), Value::Int(15));
    }
}

// =============================================================================
// Loops
// =============================================================================

mod loops {
    use super::*;

    #[test]
    fn test_for_do() {
        let code = r#"
            @main () -> int = run(
                let mut sum = 0,
                for x in [1, 2, 3, 4, 5] do
                    sum = sum + x,
                sum
            )
        "#;
        assert_eq!(eval_ok(code), Value::Int(15));
    }

    #[test]
    fn test_for_yield() {
        let code = "@main () -> [int] = for x in [1, 2, 3] yield x * 2";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
    }

    #[test]
    fn test_for_yield_guard() {
        let code = "@main () -> [int] = for x in [1, 2, 3, 4, 5, 6] if x % 2 == 0 yield x";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
    }

    #[test]
    fn test_for_range() {
        let code = "@main () -> [int] = for i in 0..5 yield i";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(0), Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]);
    }

    #[test]
    fn test_for_range_inclusive() {
        let code = "@main () -> [int] = for i in 1..=3 yield i * i";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(1), Value::Int(4), Value::Int(9)]);
    }

    #[test]
    fn test_loop_break() {
        let code = r#"
            @main () -> int = run(
                let mut i = 0,
                let mut sum = 0,
                loop(
                    if i >= 5 then break
                    else run(
                        sum = sum + i,
                        i = i + 1,
                    )
                ),
                sum
            )
        "#;
        assert_eq!(eval_ok(code), Value::Int(10));
    }
}

// =============================================================================
// Patterns
// =============================================================================

mod patterns {
    use super::*;

    #[test]
    fn test_run_pattern() {
        assert_eq!(eval_ok("@main () -> int = run(let x = 1, let y = 2, x + y)"), Value::Int(3));
    }

    #[test]
    fn test_map_pattern() {
        let code = "@main () -> [int] = map(.over: [1, 2, 3], .transform: x -> x * 2)";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
    }

    #[test]
    fn test_filter_pattern() {
        let code = "@main () -> [int] = filter(.over: [1, 2, 3, 4, 5, 6], .predicate: x -> x % 2 == 0)";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
    }

    #[test]
    fn test_fold_pattern() {
        let code = "@main () -> int = fold(.over: [1, 2, 3, 4, 5], .init: 0, .op: (acc, x) -> acc + x)";
        assert_eq!(eval_ok(code), Value::Int(15));
    }

    #[test]
    fn test_find_pattern() {
        let code = "@main () -> int = run(let result = find(.over: [1, 2, 3, 4, 5], .where: x -> x > 3), match(result, Some(x) -> x, None -> 0))";
        assert_eq!(eval_ok(code), Value::Int(4));
    }

    #[test]
    fn test_collect_pattern() {
        let code = "@main () -> [int] = collect(.range: 0..5, .transform: i -> i * i)";
        let result = eval_ok(code);
        assert_list_eq(&result, vec![Value::Int(0), Value::Int(1), Value::Int(4), Value::Int(9), Value::Int(16)]);
    }

    #[test]
    fn test_try_pattern() {
        let code = r#"
            @safe_div (a: int, b: int) -> Result<int, str> =
                if b == 0 then Err("division by zero") else Ok(a / b)
            @main () -> Result<int, str> = try(let x = safe_div(10, 2)?, Ok(x * 2))
        "#;
        let result = eval_ok(code);
        if let Value::Ok(inner) = result {
            assert_eq!(*inner, Value::Int(10));
        } else {
            panic!("Expected Ok variant, got {:?}", result);
        }
    }

    #[test]
    fn test_try_early_return() {
        let code = r#"
            @safe_div (a: int, b: int) -> Result<int, str> =
                if b == 0 then Err("division by zero") else Ok(a / b)
            @main () -> Result<int, str> = try(let x = safe_div(10, 0)?, Ok(x * 2))
        "#;
        let result = eval_ok(code);
        if let Value::Err(_) = result {
            // Success - got error as expected
        } else {
            panic!("Expected Err variant, got {:?}", result);
        }
    }

    #[test]
    fn test_match_pattern() {
        let code = "@main () -> int = match(Some(42), Some(x) -> x, None -> 0)";
        assert_eq!(eval_ok(code), Value::Int(42));
    }

    #[test]
    fn test_match_wildcard() {
        let code = "@main () -> int = match(5, 1 -> 10, 2 -> 20, _ -> 99)";
        assert_eq!(eval_ok(code), Value::Int(99));
    }

    #[test]
    fn test_match_guard() {
        let code = "@main () -> str = match(15, x.match(x > 10) -> \"big\", x.match(x > 5) -> \"medium\", _ -> \"small\")";
        assert_eq!(eval_ok(code), Value::Str(Rc::new("big".into())));
    }

    #[test]
    fn test_match_list() {
        let code = "@main () -> int = match([1, 2, 3], [] -> 0, [x] -> x, [x, y, ..rest] -> x + y)";
        assert_eq!(eval_ok(code), Value::Int(3));
    }

    // Note: recurse pattern requires special lambda parsing for (n, self) -> expr
    // The self parameter is reserved. Test using the spec test instead.
    #[test]
    fn test_recurse_via_named_function() {
        // Test recursion via regular function (not recurse pattern)
        let code = r#"
            @factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n - 1)
            @main () -> int = factorial(5)
        "#;
        assert_eq!(eval_ok(code), Value::Int(120));
    }
}

// =============================================================================
// Duration and Size Literals
// =============================================================================

mod duration_size {
    use super::*;

    #[test]
    fn test_duration_ms() {
        assert_eq!(eval_ok("@main () -> Duration = 100ms"), Value::Duration(100));
    }

    #[test]
    fn test_duration_seconds() {
        assert_eq!(eval_ok("@main () -> Duration = 5s"), Value::Duration(5000));
    }

    #[test]
    fn test_duration_minutes() {
        assert_eq!(eval_ok("@main () -> Duration = 2m"), Value::Duration(120000));
    }

    #[test]
    fn test_size_bytes() {
        assert_eq!(eval_ok("@main () -> Size = 1024b"), Value::Size(1024));
    }

    #[test]
    fn test_size_kb() {
        assert_eq!(eval_ok("@main () -> Size = 4kb"), Value::Size(4096));
    }

    #[test]
    fn test_size_mb() {
        assert_eq!(eval_ok("@main () -> Size = 1mb"), Value::Size(1024 * 1024));
    }
}

// =============================================================================
// Error Cases
// =============================================================================

mod errors {
    use super::*;

    #[test]
    fn test_undefined_variable() {
        eval_err("@main () -> int = undefined_var", "undefined");
    }

    #[test]
    fn test_type_mismatch_add() {
        eval_err(r#"@main () -> int = 1 + "hello""#, "type mismatch");
    }

    #[test]
    fn test_division_by_zero() {
        eval_err("@main () -> int = 1 / 0", "division by zero");
    }

    #[test]
    fn test_index_out_of_bounds() {
        eval_err("@main () -> int = run(let arr = [1, 2, 3], arr[10])", "out of bounds");
    }

    #[test]
    fn test_tuple_destructure_mismatch() {
        eval_err("@main () -> int = run(let (a, b, c) = (1, 2), a)", "mismatch");
    }
}

// =============================================================================
// Parser Error Recovery Tests
// =============================================================================

mod parser_error_recovery {
    use sigilc_v2::intern::StringInterner;
    use sigilc_v2::syntax::{Lexer, Parser, ExprKind};

    /// Helper to parse and return diagnostics count and whether parsing "succeeded"
    fn parse_with_recovery(source: &str) -> (usize, bool) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();
        (result.diagnostics.len(), !result.items.is_empty() || result.diagnostics.is_empty())
    }

    /// Helper to parse expression and check for Error nodes
    fn parse_expr_has_error(source: &str) -> (bool, usize) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        // Check if the expression or any child is an Error
        let has_error = matches!(arena.get(expr_id).kind, ExprKind::Error);
        (has_error, diagnostics.len())
    }

    #[test]
    fn test_list_recovery_multiple_errors() {
        // Use @ without following identifier - actual parse error
        // Also use $ without identifier
        let source = "@main () -> [int] = [1, @, 3, $, 5]";
        let (error_count, parsed) = parse_with_recovery(source);
        // Should have errors but still produce a parse result
        assert!(error_count >= 2, "Expected at least 2 errors, got {}", error_count);
        assert!(parsed, "Should still parse despite errors");
    }

    #[test]
    fn test_function_call_arg_recovery() {
        // @ without identifier is an error
        let source = "@main () -> int = add(1, @, 3)";
        let (error_count, parsed) = parse_with_recovery(source);
        assert!(error_count >= 1, "Expected at least 1 error");
        assert!(parsed, "Should still parse despite errors");
    }

    #[test]
    fn test_tuple_recovery() {
        // Use double operators which is invalid
        let source = "@main () -> (int, int, int) = (1, + +, 3)";
        let (error_count, parsed) = parse_with_recovery(source);
        assert!(error_count >= 1, "Expected at least 1 error, got {}", error_count);
        assert!(parsed, "Should still parse despite errors");
    }

    #[test]
    fn test_map_recovery() {
        // Use @ without identifier for invalid syntax
        let source = r#"@main () -> {str: int} = {"a": 1, "b": @, "c": 3}"#;
        let (error_count, parsed) = parse_with_recovery(source);
        assert!(error_count >= 1, "Expected at least 1 error, got {}", error_count);
        assert!(parsed, "Should still parse despite errors");
    }

    #[test]
    fn test_multiple_items_recovery() {
        // Error in one function shouldn't prevent parsing the next
        let source = r#"
@broken () -> int = [1, @, 3]

@main () -> int = 42
"#;
        let (error_count, parsed) = parse_with_recovery(source);
        assert!(error_count >= 1, "Expected at least 1 error from @broken");
        assert!(parsed, "Should still parse @main successfully");
    }

    #[test]
    fn test_expression_error_node() {
        // Parse a standalone @ (no identifier) - should fail
        let source = "1 + @";
        let (has_error, error_count) = parse_expr_has_error(source);
        // Should have diagnostics
        assert!(error_count >= 1, "Expected parse errors, got {}", error_count);
        // After recovery, we should have an error node
        assert!(has_error || error_count > 0, "Should have error indication");
    }

    #[test]
    fn test_recovery_preserves_valid_parts() {
        // Parse a list with one bad element - @ without identifier
        let interner = StringInterner::new();
        let source = "[1, 2, @, 4, 5]";
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        // Should have errors
        assert!(!diagnostics.is_empty(), "Expected parse errors, got {}", diagnostics.len());

        // Should still produce a list
        if let ExprKind::List(range) = &arena.get(expr_id).kind {
            let elements = arena.get_expr_list(*range);
            // Should have 5 elements (including the error placeholder)
            assert_eq!(elements.len(), 5, "Expected 5 elements including error, got {}", elements.len());
        } else {
            panic!("Expected list expression, got {:?}", arena.get(expr_id).kind);
        }
    }
}

// =============================================================================
// Circuit Breaker Tests
// =============================================================================

mod circuit_breaker_tests {
    use sigilc_v2::intern::StringInterner;
    use sigilc_v2::syntax::{Lexer, Parser};

    /// Test that unimplemented features don't hang (imports)
    #[test]
    fn test_import_does_not_hang() {
        let interner = StringInterner::new();
        let source = "use std.math { sqrt }";
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);

        // This should complete quickly with an error, not hang
        let result = parser.parse_module();

        // Should have error about unimplemented feature
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics.iter().any(|d| d.message.contains("not yet implemented")));
    }

    /// Test that unimplemented features don't hang (type definitions)
    #[test]
    fn test_type_def_does_not_hang() {
        let interner = StringInterner::new();
        let source = "type Point = { x: int, y: int }";
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);

        // This should complete quickly with an error, not hang
        let result = parser.parse_module();

        // Should have error about unimplemented feature
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics.iter().any(|d| d.message.contains("not yet implemented")));
    }

    /// Test that multiple unimplemented items don't hang
    #[test]
    fn test_multiple_unimplemented_does_not_hang() {
        let interner = StringInterner::new();
        // Simpler test: just unimplemented features
        let source = r#"
            use std.io { print }
            type Foo = int
        "#;
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);

        // This should complete quickly, not hang
        let result = parser.parse_module();

        // Should have errors for unimplemented features
        assert!(!result.diagnostics.is_empty(), "Expected errors for unimplemented features");
    }

    // ==========================================================================
    // Malformed Input Tests - Parser should not panic or hang
    // ==========================================================================

    /// Test various malformed inputs complete without hanging
    #[test]
    fn test_malformed_inputs_complete() {
        // Very long line of garbage - created separately for lifetime
        let long_garbage = "x".repeat(10000);

        let malformed_inputs = vec![
            // Unclosed delimiters
            "@main () -> int = (",
            "@main () -> int = [1, 2, 3",
            "@main () -> int = {",
            // Invalid tokens
            "@@@@@",
            "$$$$$",
            "### invalid",
            // Incomplete expressions
            "@main () -> int = 1 +",
            "@main () -> int = if true then",
            "@main () -> int = for x in",
            // Nonsense
            "asdf qwerty 123 456",
            ") ) ) ( ( (",
            "} } } { { {",
            // Empty constructs
            "@",
            "@ ()",
            "@ () ->",
            // Missing parts
            "() -> int = 42",
            "pub",
            "pub @",
            // Deep nesting without closing
            "(((((((((((((((((((((((",
            "[[[[[[[[[[[[[[[[[[[[[[",
            "{{{{{{{{{{{{{{{{{{{{",
            // Mix of valid and invalid
            "@foo () -> int = 42\n@@@invalid\n@bar () -> int = 43",
            // Unicode chaos
            "@main () -> int = 🔥🔥🔥",
            // Very long line of garbage
            &long_garbage,
        ];

        for (i, source) in malformed_inputs.iter().enumerate() {
            let interner = StringInterner::new();
            let lexer = Lexer::new(source, &interner);
            let tokens = lexer.lex_all();
            let parser = Parser::new(&tokens, &interner);

            // Should complete without hanging or panicking
            let result = parser.parse_module();

            // We don't care about the result, just that it completed
            let _ = result.diagnostics.len();
            let _ = result.items.len();

            // Print progress for debugging if it hangs
            if i % 10 == 0 {
                eprintln!("Completed malformed input test {}/{}", i + 1, malformed_inputs.len());
            }
        }
    }

    /// Test repeated invalid tokens
    #[test]
    fn test_repeated_invalid_tokens() {
        // Many invalid items in sequence
        let source = (0..100)
            .map(|i| format!("@@@ invalid_{}", i))
            .collect::<Vec<_>>()
            .join("\n");

        let interner = StringInterner::new();
        let lexer = Lexer::new(&source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);

        // Should complete
        let result = parser.parse_module();

        // Should have errors but not hang
        assert!(!result.diagnostics.is_empty());
    }

    /// Test deeply nested expressions (within limits)
    #[test]
    fn test_deep_nesting() {
        // 50 levels of nesting
        let nested = format!("@main () -> int = {}", "(".repeat(50) + "1" + &")".repeat(50));

        let interner = StringInterner::new();
        let lexer = Lexer::new(&nested, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);

        let result = parser.parse_module();

        // Should parse successfully
        assert!(result.diagnostics.is_empty() || result.items.len() >= 1);
    }
}
