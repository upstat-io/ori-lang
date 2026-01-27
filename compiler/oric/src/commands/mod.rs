//! Command handlers for the Ori compiler CLI.
//!
//! Each submodule implements a specific CLI command (run, test, check, etc.).
//! Shared utilities like `read_file` live here in the module root.

mod check;
mod compile;
mod debug;
mod explain;
mod run;
mod test;

pub(crate) use check::check_file;
pub(crate) use compile::compile_file;
pub(crate) use debug::{lex_file, parse_file};
pub(crate) use explain::explain_error;
pub(crate) use run::run_file;
pub(crate) use test::run_tests;

/// Read a file from disk, exiting with an error message on failure.
pub(super) fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading '{path}': {e}");
            std::process::exit(1);
        }
    }
}
