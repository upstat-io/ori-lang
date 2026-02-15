//! Token list with Salsa-compatible traits.

use std::fmt;
use std::hash::Hash;

use super::index::TokenFlags;
use super::{Token, TokenCapture};

/// A list of tokens with Salsa-compatible traits.
///
/// Wraps `Vec<Token>` with Clone, Eq, Hash support.
/// Uses the tokens' own Hash impl for content hashing.
///
/// Includes a parallel `tags` array of `u8` discriminant indices for fast
/// dispatch. The tags are derived from `token.kind.discriminant_index()` at
/// insertion time, enabling O(1) tag comparison without touching the full
/// 16-byte `TokenKind`.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug, Default
#[derive(Clone, Default)]
pub struct TokenList {
    tokens: Vec<Token>,
    /// Parallel array of discriminant tags, one per token.
    /// `tags[i] == tokens[i].kind.discriminant_index()` for all `i`.
    tags: Vec<u8>,
    /// Parallel array of per-token metadata flags, one per token.
    /// `flags[i]` captures whitespace/trivia context for `tokens[i]`.
    flags: Vec<TokenFlags>,
}

// Manual Eq/PartialEq/Hash: position-independent comparison.
//
// We skip `tags` (derived from `tokens.kind`) AND skip `Span` positions.
// Only `TokenKind` and `TokenFlags` are compared/hashed. This enables
// Salsa early cutoff: whitespace-only edits shift token positions but
// produce the same kinds+flags, so downstream queries (parsing, type
// checking) are not re-executed.
impl PartialEq for TokenList {
    fn eq(&self, other: &Self) -> bool {
        if self.tokens.len() != other.tokens.len() {
            return false;
        }
        self.tokens
            .iter()
            .zip(other.tokens.iter())
            .all(|(a, b)| a.kind == b.kind)
            && self.flags == other.flags
    }
}
impl Eq for TokenList {}
impl Hash for TokenList {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tokens.len().hash(state);
        for token in &self.tokens {
            token.kind.hash(state);
        }
        self.flags.hash(state);
    }
}

impl TokenList {
    /// Create a new empty token list.
    #[inline]
    pub fn new() -> Self {
        TokenList {
            tokens: Vec::new(),
            tags: Vec::new(),
            flags: Vec::new(),
        }
    }

    /// Create a new token list with pre-allocated capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        TokenList {
            tokens: Vec::with_capacity(capacity),
            tags: Vec::with_capacity(capacity),
            flags: Vec::with_capacity(capacity),
        }
    }

    /// Create from a Vec of tokens.
    ///
    /// All tokens get `TokenFlags::EMPTY` since no trivia context is available.
    #[inline]
    pub fn from_vec(tokens: Vec<Token>) -> Self {
        let tags = tokens.iter().map(|t| t.kind.discriminant_index()).collect();
        let flags = vec![TokenFlags::EMPTY; tokens.len()];
        TokenList {
            tokens,
            tags,
            flags,
        }
    }

    /// Push a token with default (empty) flags.
    #[inline]
    pub fn push(&mut self, token: Token) {
        self.tags.push(token.kind.discriminant_index());
        self.flags.push(TokenFlags::EMPTY);
        self.tokens.push(token);
    }

    /// Push a token with explicit flags.
    #[inline]
    pub fn push_with_flags(&mut self, token: Token, flags: TokenFlags) {
        self.tags.push(token.kind.discriminant_index());
        self.flags.push(flags);
        self.tokens.push(token);
    }

    /// Get the number of tokens.
    #[inline]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Check if empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Get token at index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }

    /// Get a slice of all tokens.
    #[inline]
    pub fn as_slice(&self) -> &[Token] {
        &self.tokens
    }

    /// Iterate over tokens.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Token> {
        self.tokens.iter()
    }

    /// Get tokens in a capture range.
    ///
    /// Returns an empty slice for `TokenCapture::None`.
    ///
    /// # Panics
    ///
    /// Panics if the capture range is out of bounds.
    #[inline]
    pub fn get_range(&self, capture: TokenCapture) -> &[Token] {
        match capture {
            TokenCapture::None => &[],
            TokenCapture::Range { start, end } => &self.tokens[start as usize..end as usize],
        }
    }

    /// Get tokens in a capture range, returning None if out of bounds.
    #[inline]
    pub fn try_get_range(&self, capture: TokenCapture) -> Option<&[Token]> {
        match capture {
            TokenCapture::None => Some(&[]),
            TokenCapture::Range { start, end } => self.tokens.get(start as usize..end as usize),
        }
    }

    /// Get the tag (discriminant index) at the given position.
    ///
    /// This is a fast O(1) read from the dense tag array, avoiding
    /// the need to access the full 16-byte `TokenKind`.
    #[inline]
    pub fn tag(&self, index: usize) -> u8 {
        self.tags[index]
    }

    /// Get the full tags slice.
    #[inline]
    pub fn tags(&self) -> &[u8] {
        &self.tags
    }

    /// Get the flags for the token at the given position.
    #[inline]
    pub fn flag(&self, index: usize) -> TokenFlags {
        self.flags[index]
    }

    /// Get the full flags slice.
    #[inline]
    pub fn flags(&self) -> &[TokenFlags] {
        &self.flags
    }

    /// Consume into Vec.
    #[inline]
    pub fn into_vec(self) -> Vec<Token> {
        self.tokens
    }
}

impl fmt::Debug for TokenList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenList({} tokens)", self.tokens.len())
    }
}

impl std::ops::Index<usize> for TokenList {
    type Output = Token;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.tokens[index]
    }
}

impl IntoIterator for TokenList {
    type Item = Token;
    type IntoIter = std::vec::IntoIter<Token>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens.into_iter()
    }
}

impl<'a> IntoIterator for &'a TokenList {
    type Item = &'a Token;
    type IntoIter = std::slice::Iter<'a, Token>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens.iter()
    }
}
