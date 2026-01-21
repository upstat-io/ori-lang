// CLI module for Sigil
// Handles command-line interface and command dispatch

pub mod build;
pub mod import;
pub mod repl;
pub mod run;
pub mod test;

/// Print usage information
pub fn print_usage() {
    eprintln!("Sigil v0.1.0");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  sigil run <file.si>              Interpret and run");
    eprintln!("  sigil build <file.si> [-o out]   Compile to native binary");
    eprintln!("  sigil emit <file.si> [-o out.c]  Emit C code");
    eprintln!("  sigil test                       Run all tests (parallel)");
    eprintln!("  sigil test <file.si>             Run tests for specific file");
    eprintln!("  sigil check <file.si>            Check test coverage");
    eprintln!("  sigil repl                       Start REPL");
    eprintln!("  sigil <file.si>                  Run file (shorthand)");
}

/// Get output name from command-line arguments
pub fn get_output_name(args: &[String], start: usize) -> String {
    for i in start..args.len() {
        if args[i] == "-o" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    // Default: input file name without extension
    args.get(2)
        .map(|p| p.trim_end_matches(".si").to_string())
        .unwrap_or_else(|| "output".to_string())
}
