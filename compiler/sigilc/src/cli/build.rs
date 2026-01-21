// Build command for Sigil
// Handles compiling Sigil to C and native binaries

use sigilc::{codegen, lexer, parser, types};
use sigilc::errors::DiagnosticResult;
use std::fs;
use std::process::Command;

/// Emit C code from a Sigil source file
pub fn emit_file(path: &str, output: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    match compile_to_c(&source, path) {
        Ok(c_code) => {
            let out_path = if output.ends_with(".c") {
                output.to_string()
            } else {
                format!("{}.c", output)
            };
            if let Err(e) = fs::write(&out_path, &c_code) {
                eprintln!("Error writing file '{}': {}", out_path, e);
                std::process::exit(1);
            }
            println!("Emitted: {}", out_path);
        }
        Err(diag) => {
            eprintln!("{}", diag);
            std::process::exit(1);
        }
    }
}

/// Build a native binary from a Sigil source file
pub fn build_file(path: &str, output: &str) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    // Generate C code
    let c_code = match compile_to_c(&source, path) {
        Ok(c) => c,
        Err(diag) => {
            eprintln!("{}", diag);
            std::process::exit(1);
        }
    };

    // Write temporary C file
    let c_path = format!("{}.c", output);
    if let Err(e) = fs::write(&c_path, &c_code) {
        eprintln!("Error writing file '{}': {}", c_path, e);
        std::process::exit(1);
    }

    // Compile with gcc/cc
    let compiler = if cfg!(target_os = "windows") {
        "gcc"
    } else {
        "cc"
    };
    let status = Command::new(compiler)
        .args([&c_path, "-o", output, "-O2"])
        .status();

    match status {
        Ok(s) if s.success() => {
            // Clean up temporary C file
            let _ = fs::remove_file(&c_path);
            println!("Built: {}", output);
        }
        Ok(s) => {
            eprintln!("Compiler exited with: {}", s);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run compiler '{}': {}", compiler, e);
            eprintln!("C file left at: {}", c_path);
            std::process::exit(1);
        }
    }
}

/// Compile source code to C
pub fn compile_to_c(source: &str, filename: &str) -> DiagnosticResult<String> {
    use sigilc::errors::{Diagnostic, codes::ErrorCode};

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

    // Step 4: Generate C
    codegen::generate(&typed_ast).map_err(to_diag)
}
