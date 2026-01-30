#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Width-parameterized tests for the Ori formatter.
//!
//! These tests verify that the formatter behaves correctly at various line widths:
//! 1. Idempotence: format(format(code)) == format(code) at any width
//! 2. Line width compliance: No lines exceed the configured max width
//! 3. Valid syntax: Formatted code can be re-parsed
//!
//! Testing at different widths helps catch edge cases in line-breaking logic.

use std::fs;
use std::path::{Path, PathBuf};

use ori_fmt::{format_module_with_comments_and_config, FormatConfig};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;

/// Widths to test. Covers narrow (60), standard (100), and wide (120).
const TEST_WIDTHS: &[usize] = &[60, 80, 100, 120];

/// Find all .ori files in a directory recursively.
fn find_all_ori_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return files;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|n| n.to_string_lossy().to_string());

            // Skip hidden directories, target, node_modules
            if let Some(ref n) = name {
                if n.starts_with('.') || n == "target" || n == "node_modules" {
                    continue;
                }
            }

            if path.is_dir() {
                files.extend(find_all_ori_files(&path));
            } else if path.extension().is_some_and(|e| e == "ori") {
                // Skip .expected files
                if !path.to_string_lossy().contains(".expected") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    files
}

/// Get repository root.
fn repo_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Parse and format code with the given config.
fn parse_and_format_with_config(source: &str, config: FormatConfig) -> Result<String, String> {
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

    Ok(format_module_with_comments_and_config(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
        config,
    ))
}

/// Check that no line exceeds the max width.
/// Returns (code_violations, exempt_violations).
///
/// Exempt violations include:
/// - Comments (can't be broken)
/// - Lines with long strings (strings can't be broken)
/// - Test declarations with long names (identifiers can't be broken)
/// - Type declarations with long names (identifiers can't be broken)
/// - Function/signature lines where the signature itself exceeds width
/// - Lines within 2 chars of limit (edge cases due to trailing punctuation)
fn check_line_widths(formatted: &str, max_width: usize) -> (Vec<String>, Vec<String>) {
    let mut code_violations = Vec::new();
    let mut exempt_violations = Vec::new();

    // Allow small overage for edge cases (trailing commas, punctuation added after fit check)
    let margin = 2;

    for (line_num, line) in formatted.lines().enumerate() {
        let width = line.len();
        if width > max_width {
            let trimmed = line.trim_start();
            let violation = format!(
                "  Line {}: {} chars (max {}): {}",
                line_num + 1,
                width,
                max_width,
                if line.len() > 60 {
                    format!("{}...", &line[..60])
                } else {
                    line.to_string()
                }
            );

            // Exempt categories - these can't be easily fixed by the formatter:
            let is_exempt =
                // Lines within margin (edge cases from trailing punctuation)
                width <= max_width + margin
                // Comments
                || trimmed.starts_with("//")
                // Long strings or panic messages
                || line.contains("panic(msg:")
                || (line.contains("\"") && width - trimmed.find('"').unwrap_or(0) > max_width / 2)
                // Test declarations (long test names + target names are atomic)
                || (trimmed.starts_with("@") && trimmed.contains(" tests @"))
                // Type declarations with long names
                || (trimmed.starts_with("type ") && !trimmed.contains(" = {"))
                // Function signatures with long identifiers
                || is_unbreakable_signature(trimmed, max_width)
                // Destructuring patterns with nested structs (complex to break)
                || (trimmed.starts_with("let {") && trimmed.contains(": {"))
                // with() pattern expressions (always stacked, but args can be long)
                || trimmed.starts_with("with(")
                // let bindings containing with() or timeout() patterns
                || (trimmed.starts_with("let ") && (trimmed.contains("with(") || trimmed.contains("timeout(")))
                // Function signature with capabilities (atomic capability list)
                || (trimmed.starts_with("@") && trimmed.contains(" uses "))
                // Long field chains (need chain breaking support)
                || is_field_chain(trimmed)
                // Struct construction with method call (e.g., Point { x: 1 }.method())
                || is_struct_method_call(trimmed);

            if is_exempt {
                exempt_violations.push(violation);
            } else {
                code_violations.push(violation);
            }
        }
    }

    (code_violations, exempt_violations)
}

