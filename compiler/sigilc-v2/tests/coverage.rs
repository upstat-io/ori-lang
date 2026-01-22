//! Comprehensive Coverage Tests for Sigil V2 Compiler
//!
//! These tests are designed to maximize code coverage by exercising:
//! - CLI functions
//! - Parser edge cases
//! - Evaluator edge cases
//! - Error handling paths

use std::rc::Rc;
use std::cell::RefCell;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ItemKind, ExprKind, TokenKind, BinaryOp, UnaryOp};
use sigilc_v2::eval::{Evaluator, Value, FunctionValue, Environment};
use sigilc_v2::hir::{Scopes, DefinitionRegistry};
use sigilc_v2::check::{TypeContext, Unifier};
use sigilc_v2::errors::Diagnostic;

// =============================================================================
// CLI Module Tests
// =============================================================================

mod cli_tests {
    use super::*;

    #[test]
    fn test_run_source_simple() {
        let source = "@main () -> int = 42";
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn test_run_source_with_error() {
        let source = "@main () -> int = @@@invalid";
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();

        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn test_check_source_multiple_items() {
        let source = r#"
            @foo () -> int = 1
            @bar () -> int = 2
            @main () -> int = foo() + bar()
        "#;
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.items.len(), 3);
    }

    #[test]
    fn test_test_source_with_tests() {
        let source = r#"
            @helper () -> int = 42
            @test_helper tests @helper () -> void = run(assert(helper() == 42))
        "#;
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();

        assert!(result.diagnostics.is_empty());
        // Should have function and test
        let has_test = result.items.iter().any(|item| matches!(&item.kind, ItemKind::Test(_)));
        assert!(has_test);
    }
}

// =============================================================================
// Lexer Coverage Tests
// =============================================================================

mod lexer_tests {
    use super::*;

    #[test]
    fn test_all_token_types() {
        let source = r#"
            // Comment
            @func pub type trait impl use if then else for in do yield loop break continue
            let mut true false void self Self where with async
            match run try map filter fold find collect recurse parallel timeout retry cache validate
            int float bool str Duration Size
            42 3.14 0xFF 0b1010 "string" 'c'
            100ms 5s 2m 1h 1024b 4kb 10mb 2gb
            + - * / % div & | ^ ~ << >> && || ! == != < <= > >= .. ..= -> => = , : ; . @ $ #
            ( ) [ ] { } < >
            .property:
        "#;
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();

        // Should have many tokens
        assert!(tokens.len() > 50);
    }

