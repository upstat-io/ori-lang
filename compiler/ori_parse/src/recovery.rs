//! Error recovery for the parser.
//!
//! Provides token sets and synchronization for continuing parsing after errors.
//! Uses bitset-based O(1) membership testing inspired by Go's parser.

use super::cursor::Cursor;
use ori_ir::TokenKind;

/// A set of token kinds using bitset representation for O(1) membership testing.
///
/// Each bit in the u128 corresponds to a `TokenKind` discriminant index.
/// With 115 token kinds, we need u128 (128 bits) to cover all variants.
///
/// # Performance
/// - Membership testing: O(1) via bitwise AND
/// - Set union: O(1) via bitwise OR
/// - Set intersection: O(1) via bitwise AND
///
/// # Example
/// ```ignore
/// const STMT_BOUNDARY: TokenSet = TokenSet::new()
///     .with(TokenKind::At)
///     .with(TokenKind::Use);
///
/// if STMT_BOUNDARY.contains(&TokenKind::At) {
///     // O(1) lookup
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenSet(u128);

impl TokenSet {
    /// Create an empty token set.
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create a token set containing a single token kind.
    #[inline]
    #[allow(clippy::needless_pass_by_value)] // const fn builder for static initialization
    pub const fn single(kind: TokenKind) -> Self {
        Self(1u128 << kind.discriminant_index())
    }

    /// Add a token kind to this set (builder pattern for const contexts).
    #[inline]
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // const fn builder for static initialization
    pub const fn with(self, kind: TokenKind) -> Self {
        Self(self.0 | (1u128 << kind.discriminant_index()))
    }

    /// Union of two token sets.
    #[inline]
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection of two token sets.
    #[inline]
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Check if this set contains a token kind.
    ///
    /// # Performance
    /// O(1) bitwise AND operation.
    #[inline]
    pub const fn contains(&self, kind: &TokenKind) -> bool {
        (self.0 & (1u128 << kind.discriminant_index())) != 0
    }

    /// Check if this set is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Count the number of token kinds in this set.
    #[inline]
    pub const fn count(&self) -> u32 {
        self.0.count_ones()
    }
}

impl Default for TokenSet {
    fn default() -> Self {
        Self::new()
    }
}

// Pre-defined token sets for common recovery scenarios.
// These are computed at compile time using const fn.

/// Recovery set for top-level statement boundaries.
/// Used to skip to the next function definition, import, or type declaration.
pub const STMT_BOUNDARY: TokenSet = TokenSet::new()
    .with(TokenKind::At) // Function/test definition
    .with(TokenKind::Use) // Import statement
    .with(TokenKind::Type) // Type declaration
    .with(TokenKind::Trait) // Trait definition
    .with(TokenKind::Impl) // Impl block
    .with(TokenKind::Pub) // Public declaration
    .with(TokenKind::Let) // Module-level constant
    .with(TokenKind::Extend) // Extension
    .with(TokenKind::Eof); // End of file

/// Recovery set for function-level boundaries.
/// Used when recovering inside a function definition.
pub const FUNCTION_BOUNDARY: TokenSet = TokenSet::new()
    .with(TokenKind::At) // Next function/test
    .with(TokenKind::Eof); // End of file

/// Recovery set for expression follow tokens.
/// Used when recovering inside expressions.
#[cfg(test)]
pub const EXPR_FOLLOW: TokenSet = TokenSet::new()
    .with(TokenKind::RParen) // End of call/group
    .with(TokenKind::RBracket) // End of index/list
    .with(TokenKind::RBrace) // End of block/map
    .with(TokenKind::Comma) // Separator
    .with(TokenKind::Newline); // Line break

// Additional recovery sets are defined as needed in the parser.
// See plans/ori_parse_improvements/ for planned additions.

/// Advance the cursor until reaching a token in the recovery set or EOF.
///
/// Returns `true` if a recovery token was found, `false` if EOF was reached.
pub fn synchronize(cursor: &mut Cursor<'_>, recovery: TokenSet) -> bool {
    while !cursor.is_at_end() {
        if recovery.contains(cursor.current_kind()) {
            return true;
        }
        cursor.advance();
    }
    false
}

