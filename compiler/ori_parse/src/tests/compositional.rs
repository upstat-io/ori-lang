//! Compositional tests for the parser.
//!
//! These tests verify that all combinations of types, patterns, and expressions
//! work correctly in all valid positions. This catches edge cases that individual
//! tests might miss.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test assertions use unwrap/expect for clarity"
)]
#![allow(
    clippy::uninlined_format_args,
    reason = "format strings use loop variables for constructing test code"
)]

use crate::{parse, ParseOutput};
use ori_ir::StringInterner;

fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    parse(&tokens, &interner)
}

mod type_matrix {
    use super::*;

    /// All type forms to test
    const TYPES: &[&str] = &[
        // Primitives
        "int",
        "float",
        "bool",
        "str",
        "char",
        "byte",
        "void",
        // Generic types
        "Option<int>",
        "Result<int, str>",
        // Nested generics (the >> bug)
        "Option<Option<int>>",
        "Result<Result<int, str>, str>",
        "Option<Result<int, str>>",
        "Result<Option<int>, str>",
        // Triple nested
        "Option<Option<Option<int>>>",
        "Result<Result<Result<int, str>, str>, str>",
        // Collections
        "[int]",
        "[Option<int>]",
        "{str: int}",
        "{str: Option<int>}",
        // Tuples
        "(int, str)",
        "(int, str, bool)",
        "(Option<int>, Result<str, int>)",
        // Function types
        "() -> int",
        "(int) -> str",
        "(int, str) -> bool",
        "() -> Option<int>",
        "(int) -> Result<str, int>",
        // Complex combinations
        "[Result<int, str>]",
        "{str: Result<int, str>}",
        "Option<[int]>",
        "Result<[int], str>",
    ];

    #[test]
    fn test_all_types_in_variable_annotation() {
        for ty in TYPES {
            let source = format!("@test () -> void = run(let x: {} = default(), ())", ty);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Type '{}' failed in variable annotation:\n{:?}",
                ty,
                result.errors
            );
        }
    }

    #[test]
    fn test_all_types_in_function_param() {
        for ty in TYPES {
            // Skip void - not valid as parameter type
            if *ty == "void" {
                continue;
            }
            let source = format!("@test (x: {}) -> void = ()", ty);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Type '{}' failed in function parameter:\n{:?}",
                ty,
                result.errors
            );
        }
    }

    #[test]
    fn test_all_types_in_return_type() {
        for ty in TYPES {
            let source = format!("@test () -> {} = default()", ty);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Type '{}' failed in return type:\n{:?}",
                ty,
                result.errors
            );
        }
    }

    #[test]
    fn test_all_types_in_type_alias() {
        for ty in TYPES {
            // Skip map types - they have different syntax in type alias position
            // {str: int} is parsed as struct, not map, in type alias
            if ty.starts_with('{') {
                continue;
            }
            let source = format!("type Alias = {}", ty);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Type '{}' failed in type alias:\n{:?}",
                ty,
                result.errors
            );
        }
    }

    #[test]
    fn test_nested_generic_edge_cases() {
        // Specific edge cases for the >> tokenization fix
        let cases = &[
            "Result<Result<int, str>, str>",
            "Option<Option<Option<int>>>",
            "Result<Result<Result<int, str>, str>, str>",
            "Option<Result<Option<int>, str>>",
            "[Result<Result<int, str>, str>]",
            "{str: Option<Option<int>>}",
        ];

        for ty in cases {
            let source = format!("@test () -> {} = default()", ty);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Nested generic '{}' failed:\n{:?}",
                ty,
                result.errors
            );
        }
    }

    #[test]
    fn test_shift_operators_still_work() {
        // Ensure >> and >= still work in expressions
        let cases = &[
            ("@test () -> int = 8 >> 2", "right shift"),
            ("@test () -> bool = 5 >= 3", "greater-equal"),
            ("@test () -> int = 1 << 4", "left shift"),
            ("@test () -> bool = 3 <= 5", "less-equal"),
            (
                "@test () -> int = run(let x: Result<Result<int, str>, str> = Ok(Ok(1)), 8 >> 2)",
                "nested generic + shift",
            ),
        ];

        for (source, desc) in cases {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Operator test '{}' failed:\n{:?}",
                desc,
                result.errors
            );
        }
    }
}

mod pattern_matrix {
    use super::*;

    /// All pattern forms to test
    const PATTERNS: &[&str] = &[
        // Simple patterns
        "_",
        "x",
        "42",
        "-1",
        "true",
        "false",
        r#""hello""#,
        // Char literal patterns
        "'a'",
        "'\\n'",
        // Char range patterns
        "'a'..='z'",
        "'a'..'z'",
        // Variant patterns
        "Some(x)",
        "None",
        "Ok(x)",
        "Err(e)",
        // Tuple patterns
        "(a, b)",
        "(a, b, c)",
        // Struct patterns
        "{ x }",
        "{ x, y }",
        // List patterns
        "[]",
        "[x]",
        "[a, b]",
        "[head, ..tail]",
        "[first, ..rest]",
        // Range patterns
        "0..10",
        "0..=10",
        "1..100",
        // Or patterns
        "1 | 2",
        "1 | 2 | 3",
        "Some(1) | Some(2)",
        "None | Some(0)",
        // At patterns
        "x @ Some(v)",
        "all @ [first, ..rest]",
    ];

    #[test]
    fn test_all_patterns_in_match() {
        for pat in PATTERNS {
            let source = format!(r"@test () -> int = match(value, {} -> 1, _ -> 0)", pat);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Pattern '{}' failed in match arm:\n{:?}",
                pat,
                result.errors
            );
        }
    }

    #[test]
    fn test_patterns_with_guards() {
        let patterns_with_guards = &[
            "x.match(x > 0)",
            "Some(n).match(n > 0)",
            "n.match(n >= 0 && n <= 100)",
        ];

        for pat in patterns_with_guards {
            let source = format!(r"@test () -> int = match(value, {} -> 1, _ -> 0)", pat);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Pattern with guard '{}' failed:\n{:?}",
                pat,
                result.errors
            );
        }
    }

    #[test]
    fn test_nested_patterns() {
        let nested = &[
            "Some(Some(x))",
            "Ok(Ok(v))",
            "Some((a, b))",
            "[Some(x), None]",
            "{ inner: Some(v) }",
            "(Some(a), Ok(b))",
        ];

        for pat in nested {
            let source = format!(r"@test () -> int = match(value, {} -> 1, _ -> 0)", pat);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Nested pattern '{}' failed:\n{:?}",
                pat,
                result.errors
            );
        }
    }

    #[test]
    fn test_complex_or_patterns() {
        let or_patterns = &[
            "1 | 2 | 3 | 4 | 5",
            "Some(1) | Some(2) | Some(3)",
            "Ok(true) | Ok(false)",
            "[] | [_]",
        ];

        for pat in or_patterns {
            let source = format!(r"@test () -> int = match(value, {} -> 1, _ -> 0)", pat);
            let result = parse_source(&source);
            assert!(
                !result.has_errors(),
                "Or-pattern '{}' failed:\n{:?}",
                pat,
                result.errors
            );
        }
    }
}

