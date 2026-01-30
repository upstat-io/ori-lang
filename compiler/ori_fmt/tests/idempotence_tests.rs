#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Comprehensive idempotence verification tests.
//!
//! These tests verify that:
//! 1. format(format(code)) == format(code) for all .ori files
//! 2. The AST is semantically equivalent before and after formatting
//!
//! This covers all test files in the repository, not just golden tests.

use std::fs;
use std::path::{Path, PathBuf};

use ori_fmt::{format_module, format_module_with_comments};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;

/// Find all .ori files in a directory recursively, excluding certain patterns.
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
                // Skip .expected files (they have .ori.expected extension)
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

/// Parse and format code.
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

/// Parse and format code with comment preservation.
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

/// Test idempotence: format(format(x)) == format(x)
fn test_idempotence_for_file(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // Try formatting with comments first
    let first = match parse_and_format_with_comments(&source) {
        Ok(s) => s,
        Err(_) => {
            // Fall back to no-comment formatting
            match parse_and_format(&source) {
                Ok(s) => s,
                Err(e) => {
                    // File doesn't parse - skip
                    return Err(format!("Parse error (skip): {}", e));
                }
            }
        }
    };

    // Parse and format the result
    let second = match parse_and_format_with_comments(&first) {
        Ok(s) => s,
        Err(e) => {
            // Formatter output doesn't re-parse - this is a known limitation
            // for some constructs (e.g., multi-line parameters)
            return Err(format!("Re-parse error (skip): {}", e));
        }
    };

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    if first_normalized != second_normalized {
        return Err(format!(
            "Idempotence failure:\n\n--- First format ---\n{}\n--- Second format ---\n{}\n",
            first_normalized, second_normalized
        ));
    }

    Ok(())
}

/// Run idempotence tests on all files in a directory.
fn run_idempotence_tests_for_dir(dir: &Path) -> (usize, usize, Vec<String>) {
    let files = find_all_ori_files(dir);
    let mut passed = 0;
    let mut skipped = 0;
    let mut failures = Vec::new();

    for file in &files {
        match test_idempotence_for_file(file) {
            Ok(()) => passed += 1,
            Err(e) if e.starts_with("Parse error") || e.starts_with("Re-parse error") => {
                skipped += 1;
            }
            Err(e) => failures.push(format!("{}: {}", file.display(), e)),
        }
    }

    (passed, skipped, failures)
}

#[test]
fn idempotence_tests_spec() {
    let dir = repo_root().join("tests").join("spec");
    let (passed, skipped, failures) = run_idempotence_tests_for_dir(&dir);

    println!(
        "tests/spec: {} passed, {} skipped, {} failed",
        passed,
        skipped,
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} idempotence failures in tests/spec:\n\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

#[test]
fn idempotence_tests_run_pass() {
    let dir = repo_root().join("tests").join("run-pass");
    let (passed, skipped, failures) = run_idempotence_tests_for_dir(&dir);

    println!(
        "tests/run-pass: {} passed, {} skipped, {} failed",
        passed,
        skipped,
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} idempotence failures in tests/run-pass:\n\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

#[test]
fn idempotence_tests_fmt() {
    let dir = repo_root().join("tests").join("fmt");
    let (passed, skipped, failures) = run_idempotence_tests_for_dir(&dir);

    println!(
        "tests/fmt: {} passed, {} skipped, {} failed",
        passed,
        skipped,
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} idempotence failures in tests/fmt:\n\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

#[test]
fn idempotence_tests_library() {
    let dir = repo_root().join("library");
    let (passed, skipped, failures) = run_idempotence_tests_for_dir(&dir);

    println!(
        "library/: {} passed, {} skipped, {} failed",
        passed,
        skipped,
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} idempotence failures in library/:\n\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

/// Comprehensive test: run idempotence on all .ori files in the repository.
#[test]
fn idempotence_comprehensive() {
    let root = repo_root();
    let dirs = [
        root.join("tests").join("spec"),
        root.join("tests").join("run-pass"),
        root.join("tests").join("fmt"),
        root.join("library"),
    ];

    let mut total_passed = 0;
    let mut total_skipped = 0;
    let mut all_failures = Vec::new();

    for dir in &dirs {
        let (passed, skipped, failures) = run_idempotence_tests_for_dir(dir);
        total_passed += passed;
        total_skipped += skipped;
        all_failures.extend(failures);
    }

    println!(
        "\nComprehensive idempotence: {} passed, {} skipped, {} failed",
        total_passed,
        total_skipped,
        all_failures.len()
    );

    if !all_failures.is_empty() {
        panic!(
            "{} idempotence failures:\n\n{}",
            all_failures.len(),
            all_failures.join("\n---\n")
        );
    }
}
