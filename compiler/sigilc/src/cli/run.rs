// Run command for Sigil
// Handles interpreting and running Sigil programs

use sigilc::{eval, lexer, parser, types};
use sigilc::errors::{Diagnostic, DiagnosticResult, codes::ErrorCode};
use std::fs;

/// Run a Sigil source file
pub fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    if let Err(diag) = run(&source, path) {
        eprintln!("{}", diag);
        std::process::exit(1);
    }
}

/// Run source code through the full pipeline
pub fn run(source: &str, filename: &str) -> DiagnosticResult<()> {
    // Helper to convert string errors to diagnostics
    let to_diag = |msg: String| {
        Diagnostic::error(ErrorCode::E0000, msg)
            .with_label(sigilc::errors::Span::new(filename, 0..0), "error occurred here")
    };

    // Step 1: Tokenize
    let tokens = lexer::tokenize(source, filename).map_err(to_diag)?;

    // Step 2: Parse
    let ast = parser::parse(tokens, filename).map_err(to_diag)?;

    // Step 3: Type check
    let typed_ast = types::check(ast)?;

    // Step 4: Evaluate
    eval::run(typed_ast).map(|_| ()).map_err(to_diag)
}
