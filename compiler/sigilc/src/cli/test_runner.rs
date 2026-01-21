// Test runner for Sigil
// Handles running individual test files and parallel test discovery

use sigilc::ast::{Item, Module};
use sigilc::{eval, types};

use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::import::{load_imports, parse_file};

/// Get the test file path for a given source file
pub fn get_test_file_path(source_path: &str) -> String {
    let path = Path::new(source_path);
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().unwrap().to_str().unwrap();

    // Try _test/name.test.si first
    let test_dir = parent.join("_test");
    let test_file = test_dir.join(format!("{}.test.si", stem));
    if test_file.exists() {
        return test_file.to_str().unwrap().to_string();
    }

    // Fall back to name.test.si in same directory
    let test_file = parent.join(format!("{}.test.si", stem));
    test_file.to_str().unwrap().to_string()
}

/// Get all function names from a module
pub fn get_functions(module: &Module) -> Vec<String> {
    module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(f) = item {
                Some(f.name.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Get all tested function names from a module (from test targets)
pub fn get_tested_functions(module: &Module) -> Vec<String> {
    module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Test(t) = item {
                Some(t.target.clone())
            } else {
                None
            }
        })
        .collect()
}

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
    let imported_items = match load_imports(&test_module, test_path) {
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

// ============================================================================
// Parallel Test Runner
// ============================================================================

/// Discover all test files in the current directory and subdirectories
fn discover_test_files() -> Vec<PathBuf> {
    let mut test_files = Vec::new();
    discover_test_files_recursive(Path::new("."), &mut test_files);
    test_files.sort();
    test_files
}

fn discover_test_files_recursive(dir: &Path, test_files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories and common non-source directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }

        if path.is_dir() {
            discover_test_files_recursive(&path, test_files);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".test.si") {
                test_files.push(path);
            }
        }
    }
}

/// Result of running a single test file
struct TestFileResult {
    path: String,
    passed: usize,
    failed: usize,
    errors: Vec<String>,
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
    let imported_items = match load_imports(&test_module, &path_str) {
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
