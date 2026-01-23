// Build command for Sigil
// Handles compiling Sigil to C and native binaries

use sigilc::errors::DiagnosticResult;
use std::fs;
use std::process::Command;

/// Options for code emission
#[derive(Default)]
pub struct EmitOptions {
    /// Enable verbose ARC tracking (prints all retain/release operations at runtime)
    pub verbose_arc: bool,
}

/// Emit C code from a Sigil source file
pub fn emit_file(path: &str, output: &str) {
    emit_file_with_options(path, output, EmitOptions::default())
}

/// Emit C code with verbose ARC tracking enabled
pub fn emit_file_verbose_arc(path: &str, output: &str) {
    emit_file_with_options(path, output, EmitOptions { verbose_arc: true })
}

/// Emit C code from a Sigil source file with options
pub fn emit_file_with_options(path: &str, output: &str, options: EmitOptions) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    let result = if options.verbose_arc {
        sigilc::emit_c_tir_verbose(&source, path)
    } else {
        sigilc::emit_c_tir(&source, path)
    };

    match result {
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

/// Compile source code to C using the TIR pipeline with ARC
pub fn compile_to_c(source: &str, filename: &str) -> DiagnosticResult<String> {
    sigilc::emit_c_tir(source, filename)
}

/// Compile source code to C with verbose ARC tracking
pub fn compile_to_c_verbose(source: &str, filename: &str) -> DiagnosticResult<String> {
    sigilc::emit_c_tir_verbose(source, filename)
}
