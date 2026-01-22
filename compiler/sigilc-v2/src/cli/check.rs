//! Check command - parse and type check a Sigil file

use std::fs;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser};
use sigilc_v2::errors::Diagnostic;

/// Result of checking a file
pub struct CheckResult {
    pub items_count: usize,
    pub diagnostics: Vec<Diagnostic>,
}

/// Check a Sigil source file (parse only for now, type checking TODO)
pub fn check_file(path: &str) -> Result<CheckResult, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Error reading file '{}': {}", path, e))?;

    check_source(&source, path)
}

/// Check Sigil source code
pub fn check_source(source: &str, _filename: &str) -> Result<CheckResult, String> {
    let interner = StringInterner::new();

    // Step 1: Lex
    let lexer = Lexer::new(source, &interner);
    let tokens = lexer.lex_all();

    // Step 2: Parse
    let parser = Parser::new(&tokens, &interner);
    let parse_result = parser.parse_module();

    // TODO: Step 3: Type check
    // This requires setting up DefinitionRegistry from parsed items

    Ok(CheckResult {
        items_count: parse_result.items.len(),
        diagnostics: parse_result.diagnostics,
    })
}

/// Check file and print results
pub fn check_file_and_print(path: &str) {
    match check_file(path) {
        Ok(result) => {
            // Print any diagnostics
            for diag in &result.diagnostics {
                eprintln!("{:?}", diag);
            }

            if result.diagnostics.iter().any(|d| d.is_error()) {
                eprintln!("Check failed with errors.");
                std::process::exit(1);
            } else {
                println!("Check passed: {} items parsed.", result.items_count);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
