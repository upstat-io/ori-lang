//! CLI module for Sigil V2 Compiler
//!
//! Provides command-line interface commands:
//! - `run` - Parse and evaluate a file
//! - `test` - Run tests in a file
//! - `check` - Parse and type check a file

pub mod run;
pub mod test;
pub mod check;

/// Print usage information
pub fn print_usage() {
    eprintln!("Sigil V2 Compiler v0.1.0-alpha");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  sigil-v2 run <file.si>      Parse and run a file");
    eprintln!("  sigil-v2 test <file.si>     Run tests in a file");
    eprintln!("  sigil-v2 check <file.si>    Parse and type check a file");
    eprintln!("  sigil-v2 <file.si>          Run file (shorthand for run)");
}
