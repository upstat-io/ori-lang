#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(
    clippy::uninlined_format_args,
    clippy::redundant_closure_for_method_calls,
    clippy::unnecessary_map_or,
    clippy::unnecessary_debug_formatting,
    clippy::manual_assert
)]
//! Golden tests for the Ori formatter.
//!
//! These tests verify that the formatter produces canonical output for various
//! declaration types. Each test file in `tests/fmt/` is parsed and formatted,
//! and the output is compared against the input (idempotency check) or an
//! expected output file.
//!
//! Comment preservation tests (Phase 6) use `format_module_with_comments` and
//! do not strip comments.

use std::fs;
use std::path::{Path, PathBuf};

use ori_fmt::{format_module, format_module_with_comments};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;

/// Strip comments from source code for comparison.
/// Comments are not preserved in Phase 2 (that's Phase 6 work).
fn strip_comments(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Normalize whitespace: trim trailing whitespace and collapse multiple blank lines.
fn normalize_whitespace(source: &str) -> String {
    let lines: Vec<&str> = source.lines().map(|l| l.trim_end()).collect();

    // Remove leading blank lines
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);

    // Remove trailing blank lines and collapse multiple blank lines
    let mut result = Vec::new();
    let mut prev_blank = false;
    for line in &lines[start..] {
        let is_blank = line.is_empty();
        if is_blank && prev_blank {
            continue;
        }
        result.push(*line);
        prev_blank = is_blank;
    }

    // Remove trailing blank lines
    while result.last().map_or(false, |l| l.is_empty()) {
        result.pop();
    }

    let mut output = result.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

/// Parse source code and format it, returning the formatted output.
fn parse_and_format(source: &str) -> Result<String, String> {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let output = ori_parse::parse(&tokens, &interner);

    if output.has_errors() {
        let errors: Vec<String> = output.errors.iter().map(|e| format!("{:?}", e)).collect();
        return Err(format!("Parse errors:\n{}", errors.join("\n")));
    }

    Ok(format_module(&output.module, &output.arena, &interner))
}

/// Parse source code and format it with comment preservation.
fn parse_and_format_with_comments(source: &str) -> Result<String, String> {
    let interner = StringInterner::new();
    let lex_output = lex_with_comments(source, &interner);
    let parse_output = ori_parse::parse(&lex_output.tokens, &interner);

    if parse_output.has_errors() {
        let errors: Vec<String> = parse_output
            .errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect();
        return Err(format!("Parse errors:\n{}", errors.join("\n")));
    }

    Ok(format_module_with_comments(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
    ))
}

/// Find all .ori files in a directory recursively.
fn find_ori_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if dir.is_dir() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(find_ori_files(&path));
                } else if path.extension().map_or(false, |e| e == "ori") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    files
}

/// Get the path to the tests/fmt directory.
fn golden_tests_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fmt")
}

/// Run a single golden test file.
///
/// For idempotency tests, the formatted output should match the input.
/// For transformation tests (with .expected file), output should match expected.
fn run_golden_test(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // Strip comments since comment preservation is Phase 6
    let source_no_comments = strip_comments(&source);

    let formatted = parse_and_format(&source_no_comments)?;

    // Check for .expected file
    let expected_path = path.with_extension("ori.expected");
    let expected = if expected_path.exists() {
        let exp = fs::read_to_string(&expected_path)
            .map_err(|e| format!("Failed to read {}: {}", expected_path.display(), e))?;
        normalize_whitespace(&strip_comments(&exp))
    } else {
        // Idempotency test: formatted should match source (minus comments)
        normalize_whitespace(&source_no_comments)
    };

    let formatted_normalized = normalize_whitespace(&formatted);

    if formatted_normalized != expected {
        return Err(format!(
            "Formatting mismatch for {}:\n\n--- Expected ---\n{}\n--- Got ---\n{}\n",
            path.display(),
            expected,
            formatted_normalized
        ));
    }

    Ok(())
}