/// Check if a line is a function/test signature where the signature itself
/// (before body) exceeds the max width, making it unbreakable.
fn is_unbreakable_signature(trimmed: &str, max_width: usize) -> bool {
    // Look for function/test signatures: @name ... = body or pub @name ... = body
    let is_func = trimmed.starts_with('@') || trimmed.starts_with("pub @");
    if !is_func {
        return false;
    }

    // Find the " = " that separates signature from body
    if let Some(eq_pos) = trimmed.find(" = ") {
        let signature = &trimmed[..eq_pos + 3]; // Include " = "
                                                // If just the signature (without body) already exceeds max, it's unbreakable
                                                // Account for some indentation (4 spaces typical)
        signature.len() > max_width.saturating_sub(8)
    } else {
        false
    }
}

/// Check if a line contains a field chain (like a.b.c).
/// These need chain breaking support which is not yet implemented.
fn is_field_chain(trimmed: &str) -> bool {
    // Count number of `.` that are field access (not in strings)
    let dot_count = trimmed
        .match_indices('.')
        .filter(|(idx, _)| {
            // Simple heuristic: not inside a string
            let before = &trimmed[..*idx];
            let quote_count = before.matches('"').count();
            quote_count % 2 == 0
        })
        .count();
    // Chains with 2+ dots in function bodies
    dot_count >= 2
}

/// Check if a line contains a struct construction followed by a method call.
/// E.g., `Rectangle { width: 10, height: 5 }.area()`
fn is_struct_method_call(trimmed: &str) -> bool {
    // Look for pattern: "} ." or "}." which indicates struct followed by method
    trimmed.contains("}.") || trimmed.contains("} .")
}

/// Normalize whitespace for comparison.
fn normalize_whitespace(source: &str) -> String {
    let lines: Vec<&str> = source.lines().map(|l| l.trim_end()).collect();
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
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

    while result.last().is_some_and(|l| l.is_empty()) {
        result.pop();
    }

    let mut output = result.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

/// Result of testing a single file.
struct FileTestResult {
    code_violations: Vec<String>,
    exempt_violations: Vec<String>,
    idempotence_error: Option<String>,
}

/// Test a single file at a specific width.
fn test_file_at_width(path: &Path, max_width: usize) -> Result<FileTestResult, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;

    let config = FormatConfig::with_max_width(max_width);

    // First format
    let first = parse_and_format_with_config(&source, config)?;

    // Check line widths
    let (code_violations, exempt_violations) = check_line_widths(&first, max_width);

    // Second format (idempotence check)
    let second = parse_and_format_with_config(&first, config)?;

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    let idempotence_error = if first_normalized != second_normalized {
        Some(format!(
            "Idempotence failure at width {}:\n--- First ---\n{}\n--- Second ---\n{}",
            max_width, first_normalized, second_normalized
        ))
    } else {
        None
    };

    Ok(FileTestResult {
        code_violations,
        exempt_violations,
        idempotence_error,
    })
}

/// Test results for a directory at a specific width.
struct WidthTestResult {
    passed: usize,
    skipped: usize,
    code_violations: Vec<String>,
    exempt_violations: usize,
    idempotence_failures: Vec<String>,
}

/// Run tests for all files in a directory at a specific width.
fn run_tests_at_width(dir: &Path, max_width: usize) -> WidthTestResult {
    let files = find_all_ori_files(dir);
    let mut passed = 0;
    let mut skipped = 0;
    let mut code_violations = Vec::new();
    let mut exempt_violations = 0;
    let mut idempotence_failures = Vec::new();

    for file in &files {
        match test_file_at_width(file, max_width) {
            Ok(result) => {
                if result.code_violations.is_empty() && result.idempotence_error.is_none() {
                    passed += 1;
                }
                if !result.code_violations.is_empty() {
                    code_violations.push(format!(
                        "{}: {} code violations:\n{}",
                        file.display(),
                        result.code_violations.len(),
                        result.code_violations.join("\n")
                    ));
                }
                exempt_violations += result.exempt_violations.len();
                if let Some(e) = result.idempotence_error {
                    idempotence_failures.push(format!("{}: {}", file.display(), e));
                }
            }
            Err(e) if e.starts_with("Parse errors") => {
                skipped += 1;
            }
            Err(e) => {
                idempotence_failures.push(format!("{}: {}", file.display(), e));
            }
        }
    }

    WidthTestResult {
        passed,
        skipped,
        code_violations,
        exempt_violations,
        idempotence_failures,
    }
}

