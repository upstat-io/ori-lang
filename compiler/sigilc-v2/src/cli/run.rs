//! Run command - parse and evaluate a Sigil file

use std::fs;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ItemKind};
use sigilc_v2::eval::{Evaluator, Value};
use sigilc_v2::errors::Diagnostic;

/// Result of running a file
pub struct RunResult {
    pub value: Option<Value>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Run a Sigil source file
pub fn run_file(path: &str) -> Result<RunResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Error reading file '{}': {}", path, e))?;

    run_source(&source, path)
}

/// Run Sigil source code
pub fn run_source(source: &str, _filename: &str) -> Result<RunResult, String> {
    let interner = StringInterner::new();

    // Step 1: Lex
    let lexer = Lexer::new(source, &interner);
    let tokens = lexer.lex_all();

    // Step 2: Parse
    let parser = Parser::new(&tokens, &interner);
    let parse_result = parser.parse_module();

    // Check for parse errors
    if !parse_result.diagnostics.is_empty() {
        return Ok(RunResult {
            value: None,
            diagnostics: parse_result.diagnostics,
        });
    }

    // Step 3: Find and evaluate @main function
    let mut evaluator = Evaluator::new(&interner, &parse_result.arena);

    // Look for @main function
    for item in &parse_result.items {
        if let ItemKind::Function(func) = &item.kind {
            let name = interner.lookup(func.name);
            if name == "main" {
                // Evaluate the function body
                match evaluator.eval(func.body) {
                    Ok(value) => {
                        return Ok(RunResult {
                            value: Some(value),
                            diagnostics: vec![],
                        });
                    }
                    Err(e) => {
                        return Err(format!("Runtime error: {}", e.message));
                    }
                }
            }
        }
    }

    // No @main found - just return success with no value
    Ok(RunResult {
        value: None,
        diagnostics: vec![],
    })
}

/// Run file and print results
pub fn run_file_and_print(path: &str) {
    match run_file(path) {
        Ok(result) => {
            // Print any diagnostics
            for diag in &result.diagnostics {
                eprintln!("{:?}", diag);
            }

            if result.diagnostics.iter().any(|d| d.is_error()) {
                std::process::exit(1);
            }

            // Print result value if present
            if let Some(value) = result.value {
                if !matches!(value, Value::Void) {
                    println!("{:?}", value);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
