//! Integration tests for incremental formatting.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Tests use unwrap/expect for brevity"
)]
#![allow(
    clippy::uninlined_format_args,
    reason = "Tests use explicit format args for clarity"
)]

use ori_fmt::{apply_regions, format_incremental, IncrementalResult};
use ori_ir::StringInterner;

/// Helper to parse and test incremental formatting.
fn parse_and_test_incremental(
    source: &str,
    change_start: usize,
    change_end: usize,
) -> IncrementalResult {
    let interner = StringInterner::new();
    let lex_output = ori_lexer::lex_with_comments(source, &interner);
    let parse_output = ori_parse::parse(&lex_output.tokens, &interner);

    assert!(
        !parse_output.has_errors(),
        "Parse error in test: {:?}",
        parse_output.errors
    );

    format_incremental(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
        change_start,
        change_end,
    )
}

#[test]
fn test_incremental_empty_module() {
    let result = parse_and_test_incremental("", 0, 0);
    assert!(matches!(result, IncrementalResult::NoChangeNeeded));
}

#[test]
fn test_incremental_change_in_function() {
    let source = "@foo () -> int = 42\n\n@bar () -> int = 123\n";

    // Change overlaps with first function (bytes 0-19)
    let result = parse_and_test_incremental(source, 0, 10);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 1);
            // Should format just the first function
            assert!(regions[0].formatted.contains("@foo"));
            assert!(!regions[0].formatted.contains("@bar"));
        }
        _ => panic!("Expected Regions, got {result:?}"),
    }
}

#[test]
fn test_incremental_change_in_second_function() {
    let source = "@foo () -> int = 42\n\n@bar () -> int = 123\n";

    // Change overlaps with second function (starts at byte 21)
    let result = parse_and_test_incremental(source, 22, 35);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 1);
            // Should format just the second function
            assert!(regions[0].formatted.contains("@bar"));
            assert!(!regions[0].formatted.contains("@foo"));
        }
        _ => panic!("Expected Regions, got {result:?}"),
    }
}

#[test]
fn test_incremental_change_between_declarations() {
    let source = "@foo () -> int = 42\n\n@bar () -> int = 123\n";

    // Change is in the blank line between functions (byte 20)
    let result = parse_and_test_incremental(source, 20, 21);

    // Should indicate no change needed (whitespace between declarations)
    assert!(matches!(result, IncrementalResult::NoChangeNeeded));
}

#[test]
fn test_incremental_change_in_import() {
    let source = "use std.math { sqrt }\n\n@foo () -> int = 42\n";

    // Change overlaps with import
    let result = parse_and_test_incremental(source, 0, 10);

    // Imports require full format
    assert!(matches!(result, IncrementalResult::FullFormatNeeded));
}

#[test]
fn test_incremental_change_in_config() {
    let source = "let $x = 42\n\n@foo () -> int = $x\n";

    // Change overlaps with config
    let result = parse_and_test_incremental(source, 0, 5);

    // Configs require full format
    assert!(matches!(result, IncrementalResult::FullFormatNeeded));
}

#[test]
fn test_incremental_change_in_type() {
    let source = "type Point = { x: int, y: int }\n\n@foo () -> Point = Point { x: 0, y: 0 }\n";

    // Change overlaps with type definition
    let result = parse_and_test_incremental(source, 0, 15);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 1);
            assert!(regions[0].formatted.contains("type Point"));
        }
        _ => panic!("Expected Regions, got {result:?}"),
    }
}

#[test]
fn test_incremental_change_in_trait() {
    let source = "trait Foo {\n    @bar (self) -> int\n}\n\n@baz () -> int = 42\n";

    // Change overlaps with trait definition
    let result = parse_and_test_incremental(source, 0, 20);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 1);
            assert!(regions[0].formatted.contains("trait Foo"));
        }
        _ => panic!("Expected Regions, got {result:?}"),
    }
}

#[test]
fn test_incremental_change_in_impl() {
    let source = "type Foo = { x: int }\n\nimpl Foo {\n    @get (self) -> int = self.x\n}\n";

    // Change overlaps with impl block (starts around byte 23)
    let result = parse_and_test_incremental(source, 25, 40);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 1);
            assert!(regions[0].formatted.contains("impl Foo"));
        }
        _ => panic!("Expected Regions, got {result:?}"),
    }
}

#[test]
fn test_incremental_result_matches_full_format() {
    let source = "@foo()->int=42\n\n@bar()->int=123\n";

    // Get incremental result for first function
    let result = parse_and_test_incremental(source, 0, 14);

    if let IncrementalResult::Regions(regions) = result {
        // Apply the regions
        let incremental_output = apply_regions(source, regions);

        // The formatted functions should match canonical format
        assert!(incremental_output.contains("@foo () -> int = 42"));
    }
}

#[test]
fn test_incremental_preserves_unrelated_code() {
    let source = "@foo () -> int = 42\n\n@bar () -> int = 123\n";

    // Get incremental result for first function
    let result = parse_and_test_incremental(source, 0, 19);

    if let IncrementalResult::Regions(regions) = result {
        let output = apply_regions(source, regions);

        // Second function should be unchanged in the output
        assert!(output.contains("@bar () -> int = 123"));
    }
}

#[test]
fn test_incremental_with_comments() {
    let source = "// This is foo\n@foo () -> int = 42\n\n// This is bar\n@bar () -> int = 123\n";

    // Change overlaps with second function and its comment
    let result = parse_and_test_incremental(source, 36, 60);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 1);
            // Should include the comment
            assert!(regions[0].formatted.contains("// This is bar"));
            assert!(regions[0].formatted.contains("@bar"));
        }
        _ => panic!("Expected Regions, got {result:?}"),
    }
}

#[test]
fn test_incremental_multiple_overlapping() {
    let source = "@a () -> int = 1\n@b () -> int = 2\n@c () -> int = 3\n";

    // Change spans all three functions
    let result = parse_and_test_incremental(source, 0, 50);

    match result {
        IncrementalResult::Regions(regions) => {
            assert_eq!(regions.len(), 3);
        }
        _ => panic!("Expected Regions with 3 entries, got {result:?}"),
    }
}
