//! Compositional tests for the parser.
//!
//! These tests verify that all combinations of types, patterns, and expressions
//! work correctly in all valid positions. This catches edge cases that individual
//! tests might miss.

#![allow(clippy::unwrap_used, clippy::expect_used)]
// Tests use format strings with loop variables for constructing test code
#![allow(clippy::uninlined_format_args)]

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
