//! The `check` command: type-check an Ori source file and verify test coverage.

use oric::query::{parsed, typed};
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

use super::read_file;

/// Type-check a file and verify that every function has test coverage.
pub(crate) fn check_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Check for parse errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        eprintln!("Parse errors in '{path}':");
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        std::process::exit(1);
    }

    // Check for type errors
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        eprintln!("Type errors in '{path}':");
        for error in &type_result.errors {
            eprintln!("  {error:?}");
        }
        std::process::exit(1);
    }

    // Check test coverage: every function (except @main) must have at least one test
    let interner = db.interner();
    let main_name = interner.intern("main");

    // Collect all tested function names
    let mut tested_functions: std::collections::HashSet<oric::Name> =
        std::collections::HashSet::new();
    for test in &parse_result.module.tests {
        for target in &test.targets {
            tested_functions.insert(*target);
        }
    }

    // Find functions without tests
    let mut untested: Vec<String> = Vec::new();
    for func in &parse_result.module.functions {
        if func.name != main_name && !tested_functions.contains(&func.name) {
            untested.push(interner.lookup(func.name).to_string());
        }
    }

    if !untested.is_empty() {
        eprintln!("Coverage error in '{path}':");
        eprintln!("  The following functions have no tests:");
        for name in &untested {
            eprintln!("    @{name}");
        }
        eprintln!();
        eprintln!("  Every function (except @main) requires at least one test.");
        eprintln!(
            "  Add tests using: @test_name tests @{} () -> void = ...",
            untested[0]
        );
        std::process::exit(1);
    }

    let func_count = parse_result.module.functions.len();
    let test_count = parse_result.module.tests.len();
    println!("OK: {path} ({func_count} functions, {test_count} tests, 100% coverage)");
}
