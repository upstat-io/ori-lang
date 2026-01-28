//! Embedded error documentation for `--explain` support.
//!
//! Each error code has a markdown documentation file that explains the error,
//! shows examples, and provides solutions. These are embedded at compile time
//! and can be accessed via `ErrorDocs::get()`.
//!
//! # Adding New Documentation
//!
//! 1. Create a new file `EXXXX.md` in this directory
//! 2. Add an entry to the `DOCS` array below
//! 3. Run `cargo build` to embed the new documentation

use crate::ErrorCode;

/// Registry of embedded error documentation.
///
/// Use `ErrorDocs::get(code)` to retrieve the documentation for an error code.
pub struct ErrorDocs;

impl ErrorDocs {
    /// Get the documentation for an error code.
    ///
    /// Returns `Some(markdown)` if documentation exists for the code,
    /// `None` otherwise.
    ///
    /// # Example
    ///
    /// ```text
    /// if let Some(doc) = ErrorDocs::get(ErrorCode::E2001) {
    ///     println!("{}", doc);
    /// }
    /// ```
    pub fn get(code: ErrorCode) -> Option<&'static str> {
        DOCS.iter().find(|(c, _)| *c == code).map(|(_, doc)| *doc)
    }

    /// Get all documented error codes.
    pub fn all_codes() -> impl Iterator<Item = ErrorCode> {
        DOCS.iter().map(|(code, _)| *code)
    }

    /// Check if an error code has documentation.
    pub fn has_docs(code: ErrorCode) -> bool {
        DOCS.iter().any(|(c, _)| *c == code)
    }
}

/// Embedded documentation for each error code.
///
/// Add new entries here when creating new error documentation.
static DOCS: &[(ErrorCode, &str)] = &[
    // Lexer errors (E0xxx)
    (ErrorCode::E0001, include_str!("E0001.md")),
    (ErrorCode::E0002, include_str!("E0002.md")),
    (ErrorCode::E0003, include_str!("E0003.md")),
    (ErrorCode::E0004, include_str!("E0004.md")),
    (ErrorCode::E0005, include_str!("E0005.md")),
    // Parser errors (E1xxx)
    (ErrorCode::E1001, include_str!("E1001.md")),
    (ErrorCode::E1002, include_str!("E1002.md")),
    (ErrorCode::E1003, include_str!("E1003.md")),
    (ErrorCode::E1004, include_str!("E1004.md")),
    (ErrorCode::E1005, include_str!("E1005.md")),
    (ErrorCode::E1006, include_str!("E1006.md")),
    (ErrorCode::E1007, include_str!("E1007.md")),
    (ErrorCode::E1008, include_str!("E1008.md")),
    (ErrorCode::E1009, include_str!("E1009.md")),
    (ErrorCode::E1010, include_str!("E1010.md")),
    (ErrorCode::E1011, include_str!("E1011.md")),
    (ErrorCode::E1012, include_str!("E1012.md")),
    (ErrorCode::E1013, include_str!("E1013.md")),
    (ErrorCode::E1014, include_str!("E1014.md")),
    // Type errors (E2xxx)
    (ErrorCode::E2001, include_str!("E2001.md")),
    (ErrorCode::E2002, include_str!("E2002.md")),
    (ErrorCode::E2003, include_str!("E2003.md")),
    (ErrorCode::E2004, include_str!("E2004.md")),
    (ErrorCode::E2005, include_str!("E2005.md")),
    (ErrorCode::E2006, include_str!("E2006.md")),
    (ErrorCode::E2007, include_str!("E2007.md")),
    (ErrorCode::E2008, include_str!("E2008.md")),
    (ErrorCode::E2009, include_str!("E2009.md")),
    (ErrorCode::E2010, include_str!("E2010.md")),
    (ErrorCode::E2011, include_str!("E2011.md")),
    (ErrorCode::E2012, include_str!("E2012.md")),
    (ErrorCode::E2013, include_str!("E2013.md")),
    (ErrorCode::E2014, include_str!("E2014.md")),
    // Pattern errors (E3xxx)
    (ErrorCode::E3001, include_str!("E3001.md")),
    (ErrorCode::E3002, include_str!("E3002.md")),
    (ErrorCode::E3003, include_str!("E3003.md")),
    // Internal errors (E9xxx)
    (ErrorCode::E9001, include_str!("E9001.md")),
    (ErrorCode::E9002, include_str!("E9002.md")),
];

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_get_existing_doc() {
        let doc = ErrorDocs::get(ErrorCode::E2001);
        assert!(doc.is_some());
        assert!(doc.unwrap().contains("Type Mismatch"));
    }

    #[test]
    fn test_get_internal_error_doc() {
        // E9001 now has documentation
        let doc = ErrorDocs::get(ErrorCode::E9001);
        assert!(doc.is_some());
        assert!(doc.unwrap().contains("Internal Compiler Error"));
    }

    #[test]
    fn test_has_docs() {
        assert!(ErrorDocs::has_docs(ErrorCode::E2001));
        assert!(ErrorDocs::has_docs(ErrorCode::E9001));
        assert!(ErrorDocs::has_docs(ErrorCode::E3003));
    }

    #[test]
    fn test_all_codes() {
        let codes: Vec<_> = ErrorDocs::all_codes().collect();
        assert!(codes.contains(&ErrorCode::E2001));
        assert!(codes.len() >= 35); // We have at least 35 documented errors
    }
}
