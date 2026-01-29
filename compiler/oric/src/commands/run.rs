//! The `run` command: parse, type-check, and evaluate an Ori source file.

use oric::query::{evaluated, parsed, typed};
use oric::{CompilerDb, SourceFile};
use std::path::PathBuf;

use super::read_file;

/// Run an Ori source file: parse, type-check, and evaluate it.
///
/// Accumulates all errors (parse and type) before exiting, giving the user
/// a complete picture of issues rather than stopping at the first error.
pub(crate) fn run_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let mut has_errors = false;

    // Check for parse errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        eprintln!("Parse errors in '{path}':");
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        has_errors = true;
    }

    // Check for type errors even if there were parse errors
    // This helps users see all issues at once
    let type_result = typed(&db, file);
    if type_result.has_errors() {
        eprintln!("Type errors in '{path}':");
        for error in &type_result.errors {
            let diag = error.to_diagnostic();
            eprintln!("  {diag}");
        }
        has_errors = true;
    }

    // Exit if any errors occurred
    if has_errors {
        std::process::exit(1);
    }

    // Evaluate only if no errors
    let eval_result = evaluated(&db, file);
    if eval_result.is_failure() {
        eprintln!("Runtime error: {}", eval_result.error.unwrap_or_default());
        std::process::exit(1);
    }

    // Print the result if it's not void
    if let Some(result) = eval_result.result {
        use oric::EvalOutput;
        match result {
            EvalOutput::Void => {}
            _ => println!("{}", result.display()),
        }
    }
}
