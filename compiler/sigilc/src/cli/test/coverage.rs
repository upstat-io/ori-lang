// Test coverage checking

use std::path::Path;

use super::introspect::{get_functions, get_tested_functions};
use super::paths::get_test_file_path;
use super::super::import::parse_file;

/// Check test coverage for a source file
pub fn check_coverage(source_path: &str) {
    // Parse source file
    let source_module = match parse_file(source_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let functions = get_functions(&source_module);
    let test_path = get_test_file_path(source_path);

    // Parse test file
    let tested = if Path::new(&test_path).exists() {
        match parse_file(&test_path) {
            Ok(m) => get_tested_functions(&m),
            Err(e) => {
                eprintln!("Error parsing test file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        Vec::new()
    };

    // Check coverage
    let mut missing = Vec::new();
    for func in &functions {
        if func == "main" {
            continue; // main is exempt
        }
        if !tested.contains(func) {
            missing.push(func.clone());
        }
    }

    if missing.is_empty() {
        println!("✓ All functions have tests");
        println!("  {} functions, {} tests", functions.len(), tested.len());
    } else {
        eprintln!("✗ Missing tests for {} function(s):", missing.len());
        for func in &missing {
            eprintln!("  - @{}", func);
        }
        eprintln!();
        eprintln!("Create tests in: {}", test_path);
        eprintln!("Example:");
        eprintln!(
            "  @test_{} tests @{} () -> void = run(",
            missing[0], missing[0]
        );
        eprintln!("      assert({}(...) == expected)", missing[0]);
        eprintln!("  )");
        std::process::exit(1);
    }
}
