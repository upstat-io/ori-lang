//! Token matcher for flexible spacing rule matching.

use super::TokenCategory;

/// Flexible matcher for token categories in spacing rules.
///
/// Enables declarative rules like:
/// - `Any` - matches any token
/// - `Exact(Plus)` - matches only `+`
/// - `OneOf(&[Plus, Minus, Star])` - matches arithmetic ops
/// - `Category(is_binary_op)` - matches all binary operators
#[derive(Clone, Copy, Debug)]
pub enum TokenMatcher {
    /// Matches any token category.
    Any,

    /// Matches a specific token category.
    Exact(TokenCategory),

    /// Matches any category in the provided slice.
    OneOf(&'static [TokenCategory]),

    /// Matches using a category predicate function.
    Category(fn(TokenCategory) -> bool),
}

impl TokenMatcher {
    /// Match any binary operator.
    pub const BINARY_OP: TokenMatcher = TokenMatcher::Category(TokenCategory::is_binary_op);

    /// Match any unary operator.
    pub const UNARY_OP: TokenMatcher = TokenMatcher::Category(TokenCategory::is_unary_op);

    /// Match any opening delimiter.
    pub const OPEN_DELIM: TokenMatcher = TokenMatcher::Category(TokenCategory::is_open_delim);

    /// Match any closing delimiter.
    pub const CLOSE_DELIM: TokenMatcher = TokenMatcher::Category(TokenCategory::is_close_delim);

    /// Match any literal.
    pub const LITERAL: TokenMatcher = TokenMatcher::Category(TokenCategory::is_literal);

    /// Match any keyword.
    pub const KEYWORD: TokenMatcher = TokenMatcher::Category(TokenCategory::is_keyword);

    /// Check if this matcher matches the given token category.
    #[inline]
    pub fn matches(&self, cat: TokenCategory) -> bool {
        match self {
            TokenMatcher::Any => true,
            TokenMatcher::Exact(expected) => *expected == cat,
            TokenMatcher::OneOf(categories) => categories.contains(&cat),
            TokenMatcher::Category(predicate) => predicate(cat),
        }
    }
}

impl PartialEq for TokenMatcher {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TokenMatcher::Any, TokenMatcher::Any) => true,
            (TokenMatcher::Exact(a), TokenMatcher::Exact(b)) => a == b,
            (TokenMatcher::OneOf(a), TokenMatcher::OneOf(b)) => std::ptr::eq(*a, *b),
            (TokenMatcher::Category(a), TokenMatcher::Category(b)) => std::ptr::fn_addr_eq(*a, *b),
            _ => false,
        }
    }
}

impl Eq for TokenMatcher {}

/// Convenience macro for creating `OneOf` matchers.
#[macro_export]
macro_rules! one_of {
    ($($cat:expr),+ $(,)?) => {
        $crate::spacing::TokenMatcher::OneOf(&[$($cat),+])
    };
}
