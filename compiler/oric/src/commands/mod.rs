//! Command handlers for the Ori compiler CLI.
//!
//! Each submodule implements a specific CLI command (run, test, check, etc.).
//! Shared utilities like `read_file` live here in the module root.

pub mod build;
mod check;
#[cfg(feature = "llvm")]
mod compile_common;
mod debug;
mod demangle;
mod explain;
mod fmt;
mod run;
mod target;
mod targets;
mod test;

// Public types and functions for external use (tests, library consumers)
pub use build::{
    parse_build_options, BuildOptions, DebugLevel, EmitType, LinkMode, LtoMode, OptLevel,
};

// Internal re-exports for use by the CLI binary via oric::commands::*
// These use paths like `oric::commands::build_file` from main.rs
pub use build::build_file;
pub use check::check_file;
pub use debug::{lex_file, parse_file};
pub use demangle::demangle_symbol;
pub use explain::explain_error;
pub use fmt::run_format;
pub use run::{run_file, run_file_compiled};
pub use target::{add_target, list_installed_targets, remove_target, TargetSubcommand};
pub use targets::{list_targets, TargetFilter};
pub use test::run_tests;

/// Read a file from disk, exiting with a user-friendly error message on failure.
pub(super) fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            let msg = match e.kind() {
                std::io::ErrorKind::NotFound => format!("cannot find file '{path}'"),
                std::io::ErrorKind::PermissionDenied => {
                    format!("permission denied reading '{path}'")
                }
                std::io::ErrorKind::InvalidData => {
                    format!("'{path}' contains invalid UTF-8 data")
                }
                _ => format!("error reading '{path}': {e}"),
            };
            eprintln!("{msg}");
            std::process::exit(1);
        }
    }
}