/// Advance the cursor until reaching a token in the recovery set or EOF,
/// counting the number of skipped tokens.
///
/// Returns `Some(count)` if a recovery token was found, `None` if EOF was reached.
#[cfg(test)]
pub fn synchronize_counted(cursor: &mut Cursor<'_>, recovery: TokenSet) -> Option<usize> {
    let mut count = 0;
    while !cursor.is_at_end() {
        if recovery.contains(cursor.current_kind()) {
            return Some(count);
        }
        cursor.advance();
        count += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::StringInterner;

    fn make_cursor(source: &str) -> (Cursor<'static>, StringInterner) {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let tokens = Box::leak(Box::new(tokens));
        let interner = Box::leak(Box::new(interner));
        (Cursor::new(tokens, interner), StringInterner::new())
    }

    #[test]
    fn test_token_set_empty() {
        let set = TokenSet::new();
        assert!(set.is_empty());
        assert_eq!(set.count(), 0);
        assert!(!set.contains(&TokenKind::At));
    }

    #[test]
    fn test_token_set_single() {
        let set = TokenSet::single(TokenKind::At);
        assert!(!set.is_empty());
        assert_eq!(set.count(), 1);
        assert!(set.contains(&TokenKind::At));
        assert!(!set.contains(&TokenKind::Use));
    }

    #[test]
    fn test_token_set_with() {
        let set = TokenSet::new()
            .with(TokenKind::At)
            .with(TokenKind::Use)
            .with(TokenKind::Let);

        assert_eq!(set.count(), 3);
        assert!(set.contains(&TokenKind::At));
        assert!(set.contains(&TokenKind::Use));
        assert!(set.contains(&TokenKind::Let));
        assert!(!set.contains(&TokenKind::Plus));
    }

    #[test]
    fn test_token_set_union() {
        let set1 = TokenSet::new().with(TokenKind::At).with(TokenKind::Use);
        let set2 = TokenSet::new().with(TokenKind::Let).with(TokenKind::Use);

        let union = set1.union(set2);
        assert_eq!(union.count(), 3);
        assert!(union.contains(&TokenKind::At));
        assert!(union.contains(&TokenKind::Use));
        assert!(union.contains(&TokenKind::Let));
    }

    #[test]
    fn test_token_set_intersection() {
        let set1 = TokenSet::new().with(TokenKind::At).with(TokenKind::Use);
        let set2 = TokenSet::new().with(TokenKind::Let).with(TokenKind::Use);

        let intersection = set1.intersection(set2);
        assert_eq!(intersection.count(), 1);
        assert!(!intersection.contains(&TokenKind::At));
        assert!(intersection.contains(&TokenKind::Use));
        assert!(!intersection.contains(&TokenKind::Let));
    }

    #[test]
    fn test_token_set_data_variants() {
        // Data-carrying variants should work based on discriminant only
        let set = TokenSet::new()
            .with(TokenKind::Int(42))
            .with(TokenKind::Ident(ori_ir::Name::EMPTY));

        // Different values, same discriminant - should match
        assert!(set.contains(&TokenKind::Int(999)));
        assert!(set.contains(&TokenKind::Ident(ori_ir::Name::EMPTY)));
        assert!(!set.contains(&TokenKind::Float(0)));
    }

    #[test]
    fn test_stmt_boundary_contains() {
        assert!(STMT_BOUNDARY.contains(&TokenKind::At));
        assert!(STMT_BOUNDARY.contains(&TokenKind::Use));
        assert!(STMT_BOUNDARY.contains(&TokenKind::Type));
        assert!(STMT_BOUNDARY.contains(&TokenKind::Pub));
        assert!(!STMT_BOUNDARY.contains(&TokenKind::Plus));
    }

    #[test]
    fn test_expr_follow_contains() {
        assert!(EXPR_FOLLOW.contains(&TokenKind::RParen));
        assert!(EXPR_FOLLOW.contains(&TokenKind::Comma));
        assert!(!EXPR_FOLLOW.contains(&TokenKind::Plus));
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
        let found = synchronize(&mut cursor, FUNCTION_BOUNDARY);
        assert!(found);
        assert!(cursor.check(&TokenKind::At));
    }

    #[test]
    fn test_synchronize_to_expr_follow() {
        let (mut cursor, _) = make_cursor("(broken + , next)");

        cursor.advance(); // (
        cursor.advance(); // broken
        cursor.advance(); // +

        // Synchronize to expression follow
        let found = synchronize(&mut cursor, EXPR_FOLLOW);
        assert!(found);
        assert!(cursor.check(&TokenKind::Comma));
    }

    #[test]
    fn test_synchronize_eof() {
        let (mut cursor, _) = make_cursor("let x = 42");

        // Try to sync to non-existent token
        let found = synchronize(&mut cursor, FUNCTION_BOUNDARY);
        assert!(!found);
        assert!(cursor.is_at_end());
    }

    #[test]
    fn test_synchronize_counted() {
        let (mut cursor, _) = make_cursor("a b c @func");

        let result = synchronize_counted(&mut cursor, FUNCTION_BOUNDARY);
        assert_eq!(result, Some(3)); // Skipped: a, b, c
        assert!(cursor.check(&TokenKind::At));
    }

    #[test]
    fn test_const_token_sets() {
        // Verify const token sets are computed at compile time
        const TEST_SET: TokenSet = TokenSet::new().with(TokenKind::Plus).with(TokenKind::Minus);

        assert!(TEST_SET.contains(&TokenKind::Plus));
        assert!(TEST_SET.contains(&TokenKind::Minus));
        assert!(!TEST_SET.contains(&TokenKind::Star));
    }
}
