//! Centralized debug flags for the Sigil compiler.
//!
//! This module provides a unified way to control debug output across
//! all compiler phases. Debug flags can be set via the `SIGIL_DEBUG`
//! environment variable.
//!
//! # Environment Variable
//!
//! Set `SIGIL_DEBUG` to a comma-separated list of flags:
//! - `tokens` - Print lexer output
//! - `ast` - Print parsed AST
//! - `types` - Print type inference results
//! - `eval` - Print evaluation steps
//! - `all` - Enable all debug output
//!
//! Example: `SIGIL_DEBUG=tokens,types cargo run`

use std::sync::OnceLock;

/// Debug flags for controlling compiler debug output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DebugFlags(u32);

impl DebugFlags {
    /// No debug flags.
    pub const NONE: Self = Self(0);
    /// Print lexer token output.
    pub const TOKENS: Self = Self(0b0000_0001);
    /// Print parsed AST.
    pub const AST: Self = Self(0b0000_0010);
    /// Print type inference results.
    pub const TYPES: Self = Self(0b0000_0100);
    /// Print evaluation steps.
    pub const EVAL: Self = Self(0b0000_1000);
    /// Print import resolution.
    pub const IMPORTS: Self = Self(0b0001_0000);
    /// Print pattern evaluation.
    pub const PATTERNS: Self = Self(0b0010_0000);
    /// All debug flags.
    pub const ALL: Self = Self(
        Self::TOKENS.0 | Self::AST.0 | Self::TYPES.0
        | Self::EVAL.0 | Self::IMPORTS.0 | Self::PATTERNS.0
    );

    /// Create empty flags.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Check if flags contain another flag.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Parse debug flags from a comma-separated string.
    pub fn parse(s: &str) -> Self {
        let mut flags = 0u32;
        for part in s.split(',') {
            match part.trim().to_lowercase().as_str() {
                "tokens" => flags |= Self::TOKENS.0,
                "ast" => flags |= Self::AST.0,
                "types" => flags |= Self::TYPES.0,
                "eval" => flags |= Self::EVAL.0,
                "imports" => flags |= Self::IMPORTS.0,
                "patterns" => flags |= Self::PATTERNS.0,
                "all" => flags |= Self::ALL.0,
                _ => {}
            }
        }
        Self(flags)
    }
}

impl std::ops::BitOr for DebugFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for DebugFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for DebugFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Sub for DebugFlags {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0)
    }
}

impl std::ops::SubAssign for DebugFlags {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 &= !rhs.0;
    }
}

/// Global debug flags, initialized from environment.
static DEBUG_FLAGS: OnceLock<DebugFlags> = OnceLock::new();

/// Get the current debug flags.
///
/// Reads from `SIGIL_DEBUG` environment variable on first call,
/// then returns cached value.
pub fn debug_flags() -> DebugFlags {
    *DEBUG_FLAGS.get_or_init(|| {
        std::env::var("SIGIL_DEBUG")
            .ok()
            .map(|s| DebugFlags::parse(&s))
            .unwrap_or_default()
    })
}

/// Check if a specific debug flag is enabled.
pub fn is_debug_enabled(flag: DebugFlags) -> bool {
    debug_flags().contains(flag)
}

/// Print debug message if the specified flag is enabled.
#[macro_export]
macro_rules! debug_print {
    ($flag:expr, $($arg:tt)*) => {
        if $crate::debug::is_debug_enabled($flag) {
            eprintln!($($arg)*);
        }
    };
}

/// Print tokens if TOKENS debug flag is enabled.
#[macro_export]
macro_rules! debug_tokens {
    ($($arg:tt)*) => {
        $crate::debug_print!($crate::debug::DebugFlags::TOKENS, $($arg)*)
    };
}

/// Print AST if AST debug flag is enabled.
#[macro_export]
macro_rules! debug_ast {
    ($($arg:tt)*) => {
        $crate::debug_print!($crate::debug::DebugFlags::AST, $($arg)*)
    };
}

/// Print types if TYPES debug flag is enabled.
#[macro_export]
macro_rules! debug_types {
    ($($arg:tt)*) => {
        $crate::debug_print!($crate::debug::DebugFlags::TYPES, $($arg)*)
    };
}

/// Print eval if EVAL debug flag is enabled.
#[macro_export]
macro_rules! debug_eval {
    ($($arg:tt)*) => {
        $crate::debug_print!($crate::debug::DebugFlags::EVAL, $($arg)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_flags_empty() {
        let flags = DebugFlags::empty();
        assert!(!flags.contains(DebugFlags::TOKENS));
        assert!(!flags.contains(DebugFlags::AST));
    }

    #[test]
    fn test_debug_flags_parse_single() {
        let flags = DebugFlags::parse("tokens");
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(!flags.contains(DebugFlags::AST));
    }

    #[test]
    fn test_debug_flags_parse_multiple() {
        let flags = DebugFlags::parse("tokens,ast,types");
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));
        assert!(flags.contains(DebugFlags::TYPES));
        assert!(!flags.contains(DebugFlags::EVAL));
    }

    #[test]
    fn test_debug_flags_parse_all() {
        let flags = DebugFlags::parse("all");
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));
        assert!(flags.contains(DebugFlags::TYPES));
        assert!(flags.contains(DebugFlags::EVAL));
        assert!(flags.contains(DebugFlags::IMPORTS));
        assert!(flags.contains(DebugFlags::PATTERNS));
    }

    #[test]
    fn test_debug_flags_parse_case_insensitive() {
        let flags = DebugFlags::parse("TOKENS,Ast,TyPeS");
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));
        assert!(flags.contains(DebugFlags::TYPES));
    }

    #[test]
    fn test_debug_flags_parse_with_spaces() {
        let flags = DebugFlags::parse("tokens , ast , types");
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));
        assert!(flags.contains(DebugFlags::TYPES));
    }

    #[test]
    fn test_debug_flags_parse_unknown() {
        let flags = DebugFlags::parse("tokens,unknown,ast");
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));
        // Unknown flags are silently ignored
    }

    #[test]
    fn test_debug_flags_default() {
        let flags = DebugFlags::default();
        assert!(flags.is_empty());
    }

    #[test]
    fn test_debug_flags_bitwise_ops() {
        let mut flags = DebugFlags::TOKENS;
        flags |= DebugFlags::AST;
        assert!(flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));

        flags -= DebugFlags::TOKENS;
        assert!(!flags.contains(DebugFlags::TOKENS));
        assert!(flags.contains(DebugFlags::AST));
    }
}
