//! Error recovery for the parser.
//!
//! Provides token sets and synchronization for continuing parsing after errors.
//! Uses bitset-based O(1) membership testing inspired by Go's parser.

use super::cursor::Cursor;
use ori_ir::TokenKind;

// TokenSet uses a [u128; 2] bitset (256 bits), covering all possible u8
// discriminant indices (0-255). No compile-time bound check needed since
// TokenTag discriminants are repr(u8), which is inherently < 256.

/// A set of token kinds using bitset representation for O(1) membership testing.
///
/// Uses `[u128; 2]` (256 bits) to cover all possible `TokenKind` discriminant
/// indices (0-255). Indices 0-127 are stored in `self.0[0]`, 128-255 in `self.0[1]`.
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
pub struct TokenSet([u128; 2]);

impl TokenSet {
    /// Create an empty token set.
    #[inline]
    pub const fn new() -> Self {
        Self([0; 2])
    }

    /// Create a token set containing a single token kind.
    #[inline]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "const fn builder API; by-value required for static init"
    )]
    pub const fn single(kind: TokenKind) -> Self {
        let idx = kind.discriminant_index();
        let mut bits = [0u128; 2];
        if idx < 128 {
            bits[0] = 1u128 << idx;
        } else {
            bits[1] = 1u128 << (idx - 128);
        }
        Self(bits)
    }

    /// Add a token kind to this set (builder pattern for const contexts).
    #[inline]
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "const fn builder API; by-value required for static init"
    )]
    pub const fn with(self, kind: TokenKind) -> Self {
        let idx = kind.discriminant_index();
        let mut bits = self.0;
        if idx < 128 {
            bits[0] |= 1u128 << idx;
        } else {
            bits[1] |= 1u128 << (idx - 128);
        }
        Self(bits)
    }

    /// Union of two token sets.
    #[inline]
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self([self.0[0] | other.0[0], self.0[1] | other.0[1]])
    }

    /// Intersection of two token sets.
    #[inline]
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self([self.0[0] & other.0[0], self.0[1] & other.0[1]])
    }

    /// Check if this set contains a token kind.
    ///
    /// # Performance
    /// O(1) bitwise AND operation.
    #[inline]
    pub const fn contains(&self, kind: &TokenKind) -> bool {
        let idx = kind.discriminant_index();
        if idx < 128 {
            (self.0[0] & (1u128 << idx)) != 0
        } else {
            (self.0[1] & (1u128 << (idx - 128))) != 0
        }
    }

    /// Check if this set is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0
    }

    /// Count the number of token kinds in this set.
    #[inline]
    pub const fn count(&self) -> u32 {
        self.0[0].count_ones() + self.0[1].count_ones()
    }

    /// Iterate over the discriminant indices in this set.
    ///
    /// Returns an iterator of `u8` discriminant indices. Use
    /// `TokenKind::from_discriminant_index()` to convert back to token kinds
    /// for display purposes.
    pub fn iter_indices(&self) -> TokenSetIterator {
        TokenSetIterator {
            lo: self.0[0],
            hi: self.0[1],
        }
    }

    /// Add a token kind to this set (non-const mutation).
    #[inline]
    pub fn insert(&mut self, kind: &TokenKind) {
        let idx = kind.discriminant_index();
        if idx < 128 {
            self.0[0] |= 1u128 << idx;
        } else {
            self.0[1] |= 1u128 << (idx - 128);
        }
    }

    /// Union with another set (non-const mutation).
    #[inline]
    pub fn union_with(&mut self, other: &Self) {
        self.0[0] |= other.0[0];
        self.0[1] |= other.0[1];
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
///
/// Iterates the low 128 bits first, then the high 128 bits.
pub struct TokenSetIterator {
    lo: u128,
    hi: u128,
}

impl Iterator for TokenSetIterator {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.lo != 0 {
            let idx = self.lo.trailing_zeros();
            self.lo &= self.lo - 1; // Clear the lowest set bit
            #[expect(
                clippy::cast_possible_truncation,
                reason = "u128::trailing_zeros() max is 127"
            )]
            Some(idx as u8)
        } else if self.hi != 0 {
            let idx = self.hi.trailing_zeros();
            self.hi &= self.hi - 1;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "128 + u128::trailing_zeros() max is 255, fits u8"
            )]
            Some((128 + idx) as u8)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = (self.lo.count_ones() + self.hi.count_ones()) as usize;
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
