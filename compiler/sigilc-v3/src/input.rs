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
mod tests {
    use super::*;
    use crate::db::CompilerDb;
    use salsa::Setter;

    #[test]
    fn test_source_file_creation() {
        let db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test/file.si"),
            "let x = 42".to_string(),
        );

        assert_eq!(file.path(&db), &PathBuf::from("/test/file.si"));
        assert_eq!(file.text(&db), "let x = 42");
    }

    #[test]
    fn test_source_file_mutation() {
        let mut db = CompilerDb::new();

        let file = SourceFile::new(
            &db,
            PathBuf::from("/test/file.si"),
            "let x = 42".to_string(),
        );

        assert_eq!(file.text(&db), "let x = 42");

        // Mutate the source using Salsa's Setter trait
        file.set_text(&mut db).to("let x = 100".to_string());

        assert_eq!(file.text(&db), "let x = 100");
    }
}
