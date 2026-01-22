//! Integration Tests for Sigil V2 Compiler
//!
//! These tests exercise the full compiler pipeline (lex → parse → evaluate)
//! to ensure code coverage of the actual compiler implementation.

use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::arena::ExprArena;
use sigilc_v2::syntax::lexer::lex;
use sigilc_v2::syntax::parser::Parser;
use sigilc_v2::eval::{Evaluator, Value};

/// Helper to run Sigil code and return the result
fn eval(source: &str) -> Result<Value, String> {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner).map_err(|e| format!("Lex error: {:?}", e))?;
    let arena = ExprArena::new();
    let mut parser = Parser::new(&tokens, &arena, &interner);
    let module = parser.parse_module().map_err(|e| format!("Parse error: {:?}", e))?;
    let mut evaluator = Evaluator::new(&interner);
    evaluator.eval_module(&module).map_err(|e| format!("Eval error: {}", e.message))
}

/// Helper to check if code evaluates successfully
fn eval_ok(source: &str) -> Value {
    eval(source).expect("should evaluate successfully")
}

/// Helper to check if code fails with expected error
fn eval_err(source: &str, expected: &str) {
    match eval(source) {
        Err(e) => assert!(e.contains(expected), "Expected '{}' in error: {}", expected, e),
        Ok(v) => panic!("Expected error containing '{}', got: {:?}", expected, v),
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
    fn test_string_comparison() {
        assert_eq!(eval_ok(r#"@main () -> bool = "a" < "b""#), Value::Bool(true));
        assert_eq!(eval_ok(r#"@main () -> bool = "hello" == "hello""#), Value::Bool(true));
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
        assert_eq!(eval_ok(r#"@main () -> str = "hello""#), Value::String("hello".into()));
        assert_eq!(eval_ok(r#"@main () -> str = """#), Value::String("".into()));
    }

    #[test]
    fn test_string_escapes() {
        assert_eq!(eval_ok(r#"@main () -> str = "line1\nline2""#), Value::String("line1\nline2".into()));
        assert_eq!(eval_ok(r#"@main () -> str = "tab\there""#), Value::String("tab\there".into()));
        assert_eq!(eval_ok(r#"@main () -> str = "quote\"here""#), Value::String("quote\"here".into()));
    }

    #[test]
    fn test_string_concatenation() {
        assert_eq!(eval_ok(r#"@main () -> str = "hello" + " " + "world""#), Value::String("hello world".into()));
    }

    #[test]
    fn test_char_literals() {
        assert_eq!(eval_ok("@main () -> char = 'a'"), Value::Char('a'));
        assert_eq!(eval_ok("@main () -> char = '\\n'"), Value::Char('\n'));
        assert_eq!(eval_ok("@main () -> char = '\\t'"), Value::Char('\t'));
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
        if let Value::List(items) = result {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Int(1));
            assert_eq!(items[1], Value::Int(2));
            assert_eq!(items[2], Value::Int(3));
        } else {
            panic!("Expected list, got {:?}", result);
        }
    }

    #[test]
    fn test_empty_list() {
        let result = eval_ok("@main () -> [int] = []");
        if let Value::List(items) = result {
            assert!(items.is_empty());
        } else {
            panic!("Expected list, got {:?}", result);
        }
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
            assert_eq!(items[1], Value::String("hello".into()));
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
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_for_yield_guard() {
        let code = "@main () -> [int] = for x in [1, 2, 3, 4, 5, 6] if x % 2 == 0 yield x";
        let result = eval_ok(code);
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_for_range() {
        let code = "@main () -> [int] = for i in 0..5 yield i";
        let result = eval_ok(code);
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(0), Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_for_range_inclusive() {
        let code = "@main () -> [int] = for i in 1..=3 yield i * i";
        let result = eval_ok(code);
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(1), Value::Int(4), Value::Int(9)]);
        } else {
            panic!("Expected list");
        }
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
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_filter_pattern() {
        let code = "@main () -> [int] = filter(.over: [1, 2, 3, 4, 5, 6], .predicate: x -> x % 2 == 0)";
        let result = eval_ok(code);
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
        } else {
            panic!("Expected list");
        }
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
        if let Value::List(items) = result {
            assert_eq!(items, vec![Value::Int(0), Value::Int(1), Value::Int(4), Value::Int(9), Value::Int(16)]);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_try_pattern() {
        let code = r#"
            @safe_div (a: int, b: int) -> Result<int, str> =
                if b == 0 then Err("division by zero") else Ok(a / b)
            @main () -> Result<int, str> = try(let x = safe_div(10, 2)?, Ok(x * 2))
        "#;
        let result = eval_ok(code);
        if let Value::Variant { name, value } = result {
            assert_eq!(name, "Ok");
            assert_eq!(*value, Value::Int(10));
        } else {
            panic!("Expected Ok variant");
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
        if let Value::Variant { name, .. } = result {
            assert_eq!(name, "Err");
        } else {
            panic!("Expected Err variant");
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
        assert_eq!(eval_ok(code), Value::String("big".into()));
    }

    #[test]
    fn test_match_list() {
        let code = "@main () -> int = match([1, 2, 3], [] -> 0, [x] -> x, [x, y, ..rest] -> x + y)";
        assert_eq!(eval_ok(code), Value::Int(3));
    }

    #[test]
    fn test_recurse_pattern() {
        let code = "@main () -> int = recurse(.cond: n -> n <= 1, .base: n -> 1, .step: (n, self) -> n * self(n - 1))(5)";
        assert_eq!(eval_ok(code), Value::Int(120));
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