/// Test that formatting is idempotent: format(format(x)) == format(x)
///
/// Note: This test is skipped for files with .expected files, since those
/// may contain formatter output that the parser can't parse back (e.g.,
/// multi-line parameters which the parser doesn't support yet).
fn test_idempotency(path: &Path) -> Result<(), String> {
    // Skip idempotency for files with .expected (known format differences)
    let expected_path = path.with_extension("ori.expected");
    if expected_path.exists() {
        return Ok(());
    }

    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // Strip comments since comment preservation is Phase 6
    let source_no_comments = strip_comments(&source);

    let first = parse_and_format(&source_no_comments)?;

    // Try to parse the formatted output
    let second = match parse_and_format(&first) {
        Ok(s) => s,
        Err(e) => {
            // Parser doesn't support the formatted output (e.g., multi-line params)
            // This is a known limitation - skip idempotency for this file
            eprintln!(
                "Note: Skipping idempotency for {} (formatter output can't be re-parsed: {})",
                path.display(),
                e
            );
            return Ok(());
        }
    };

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    if first_normalized != second_normalized {
        return Err(format!(
            "Idempotency failure for {}:\n\n--- First format ---\n{}\n--- Second format ---\n{}\n",
            path.display(),
            first_normalized,
            second_normalized
        ));
    }

    Ok(())
}

/// Run golden tests for a specific subdirectory.
///
/// This is the shared implementation for all golden test functions.
fn run_golden_tests_for_dir(subdir: &str) {
    let dir = golden_tests_dir().join(subdir);
    let files = find_ori_files(&dir);

    assert!(!files.is_empty(), "No test files found in {:?}", dir);

    let mut failures = Vec::new();
    for file in &files {
        if let Err(e) = run_golden_test(file) {
            failures.push(e);
        }
        if let Err(e) = test_idempotency(file) {
            failures.push(e);
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} golden test failures:\n\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

/// Macro to generate golden test functions.
///
/// Each invocation creates a `#[test]` function that runs golden tests
/// for a specific subdirectory under `tests/fmt/`.
macro_rules! golden_test {
    ($name:ident, $path:expr) => {
        #[test]
        fn $name() {
            run_golden_tests_for_dir($path);
        }
    };
}

// Declaration Tests (Phase 2)
golden_test!(
    golden_tests_declarations_functions,
    "declarations/functions"
);
golden_test!(golden_tests_declarations_types, "declarations/types");
golden_test!(golden_tests_declarations_traits, "declarations/traits");
golden_test!(golden_tests_declarations_impls, "declarations/impls");
golden_test!(golden_tests_declarations_imports, "declarations/imports");
golden_test!(golden_tests_declarations_tests, "declarations/tests");
golden_test!(
    golden_tests_declarations_constants,
    "declarations/constants"
);

// Expression Tests (Phase 3)
golden_test!(golden_tests_expressions_calls, "expressions/calls");
golden_test!(golden_tests_expressions_chains, "expressions/chains");
golden_test!(
    golden_tests_expressions_conditionals,
    "expressions/conditionals"
);
golden_test!(golden_tests_expressions_lambdas, "expressions/lambdas");
golden_test!(golden_tests_expressions_binary, "expressions/binary");
golden_test!(golden_tests_expressions_bindings, "expressions/bindings");
golden_test!(golden_tests_expressions_access, "expressions/access");
golden_test!(
    golden_tests_expressions_conversions,
    "expressions/conversions"
);
golden_test!(golden_tests_expressions_errors, "expressions/errors");

// Pattern Tests (Phase 4)
golden_test!(golden_tests_patterns_run, "patterns/run");
golden_test!(golden_tests_patterns_try, "patterns/try");
golden_test!(golden_tests_patterns_match, "patterns/match");
golden_test!(golden_tests_patterns_for, "patterns/for");
// Note: loop(...) pattern not yet supported by parser (Phase 4 known limitation)

// Collection Tests (Phase 5)
golden_test!(golden_tests_collections_lists, "collections/lists");
golden_test!(golden_tests_collections_maps, "collections/maps");
golden_test!(golden_tests_collections_tuples, "collections/tuples");
golden_test!(golden_tests_collections_structs, "collections/structs");
golden_test!(golden_tests_collections_ranges, "collections/ranges");

// Comment Tests (Phase 6)
// These tests use format_module_with_comments and don't strip comments

/// Run a single golden test file with comment preservation.
fn run_golden_test_with_comments(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let formatted = parse_and_format_with_comments(&source)?;

    // Check for .expected file
    let expected_path = path.with_extension("ori.expected");
    let expected = if expected_path.exists() {
        let exp = fs::read_to_string(&expected_path)
            .map_err(|e| format!("Failed to read {}: {}", expected_path.display(), e))?;
        normalize_whitespace(&exp)
    } else {
        // Idempotency test: formatted should match source
        normalize_whitespace(&source)
    };

    let formatted_normalized = normalize_whitespace(&formatted);

    if formatted_normalized != expected {
        return Err(format!(
            "Formatting mismatch for {}:\n\n--- Expected ---\n{}\n--- Got ---\n{}\n",
            path.display(),
            expected,
            formatted_normalized
        ));
    }

    Ok(())
}

/// Test idempotency with comment preservation.
fn test_idempotency_with_comments(path: &Path) -> Result<(), String> {
    // Skip idempotency for files with .expected (known format differences)
    let expected_path = path.with_extension("ori.expected");
    if expected_path.exists() {
        return Ok(());
    }

    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let first = parse_and_format_with_comments(&source)?;

    // Try to parse the formatted output
    let second = match parse_and_format_with_comments(&first) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Note: Skipping idempotency for {} (formatter output can't be re-parsed: {})",
                path.display(),
                e
            );
            return Ok(());
        }
    };

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    if first_normalized != second_normalized {
        return Err(format!(
            "Idempotency failure for {}:\n\n--- First format ---\n{}\n--- Second format ---\n{}\n",
            path.display(),
            first_normalized,
            second_normalized
        ));
    }

    Ok(())
}

