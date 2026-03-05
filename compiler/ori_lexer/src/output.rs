//! Output types for the lexer.
//!
//! Contains [`LexOutput`] (full metadata), [`LexResult`] (tokens + errors),
//! and related type aliases.

use ori_ir::{CommentList, ModuleExtra, TokenList};

use crate::lex_error::{DetachedDocWarning, LexError};

/// Output from lexing with comment capture and metadata.
///
/// Contains both the token stream (for parsing) and formatting metadata,
/// plus accumulated lexer errors and warnings.
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`, `Default`
///
/// # Field Visibility
/// All fields are intentionally `pub` — accessed directly by `oric`, `ori_parse`,
/// `ori_fmt`, and `ori_compiler`. Narrowing would require accessor methods +
/// updating many downstream sites for no behavioral benefit.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct LexOutput {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Comments captured during lexing.
    pub comments: CommentList,
    /// Byte positions of blank lines (consecutive newlines).
    pub blank_lines: Vec<u32>,
    /// Byte positions of all newlines.
    pub newlines: Vec<u32>,
    /// Accumulated lexer errors.
    pub errors: Vec<LexError>,
    /// Accumulated warnings (e.g., detached doc comments).
    pub warnings: Vec<DetachedDocWarning>,
}

impl std::fmt::Debug for LexOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LexOutput")
            .field("tokens", &self.tokens.len())
            .field("comments", &self.comments.len())
            .field("blank_lines", &self.blank_lines.len())
            .field("newlines", &self.newlines.len())
            .field("errors", &self.errors.len())
            .field("warnings", &self.warnings.len())
            .finish()
    }
}

impl LexOutput {
    /// Create a new empty lex output.
    pub fn new() -> Self {
        LexOutput {
            tokens: TokenList::new(),
            comments: CommentList::new(),
            blank_lines: Vec::new(),
            newlines: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create with pre-allocated capacity based on source length.
    ///
    /// Ori's dense syntax (short keywords, single-char operators, `@` prefixes)
    /// produces roughly 1 token per 2-3 bytes of source. Using `source_len / 2`
    /// slightly over-allocates but eliminates Vec reallocations, which callgrind
    /// showed as 5.7% of total lexer instructions.
    pub fn with_capacity(source_len: usize) -> Self {
        LexOutput {
            tokens: TokenList::with_capacity(source_len / 2 + 1),
            comments: CommentList::new(),
            blank_lines: Vec::with_capacity(source_len / 400),
            newlines: Vec::with_capacity(source_len / 40),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create with pre-allocated token capacity only (no metadata capacity).
    ///
    /// Used by the non-metadata lexer path where comments, blank lines, and
    /// newlines are not collected.
    pub(crate) fn with_token_capacity(source_len: usize) -> Self {
        LexOutput {
            tokens: TokenList::with_capacity(source_len / 2 + 1),
            comments: CommentList::new(),
            blank_lines: Vec::new(),
            newlines: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Check if any lexer errors were accumulated.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get the accumulated lexer errors.
    pub fn errors(&self) -> &[LexError] {
        &self.errors
    }

    /// Convert the lexer output into a `ModuleExtra` for the parser.
    ///
    /// This transfers ownership of comments and positions into a format
    /// suitable for `ParseOutput`.
    pub fn into_metadata(self) -> ModuleExtra {
        ModuleExtra {
            comments: self.comments,
            blank_lines: self.blank_lines,
            newlines: self.newlines,
            trailing_commas: Vec::new(), // filled in by the parser
        }
    }

    /// Decompose into tokens and metadata.
    ///
    /// This is the preferred way to use `LexOutput` with `parse_with_metadata`:
    ///
    /// ```ignore
    /// let lex_output = lex_with_comments(source, &interner);
    /// let (tokens, metadata) = lex_output.into_parts();
    /// let parse_output = parse_with_metadata(&tokens, metadata, &interner);
    /// ```
    pub fn into_parts(self) -> (TokenList, ModuleExtra) {
        let metadata = ModuleExtra {
            comments: self.comments,
            blank_lines: self.blank_lines,
            newlines: self.newlines,
            trailing_commas: Vec::new(),
        };
        (self.tokens, metadata)
    }
}

/// Result of lexing: tokens plus accumulated errors.
///
/// This is the primary output for the parsing pipeline, carrying both the
/// token stream and any lexer errors (unterminated strings, `===`, `;`, etc.).
///
/// # Salsa Compatibility
/// Has all required traits: `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct LexResult {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Accumulated lexer errors.
    pub errors: Vec<LexError>,
}
