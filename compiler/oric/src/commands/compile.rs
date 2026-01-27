//! The `compile` command: generate C code from an Ori source file.

use oric::query::codegen_c;
use oric::{CompilerDb, SourceFile};
use std::path::PathBuf;

use super::read_file;

/// Compile an Ori source file to C, writing the output to `output_path`.
pub(crate) fn compile_file(input_path: &str, output_path: &str) {
    let content = read_file(input_path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(input_path), content);

    // Generate C code
    let result = codegen_c(&db, file);

    if result.has_errors() {
        eprintln!("Compilation errors:");
        for error in &result.errors {
            eprintln!("  {}", error.message);
        }
        std::process::exit(1);
    }

    // Write output
    if let Err(e) = std::fs::write(output_path, &result.code) {
        eprintln!("Error writing '{output_path}': {e}");
        std::process::exit(1);
    }

    println!("Generated: {output_path}");
}