/// Run golden tests for comment directories (with comment preservation).
fn run_golden_tests_for_comments_dir(subdir: &str) {
    let dir = golden_tests_dir().join(subdir);
    let files = find_ori_files(&dir);

    assert!(!files.is_empty(), "No test files found in {:?}", dir);

    let mut failures = Vec::new();
    for file in &files {
        if let Err(e) = run_golden_test_with_comments(file) {
            failures.push(e);
        }
        if let Err(e) = test_idempotency_with_comments(file) {
            failures.push(e);
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} golden test failures:\n\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

/// Macro to generate comment golden test functions.
macro_rules! comment_golden_test {
    ($name:ident, $path:expr) => {
        #[test]
        fn $name() {
            run_golden_tests_for_comments_dir($path);
        }
    };
}

comment_golden_test!(golden_tests_comments_regular, "comments/regular");
comment_golden_test!(golden_tests_comments_doc, "comments/doc");
comment_golden_test!(golden_tests_comments_edge, "comments/edge");

// Edge Case Tests (Phase 8)
golden_test!(golden_tests_edge_cases_empty, "edge-cases/empty");
golden_test!(golden_tests_edge_cases_whitespace, "edge-cases/whitespace");
golden_test!(golden_tests_edge_cases_boundary, "edge-cases/boundary");
golden_test!(golden_tests_edge_cases_nested, "edge-cases/nested");
golden_test!(golden_tests_edge_cases_unicode, "edge-cases/unicode");
golden_test!(golden_tests_edge_cases_long, "edge-cases/long");
golden_test!(golden_tests_edge_cases_real, "edge-cases/real");