mod expression_context {
    use super::*;

    #[test]
    fn test_struct_literal_contexts() {
        // Struct literal allowed in normal expression
        let result = parse_source("type P = { x: int }\n@test () -> int = P { x: 1 }.x");
        assert!(!result.has_errors(), "Struct literal in expression failed");

        // Struct literal allowed in if body
        let result =
            parse_source("type P = { x: int }\n@test () -> int = if true then P { x: 1 }.x else 0");
        assert!(!result.has_errors(), "Struct literal in if body failed");

        // Struct literal NOT allowed in if condition (ambiguous with block)
        let result = parse_source(
            "type P = { x: int }\n@test () -> int = if P { x: 1 }.x > 0 then 1 else 0",
        );
        assert!(
            result.has_errors(),
            "Struct literal in if condition should fail"
        );
    }

    #[test]
    fn test_lambda_contexts() {
        let lambdas = &[
            "@test () -> int = (x -> x + 1)(5)",
            "@test () -> int = ((x, y) -> x + y)(1, 2)",
            "@test () -> int = run(let f = x -> x * 2, f(5))",
            "@test () -> int = [1, 2, 3].map(transform: x -> x * 2).fold(initial: 0, op: (a, b) -> a + b)",
        ];

        for source in lambdas {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Lambda context failed for:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_for_loop_in_run() {
        let sources = &[
            "@test () -> int = run(for x in [1, 2, 3] do print(msg: str(x)), 0)",
            "@test () -> [int] = run(let result = for x in [1, 2, 3] yield x * 2, result)",
            "@test () -> [int] = run(for x in [1, 2, 3] if x > 1 yield x * 2)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "For loop in run failed for:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_method_chains() {
        let chains = &[
            "@test () -> int = [1, 2, 3].map(transform: x -> x * 2).fold(initial: 0, op: (a, b) -> a + b)",
            "@test () -> Option<int> = Some(5).map(transform: x -> x * 2).filter(predicate: x -> x > 5)",
            "@test () -> Result<int, str> = Ok(5).map(transform: x -> x * 2).map_err(transform: e -> e)",
        ];

        for source in chains {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Method chain failed for:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_binary_operators_with_types() {
        // Test that operators work alongside generic types
        let sources = &[
            "@test () -> Result<Result<int, str>, str> = run(let x = 8 >> 2, Ok(Ok(x)))",
            "@test () -> Option<Option<bool>> = run(let x = 5 >= 3, Some(Some(x)))",
            "@test () -> [Result<int, str>] = run(let x = 1 << 4, [Ok(x)])",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Operators with generic types failed for:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Mixed Declaration Tests
// =============================================================================
// Tests combining generics, where clauses, capabilities, and complex signatures

mod mixed_declarations {
    use super::*;

    #[test]
    fn test_generics_with_where_clause() {
        let sources = &[
            // Single where constraint
            "@process<T> (x: T) -> T where T: Clone = x",
            // Multiple where constraints
            "@process<T, U> (x: T, y: U) -> T where T: Clone, U: Eq = x",
            // Where with multiple bounds per constraint
            "@process<T> (x: T) -> T where T: Clone + Eq + Hashable = x",
            // Complex nested generics with where
            "@transform<T, U> (x: Option<T>) -> Result<U, str> where T: Clone, U: Default = Err(\"not impl\")",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Generics with where clause failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_generics_with_capabilities() {
        let sources = &[
            // Single capability
            "@fetch<T> (url: str) -> T uses Http = panic(msg: \"not impl\")",
            // Multiple capabilities
            "@save<T> (data: T) -> void uses FileSystem, Logger = ()",
            // Generics + capabilities + where clause (uses before where)
            "@process<T> (x: T) -> T uses Logger where T: Clone = x",
            // Multiple type params + multiple capabilities + where
            "@sync<T, U> (a: T, b: U) -> void uses Http, Cache where T: Clone, U: Eq = ()",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Generics with capabilities failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_complex_function_signatures() {
        let sources = &[
            // Function returning function type
            "@curry (x: int) -> (int) -> int = y -> x + y",
            // Function taking function type with generics
            "@apply<T, U> (f: (T) -> U, x: T) -> U = f(x)",
            // Higher-order with multiple function params
            "@compose<A, B, C> (f: (B) -> C, g: (A) -> B) -> (A) -> C = x -> f(g(x))",
            // Function returning tuple of generics
            "@split<T> (x: T) -> (T, T) where T: Clone = (x.clone(), x)",
            // Variadic with generic
            "@collect<T> (items: ...T) -> [T] = items",
            // Variadic with bounds
            "@print_all<T: Printable> (items: ...T) -> void = ()",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex function signature failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_const_generics_combinations() {
        let sources = &[
            // Basic const generic type parameter
            "type FixedArray<T, $N: int> = { items: [T] }",
            // Const generic with type bounds
            "@make<T: Clone, $N: int> (default: T) -> [T] = []",
            // Multiple const generics
            "type Matrix<T, $R: int, $C: int> = { data: [T] }",
            // Const generic with where clause
            "@sized<T, $N: int> (x: T) -> [T] where T: Clone = []",
            // Bool const generic
            "type Conditional<T, $Debug: bool> = { value: T }",
            // Fixed capacity with literal
            "@test () -> [int, max 10] = []",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Const generics combination failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_trait_definitions_complex() {
        let sources = &[
            // Trait with associated type
            "trait Container { type Item\n@get (self) -> Self.Item }",
            // Trait with default associated type
            "trait Container { type Item = int\n@get (self) -> Self.Item }",
            // Trait with generic parameter
            "trait Converter<T> { @convert (self) -> T }",
            // Trait extending another
            "trait OrderedContainer: Container { @sorted (self) -> Self }",
            // Trait with default method
            "trait Printable { @to_str (self) -> str = \"default\" }",
            // Trait with multiple associated types
            "trait BiContainer { type Left\ntype Right\n@get_left (self) -> Self.Left\n@get_right (self) -> Self.Right }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex trait definition failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_impl_blocks_complex() {
        let sources = &[
            // Generic impl
            "impl<T> Box<T> { @new (value: T) -> Self = Box { value } }",
            // Impl with where clause
            "impl<T> Box<T> where T: Clone { @clone_inner (self) -> T = self.value.clone() }",
            // Trait impl for generic type
            "impl<T: Clone> Clone for Box<T> { @clone (self) -> Self = Box { value: self.value.clone() } }",
            // Impl with multiple bounds
            "impl<T: Eq + Hashable> Box<T> { @hash_value (self) -> int = self.value.hash() }",
            // Impl for nested generic
            "impl<T> Option<Option<T>> { @flatten (self) -> Option<T> = None }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex impl block failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_type_definitions_complex() {
        let sources = &[
            // Sum type with generic payload
            "type Tree<T> = Leaf(value: T) | Node(left: Tree<T>, right: Tree<T>)",
            // Sum type with multiple payloads
            "type Event = Click(x: int, y: int) | Key(code: int, shift: bool) | Resize(w: int, h: int)",
            // Struct with nested generics
            "type Cache<K, V> = { data: {K: Option<V>}, max_size: int }",
            // Newtype
            "type UserId = int",
            // Generic newtype
            "type Wrapper<T> = { inner: T }",
            // Type with const generic
            "type Buffer<$N: int> = { size: int, data: [byte] }",
            // Fixed capacity with literal
            "type SmallBuffer = { data: [byte, max 256] }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex type definition failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_function_clauses() {
        let sources = &[
            // Pattern matching clause with literal
            "@factorial (0: int) -> int = 1\n@factorial (n: int) -> int = n * factorial(n: n - 1)",
            // Clause with guard
            "@abs (n: int) -> int if n >= 0 = n\n@abs (n: int) -> int = -n",
            // Multiple pattern clauses
            "@fib (0: int) -> int = 0\n@fib (1: int) -> int = 1\n@fib (n: int) -> int = fib(n: n - 1) + fib(n: n - 2)",
            // Simple named parameters
            "@add (a: int, b: int) -> int = a + b",
            // Default parameter values
            "@greet (name: str = \"world\") -> str = name",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Function clause failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_attributes_on_declarations() {
        let sources = &[
            // Derive attribute
            "#derive(Eq, Clone)\ntype Point = { x: int, y: int }",
            // Skip attribute on test with target
            "#skip(\"not implemented\")\n@test_something tests @target () -> void = ()",
            // Multiple attributes
            "#derive(Eq)\n#derive(Clone)\ntype Data = { value: int }",
            // Compile fail attribute
            "#compile_fail(\"type error\")\n@bad () -> int = \"not an int\"",
            // Fail attribute
            "#fail(\"expected panic\")\n@test_panic tests @target () -> void = panic(msg: \"oops\")",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Attribute on declaration failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    // Note: extern blocks with 'as' alias syntax may not be fully implemented
    // This test is commented out until the feature is verified
    // #[test]
    // fn test_extern_blocks() { ... }

    #[test]
    fn test_def_impl_blocks() {
        let sources = &[
            // Basic def impl
            "def impl Printable { @to_str (self) -> str = \"default\" }",
            // Def impl with multiple methods
            "def impl Debug { @debug (self) -> str = \"<unknown>\"\n@short_debug (self) -> str = \"?\" }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Def impl block failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_test_declarations() {
        let sources = &[
            // Basic test with target
            "@test_add tests @add () -> void = ()",
            // Chained test targets
            "@test_both tests @foo tests @bar () -> void = ()",
            // Test with complex body
            "@test_complex tests @target () -> void = run(let x = 1, let y = 2, assert(cond: x + y == 3))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Test declaration failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Mixed Expression Tests
// =============================================================================
// Tests combining multiple expression types in complex ways

mod mixed_expressions {
    use super::*;

    #[test]
    fn test_nested_control_flow() {
        let sources = &[
            // if inside run
            "@test () -> int = run(let x = 1, if x > 0 then x else -x)",
            // match inside run
            "@test () -> int = run(let opt = Some(5), match(opt, Some(x) -> x, None -> 0))",
            // for inside run
            "@test () -> [int] = run(let items = [1, 2, 3], for x in items yield x * 2)",
            // if inside for
            "@test () -> [int] = for x in [1, 2, 3] yield if x > 1 then x * 2 else x",
            // match inside for
            "@test () -> [int] = for opt in [Some(1), None, Some(2)] yield match(opt, Some(x) -> x, None -> 0)",
            // Triple nesting: for inside match inside run
            "@test () -> [int] = run(let opt = Some([1, 2, 3]), match(opt, Some(items) -> for x in items yield x * 2, None -> []))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Nested control flow failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_try_expressions_complex() {
        let sources = &[
            // Basic try
            "@test () -> Result<int, str> = try(let x = Ok(1)?, Ok(x))",
            // Multiple bindings in try
            "@test () -> Result<int, str> = try(let a = Ok(1)?, let b = Ok(2)?, Ok(a + b))",
            // Try with method call
            "@test () -> Result<int, str> = try(let x = get_value()?, let y = x.transform()?, Ok(y))",
            // Nested try
            "@test () -> Result<int, str> = try(let outer = try(let inner = Ok(1)?, Ok(inner))?, Ok(outer))",
            // Try with match inside
            "@test () -> Result<int, str> = try(let x = Ok(Some(5))?, match(x, Some(v) -> Ok(v), None -> Err(\"none\")))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex try expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_match_expressions_complex() {
        let sources = &[
            // Match with complex guards
            "@test () -> int = match(value, x.match(x > 0 && x < 100) -> x, _ -> 0)",
            // Match with nested patterns and guards
            "@test () -> int = match(pair, (a, b).match(a > b) -> a, (a, b) -> b)",
            // Match on nested Option
            "@test () -> int = match(opt, Some(Some(x)) -> x, Some(None) -> -1, None -> 0)",
            // Match on Result of Option
            "@test () -> int = match(res, Ok(Some(x)) -> x, Ok(None) -> -1, Err(_) -> -2)",
            // Match with or-patterns and guards
            "@test () -> int = match(n, (1 | 2 | 3).match(n > 1) -> n * 2, _ -> 0)",
            // Match with at-pattern
            "@test () -> int = match(list, all @ [first, ..rest].match(len(collection: all) > 2) -> first, _ -> 0)",
            // Match with struct pattern
            "@test () -> int = match(point, { x, y }.match(x > 0 && y > 0) -> x + y, _ -> 0)",
            // Match producing lambda
            "@test () -> (int) -> int = match(flag, true -> (x -> x * 2), false -> (x -> x + 1))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex match expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_method_style_match() {
        let sources = &[
            // Basic method-style match
            "@test () -> int = x.match(0 -> 1, _ -> 2)",
            // With guards
            "@test () -> str = n.match(x.match(x > 0) -> \"pos\", _ -> \"neg\")",
            // Nested method-style match
            "@test () -> str = x.match(0 -> y.match(0 -> \"a\", _ -> \"b\"), _ -> \"c\")",
            // Expression as receiver
            "@test () -> int = (a + b).match(0 -> 1, _ -> 2)",
            // Chained with postfix ops
            "@test () -> int = x.match(0 -> [1], _ -> [2]).len()",
            // Method-style equivalent of match(x, ...)
            "@test () -> int = val.match(Some(n) -> n, None -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Method-style match failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_with_capability_expressions() {
        let sources = &[
            // Basic with
            "@test () -> int = with Http = MockHttp in fetch(url: \"/data\")",
            // Nested with (one at a time)
            "@test () -> int = with Http = MockHttp in with Cache = MockCache in fetch(url: \"/data\")",
            // With in run
            "@test () -> int = run(let mock = MockHttp, with Http = mock in fetch(url: \"/data\"))",
            // With containing match
            "@test () -> int = with Http = MockHttp in match(fetch(url: \"/\"), Ok(x) -> x, Err(_) -> 0)",
            // With containing for
            "@test () -> [int] = with Http = MockHttp in for url in urls yield fetch(url: url)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "With capability expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_lambda_expressions_complex() {
        let sources = &[
            // Lambda with type annotation
            "@test () -> int = ((x: int) -> int = x * 2)(5)",
            // Multi-param typed lambda
            "@test () -> int = ((a: int, b: int) -> int = a + b)(1, 2)",
            // Lambda returning lambda (currying)
            "@test () -> (int) -> int = (x -> (y -> x + y))(10)",
            // Lambda with complex body
            "@test () -> int = (x -> if x > 0 then x * 2 else -x)(5)",
            // Lambda with match body
            "@test () -> int = (opt -> match(opt, Some(x) -> x, None -> 0))(Some(5))",
            // Lambda in method chain
            "@test () -> int = [1, 2, 3].map(transform: x -> x * 2).filter(predicate: x -> x > 2).fold(initial: 0, op: (a, b) -> a + b)",
            // Nested lambdas
            "@test () -> int = (f -> f(5))(x -> x * 2)",
            // Lambda capturing complex expression
            "@test () -> int = run(let multiplier = 10, (x -> x * multiplier)(5))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex lambda expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_loop_expressions() {
        let sources = &[
            // Basic loop with break
            "@test () -> int = loop(run(let x = get_next(), if done() then break x else continue))",
            // Loop with break value
            "@test () -> int = loop(if condition() then break 100 else next())",
            // Loop inside run
            "@test () -> int = run(let count = 0, loop(if count > 10 then break count else continue))",
            // Loop with match inside
            "@test () -> int = loop(match(get_result(), Ok(x) -> break x, Err(_) -> continue))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Loop expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_for_expressions_complex() {
        let sources = &[
            // For with filter
            "@test () -> [int] = for x in [1, 2, 3, 4, 5] if x > 2 yield x * 2",
            // For with complex filter
            "@test () -> [int] = for x in items if x > 0 && x < 100 yield x",
            // For with method call in body
            "@test () -> [str] = for x in items yield x.to_str()",
            // For iterating over range
            "@test () -> [int] = for i in 0..10 yield i * i",
            // For with inclusive range
            "@test () -> [int] = for i in 0..=10 yield i",
            // For with stepped range
            "@test () -> [int] = for i in 0..100 by 10 yield i",
            // Nested for (inner as expression)
            "@test () -> [[int]] = for x in [1, 2] yield for y in [10, 20] yield x + y",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex for expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_if_expressions_complex() {
        let sources = &[
            // Chained else if
            "@test () -> int = if x > 100 then 3 else if x > 10 then 2 else if x > 0 then 1 else 0",
            // If with complex condition
            "@test () -> int = if x > 0 && x < 100 || y == 0 then 1 else 0",
            // If with method call condition
            "@test () -> int = if list.is_empty() then 0 else list[0]",
            // If producing different types (must be same)
            "@test () -> Option<int> = if found then Some(value) else None",
            // Nested if
            "@test () -> int = if a then if b then 1 else 2 else if c then 3 else 4",
            // If with for in branches
            "@test () -> [int] = if use_double then for x in items yield x * 2 else for x in items yield x",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex if expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_pattern_constructs() {
        let sources = &[
            // recurse pattern
            "@test () -> int = recurse(condition: n <= 1, base: 1, step: n * recurse(n: n - 1))",
            // parallel pattern
            "@test () -> [Result<int, str>] = parallel(tasks: [task1, task2, task3], max_concurrent: 2)",
            // spawn pattern
            "@test () -> void = spawn(tasks: [task1, task2])",
            // timeout pattern
            "@test () -> Result<int, str> = timeout(operation: long_task(), after: 5s)",
            // cache pattern
            "@test () -> int = cache(key: \"mykey\", op: expensive(), ttl: 1h)",
            // catch pattern
            "@test () -> Result<int, str> = catch(expr: might_panic())",
            // Nested patterns
            "@test () -> Result<int, str> = timeout(operation: cache(key: \"k\", op: fetch(), ttl: 5m), after: 10s)",
            // nursery pattern
            "@test () -> void = nursery(body: n -> spawn_tasks(n: n), on_error: CancelRemaining)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Pattern construct failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_method_chains_complex() {
        let sources = &[
            // Long chain with different transforms
            "@test () -> int = items.iter().filter(predicate: x -> x > 0).map(transform: x -> x * 2).take(count: 10).fold(initial: 0, op: (a, b) -> a + b)",
            // Chain with Option methods
            "@test () -> int = opt.map(transform: x -> x * 2).and_then(transform: x -> if x > 10 then Some(x) else None).unwrap_or(default: 0)",
            // Chain with Result methods
            "@test () -> int = res.map(transform: x -> x * 2).map_err(transform: e -> e).unwrap_or(default: 0)",
            // Chain on nested generic
            "@test () -> int = opt_opt.and_then(transform: inner -> inner).unwrap_or(default: 0)",
            // Chain with type conversions
            "@test () -> str = value.to_str()",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex method chain failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_binary_operators_complex() {
        let sources = &[
            // All arithmetic operators
            "@test () -> int = 1 + 2 - 3 * 4 / 5 % 6",
            // Floor division
            "@test () -> int = 17 div 5",
            // Bitwise operators
            "@test () -> int = (a & b) | (c ^ d)",
            // Shift operators
            "@test () -> int = (x << 2) >> 1",
            // Comparison chain (requires grouping)
            "@test () -> bool = a < b && b < c",
            // Logical operators
            "@test () -> bool = a && b || c && !d",
            // Mixed precedence
            "@test () -> bool = x + y * z > a - b / c && flag",
            // Coalesce operator
            "@test () -> int = opt ?? default",
            // Chained coalesce
            "@test () -> int = first ?? second ?? third ?? 0",
            // Coalesce with method chain
            "@test () -> int = get_opt().map(transform: x -> x * 2) ?? 0",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex binary operator expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_range_expressions() {
        let sources = &[
            // Basic exclusive range
            "@test () -> Range<int> = 0..10",
            // Inclusive range
            "@test () -> Range<int> = 0..=10",
            // Range with step
            "@test () -> Range<int> = 0..100 by 5",
            // Descending range
            "@test () -> Range<int> = 10..0 by -1",
            // Open-ended range
            "@test () -> Range<int> = 0..",
            // Range in for loop
            "@test () -> [int] = for i in 1..=100 by 2 yield i",
            // Range with expressions
            "@test () -> Range<int> = start..end by step",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Range expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_index_expressions() {
        let sources = &[
            // Basic index
            "@test () -> int = list[0]",
            // Index with expression
            "@test () -> int = list[i + 1]",
            // Index with length reference
            "@test () -> int = list[# - 1]",
            // Chained index
            "@test () -> int = matrix[0][1]",
            // Index on method result
            "@test () -> int = get_list()[0]",
            // Map index (returns Option)
            "@test () -> Option<int> = map[\"key\"]",
            // Index with complex expression
            "@test () -> int = list[if flag then 0 else # - 1]",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Index expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_type_conversions() {
        let sources = &[
            // Infallible as
            "@test () -> float = 42 as float",
            // Fallible as?
            "@test () -> Option<int> = \"42\" as? int",
            // Chain with as
            "@test () -> int = (value as float * 1.5) as int",
            // as with generics
            "@test () -> [float] = for x in ints yield x as float",
            // Conversion function syntax
            "@test () -> float = float(42)",
            "@test () -> int = int(3.14)",
            "@test () -> str = str(42)",
            "@test () -> byte = byte(65)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Type conversion expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_error_propagation() {
        let sources = &[
            // Basic ?
            "@test () -> Result<int, str> = try(let x = fallible()?, Ok(x))",
            // Chained ?
            "@test () -> Result<int, str> = try(let a = first()?, let b = second(a: a)?, Ok(b))",
            // ? on method chain
            "@test () -> Result<int, str> = try(let x = get_opt().ok_or(error: \"none\")?, Ok(x))",
            // ? in expression
            "@test () -> Result<int, str> = try(Ok(compute()? + 1))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Error propagation expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_unsafe_expressions() {
        let sources = &[
            // Basic unsafe
            "@test () -> int = unsafe(ptr_read(ptr: p))",
            // Unsafe in run
            "@test () -> int = run(let ptr = get_ptr(), unsafe(ptr_read(ptr: ptr)))",
            // Unsafe with method call
            "@test () -> void = unsafe(ptr.write(value: 42))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Unsafe expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Mixed Literal Tests
// =============================================================================
// Tests for complex literal combinations

mod mixed_literals {
    use super::*;

    #[test]
    fn test_duration_literals_in_expressions() {
        let sources = &[
            // Basic duration
            "@test () -> Duration = 5s",
            // Duration arithmetic
            "@test () -> Duration = 1h + 30m + 45s",
            // Duration in timeout
            "@test () -> Result<int, str> = timeout(operation: task(), after: 5s)",
            // Duration comparison
            "@test () -> bool = elapsed > 1m",
            // All duration units
            "@test () -> [Duration] = [1ns, 1us, 1ms, 1s, 1m, 1h]",
            // Duration in method call
            "@test () -> void = sleep(duration: 100ms)",
            // Duration in if condition
            "@test () -> int = if elapsed > 5s then 1 else 0",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Duration literal in expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_size_literals_in_expressions() {
        let sources = &[
            // Basic size
            "@test () -> Size = 1kb",
            // Size arithmetic
            "@test () -> Size = 1gb + 512mb",
            // All size units
            "@test () -> [Size] = [1b, 1kb, 1mb, 1gb, 1tb]",
            // Size comparison
            "@test () -> bool = file_size > 10mb",
            // Size in method call
            "@test () -> void = allocate(size: 4kb)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Size literal in expression failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_list_literals_complex() {
        let sources = &[
            // Empty list with type context
            "@test () -> [int] = []",
            // List with expressions
            "@test () -> [int] = [1 + 2, 3 * 4, 5 - 6]",
            // Nested lists
            "@test () -> [[int]] = [[1, 2], [3, 4], [5, 6]]",
            // List with method calls
            "@test () -> [int] = [get_first(), get_second(), get_third()]",
            // List with if expressions
            "@test () -> [int] = [if a then 1 else 0, if b then 2 else 0]",
            // List with lambdas
            "@test () -> [(int) -> int] = [x -> x + 1, x -> x * 2, x -> x - 1]",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex list literal failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_map_literals_complex() {
        let sources = &[
            // Empty map
            "@test () -> {str: int} = {}",
            // Basic map
            "@test () -> {str: int} = {\"a\": 1, \"b\": 2}",
            // Map with identifier keys (shorthand for string)
            "@test () -> {str: int} = {a: 1, b: 2}",
            // Map with computed keys
            "@test () -> {str: int} = {[key_expr]: value}",
            // Map with complex values
            "@test () -> {str: Option<int>} = {a: Some(1), b: None, c: Some(3)}",
            // Nested maps
            "@test () -> {str: {str: int}} = {outer: {inner: 42}}",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex map literal failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_struct_literals_complex() {
        let sources = &[
            // Basic struct
            "type P = { x: int, y: int }\n@test () -> P = P { x: 1, y: 2 }",
            // Struct with field shorthand
            "type P = { x: int, y: int }\n@test () -> P = run(let x = 1, let y = 2, P { x, y })",
            // Nested struct
            "type Inner = { v: int }\ntype Outer = { inner: Inner }\n@test () -> Outer = Outer { inner: Inner { v: 42 } }",
            // Generic struct literal
            "type Box<T> = { value: T }\n@test () -> Box<int> = Box { value: 42 }",
            // Struct with complex field values
            "type Data = { items: [int], count: int }\n@test () -> Data = Data { items: [1, 2, 3], count: 3 }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex struct literal failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_tuple_literals() {
        let sources = &[
            // Basic tuple
            "@test () -> (int, str) = (1, \"hello\")",
            // Triple tuple
            "@test () -> (int, str, bool) = (1, \"hello\", true)",
            // Nested tuple
            "@test () -> ((int, int), str) = ((1, 2), \"pair\")",
            // Tuple with complex elements
            "@test () -> (Option<int>, [str]) = (Some(1), [\"a\", \"b\"])",
            // Unit tuple
            "@test () -> () = ()",
            // Single element (not a tuple, just grouping)
            "@test () -> int = (1 + 2)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Tuple literal failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_char_literals() {
        let sources = &[
            // Basic char
            "@test () -> char = 'a'",
            // Escape sequences
            "@test () -> char = '\\n'",
            "@test () -> char = '\\t'",
            "@test () -> char = '\\r'",
            "@test () -> char = '\\0'",
            "@test () -> char = '\\''",
            "@test () -> char = '\\\\'",
            // Char in list
            "@test () -> [char] = ['a', 'b', 'c']",
            // Char comparison
            "@test () -> bool = c == 'x'",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Char literal failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_numeric_literals() {
        let sources = &[
            // Basic int
            "@test () -> int = 42",
            // Negative int
            "@test () -> int = -42",
            // Int with underscores
            "@test () -> int = 1_000_000",
            // Hex int
            "@test () -> int = 0xFF",
            "@test () -> int = 0xDEAD_BEEF",
            // Basic float
            "@test () -> float = 3.14",
            // Float with exponent
            "@test () -> float = 2.5e10",
            "@test () -> float = 1.0e-8",
            // Negative float
            "@test () -> float = -3.14",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Numeric literal failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Mixed Type Tests
// =============================================================================
// Tests for complex type combinations beyond the basic matrix

mod mixed_types {
    use super::*;

    #[test]
    fn test_function_types_complex() {
        let sources = &[
            // Function returning function
            "@test () -> (int) -> (int) -> int = x -> y -> x + y",
            // Function taking function returning function
            "@test (f: (int) -> (int) -> int) -> int = f(1)(2)",
            // Function with tuple param
            "@test (p: (int, int)) -> int = match(p, (a, b) -> a + b)",
            // Function returning tuple
            "@test () -> (int, str) = (1, \"hello\")",
            // Higher-order with generics
            "@apply<T, U> (f: (T) -> U, x: T) -> U = f(x)",
            // Function type in generic position
            "@test () -> Option<(int) -> int> = Some(x -> x * 2)",
            // Multiple function params
            "@test (f: (int) -> int, g: (int) -> int) -> (int) -> int = x -> f(g(x))",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex function type failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_fixed_capacity_list_types() {
        let sources = &[
            // Basic fixed list with literal size
            "@test () -> [int, max 10] = []",
            // Fixed list in struct
            "type Buffer = { data: [byte, max 1024] }",
            // Nested fixed list
            "@test () -> [[int, max 3], max 3] = []",
            // Fixed list of generic
            "@test () -> [Option<int>, max 5] = []",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Fixed capacity list type failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_trait_object_types() {
        let sources = &[
            // Single trait object param
            "@test (x: Printable) -> void = print(msg: x.to_str())",
            // Trait object in collection
            "@test () -> [Printable] = []",
            // Trait object in Option
            "@test () -> Option<Printable> = None",
            // Trait object return
            "@test () -> Printable = 42",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Trait object type failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_deeply_nested_types() {
        let sources = &[
            // Four-level nesting
            "@test () -> Option<Option<Option<Option<int>>>> = Some(Some(Some(Some(1))))",
            // Mixed nesting
            "@test () -> Result<Option<[Result<int, str>]>, str> = Ok(Some([Ok(1)]))",
            // List of maps of options
            "@test () -> [{str: Option<int>}] = []",
            // Map of lists of results
            "@test () -> {str: [Result<int, str>]} = {}",
            // Tuple containing nested generics
            "@test () -> (Option<Option<int>>, Result<Result<str, int>, str>) = (None, Err(\"error\"))",
            // Function type with nested generics
            "@test () -> (Option<int>) -> Result<Option<str>, int> = opt -> Ok(None)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Deeply nested type failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_channel_types() {
        let sources = &[
            // Basic channel types
            "@test () -> (Producer<int>, Consumer<int>) = channel(buffer: 10)",
            // Cloneable channels
            "@test () -> (CloneableProducer<str>, CloneableConsumer<str>) = channel_all(buffer: 5)",
            // Channel in struct
            "type WorkQueue<T> = { producer: Producer<T>, consumer: Consumer<T> }",
            // Channel with complex element type
            "@test () -> (Producer<Result<int, str>>, Consumer<Result<int, str>>) = channel(buffer: 100)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Channel type failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_channel_generic_syntax() {
        let sources = &[
            // Basic generic channel
            "@test () -> void = channel<int>(buffer: 10)",
            // Channel with string type arg
            "@test () -> void = channel_in<str>(buffer: 5)",
            // Channel out
            "@test () -> void = channel_out<bool>(buffer: 1)",
            // Cloneable channel
            "@test () -> void = channel_all<float>(buffer: 20)",
            // Channel with nested generic type arg
            "@test () -> void = channel<Result<int, str>>(buffer: 100)",
            // Channel without generics still works
            "@test () -> void = channel(buffer: 10)",
            // Channel in let binding
            "@test () -> void = let pair = channel<int>(buffer: 5)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Channel generic syntax failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_never_type() {
        let sources = &[
            // Never in Result (always Err)
            "@test () -> Result<Never, str> = Err(\"always fails\")",
            // Never in Result (always Ok)
            "@test () -> Result<int, Never> = Ok(42)",
            // Function returning Never
            "@fail () -> Never = panic(msg: \"always panics\")",
            // Never from todo
            "@incomplete () -> Never = todo()",
            // Never from unreachable
            "@impossible () -> Never = unreachable()",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Never type failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Mixed Pattern Tests
// =============================================================================
// Tests for complex pattern combinations in various contexts

mod mixed_patterns {
    use super::*;

    #[test]
    fn test_deeply_nested_patterns() {
        let sources = &[
            // Triple nested Option
            "@test () -> int = match(x, Some(Some(Some(v))) -> v, _ -> 0)",
            // Result containing Option containing tuple
            "@test () -> int = match(x, Ok(Some((a, b))) -> a + b, _ -> 0)",
            // Tuple of Options
            "@test () -> int = match(x, (Some(a), Some(b), Some(c)) -> a + b + c, _ -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Deeply nested pattern failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_or_patterns_complex() {
        let sources = &[
            // Or with nested patterns
            "@test () -> int = match(x, Some(1) | Some(2) | Some(3) -> 1, _ -> 0)",
            // Or with tuples
            "@test () -> int = match(x, (1, _) | (_, 1) -> 1, _ -> 0)",
            // Or with ranges
            "@test () -> int = match(x, 0..10 | 90..=100 -> 1, _ -> 0)",
            // Or with lists
            "@test () -> int = match(x, [] | [_] -> 1, _ -> 0)",
            // Nested or patterns
            "@test () -> int = match(x, Some(1 | 2) | Some(3 | 4) -> 1, _ -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex or pattern failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_at_patterns_complex() {
        let sources = &[
            // At with list rest
            "@test () -> int = match(x, all @ [first, ..rest] -> len(collection: all), _ -> 0)",
            // At with struct
            "@test () -> int = match(x, p @ { x, y } -> p.x + p.y, _ -> 0)",
            // At with variant
            "@test () -> int = match(x, opt @ Some(v) -> v, None -> 0)",
            // At with or pattern
            "@test () -> int = match(x, n @ (1 | 2 | 3) -> n * 2, _ -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex at pattern failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_struct_patterns_complex() {
        let sources = &[
            // Struct with nested pattern
            "@test () -> int = match(x, { inner: Some(v) } -> v, _ -> 0)",
            // Struct with multiple fields and patterns
            "@test () -> int = match(x, { a: Some(x), b: Ok(y), c } -> x + y + c, _ -> 0)",
            // Named type struct pattern
            "@test () -> int = match(x, Point { x, y } -> x + y, _ -> 0)",
            // Struct with renamed binding
            "@test () -> int = match(x, { x: px, y: py } -> px + py, _ -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex struct pattern failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_list_patterns_complex() {
        let sources = &[
            // Empty and single
            "@test () -> int = match(x, [] -> 0, [single] -> single, _ -> -1)",
            // Head and rest
            "@test () -> int = match(x, [first, second, ..rest] -> first + second, _ -> 0)",
            // Fixed length
            "@test () -> int = match(x, [a, b, c, d] -> a + b + c + d, _ -> 0)",
            // Nested list patterns
            "@test () -> int = match(x, [[a, b], [c, d]] -> a + b + c + d, _ -> 0)",
            // List of Options
            "@test () -> int = match(x, [Some(a), Some(b)] -> a + b, _ -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex list pattern failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_patterns_with_complex_guards() {
        let sources = &[
            // Guard with method call
            "@test () -> int = match(x, list.match(!list.is_empty()) -> list[0], _ -> 0)",
            // Guard with multiple conditions
            "@test () -> int = match(x, n.match(n > 0 && n < 100 && n % 2 == 0) -> n, _ -> 0)",
            // Guard with function call
            "@test () -> int = match(x, s.match(is_valid(s: s)) -> 1, _ -> 0)",
            // Guard accessing pattern bindings
            "@test () -> int = match(x, (a, b).match(a < b) -> b - a, (a, b) -> a - b)",
            // Guard with nested pattern access
            "@test () -> int = match(x, Some({ value }).match(value > 0) -> value, _ -> 0)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Pattern with complex guard failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_binding_patterns_in_let() {
        let sources = &[
            // Tuple destructuring
            "@test () -> int = run(let (a, b) = pair, a + b)",
            // Nested tuple destructuring
            "@test () -> int = run(let ((a, b), c) = nested, a + b + c)",
            // Struct destructuring
            "@test () -> int = run(let { x, y } = point, x + y)",
            // List destructuring
            "@test () -> int = run(let [first, second, ..rest] = items, first + second)",
            // Immutable binding with $
            "@test () -> int = run(let $x = 42, x)",
            // Renamed struct field
            "@test () -> int = run(let { x: px, y: py } = point, px + py)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Binding pattern in let failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Import and Module Tests
// =============================================================================
// Tests for import syntax combinations

mod mixed_imports {
    use super::*;

    #[test]
    fn test_import_variants() {
        let sources = &[
            // Relative import
            "use \"./utils\" { helper }",
            // Parent relative
            "use \"../common\" { shared }",
            // Deep relative
            "use \"./deeply/nested/module\" { func }",
            // Module path import
            "use std.collections { HashMap, HashSet }",
            // Import with alias
            "use std.io { read as file_read, write as file_write }",
            // Import private
            "use \"./internal\" { ::private_func }",
            // Module alias
            "use std.collections as col",
            // Multiple items with mixed features
            "use \"./mixed\" { public_func, ::private_func, Type as AliasedType }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Import variant failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_reexports() {
        let sources = &[
            // Basic reexport
            "pub use \"./internal\" { helper }",
            // Reexport with alias
            "pub use std.io { read as public_read }",
            // Reexport multiple
            "pub use \"./types\" { TypeA, TypeB, TypeC }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Reexport failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_extension_imports() {
        let sources = &[
            // Basic extension import
            "extension std.iter.extensions { Iterator.count }",
            // Multiple extension methods
            "extension std.text.extensions { Text.trim, Text.split, Text.join }",
            // Public extension import
            "pub extension std.collections.extensions { Vec.sort, Vec.reverse }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Extension import failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}

// =============================================================================
// Edge Cases
// =============================================================================
// Tests for particularly tricky parsing situations

mod edge_cases {
    use super::*;

    #[test]
    fn test_operator_generic_ambiguity() {
        // These test the >> vs > > ambiguity resolution
        let sources = &[
            // Nested generic followed by shift
            "@test () -> int = run(let x: Result<Result<int, str>, str> = Ok(Ok(1)), 8 >> 2)",
            // Triple nested with comparison
            "@test () -> bool = run(let x: Option<Option<Option<int>>> = Some(Some(Some(1))), 5 >= 3)",
            // Nested generic in expression context
            "@test () -> Option<Option<int>> = if true then Some(Some(1)) else None",
            // Nested generics in list type with shift
            "@test () -> int = run(let x: [Result<Result<int, str>, str>] = [], 4 >> 1)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Operator/generic ambiguity test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_lambda_arrow_ambiguity() {
        let sources = &[
            // Lambda vs function type in various positions
            "@test () -> int = (x -> x + 1)(5)",
            // Lambda in list
            "@test () -> [(int) -> int] = [x -> x, x -> x + 1]",
            // Lambda as method argument
            "@test () -> [int] = items.map(transform: x -> x * 2)",
            // Nested lambda
            "@test () -> int = (x -> (y -> x + y))(1)(2)",
            // Lambda with complex body
            "@test () -> int = (x -> if x > 0 then x else -x)(-5)",
            // Lambda returning lambda
            "@test () -> (int) -> int = (x -> (y -> x + y))(10)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Lambda/arrow ambiguity test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_range_ambiguity() {
        let sources = &[
            // Range vs method chain
            "@test () -> [int] = for i in 0..10 yield i",
            // Range in comparison
            "@test () -> bool = run(let r = 0..10, true)",
            // Range with by
            "@test () -> [int] = for i in 0..100 by 10 yield i",
            // Inclusive range followed by method
            "@test () -> [int] = (0..=10).iter().collect()",
            // Range inside parentheses
            "@test () -> Range<int> = (start..end)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Range ambiguity test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_complex_precedence() {
        let sources = &[
            // Arithmetic and comparison mixed
            "@test () -> bool = a + b * c > d - e / f",
            // Logical with bitwise
            "@test () -> bool = (a & b) != 0 && (c | d) != 0",
            // Coalesce with comparison
            "@test () -> bool = (x ?? 0) > (y ?? 0)",
            // Method chain with operators
            "@test () -> int = a.get() + b.get() * c.get()",
            // Unary operators in chain
            "@test () -> int = -a + !b * ~c",
            // Complex nested precedence
            "@test () -> bool = a + b > c && d * e < f || g == h",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex precedence test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_deeply_nested_expressions() {
        let sources = &[
            // Deep nesting of run
            "@test () -> int = run(let a = run(let b = run(let c = 1, c), b), a)",
            // Deep nesting of if
            "@test () -> int = if a then if b then if c then 1 else 2 else 3 else 4",
            // Deep nesting of match
            "@test () -> int = match(x, Some(y) -> match(y, Some(z) -> z, None -> 0), None -> -1)",
            // Deep method chain
            "@test () -> int = a.b().c().d().e().f().g()",
            // Mixed deep nesting
            "@test () -> int = run(let x = if cond then match(opt, Some(v) -> v, None -> 0) else -1, x * 2)",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Deeply nested expression test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_multiline_expressions() {
        let sources = &[
            // Multiline run
            r"@test () -> int = run(
                let a = 1,
                let b = 2,
                let c = 3,
                a + b + c
            )",
            // Multiline match
            r"@test () -> int = match(
                value,
                Some(x) -> x,
                None -> 0
            )",
            // Multiline if
            r"@test () -> int = if condition
                then positive_branch
                else negative_branch",
            // Multiline method chain
            r"@test () -> int = items
                .iter()
                .filter(predicate: x -> x > 0)
                .map(transform: x -> x * 2)
                .fold(initial: 0, op: (a, b) -> a + b)",
            // Multiline type definition
            r"type Complex = {
                real: float,
                imag: float
            }",
            // Multiline trait
            r"trait Container {
                type Item
                @get (self) -> Self.Item
                @set (self, item: Self.Item) -> Self
            }",
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Multiline expression test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }

    #[test]
    fn test_complex_full_programs() {
        // Full mini-programs combining many features
        let sources = &[
            // Generic data structure with methods
            r"
type Stack<T> = { items: [T] }

impl<T> Stack<T> {
    @new () -> Self = Stack { items: [] }
    @push (self, item: T) -> Self = Stack { items: [item] }
    @is_empty (self) -> bool = is_empty(collection: self.items)
}

@use_stack<T> (item: T) -> Stack<T> = Stack.new().push(item: item)

@test_stack tests @use_stack () -> void = run(
    let stack = use_stack(item: 1),
    assert(cond: !stack.is_empty())
)
",
            // Trait with impl and usage
            r"
trait Stringable {
    @to_string (self) -> str
}

@use_trait<T: Stringable> (x: T) -> str = x.to_string()

@test_trait tests @use_trait () -> void = ()
",
            // Complex function with generics, where, and capabilities
            r#"
@fetch_data<T: Clone, K: Hashable> (key: K, fallback: T) -> Result<T, str> uses Http where T: Clone, K: Hashable = Ok(fallback)

@test_fetch tests @fetch_data () -> void =
    with Http = MockHttp in
        run(
            let result = fetch_data(key: "test", fallback: 42),
            assert(cond: is_ok(result: result))
        )
"#,
        ];

        for source in sources {
            let result = parse_source(source);
            assert!(
                !result.has_errors(),
                "Complex full program test failed:\n{}\nErrors: {:?}",
                source,
                result.errors
            );
        }
    }
}
