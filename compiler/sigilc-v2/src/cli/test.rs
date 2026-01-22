//! Test command - run tests in a Sigil file

use std::fs;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ItemKind};
use sigilc_v2::eval::Evaluator;
use sigilc_v2::errors::Diagnostic;

/// Result of running tests
pub struct TestResult {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub diagnostics: Vec<Diagnostic>,
}

/// Run tests in a Sigil source file
pub fn test_file(path: &str) -> Result<TestResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Error reading file '{}': {}", path, e))?;

    test_source(&source, path)
}

/// Run tests in Sigil source code
pub fn test_source(source: &str, _filename: &str) -> Result<TestResult, String> {
    let interner = StringInterner::new();

    // Step 1: Lex
    let lexer = Lexer::new(source, &interner);
    let tokens = lexer.lex_all();

    // Step 2: Parse
    let parser = Parser::new(&tokens, &interner);
    let parse_result = parser.parse_module();

    // Check for parse errors
    if !parse_result.diagnostics.is_empty() {
        return Ok(TestResult {
            passed: 0,
            failed: 0,
            skipped: 0,
            diagnostics: parse_result.diagnostics,
        });
    }

    // Step 3: Find and run test functions
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut evaluator = Evaluator::new(&interner, &parse_result.arena);
    evaluator.register_prelude();

    // First pass: register all functions
    for item in &parse_result.items {
        if let ItemKind::Function(func) = &item.kind {
            // Create a function value and register it
            let params: Vec<_> = parse_result.arena.get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();
            let func_value = sigilc_v2::eval::Value::Function(sigilc_v2::eval::FunctionValue {
                params,
                body: func.body,
                captures: std::rc::Rc::new(std::cell::RefCell::new(Default::default())),
            });
            evaluator.env_mut().define_global(func.name, func_value);
        }
    }

    // Second pass: run tests
    for item in &parse_result.items {
        // Run formal tests (with `tests @target` syntax)
        if let ItemKind::Test(test) = &item.kind {
            let test_name = interner.lookup(test.name);

            // Check if test is skipped
            if let Some(reason) = test.skip_reason {
                let reason_str = interner.lookup(reason);
                println!("  [SKIP] {} - {}", test_name, reason_str);
                skipped += 1;
                continue;
            }

            // Evaluate the test body
            match evaluator.eval(test.body) {
                Ok(_) => {
                    println!("  [PASS] {}", test_name);
                    passed += 1;
                }
                Err(e) => {
                    println!("  [FAIL] {} - {}", test_name, e.message);
                    failed += 1;
                }
            }
        }

        // Also run functions whose names start with "test_" (test functions without targets)
        if let ItemKind::Function(func) = &item.kind {
            let func_name = interner.lookup(func.name);
            if func_name.starts_with("test_") {
                // Call the test function with no arguments
                match evaluator.eval(func.body) {
                    Ok(_) => {
                        println!("  [PASS] {}", func_name);
                        passed += 1;
                    }
                    Err(e) => {
                        println!("  [FAIL] {} - {}", func_name, e.message);
                        failed += 1;
                    }
                }
            }
        }
    }

    Ok(TestResult {
        passed,
        failed,
        skipped,
        diagnostics: vec![],
    })
}

/// Run tests and print results
pub fn test_file_and_print(path: &str) {
    println!("Running tests in {}...", path);

    match test_file(path) {
        Ok(result) => {
            // Print any diagnostics
            for diag in &result.diagnostics {
                eprintln!("{:?}", diag);
            }

            if result.diagnostics.iter().any(|d| d.is_error()) {
                std::process::exit(1);
            }

            println!();
            if result.failed == 0 && result.skipped == 0 {
                println!("All {} tests passed.", result.passed);
            } else if result.failed == 0 {
                println!("{} passed, {} skipped.", result.passed, result.skipped);
            } else if result.skipped == 0 {
                println!("{} passed, {} failed.", result.passed, result.failed);
                std::process::exit(1);
            } else {
                println!("{} passed, {} failed, {} skipped.", result.passed, result.failed, result.skipped);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