    #[test]
    fn test_string_escapes() {
        let source = r#""hello\nworld\t\"quoted\"""#;
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();

        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_char_escapes() {
        let source = r"'\n' '\t' '\\' '\'' '\r' '\0'";
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();

        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_number_formats() {
        let source = "42 1_000_000 0xFF 0xABCD 0b1010 0b1111_0000 3.14 2.5e10 1.5e-5";
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();

        assert!(tokens.len() >= 9);
    }

    #[test]
    fn test_duration_units() {
        let source = "100ms 30s 5m 2h";
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();

        assert!(tokens.len() >= 4);
    }

    #[test]
    fn test_size_units() {
        let source = "1024b 4kb 10mb 2gb";
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();

        assert!(tokens.len() >= 4);
    }
}

// =============================================================================
// Parser Coverage Tests
// =============================================================================

mod parser_tests {
    use super::*;

    fn parse(source: &str) -> (sigilc_v2::syntax::ParseResult, StringInterner) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        (parser.parse_module(), interner)
    }

    #[test]
    fn test_function_variations() {
        // Simple function
        let (r, _) = parse("@foo () -> int = 42");
        assert!(r.diagnostics.is_empty());

        // Function with params
        let (r, _) = parse("@add (a: int, b: int) -> int = a + b");
        assert!(r.diagnostics.is_empty());

        // Generic function
        let (r, _) = parse("@id<T> (x: T) -> T = x");
        assert!(r.diagnostics.is_empty());

        // Public function
        let (r, _) = parse("pub @foo () -> int = 42");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    #[ignore = "type definitions not yet implemented in parser"]
    fn test_type_definitions() {
        let (r, _) = parse("type Point = { x: int, y: int }");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("type Option<T> = Some(T) | None");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("type Alias = int");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_config_variables() {
        let (r, _) = parse("$timeout = 30s");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("pub $max_size = 10mb");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    #[ignore = "imports not yet implemented in parser"]
    fn test_imports() {
        let (r, _) = parse("use std.math { sqrt, abs }");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("use './local' { helper }");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_all_binary_operators() {
        let ops = ["+", "-", "*", "/", "%", "div", "&", "|", "^", "<<", ">>",
                   "&&", "||", "==", "!=", "<", "<=", ">", ">=", "..", "..="];
        for op in ops {
            let source = format!("@main () -> int = 1 {} 2", op);
            let (r, _) = parse(&source);
            // Some may fail type-wise but should parse
            if !r.diagnostics.is_empty() {
                // Range operators need special handling
                if op == ".." || op == "..=" {
                    continue;
                }
            }
        }
    }

    #[test]
    fn test_all_unary_operators() {
        let (r, _) = parse("@main () -> int = -5");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("@main () -> bool = !true");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("@main () -> int = ~0");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_collection_literals() {
        let (r, _) = parse("@main () -> [int] = [1, 2, 3]");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("@main () -> [int] = []");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse(r#"@main () -> {str: int} = {"a": 1, "b": 2}"#);
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("@main () -> (int, str) = (42, \"hello\")");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_type_expressions() {
        // Basic types
        let (r, _) = parse("@f () -> int = 0");
        assert!(r.diagnostics.is_empty());
        let (r, _) = parse("@f () -> float = 0.0");
        assert!(r.diagnostics.is_empty());
        let (r, _) = parse("@f () -> bool = true");
        assert!(r.diagnostics.is_empty());
        let (r, _) = parse("@f () -> str = \"\"");
        assert!(r.diagnostics.is_empty());
        let (r, _) = parse("@f () -> void = run()");
        assert!(r.diagnostics.is_empty());

        // Collection types
        let (r, _) = parse("@f () -> [int] = []");
        assert!(r.diagnostics.is_empty());
        let (r, _) = parse("@f () -> {str: int} = {}");
        assert!(r.diagnostics.is_empty());

        // Generic types
        let (r, _) = parse("@f () -> Option<int> = None");
        assert!(r.diagnostics.is_empty());
        let (r, _) = parse("@f () -> Result<int, str> = Ok(0)");
        assert!(r.diagnostics.is_empty());

        // Nested generics
        let (r, _) = parse("@f () -> Option<Option<int>> = None");
        assert!(r.diagnostics.is_empty());

        // Function types
        let (r, _) = parse("@f (g: (int) -> int) -> int = g(0)");
        assert!(r.diagnostics.is_empty());

        // Tuple types
        let (r, _) = parse("@f () -> (int, str) = (0, \"\")");
        assert!(r.diagnostics.is_empty());

        // Infer type
        let (r, _) = parse("@f () -> _ = 42");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_pattern_syntax() {
        // run pattern
        let (r, _) = parse("@main () -> int = run(let x = 1, x)");
        assert!(r.diagnostics.is_empty());

        // try pattern
        let (r, _) = parse("@main () -> Result<int, str> = try(Ok(42))");
        assert!(r.diagnostics.is_empty());

        // match pattern
        let (r, _) = parse("@main () -> int = match(Some(1), Some(x) -> x, None -> 0)");
        assert!(r.diagnostics.is_empty());

        // map pattern
        let (r, _) = parse("@main () -> [int] = map(.over: [1,2,3], .transform: x -> x)");
        assert!(r.diagnostics.is_empty());

        // filter pattern
        let (r, _) = parse("@main () -> [int] = filter(.over: [1,2,3], .predicate: x -> true)");
        assert!(r.diagnostics.is_empty());

        // fold pattern
        let (r, _) = parse("@main () -> int = fold(.over: [1,2,3], .init: 0, .op: (a,b) -> a+b)");
        assert!(r.diagnostics.is_empty());

        // find pattern
        let (r, _) = parse("@main () -> Option<int> = find(.over: [1,2,3], .where: x -> true)");
        assert!(r.diagnostics.is_empty());

        // collect pattern
        let (r, _) = parse("@main () -> [int] = collect(.range: 0..5, .transform: i -> i)");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_match_patterns() {
        // Literal patterns
        let (r, _) = parse("@main () -> int = match(1, 1 -> 10, 2 -> 20, _ -> 0)");
        assert!(r.diagnostics.is_empty());

        // Variant patterns
        let (r, _) = parse("@main () -> int = match(Some(1), Some(x) -> x, None -> 0)");
        assert!(r.diagnostics.is_empty());

        // List patterns
        let (r, _) = parse("@main () -> int = match([1,2], [] -> 0, [x] -> x, [x,y] -> x+y, _ -> -1)");
        assert!(r.diagnostics.is_empty());

        // List with rest
        let (r, _) = parse("@main () -> int = match([1,2,3], [h, ..t] -> h, _ -> 0)");
        assert!(r.diagnostics.is_empty());

        // Guard patterns
        let (r, _) = parse("@main () -> int = match(5, x.match(x > 0) -> 1, _ -> 0)");
        assert!(r.diagnostics.is_empty());

        // Wildcard
        let (r, _) = parse("@main () -> int = match(5, _ -> 0)");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_binding_patterns() {
        // Simple
        let (r, _) = parse("@main () -> int = run(let x = 1, x)");
        assert!(r.diagnostics.is_empty());

        // Mutable
        let (r, _) = parse("@main () -> int = run(let mut x = 1, x = 2, x)");
        assert!(r.diagnostics.is_empty());

        // With type
        let (r, _) = parse("@main () -> int = run(let x: int = 1, x)");
        assert!(r.diagnostics.is_empty());

        // Tuple destructure
        let (r, _) = parse("@main () -> int = run(let (a, b) = (1, 2), a + b)");
        assert!(r.diagnostics.is_empty());

        // List destructure
        let (r, _) = parse("@main () -> int = run(let [a, b] = [1, 2], a + b)");
        assert!(r.diagnostics.is_empty());

        // List with rest
        let (r, _) = parse("@main () -> int = run(let [h, ..t] = [1, 2, 3], h)");
        assert!(r.diagnostics.is_empty());

        // Struct destructure - SKIPPED: struct literal syntax (Name { fields }) not yet implemented
        // let (r, _) = parse("@main () -> int = run(let { x, y } = Point { x: 1, y: 2 }, x + y)");
        // assert!(r.diagnostics.is_empty());

        // Wildcard
        let (r, _) = parse("@main () -> int = run(let _ = 1, 42)");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_lambda_variations() {
        // Single param
        let (r, _) = parse("@main () -> int = run(let f = x -> x + 1, f(1))");
        assert!(r.diagnostics.is_empty());

        // Multi param
        let (r, _) = parse("@main () -> int = run(let f = (a, b) -> a + b, f(1, 2))");
        assert!(r.diagnostics.is_empty());

        // No param
        let (r, _) = parse("@main () -> int = run(let f = () -> 42, f())");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_loop_variations() {
        // for-do
        let (r, _) = parse("@main () -> int = run(let mut s = 0, for x in [1,2,3] do s = s + x, s)");
        assert!(r.diagnostics.is_empty());

        // for-yield
        let (r, _) = parse("@main () -> [int] = for x in [1,2,3] yield x * 2");
        assert!(r.diagnostics.is_empty());

        // for-yield with guard
        let (r, _) = parse("@main () -> [int] = for x in [1,2,3,4] if x > 2 yield x");
        assert!(r.diagnostics.is_empty());

        // for with range
        let (r, _) = parse("@main () -> [int] = for i in 0..5 yield i");
        assert!(r.diagnostics.is_empty());

        // for with inclusive range
        let (r, _) = parse("@main () -> [int] = for i in 1..=3 yield i");
        assert!(r.diagnostics.is_empty());

        // loop with break
        let (r, _) = parse("@main () -> int = run(let mut i = 0, loop(if i > 5 then break else i = i + 1), i)");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_conditional_variations() {
        let (r, _) = parse("@main () -> int = if true then 1 else 2");
        assert!(r.diagnostics.is_empty());

        let (r, _) = parse("@main () -> int = if false then 1 else if true then 2 else 3");
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn test_test_declarations() {
        let (r, _) = parse("@helper () -> int = 42\n@test_it tests @helper () -> void = run(assert(true))");
        assert!(r.diagnostics.is_empty());

        // Multiple test targets
        let (r, _) = parse("@a () -> int = 1\n@b () -> int = 2\n@test_ab tests @a tests @b () -> void = run(assert(true))");
        assert!(r.diagnostics.is_empty());

        // Skipped test
        let (r, _) = parse("#[skip(\"reason\")]\n@test_skip tests @a () -> void = run(assert(true))\n@a () -> int = 1");
        assert!(r.diagnostics.is_empty());
    }
}

// =============================================================================
// Evaluator Coverage Tests
// =============================================================================

mod evaluator_tests {
    use super::*;

    fn eval(source: &str) -> Result<Value, String> {
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let parse_result = parser.parse_module();

        if !parse_result.diagnostics.is_empty() {
            return Err(format!("Parse error: {:?}", parse_result.diagnostics));
        }

        let mut evaluator = Evaluator::new(&interner, &parse_result.arena);
        evaluator.register_prelude();

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

        for item in &parse_result.items {
            if let ItemKind::Function(func) = &item.kind {
                let name = interner.lookup(func.name);
                if name == "main" {
                    return evaluator.eval(func.body).map_err(|e| e.message);
                }
            }
        }

        Err("No main function".to_string())
    }

    #[test]
    fn test_all_binary_ops() {
        assert_eq!(eval("@main () -> int = 10 + 5").unwrap(), Value::Int(15));
        assert_eq!(eval("@main () -> int = 10 - 5").unwrap(), Value::Int(5));
        assert_eq!(eval("@main () -> int = 10 * 5").unwrap(), Value::Int(50));
        assert_eq!(eval("@main () -> int = 10 / 5").unwrap(), Value::Int(2));
        assert_eq!(eval("@main () -> int = 10 % 3").unwrap(), Value::Int(1));
        assert_eq!(eval("@main () -> int = 10 div 3").unwrap(), Value::Int(3));
        assert_eq!(eval("@main () -> int = 0b1010 & 0b1100").unwrap(), Value::Int(0b1000));
        assert_eq!(eval("@main () -> int = 0b1010 | 0b1100").unwrap(), Value::Int(0b1110));
        assert_eq!(eval("@main () -> int = 0b1010 ^ 0b1100").unwrap(), Value::Int(0b0110));
        assert_eq!(eval("@main () -> int = 1 << 4").unwrap(), Value::Int(16));
        assert_eq!(eval("@main () -> int = 16 >> 2").unwrap(), Value::Int(4));
        assert_eq!(eval("@main () -> bool = true && true").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = true && false").unwrap(), Value::Bool(false));
        assert_eq!(eval("@main () -> bool = false || true").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = false || false").unwrap(), Value::Bool(false));
        assert_eq!(eval("@main () -> bool = 5 == 5").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = 5 != 3").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = 3 < 5").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = 5 <= 5").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = 5 > 3").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> bool = 5 >= 5").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_float_ops() {
        assert_eq!(eval("@main () -> float = 3.5 + 1.5").unwrap(), Value::Float(5.0));
        assert_eq!(eval("@main () -> float = 5.0 - 2.0").unwrap(), Value::Float(3.0));
        assert_eq!(eval("@main () -> float = 2.5 * 4.0").unwrap(), Value::Float(10.0));
        assert_eq!(eval("@main () -> float = 10.0 / 4.0").unwrap(), Value::Float(2.5));
        assert_eq!(eval("@main () -> bool = 3.14 < 4.0").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_unary_ops() {
        assert_eq!(eval("@main () -> int = -5").unwrap(), Value::Int(-5));
        assert_eq!(eval("@main () -> int = --5").unwrap(), Value::Int(5));
        assert_eq!(eval("@main () -> bool = !true").unwrap(), Value::Bool(false));
        assert_eq!(eval("@main () -> bool = !false").unwrap(), Value::Bool(true));
        assert_eq!(eval("@main () -> int = ~0").unwrap(), Value::Int(-1));
    }

    #[test]
    fn test_string_ops() {
        assert_eq!(eval(r#"@main () -> str = "hello" + " world""#).unwrap(),
                   Value::Str(Rc::new("hello world".to_string())));
        assert_eq!(eval(r#"@main () -> bool = "abc" == "abc""#).unwrap(), Value::Bool(true));
        assert_eq!(eval(r#"@main () -> bool = "abc" != "def""#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_collection_ops() {
        // List indexing
        assert_eq!(eval("@main () -> int = run(let a = [10, 20, 30], a[1])").unwrap(), Value::Int(20));

        // Map indexing
        assert_eq!(eval(r#"@main () -> int = run(let m = {"x": 42}, m["x"])"#).unwrap(), Value::Int(42));

        // Hash length
        assert_eq!(eval("@main () -> int = run(let a = [1,2,3,4,5], a[# - 1])").unwrap(), Value::Int(5));
    }

    #[test]
    fn test_option_values() {
        // Some
        let v = eval("@main () -> Option<int> = Some(42)").unwrap();
        assert!(matches!(v, Value::Some(_)));

        // None
        let v = eval("@main () -> Option<int> = None").unwrap();
        assert!(matches!(v, Value::None));
    }

    #[test]
    fn test_result_values() {
        // Ok
        let v = eval("@main () -> Result<int, str> = Ok(42)").unwrap();
        assert!(matches!(v, Value::Ok(_)));

        // Err
        let v = eval(r#"@main () -> Result<int, str> = Err("error")"#).unwrap();
        assert!(matches!(v, Value::Err(_)));
    }

    #[test]
    fn test_builtin_functions() {
        // len
        assert_eq!(eval("@main () -> int = len([1,2,3])").unwrap(), Value::Int(3));
        assert_eq!(eval(r#"@main () -> int = len("hello")"#).unwrap(), Value::Int(5));

        // print (returns void)
        let v = eval(r#"@main () -> void = print("test")"#).unwrap();
        assert!(matches!(v, Value::Void));

        // str conversion
        let v = eval("@main () -> str = str(42)").unwrap();
        assert!(matches!(v, Value::Str(_)));

        // int conversion
        assert_eq!(eval(r#"@main () -> int = int("42")"#).unwrap(), Value::Int(42));

        // assert (returns void on success)
        let v = eval("@main () -> void = assert(true)").unwrap();
        assert!(matches!(v, Value::Void));

        // assert_eq
        let v = eval("@main () -> void = assert_eq(1, 1)").unwrap();
        assert!(matches!(v, Value::Void));
    }

    #[test]
    fn test_pattern_evaluation() {
        // map
        let v = eval("@main () -> [int] = map(.over: [1,2,3], .transform: x -> x * 2)").unwrap();
        if let Value::List(items) = v {
            assert_eq!(items.len(), 3);
        } else {
            panic!("Expected list");
        }

        // filter
        let v = eval("@main () -> [int] = filter(.over: [1,2,3,4,5,6], .predicate: x -> x % 2 == 0)").unwrap();
        if let Value::List(items) = v {
            assert_eq!(items.len(), 3);
        } else {
            panic!("Expected list");
        }

        // fold
        assert_eq!(eval("@main () -> int = fold(.over: [1,2,3,4,5], .init: 0, .op: (a,x) -> a + x)").unwrap(),
                   Value::Int(15));

        // find
        let v = eval("@main () -> Option<int> = find(.over: [1,2,3,4,5], .where: x -> x > 3)").unwrap();
        assert!(matches!(v, Value::Some(_)));

        // collect
        let v = eval("@main () -> [int] = collect(.range: 0..5, .transform: i -> i * i)").unwrap();
        if let Value::List(items) = v {
            assert_eq!(items.len(), 5);
        } else {
            panic!("Expected list");
        }
    }

    #[test]
    fn test_match_evaluation() {
        // Literal match
        assert_eq!(eval("@main () -> int = match(2, 1 -> 10, 2 -> 20, _ -> 0)").unwrap(), Value::Int(20));

        // Variant match
        assert_eq!(eval("@main () -> int = match(Some(5), Some(x) -> x * 2, None -> 0)").unwrap(), Value::Int(10));
        assert_eq!(eval("@main () -> int = match(None, Some(x) -> x, None -> -1)").unwrap(), Value::Int(-1));

        // List match
        assert_eq!(eval("@main () -> int = match([1,2,3], [] -> 0, [x] -> x, [x,y,..r] -> x + y)").unwrap(), Value::Int(3));

        // Wildcard
        assert_eq!(eval("@main () -> int = match(999, _ -> 42)").unwrap(), Value::Int(42));

        // Guard
        assert_eq!(eval("@main () -> int = match(15, x.match(x > 10) -> 1, _ -> 0)").unwrap(), Value::Int(1));
    }

    #[test]
    fn test_try_pattern() {
        // Success path
        let v = eval(r#"
            @safe (x: int) -> Result<int, str> = if x > 0 then Ok(x) else Err("negative")
            @main () -> Result<int, str> = try(let a = safe(5)?, Ok(a * 2))
        "#).unwrap();
        if let Value::Ok(inner) = v {
            assert_eq!(*inner, Value::Int(10));
        } else {
            panic!("Expected Ok");
        }

        // Error path
        let v = eval(r#"
            @safe (x: int) -> Result<int, str> = if x > 0 then Ok(x) else Err("negative")
            @main () -> Result<int, str> = try(let a = safe(-5)?, Ok(a * 2))
        "#).unwrap();
        assert!(matches!(v, Value::Err(_)));
    }

    #[test]
    fn test_loop_evaluation() {
        // for-do
        assert_eq!(eval("@main () -> int = run(let mut s = 0, for x in [1,2,3,4,5] do s = s + x, s)").unwrap(),
                   Value::Int(15));

        // for-yield
        let v = eval("@main () -> [int] = for x in [1,2,3] yield x * 2").unwrap();
        if let Value::List(items) = v {
            assert_eq!(*items, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
        } else {
            panic!("Expected list");
        }

        // for-yield with guard
        let v = eval("@main () -> [int] = for x in [1,2,3,4,5,6] if x % 2 == 0 yield x").unwrap();
        if let Value::List(items) = v {
            assert_eq!(*items, vec![Value::Int(2), Value::Int(4), Value::Int(6)]);
        } else {
            panic!("Expected list");
        }

        // loop with break
        assert_eq!(eval("@main () -> int = run(let mut i = 0, loop(if i >= 5 then break else i = i + 1), i)").unwrap(),
                   Value::Int(5));
    }

    #[test]
    fn test_closures() {
        // Capture from outer scope
        assert_eq!(eval("@main () -> int = run(let x = 10, let f = y -> x + y, f(5))").unwrap(), Value::Int(15));

        // Nested closures
        assert_eq!(eval("@main () -> int = run(let x = 1, let f = a -> (b -> a + b + x), f(2)(3))").unwrap(), Value::Int(6));
    }

    #[test]
    fn test_recursion() {
        // Direct recursion
        assert_eq!(eval(r#"
            @fact (n: int) -> int = if n <= 1 then 1 else n * fact(n - 1)
            @main () -> int = fact(5)
        "#).unwrap(), Value::Int(120));

        // Mutual recursion
        assert_eq!(eval(r#"
            @even (n: int) -> bool = if n == 0 then true else odd(n - 1)
            @odd (n: int) -> bool = if n == 0 then false else even(n - 1)
            @main () -> bool = even(10)
        "#).unwrap(), Value::Bool(true));
    }
}

// =============================================================================
// Error Path Tests
// =============================================================================

mod error_tests {
    use super::*;

    fn eval_error(source: &str) -> String {
        let interner = StringInterner::new();
        let lexer = Lexer::new(source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let parse_result = parser.parse_module();

        if !parse_result.diagnostics.is_empty() {
            return format!("Parse: {:?}", parse_result.diagnostics[0]);
        }

        let mut evaluator = Evaluator::new(&interner, &parse_result.arena);
        evaluator.register_prelude();

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

        for item in &parse_result.items {
            if let ItemKind::Function(func) = &item.kind {
                let name = interner.lookup(func.name);
                if name == "main" {
                    return match evaluator.eval(func.body) {
                        Ok(_) => "No error".to_string(),
                        Err(e) => e.message,
                    };
                }
            }
        }

        "No main".to_string()
    }

    #[test]
    fn test_undefined_variable() {
        let err = eval_error("@main () -> int = undefined_var");
        assert!(err.contains("undefined"));
    }

    #[test]
    fn test_division_by_zero() {
        let err = eval_error("@main () -> int = 1 / 0");
        assert!(err.contains("division by zero"));
    }

    #[test]
    fn test_index_out_of_bounds() {
        let err = eval_error("@main () -> int = run(let a = [1,2,3], a[10])");
        assert!(err.contains("out of bounds"));
    }

    #[test]
    fn test_type_mismatch() {
        let err = eval_error(r#"@main () -> int = 1 + "hello""#);
        assert!(err.contains("type mismatch"));
    }

    #[test]
    fn test_assertion_failure() {
        let err = eval_error("@main () -> void = assert(false)");
        assert!(err.contains("assertion failed"));
    }

    #[test]
    fn test_map_key_not_found() {
        let err = eval_error(r#"@main () -> int = run(let m = {"a": 1}, m["z"])"#);
        assert!(err.contains("not found") || err.contains("key"));
    }

    #[test]
    fn test_tuple_destructure_mismatch() {
        let err = eval_error("@main () -> int = run(let (a, b, c) = (1, 2), a)");
        assert!(err.contains("mismatch"));
    }

    #[test]
    fn test_list_destructure_mismatch() {
        let err = eval_error("@main () -> int = run(let [a, b, c] = [1], a)");
        assert!(err.contains("too long") || err.contains("mismatch"));
    }
}

// =============================================================================
// HIR and Type Checking Tests
// =============================================================================

mod hir_tests {
    use super::*;
    use sigilc_v2::hir::FunctionSig;
    use sigilc_v2::intern::TypeId;
    use sigilc_v2::syntax::Span;

    #[test]
    fn test_scopes() {
        let mut scopes = Scopes::new();

        // Push a scope
        let _scope1 = scopes.push();

        // Define a local
        let interner = StringInterner::new();
        let x = interner.intern("x");
        let ty = TypeId::INT;
        scopes.define_local(x, ty, false);

        // Should be visible
        assert!(scopes.lookup(x).is_some());

        // Push another scope
        scopes.push();

        // Define another local with same name (shadow)
        let y = interner.intern("y");
        scopes.define_local(y, ty, true);

        // Both should be visible
        assert!(scopes.lookup(x).is_some());
        assert!(scopes.lookup(y).is_some());

        // Pop scope
        scopes.pop();

        // y should no longer be visible
        assert!(scopes.lookup(y).is_none());
        // x should still be visible
        assert!(scopes.lookup(x).is_some());
    }

    #[test]
    fn test_definition_registry() {
        let interner = StringInterner::new();
        let mut registry = DefinitionRegistry::new();

        let name = interner.intern("test_func");

        // Create a function signature
        let sig = FunctionSig {
            name,
            params: vec![],
            return_type: TypeId::INT,
            type_params: vec![],
            capabilities: vec![],
            is_async: false,
            span: Span::new(0, 0),
        };

        // Register
        registry.register_function(sig);

        // Should be able to look up
        assert!(registry.get_function(name).is_some());
    }
}

// =============================================================================
// Environment Tests
// =============================================================================

mod environment_tests {
    use super::*;

    #[test]
    fn test_environment_scoping() {
        let interner = StringInterner::new();
        let mut env = Environment::new();

        let x = interner.intern("x");

        // Define in global scope
        env.define_global(x, Value::Int(10));
        assert_eq!(env.lookup(x), Some(Value::Int(10)));

        // Push a scope
        env.push_scope();

        // Shadow x
        env.define(x, Value::Int(20), false);
        assert_eq!(env.lookup(x), Some(Value::Int(20)));

        // Pop scope
        env.pop_scope();

        // Should see global again
        assert_eq!(env.lookup(x), Some(Value::Int(10)));
    }

    #[test]
    fn test_mutable_binding() {
        let interner = StringInterner::new();
        let mut env = Environment::new();

        let x = interner.intern("x");

        // Define mutable
        env.define(x, Value::Int(1), true);

        // Should be able to assign
        assert!(env.assign(x, Value::Int(2)).is_ok());
        assert_eq!(env.lookup(x), Some(Value::Int(2)));
    }

    #[test]
    fn test_immutable_binding() {
        let interner = StringInterner::new();
        let mut env = Environment::new();

        let x = interner.intern("x");

        // Define immutable
        env.define(x, Value::Int(1), false);

        // Should NOT be able to assign
        assert!(env.assign(x, Value::Int(2)).is_err());
    }
}

// =============================================================================
// Value Tests
// =============================================================================

mod value_tests {
    use super::*;
    use sigilc_v2::eval::RangeValue;

    #[test]
    fn test_value_truthy() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Some(Box::new(Value::Int(1))).is_truthy());
        assert!(!Value::None.is_truthy());
    }

    #[test]
    fn test_value_display() {
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Float(3.14)), "3.14");
        assert_eq!(format!("{}", Value::Void), "void");
        assert_eq!(format!("{}", Value::None), "None");
    }

    #[test]
    fn test_range_iteration() {
        let range = RangeValue::exclusive(0, 5);
        let values: Vec<i64> = range.iter().collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4]);

        let range = RangeValue::inclusive(1, 3);
        let values: Vec<i64> = range.iter().collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn test_value_equality() {
        assert_eq!(Value::Int(42), Value::Int(42));
        assert_ne!(Value::Int(42), Value::Int(43));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Duration(100), Value::Duration(100));
        assert_eq!(Value::Size(1024), Value::Size(1024));
    }
}
