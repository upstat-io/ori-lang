//! Error recovery for the parser.
//!
//! Provides token sets and synchronization for continuing parsing after errors.
//! Uses bitset-based O(1) membership testing inspired by Go's parser.

use super::cursor::Cursor;
use ori_ir::TokenKind;

// Compile-time assertion: TokenSet uses a u128 bitset, so all discriminant
// indices must fit in 0..127. If this fails, TokenSet needs a wider backing type.
const _: () = assert!(
    ori_ir::TokenTag::MAX_DISCRIMINANT <= 127,
    "TokenSet uses u128 bitset; all discriminant indices must be < 128"
);

/// A set of token kinds using bitset representation for O(1) membership testing.
///
/// Each bit in the u128 corresponds to a `TokenKind` discriminant index.
/// With 116 token kinds, we need u128 (128 bits) to cover all variants.
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
    #[allow(
        clippy::needless_pass_by_value,
        reason = "const fn builder API; by-value required for static init"
    )]
    pub const fn single(kind: TokenKind) -> Self {
        Self(1u128 << kind.discriminant_index())
    }

    /// Add a token kind to this set (builder pattern for const contexts).
    #[inline]
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "const fn builder API; by-value required for static init"
    )]
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

    /// Get the raw bits of this set (for iteration).
    #[inline]
    pub const fn bits(&self) -> u128 {
        self.0
    }

    /// Iterate over the discriminant indices in this set.
    ///
    /// Returns an iterator of `u8` discriminant indices. Use
    /// `TokenKind::from_discriminant_index()` to convert back to token kinds
    /// for display purposes.
    pub fn iter_indices(&self) -> TokenSetIterator {
        TokenSetIterator { bits: self.0 }
    }

    /// Add a token kind to this set (non-const mutation).
    #[inline]
    pub fn insert(&mut self, kind: &TokenKind) {
        self.0 |= 1u128 << kind.discriminant_index();
    }

    /// Union with another set (non-const mutation).
    #[inline]
    pub fn union_with(&mut self, other: &Self) {
        self.0 |= other.0;
    }

    /// Format this token set as a human-readable list for error messages.
    ///
    /// Returns a string like "`,`, `)`, or `}`" for multiple tokens,
    /// or "`(`" for a single token, or "nothing" for empty set.
    pub fn format_expected(&self) -> String {
        use ori_ir::TokenKind;

        let names: Vec<&'static str> = self
            .iter_indices()
            .filter_map(TokenKind::friendly_name_from_index)
            .collect();

        match names.as_slice() {
            [] => "nothing".to_string(),
            [single] => format!("`{single}`"),
            [first, second] => format!("`{first}` or `{second}`"),
            [rest @ .., last] => {
                let rest_str = rest
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{rest_str}, or `{last}`")
            }
        }
    }
}

/// Iterator over discriminant indices in a `TokenSet`.
pub struct TokenSetIterator {
    bits: u128,
}

impl Iterator for TokenSetIterator {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bits == 0 {
            return None;
        }
        // SAFETY: trailing_zeros() on u128 returns 0-127, which fits in u8
        let idx = self.bits.trailing_zeros();
        debug_assert!(idx <= 127, "TokenSet index out of u8 range");
        #[expect(
            clippy::cast_possible_truncation,
            reason = "u128::trailing_zeros() max is 127"
        )]
        let idx = idx as u8;
        self.bits &= self.bits - 1; // Clear the lowest set bit
        Some(idx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.bits.count_ones() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for TokenSetIterator {}

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
mod tests;
