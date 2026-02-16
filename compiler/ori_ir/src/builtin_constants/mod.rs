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
//! - `iterator`: Internal method names for type-directed specialization
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
/// Uses SI units (1000-based): 1kb = 1000 bytes, 1mb = 1,000,000 bytes, etc.
/// For exact powers of 1024, use explicit byte counts: `1024b`, `1048576b`.
pub mod size {
    /// Bytes per kilobyte (1000, SI units).
    pub const BYTES_PER_KB: u64 = 1000;
    /// Bytes per megabyte (1000^2 = 1,000,000, SI units).
    pub const BYTES_PER_MB: u64 = 1_000_000;
    /// Bytes per gigabyte (1000^3 = 1,000,000,000, SI units).
    pub const BYTES_PER_GB: u64 = 1_000_000_000;
    /// Bytes per terabyte (1000^4 = 1,000,000,000,000, SI units).
    pub const BYTES_PER_TB: u64 = 1_000_000_000_000;
}

/// Internal method names injected by canonicalization.
///
/// These names are rewritten by the canonicalizer during type-directed
/// specialization and consumed by the evaluator's method resolver. They
/// are not user-facing API — users write `collect()`, and the canonicalizer
/// rewrites to `__collect_set` when the target type is `Set<T>`.
pub mod iterator {
    /// Internal method name for type-directed `collect()` → `Set<T>`.
    pub const COLLECT_SET_METHOD: &str = "__collect_set";
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
mod tests;
