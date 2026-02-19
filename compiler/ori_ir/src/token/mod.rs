//! Token types for the Ori lexer.
//!
//! Provides token representation with all Salsa-required traits (Clone, Eq, Hash, Debug).
//!
//! # Specification
//!
//! - Lexical grammar: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § LEXICAL GRAMMAR
//! - Prose: `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md`

mod capture;
mod index;
mod kind;
mod list;
mod tag;
mod units;

pub use capture::TokenCapture;
pub use index::{TokenFlags, TokenIdx};
pub use kind::TokenKind;
pub use list::TokenList;
pub use tag::TokenTag;
pub use units::{DurationUnit, SizeUnit};

/// Number of [`TokenKind`] variants. Used for bitset sizing and test verification.
#[cfg(test)]
pub(crate) const TOKEN_KIND_COUNT: usize = 122;

use std::fmt;

use super::Span;

/// A token with its span in the source.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    #[inline]
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    /// Create a dummy token for testing/generated code.
    pub fn dummy(kind: TokenKind) -> Self {
        Token {
            kind,
            span: Span::DUMMY,
        }
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {}", self.kind, self.span)
    }
}

// Size assertions to prevent accidental regressions in frequently-allocated types.
// These are compile-time checks that will fail the build if sizes change.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{DurationUnit, SizeUnit, Token, TokenCapture, TokenKind};
    // Token is frequently allocated in TokenList, keep it compact.
    // Contains: TokenKind (16 bytes) + Span (8 bytes) = 24 bytes
    crate::static_assert_size!(Token, 24);
    // TokenKind largest variant: Duration(u64, DurationUnit) or Int(u64)
    // 8 bytes payload + 8 bytes discriminant/padding = 16 bytes
    crate::static_assert_size!(TokenKind, 16);
    // Compact unit types
    crate::static_assert_size!(DurationUnit, 1);
    crate::static_assert_size!(SizeUnit, 1);
    // TokenCapture: discriminant (4 bytes) + start (4 bytes) + end (4 bytes) = 12 bytes
    // Optimized to 12 bytes thanks to niche optimization (None has no payload)
    crate::static_assert_size!(TokenCapture, 12);
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
