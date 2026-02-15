//! Token index and per-token metadata flags.

/// Typed index into a `TokenList`.
///
/// Provides type safety over raw `u32` indices when referring to tokens.
/// Uses `u32::MAX` as a sentinel for "no token".
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct TokenIdx(u32);

impl TokenIdx {
    /// Sentinel value indicating no token.
    pub const NONE: TokenIdx = TokenIdx(u32::MAX);

    /// Create a `TokenIdx` from a raw index.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        TokenIdx(raw)
    }

    /// Get the raw `u32` index.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a valid index (not the `NONE` sentinel).
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

// Compile-time assertion: TokenIdx is exactly 4 bytes.
const _: () = assert!(size_of::<TokenIdx>() == 4);

/// Per-token metadata flags packed into a single byte.
///
/// These flags capture the whitespace/trivia context preceding each token,
/// enabling downstream consumers (formatter, parser) to reconstruct layout
/// without storing trivia tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TokenFlags(u8);

impl TokenFlags {
    /// Whitespace preceded this token (spaces or tabs).
    pub const SPACE_BEFORE: u8 = 1 << 0;
    /// A newline preceded this token.
    pub const NEWLINE_BEFORE: u8 = 1 << 1;
    /// A comment preceded this token.
    pub const TRIVIA_BEFORE: u8 = 1 << 2;
    /// Token is the first non-trivia token on its line.
    pub const LINE_START: u8 = 1 << 3;
    /// Cooking detected an error in this token.
    pub const HAS_ERROR: u8 = 1 << 4;
    /// A doc comment preceded this token (markers: `#`, `*`, `!`, `>`).
    pub const IS_DOC: u8 = 1 << 5;
    /// No whitespace, newline, or trivia preceded this token (adjacent to previous).
    pub const ADJACENT: u8 = 1 << 6;
    /// Token was resolved as a context-sensitive keyword (soft keyword with `(` lookahead).
    pub const CONTEXTUAL_KW: u8 = 1 << 7;

    /// Empty flags (no bits set).
    pub const EMPTY: Self = TokenFlags(0);

    /// Create flags from raw bits.
    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        TokenFlags(bits)
    }

    /// Get the raw bits.
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Check if a specific flag is set.
    #[inline]
    pub const fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    /// Set a flag.
    #[inline]
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Check if space preceded this token.
    #[inline]
    pub const fn has_space_before(self) -> bool {
        self.contains(Self::SPACE_BEFORE)
    }

    /// Check if a newline preceded this token.
    #[inline]
    pub const fn has_newline_before(self) -> bool {
        self.contains(Self::NEWLINE_BEFORE)
    }

    /// Check if a comment preceded this token.
    #[inline]
    pub const fn has_trivia_before(self) -> bool {
        self.contains(Self::TRIVIA_BEFORE)
    }

    /// Check if this token is first on its line.
    #[inline]
    pub const fn is_line_start(self) -> bool {
        self.contains(Self::LINE_START)
    }

    /// Check if cooking detected an error.
    #[inline]
    pub const fn has_error(self) -> bool {
        self.contains(Self::HAS_ERROR)
    }

    /// Check if a doc comment preceded this token.
    #[inline]
    pub const fn is_doc(self) -> bool {
        self.contains(Self::IS_DOC)
    }

    /// Check if this token is adjacent to the previous (no whitespace/trivia between).
    #[inline]
    pub const fn is_adjacent(self) -> bool {
        self.contains(Self::ADJACENT)
    }

    /// Check if this token was resolved as a context-sensitive keyword.
    #[inline]
    pub const fn is_contextual_kw(self) -> bool {
        self.contains(Self::CONTEXTUAL_KW)
    }
}

// Compile-time assertion: TokenFlags is exactly 1 byte.
const _: () = assert!(size_of::<TokenFlags>() == 1);
