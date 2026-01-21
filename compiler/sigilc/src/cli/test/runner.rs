// Test execution

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;
use sigilc::ast::{Item, Module};
use sigilc::{eval, types};

use super::discovery::discover_test_files;
use super::result::TestFileResult;
use super::super::import::{load_imports, parse_file};

/// Run tests from a single test file
pub fn test_file(test_path: &str) {
    // Parse test file
    let test_module = match parse_file(test_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error parsing test file: {}", e);
            std::process::exit(1);
        }
    };

    // Check for imports
    let has_imports = test_module.items.iter().any(|i| matches!(i, Item::Use(_)));
    if !has_imports {
        eprintln!("Error: Test file must import the module being tested");
        eprintln!("Add: use module_name {{ function1, function2 }}");
        std::process::exit(1);
    }

    // Load imported modules
    let imported_items: Vec<Item> = match load_imports(&test_module, test_path) {
        Ok(items) => items,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // Combine imported items with test items
    let mut combined_items = imported_items;
    combined_items.extend(test_module.items.clone());

    let combined = Module {
        name: test_module.name.clone(),
        items: combined_items,
    };

    // Type check
    let typed = match types::check(combined) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Type error: {}", e);
            std::process::exit(1);
        }
    };

    // Run tests
    let tests: Vec<_> = typed
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Test(t) = item {
                Some(t)
            } else {
                None
            }
        })
        .collect();

    if tests.is_empty() {
        eprintln!("No tests found in {}", test_path);
        std::process::exit(1);
    }

    println!("Running {} test(s)...\n", tests.len());

    let mut passed = 0;
    let mut failed = 0;

    for test in &tests {
        print!("  @{} tests @{} ... ", test.name, test.target);
        match eval::run_test(&typed, test) {
            Ok(()) => {
                println!("✓");
                passed += 1;
            }
            Err(e) => {
                println!("✗");
                eprintln!("    {}", e);
                failed += 1;
            }
        }
    }

    println!();
    if failed == 0 {
        println!("All {} test(s) passed!", passed);
    } else {
        eprintln!("{} passed, {} failed", passed, failed);
        std::process::exit(1);
    }
}

/// Run a single test file and return results
fn run_test_file(test_path: &Path) -> TestFileResult {
    let path_str = test_path.to_string_lossy().to_string();

    // Parse test file
    let test_module = match parse_file(&path_str) {
        Ok(m) => m,
        Err(e) => {
            return TestFileResult {
                path: path_str,
                passed: 0,
                failed: 1,
                errors: vec![format!("Parse error: {}", e)],
            };
        }
    };

    // Check for imports
    let has_imports = test_module.items.iter().any(|i| matches!(i, Item::Use(_)));
    if !has_imports {
        return TestFileResult {
            path: path_str,
            passed: 0,
            failed: 1,
            errors: vec!["Test file must import the module being tested".to_string()],
        };
    }

    // Load imported modules
    let imported_items: Vec<Item> = match load_imports(&test_module, &path_str) {
        Ok(items) => items,
        Err(e) => {
            return TestFileResult {
                path: path_str,
                passed: 0,
                failed: 1,
                errors: vec![e],
            };
        }
    };

    // Combine imported items with test items
    let mut combined_items = imported_items;
    combined_items.extend(test_module.items.clone());

    let combined = Module {
        name: test_module.name.clone(),
        items: combined_items,
    };

    // Type check
    let typed = match types::check(combined) {
        Ok(t) => t,
        Err(e) => {
            return TestFileResult {
                path: path_str,
                passed: 0,
                failed: 1,
                errors: vec![format!("Type error: {}", e)],
            };
        }
    };

    // Run tests
    let tests: Vec<_> = typed
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Test(t) = item {
                Some(t)
            } else {
                None
            }
        })
        .collect();

    if tests.is_empty() {
        return TestFileResult {
            path: path_str,
            passed: 0,
            failed: 0,
            errors: vec!["No tests found".to_string()],
        };
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for test in &tests {
        match eval::run_test(&typed, test) {
            Ok(()) => {
                passed += 1;
            }
            Err(e) => {
                errors.push(format!("@{}: {}", test.name, e));
                failed += 1;
            }
        }
    }

    TestFileResult {
        path: path_str,
        passed,
        failed,
        errors,
    }
}

/// Run all tests in parallel
pub fn test_all() {
    let start_time = Instant::now();

    // Discover test files
    let test_files = discover_test_files();

    if test_files.is_empty() {
        eprintln!("No test files found (*.test.si)");
        std::process::exit(1);
    }

    println!("Discovering tests...");
    println!("Found {} test file(s)\n", test_files.len());

    // Counters for progress
    let total_passed = AtomicUsize::new(0);
    let total_failed = AtomicUsize::new(0);
    let files_done = AtomicUsize::new(0);
    let total_files = test_files.len();

    // Run tests in parallel
    let results: Vec<TestFileResult> = test_files
        .par_iter()
        .map(|path| {
            let result = run_test_file(path);

            // Update counters
            total_passed.fetch_add(result.passed, Ordering::Relaxed);
            total_failed.fetch_add(result.failed, Ordering::Relaxed);
            let done = files_done.fetch_add(1, Ordering::Relaxed) + 1;

            // Print progress
            let status = if result.failed == 0 && result.errors.is_empty() {
                "✓"
            } else {
                "✗"
            };
            println!(
                "[{}/{}] {} {} ({} passed)",
                done, total_files, status, result.path, result.passed
            );

            result
        })
        .collect();

    let elapsed = start_time.elapsed();
    let passed = total_passed.load(Ordering::Relaxed);
    let failed = total_failed.load(Ordering::Relaxed);

    // Print summary
    println!("\n{}", "=".repeat(60));

    // Print failures
    let failures: Vec<_> = results.iter().filter(|r| !r.errors.is_empty()).collect();

    if !failures.is_empty() {
        println!("\nFailures:\n");
        for result in &failures {
            println!("  {}", result.path);
            for error in &result.errors {
                println!("    {}", error);
            }
        }
        println!();
    }

    // Print final summary
    println!(
        "Test Results: {} passed, {} failed ({:.2}s)",
        passed,
        failed,
        elapsed.as_secs_f64()
    );

    if failed > 0 {
        std::process::exit(1);
    }
}
