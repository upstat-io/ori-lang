//! Error recovery for the parser.
//!
//! Provides recovery sets and synchronization for continuing parsing after errors.

use sigil_ir::TokenKind;
use super::cursor::Cursor;

/// A set of tokens to synchronize to during error recovery.
///
/// Recovery sets define "safe" positions where the parser can resume
/// after encountering an error. Common recovery points include:
/// - Statement boundaries (function definitions, imports)
/// - Expression terminators (closing parens, brackets)
/// - Block boundaries (closing braces)
#[derive(Clone, Copy, Debug)]
pub struct RecoverySet {
    tokens: &'static [TokenKind],
}

impl RecoverySet {
    /// Recovery set for top-level statement boundaries.
    /// Used to skip to the next function definition or import.
    pub const STMT_BOUNDARY: Self = Self {
        tokens: &[
            TokenKind::At,      // Function/test definition
            TokenKind::Use,     // Import statement
        ],
    };

    /// Recovery set for function-level boundaries.
    /// Used when recovering inside a function definition.
    pub const FUNCTION_BOUNDARY: Self = Self {
        tokens: &[
            TokenKind::At,      // Next function/test
        ],
    };

    /// Recovery set for expression follow tokens.
    /// Used when recovering inside expressions.
    pub const EXPR_FOLLOW: Self = Self {
        tokens: &[
            TokenKind::RParen,   // End of call/group
            TokenKind::RBracket, // End of index/list
            TokenKind::RBrace,   // End of block/map
            TokenKind::Comma,    // Separator
            TokenKind::Newline,  // Line break
        ],
    };

    /// Recovery set for block-level boundaries.
    /// Used when recovering inside blocks.
    pub const BLOCK_BOUNDARY: Self = Self {
        tokens: &[
            TokenKind::RBrace,   // End of block
            TokenKind::At,       // Next function
        ],
    };

    /// Recovery set for import statement recovery.
    pub const IMPORT_FOLLOW: Self = Self {
        tokens: &[
            TokenKind::RBrace,   // End of import items
            TokenKind::Comma,    // Next import item
            TokenKind::At,       // Next statement
            TokenKind::Use,      // Next import
        ],
    };

    /// Check if a token kind is in this recovery set.
    #[inline]
    pub fn contains(&self, kind: &TokenKind) -> bool {
        // Use discriminant comparison for efficiency
        let kind_disc = std::mem::discriminant(kind);
        self.tokens.iter().any(|t| std::mem::discriminant(t) == kind_disc)
    }
}

/// Advance the cursor until reaching a token in the recovery set or EOF.
///
/// Returns `true` if a recovery token was found, `false` if EOF was reached.
pub fn synchronize(cursor: &mut Cursor<'_>, recovery: RecoverySet) -> bool {
    while !cursor.is_at_end() {
        if recovery.contains(&cursor.current_kind()) {
            return true;
        }
        cursor.advance();
    }
    false
}


#[cfg(test)]
mod tests {
    use super::*;
    use sigil_ir::StringInterner;
    use sigil_lexer;

    fn make_cursor(source: &str) -> (Cursor<'static>, StringInterner) {
        let interner = StringInterner::new();
        let tokens = sigil_lexer::lex(source, &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        (Cursor::new(tokens, interner), StringInterner::new())
    }

    #[test]
    fn test_recovery_set_contains() {
        assert!(RecoverySet::STMT_BOUNDARY.contains(&TokenKind::At));
        assert!(RecoverySet::STMT_BOUNDARY.contains(&TokenKind::Use));
        assert!(!RecoverySet::STMT_BOUNDARY.contains(&TokenKind::Let));

        assert!(RecoverySet::EXPR_FOLLOW.contains(&TokenKind::RParen));
        assert!(RecoverySet::EXPR_FOLLOW.contains(&TokenKind::Comma));
        assert!(!RecoverySet::EXPR_FOLLOW.contains(&TokenKind::Plus));
    }

    #[test]
    fn test_synchronize_to_function() {
        let (mut cursor, _) = make_cursor("let x = broken + @next_func () -> int = 42");

        // Start parsing, encounter error, need to sync
        cursor.advance(); // let
        cursor.advance(); // x
        cursor.advance(); // =
        cursor.advance(); // broken
        cursor.advance(); // +

        // Synchronize to next function
        let found = synchronize(&mut cursor, RecoverySet::FUNCTION_BOUNDARY);
        assert!(found);
        assert!(cursor.check(TokenKind::At));
    }

    #[test]
    fn test_synchronize_to_expr_follow() {
        let (mut cursor, _) = make_cursor("(broken + , next)");

        cursor.advance(); // (
        cursor.advance(); // broken
        cursor.advance(); // +

        // Synchronize to expression follow
        let found = synchronize(&mut cursor, RecoverySet::EXPR_FOLLOW);
        assert!(found);
        assert!(cursor.check(TokenKind::Comma));
    }

    #[test]
    fn test_synchronize_eof() {
        let (mut cursor, _) = make_cursor("let x = 42");

        // Try to sync to non-existent token
        let found = synchronize(&mut cursor, RecoverySet::FUNCTION_BOUNDARY);
        assert!(!found);
        assert!(cursor.is_at_end());
    }
}
