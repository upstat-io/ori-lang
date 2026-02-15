//! Salsa Inputs - User-provided data that can change
//!
//! Inputs are the "leaves" of the query graph. When an input changes,
//! Salsa automatically invalidates all queries that depend on it.

use std::path::PathBuf;

/// A source file input.
///
/// This is a Salsa input - the user can create and modify it,
/// and Salsa tracks which queries depend on it.
///
/// When `text` changes, all queries that read from this file
/// are automatically invalidated and will re-run on next access.
#[salsa::input]
pub struct SourceFile {
    /// Absolute path to the file (for error messages).
    #[return_ref]
    pub path: PathBuf,

    /// Source text content.
    #[return_ref]
    pub text: String,
}

#[cfg(test)]
mod tests;
