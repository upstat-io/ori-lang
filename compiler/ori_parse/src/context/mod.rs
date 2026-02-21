//! Parse context flags for context-sensitive parsing.
//!
//! The parser uses context flags to handle ambiguous syntax and enforce
//! context-specific rules. For example, struct literals are disallowed
//! in certain positions to avoid ambiguity.

/// Context flags for parsing.
///
/// These flags control context-sensitive parsing behavior.
/// Multiple flags can be combined using bitwise OR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseContext(u16);

impl ParseContext {
    /// No special context.
    pub const NONE: Self = Self(0);

    /// Parsing a pattern (match arms, let bindings).
    /// Affects how identifiers and literals are interpreted.
    pub const IN_PATTERN: Self = Self(1 << 0);

    /// Parsing a type annotation.
    /// Affects how `>` is interpreted (closes generic vs comparison).
    pub const IN_TYPE: Self = Self(1 << 1);

    /// Struct literals are not allowed in this context.
    /// Used in `if` conditions to avoid ambiguity with blocks.
    pub const NO_STRUCT_LIT: Self = Self(1 << 2);

    /// Parsing a compile-time constant expression.
    /// Restricts allowed constructs to those evaluable at compile time.
    pub const CONST_EXPR: Self = Self(1 << 3);

    /// Inside a loop body.
    /// Makes `break` and `continue` valid.
    pub const IN_LOOP: Self = Self(1 << 4);

    /// `yield` expressions are allowed.
    /// Used in `for...yield` constructs.
    pub const ALLOW_YIELD: Self = Self(1 << 5);

    /// Inside a function body.
    /// Affects scoping and return handling.
    pub const IN_FUNCTION: Self = Self(1 << 6);

    /// Inside an index expression `[...]`.
    /// Makes `#` valid as the length symbol.
    pub const IN_INDEX: Self = Self(1 << 7);

    /// `|` is a separator (not bitwise OR).
    /// Used in `pre()` / `post()` contracts where `|` introduces a message string.
    pub const PIPE_IS_SEPARATOR: Self = Self(1 << 8);

    /// Create a new context with no flags set.
    #[inline]
    pub const fn new() -> Self {
        Self::NONE
    }

    /// Check if a flag is set.
    #[inline]
    pub const fn has(self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Add a flag to the context.
    #[inline]
    #[must_use]
    pub const fn with(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }

    /// Remove a flag from the context.
    #[inline]
    #[must_use]
    pub const fn without(self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }

    /// Combine two contexts (union of flags).
    #[inline]
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if struct literals are allowed in this context.
    #[inline]
    pub const fn allows_struct_lit(self) -> bool {
        !self.has(Self::NO_STRUCT_LIT)
    }

    /// Check if we're parsing a pattern.
    #[inline]
    pub const fn in_pattern(self) -> bool {
        self.has(Self::IN_PATTERN)
    }

    /// Check if we're parsing a type.
    #[inline]
    pub const fn in_type(self) -> bool {
        self.has(Self::IN_TYPE)
    }

    /// Check if we're inside a loop.
    #[inline]
    pub const fn in_loop(self) -> bool {
        self.has(Self::IN_LOOP)
    }

    /// Check if we're inside a function body.
    #[inline]
    pub const fn in_function(self) -> bool {
        self.has(Self::IN_FUNCTION)
    }

    /// Check if yield is allowed.
    #[inline]
    pub const fn allows_yield(self) -> bool {
        self.has(Self::ALLOW_YIELD)
    }

    /// Check if we're in a const expression context.
    #[inline]
    pub const fn in_const_expr(self) -> bool {
        self.has(Self::CONST_EXPR)
    }

    /// Check if we're inside an index expression.
    /// When true, `#` is valid as the length symbol.
    #[inline]
    pub const fn in_index(self) -> bool {
        self.has(Self::IN_INDEX)
    }
}

#[cfg(test)]
mod tests;
