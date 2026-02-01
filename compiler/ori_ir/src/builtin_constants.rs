//! Shared constants for built-in types.
//!
//! This module provides a single source of truth for magic constants used across
//! the compiler backends (typeck, eval, llvm). Instead of defining these constants
//! in each crate, they are centralized here to eliminate duplication and ensure
//! consistency.
//!
//! # Design
//!
//! Each built-in type with associated constants gets its own submodule:
//! - `duration`: Nanosecond conversion multipliers
//! - `size`: Byte conversion multipliers (binary 1024-based)
//! - `ordering`: Variant tag constants
//!
//! Using submodules allows for scoped imports like:
//! ```ignore
//! use ori_ir::builtin_constants::duration;
//! let ms = duration::NS_PER_MS;
//! ```

/// Duration constants for nanosecond-based time representation.
///
/// Duration values are stored as `i64` nanoseconds, allowing both positive
/// and negative values (for time differences).
pub mod duration {
    /// Nanoseconds per microsecond.
    pub const NS_PER_US: i64 = 1_000;
    /// Nanoseconds per millisecond.
    pub const NS_PER_MS: i64 = 1_000_000;
    /// Nanoseconds per second.
    pub const NS_PER_S: i64 = 1_000_000_000;
    /// Nanoseconds per minute.
    pub const NS_PER_M: i64 = 60 * NS_PER_S;
    /// Nanoseconds per hour.
    pub const NS_PER_H: i64 = 60 * NS_PER_M;

    /// Unsigned variants for formatting operations.
    pub mod unsigned {
        /// Nanoseconds per microsecond (u64).
        pub const NS_PER_US: u64 = 1_000;
        /// Nanoseconds per millisecond (u64).
        pub const NS_PER_MS: u64 = 1_000_000;
        /// Nanoseconds per second (u64).
        pub const NS_PER_S: u64 = 1_000_000_000;
        /// Nanoseconds per minute (u64).
        pub const NS_PER_M: u64 = 60 * NS_PER_S;
        /// Nanoseconds per hour (u64).
        pub const NS_PER_H: u64 = 60 * NS_PER_M;
    }
}

/// Size constants for byte-based storage representation.
///
/// Size values are stored as `u64` bytes (semantically non-negative).
/// Uses binary units (1024-based), not SI units (1000-based).
pub mod size {
    /// Bytes per kilobyte (1024).
    pub const BYTES_PER_KB: u64 = 1024;
    /// Bytes per megabyte (1024^2).
    pub const BYTES_PER_MB: u64 = 1024 * 1024;
    /// Bytes per gigabyte (1024^3).
    pub const BYTES_PER_GB: u64 = 1024 * 1024 * 1024;
    /// Bytes per terabyte (1024^4).
    pub const BYTES_PER_TB: u64 = 1024 * 1024 * 1024 * 1024;
}

/// Ordering variant tag constants.
///
/// Ordering is represented as `i8` with three variants:
/// - `LESS` (0): Left operand is less than right
/// - `EQUAL` (1): Operands are equal
/// - `GREATER` (2): Left operand is greater than right
///
/// The numeric ordering is intentional: `LESS < EQUAL < GREATER`.
pub mod ordering {
    /// Tag value for `Ordering::Less`.
    pub const LESS: i8 = 0;
    /// Tag value for `Ordering::Equal`.
    pub const EQUAL: i8 = 1;
    /// Tag value for `Ordering::Greater`.
    pub const GREATER: i8 = 2;

    /// Unsigned variants for LLVM codegen (`const_int` takes u64).
    pub mod unsigned {
        /// Tag value for `Ordering::Less` (u64).
        pub const LESS: u64 = 0;
        /// Tag value for `Ordering::Equal` (u64).
        pub const EQUAL: u64 = 1;
        /// Tag value for `Ordering::Greater` (u64).
        pub const GREATER: u64 = 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_constants() {
        assert_eq!(duration::NS_PER_US, 1_000);
        assert_eq!(duration::NS_PER_MS, 1_000_000);
        assert_eq!(duration::NS_PER_S, 1_000_000_000);
        assert_eq!(duration::NS_PER_M, 60_000_000_000);
        assert_eq!(duration::NS_PER_H, 3_600_000_000_000);
    }

    #[test]
    fn test_size_constants() {
        assert_eq!(size::BYTES_PER_KB, 1024);
        assert_eq!(size::BYTES_PER_MB, 1024 * 1024);
        assert_eq!(size::BYTES_PER_GB, 1024 * 1024 * 1024);
        assert_eq!(size::BYTES_PER_TB, 1024 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_ordering_constants() {
        assert_eq!(ordering::LESS, 0);
        assert_eq!(ordering::EQUAL, 1);
        assert_eq!(ordering::GREATER, 2);
    }
}
