//! Command handlers for the Ori compiler CLI.
//!
//! Each submodule implements a specific CLI command (run, test, check, etc.).
//! Shared utilities like `read_file` live here in the module root.

mod build;
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

pub(crate) use build::{build_file, parse_build_options, BuildOptions};
pub(crate) use check::check_file;
pub(crate) use debug::{lex_file, parse_file};
pub(crate) use demangle::demangle_symbol;
pub(crate) use explain::explain_error;
pub(crate) use fmt::run_format;
pub(crate) use run::{run_file, run_file_compiled};
pub(crate) use target::{add_target, list_installed_targets, remove_target, TargetSubcommand};
pub(crate) use targets::list_targets;
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
