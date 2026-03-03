//! Duration and size unit types for literal tokens.

use std::fmt;

/// Duration unit for duration literals.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum DurationUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl DurationUnit {
    /// All duration unit variants.
    ///
    /// Used by tests to verify that suffix detection covers every variant.
    pub const fn all() -> [DurationUnit; 6] {
        [
            DurationUnit::Nanoseconds,
            DurationUnit::Microseconds,
            DurationUnit::Milliseconds,
            DurationUnit::Seconds,
            DurationUnit::Minutes,
            DurationUnit::Hours,
        ]
    }

    /// Nanosecond multiplier for this unit.
    ///
    /// Used by the lexer to convert decimal duration literals to nanoseconds
    /// via integer arithmetic (no floats involved).
    #[inline]
    pub fn nanos_multiplier(self) -> u64 {
        match self {
            DurationUnit::Nanoseconds => 1,
            DurationUnit::Microseconds => 1_000,
            DurationUnit::Milliseconds => 1_000_000,
            DurationUnit::Seconds => 1_000_000_000,
            DurationUnit::Minutes => 60_000_000_000,
            DurationUnit::Hours => 3_600_000_000_000,
        }
    }

    /// Convert value to nanoseconds.
    ///
    /// For `Nanoseconds` unit (multiplier = 1), the value is a bit-pattern
    /// of a signed i64 (negative durations from `const_fold` use `cast_unsigned()`),
    /// so it is reinterpreted via `cast_signed()` without bounds checking.
    ///
    /// For other units, returns `None` if the multiplication overflows `u64`
    /// or the result exceeds `i64::MAX`.
    #[inline]
    pub fn to_nanos(self, value: u64) -> Option<i64> {
        if self.nanos_multiplier() == 1 {
            // Nanoseconds: value is a raw i64 bit-pattern (may be negative).
            Some(value.cast_signed())
        } else {
            let ns = value.checked_mul(self.nanos_multiplier())?;
            i64::try_from(ns).ok()
        }
    }

    /// Get the suffix string.
    #[inline]
    pub fn suffix(self) -> &'static str {
        match self {
            DurationUnit::Nanoseconds => "ns",
            DurationUnit::Microseconds => "us",
            DurationUnit::Milliseconds => "ms",
            DurationUnit::Seconds => "s",
            DurationUnit::Minutes => "m",
            DurationUnit::Hours => "h",
        }
    }
}

impl fmt::Debug for DurationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// Size unit for size literals.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl SizeUnit {
    /// All size unit variants.
    ///
    /// Used by tests to verify that suffix detection covers every variant.
    pub const fn all() -> [SizeUnit; 5] {
        [
            SizeUnit::Bytes,
            SizeUnit::Kilobytes,
            SizeUnit::Megabytes,
            SizeUnit::Gigabytes,
            SizeUnit::Terabytes,
        ]
    }

    /// Byte multiplier for this unit (SI, powers of 1000).
    ///
    /// Used by the lexer to convert decimal size literals to bytes
    /// via integer arithmetic (no floats involved).
    #[inline]
    pub fn bytes_multiplier(self) -> u64 {
        match self {
            SizeUnit::Bytes => 1,
            SizeUnit::Kilobytes => 1_000,
            SizeUnit::Megabytes => 1_000_000,
            SizeUnit::Gigabytes => 1_000_000_000,
            SizeUnit::Terabytes => 1_000_000_000_000,
        }
    }

    /// Convert value to bytes using SI units (powers of 1000).
    ///
    /// SI units: 1kb = 1000 bytes, 1mb = 1,000,000 bytes, etc.
    /// For exact powers of 1024, use explicit byte counts: `1024b`, `1048576b`.
    ///
    /// Returns `None` if the multiplication overflows `u64`.
    #[inline]
    pub fn to_bytes(self, value: u64) -> Option<u64> {
        value.checked_mul(self.bytes_multiplier())
    }

    /// Get the suffix string.
    #[inline]
    pub fn suffix(self) -> &'static str {
        match self {
            SizeUnit::Bytes => "b",
            SizeUnit::Kilobytes => "kb",
            SizeUnit::Megabytes => "mb",
            SizeUnit::Gigabytes => "gb",
            SizeUnit::Terabytes => "tb",
        }
    }
}

impl fmt::Debug for SizeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}