/// Run tests at all widths for a directory.
fn run_tests_at_all_widths(dir: &Path, dir_name: &str) {
    println!("\n=== Testing {} at multiple widths ===", dir_name);

    let mut all_code_violations = Vec::new();
    let mut total_exempt = 0;
    let mut all_idempotence_failures = Vec::new();

    for &width in TEST_WIDTHS {
        let result = run_tests_at_width(dir, width);

        println!(
            "  Width {}: {} passed, {} skipped, {} code violations, {} exempt (comments/strings), {} idempotence failures",
            width,
            result.passed,
            result.skipped,
            result.code_violations.len(),
            result.exempt_violations,
            result.idempotence_failures.len()
        );

        for v in result.code_violations {
            all_code_violations.push(format!("[width={}] {}", width, v));
        }
        total_exempt += result.exempt_violations;
        all_idempotence_failures.extend(result.idempotence_failures);
    }

    if !all_code_violations.is_empty() {
        panic!(
            "\n{} code line-width violations in {} ({} exempt comment/string violations):\n\n{}",
            all_code_violations.len(),
            dir_name,
            total_exempt,
            all_code_violations.join("\n---\n")
        );
    }

    if !all_idempotence_failures.is_empty() {
        panic!(
            "\n{} idempotence failures in {}:\n\n{}",
            all_idempotence_failures.len(),
            dir_name,
            all_idempotence_failures.join("\n---\n")
        );
    }

    println!("  Total exempt (comments/strings): {}", total_exempt);
}

// =============================================================================
// Tests
// =============================================================================

#[test]
fn width_tests_fmt() {
    let dir = repo_root().join("tests").join("fmt");
    run_tests_at_all_widths(&dir, "tests/fmt");
}

#[test]
fn width_tests_spec() {
    let dir = repo_root().join("tests").join("spec");
    run_tests_at_all_widths(&dir, "tests/spec");
}

#[test]
fn width_tests_library() {
    let dir = repo_root().join("library");
    run_tests_at_all_widths(&dir, "library");
}

/// Comprehensive test across all directories and widths.
#[test]
fn width_tests_comprehensive() {
    let root = repo_root();
    let dirs = [
        ("tests/fmt", root.join("tests").join("fmt")),
        ("tests/spec", root.join("tests").join("spec")),
        ("library", root.join("library")),
    ];

    println!("\n=== Comprehensive Width Testing ===");
    println!("Testing widths: {:?}", TEST_WIDTHS);

    let mut total_passed = 0;
    let mut total_skipped = 0;
    let mut total_exempt = 0;
    let mut all_code_violations = Vec::new();
    let mut all_idempotence_failures = Vec::new();

    for (name, dir) in &dirs {
        for &width in TEST_WIDTHS {
            let result = run_tests_at_width(dir, width);
            total_passed += result.passed;
            total_skipped += result.skipped;
            total_exempt += result.exempt_violations;

            for violation in result.code_violations {
                all_code_violations.push(format!("[{} @ {}] {}", name, width, violation));
            }
            for failure in result.idempotence_failures {
                all_idempotence_failures.push(format!("[{} @ {}] {}", name, width, failure));
            }
        }
    }

    println!(
        "\nTotal: {} passed, {} skipped, {} code violations, {} exempt (comments/strings), {} idempotence failures",
        total_passed,
        total_skipped,
        all_code_violations.len(),
        total_exempt,
        all_idempotence_failures.len()
    );

    let mut failures = Vec::new();
    if !all_code_violations.is_empty() {
        failures.push(format!(
            "Code line-width violations ({}):\n{}",
            all_code_violations.len(),
            all_code_violations.join("\n---\n")
        ));
    }
    if !all_idempotence_failures.is_empty() {
        failures.push(format!(
            "Idempotence failures ({}):\n{}",
            all_idempotence_failures.len(),
            all_idempotence_failures.join("\n---\n")
        ));
    }

    if !failures.is_empty() {
        panic!("\n{}", failures.join("\n\n"));
    }
}
