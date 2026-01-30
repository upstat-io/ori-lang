//! Command handlers for the Ori compiler CLI.
//!
//! Each submodule implements a specific CLI command (run, test, check, etc.).
//! Shared utilities like `read_file` live here in the module root.

mod check;
mod debug;
mod explain;
mod fmt;
mod run;
mod test;

pub(crate) use check::check_file;
pub(crate) use debug::{lex_file, parse_file};
pub(crate) use explain::explain_error;
pub(crate) use fmt::run_format;
pub(crate) use run::run_file;
pub(crate) use test::run_tests;

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
