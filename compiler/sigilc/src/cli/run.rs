// Run command for Sigil
// Handles interpreting and running Sigil programs

use sigilc::{eval, lexer, parser, types};
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

    if let Err(e) = run(&source, path) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

/// Run source code through the full pipeline
pub fn run(source: &str, filename: &str) -> Result<(), String> {
    // Step 1: Tokenize
    let tokens = lexer::tokenize(source, filename)?;

    // Step 2: Parse
    let ast = parser::parse(tokens, filename)?;

    // Step 3: Type check
    let typed_ast = types::check(ast)?;

    // Step 4: Evaluate
    eval::run(typed_ast)?;

    Ok(())
}
