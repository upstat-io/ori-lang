//! Integer newtype that prevents unchecked arithmetic.
//!
//! `ScalarInt` wraps `i64` and intentionally does NOT implement `Add`, `Sub`,
//! `Mul`, `Div`, `Rem`, or `Neg`. All arithmetic must go through checked methods
//! that return `Option<ScalarInt>`, making integer overflow impossible to miss.
//!
//! Bitwise traits (`BitAnd`, `BitOr`, `BitXor`, `Not`) are implemented because
//! they cannot overflow.
//!
//! This mirrors the approach used by the Rust compiler's `ScalarInt`.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{BitAnd, BitOr, BitXor, Not};

/// A 64-bit signed integer that prevents unchecked arithmetic.
///
/// All arithmetic operations require explicit checked methods.
/// Using `+`, `-`, `*`, `/` directly on `ScalarInt` is a compile error.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct ScalarInt(i64);

impl ScalarInt {
    /// The zero value.
    pub const ZERO: Self = Self(0);

    /// The one value.
    pub const ONE: Self = Self(1);

    /// The minimum value (`i64::MIN`).
    pub const MIN: Self = Self(i64::MIN);

    /// The maximum value (`i64::MAX`).
    pub const MAX: Self = Self(i64::MAX);

    /// Create a new `ScalarInt` from a raw `i64`.
    #[inline]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// Extract the raw `i64` value.
    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Check if this value is zero.
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Checked addition. Returns `None` on overflow.
    #[inline]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked subtraction. Returns `None` on overflow.
    #[inline]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked multiplication. Returns `None` on overflow.
    #[inline]
    pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
        match self.0.checked_mul(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked division. Returns `None` on division by zero or overflow
    /// (e.g. `i64::MIN / -1`).
    #[inline]
    pub const fn checked_div(self, rhs: Self) -> Option<Self> {
        match self.0.checked_div(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked remainder. Returns `None` on division by zero or overflow.
    #[inline]
    pub const fn checked_rem(self, rhs: Self) -> Option<Self> {
        match self.0.checked_rem(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked negation. Returns `None` on overflow (`i64::MIN`).
    #[inline]
    pub const fn checked_neg(self) -> Option<Self> {
        match self.0.checked_neg() {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked floor division. Returns `None` on division by zero or overflow.
    ///
    /// Floor division rounds towards negative infinity, unlike truncating
    /// division which rounds towards zero. For example:
    /// - `7.checked_floor_div(2)` = `Some(3)` (same as truncating)
    /// - `(-7).checked_floor_div(2)` = `Some(-4)` (not -3)
    pub fn checked_floor_div(self, rhs: Self) -> Option<Self> {
        let div = self.0.checked_div(rhs.0)?;
        let rem = self.0.checked_rem(rhs.0)?;
        if rem != 0 && (self.0 < 0) != (rhs.0 < 0) {
            div.checked_sub(1).map(Self)
        } else {
            Some(Self(div))
        }
    }

    /// Checked left shift. Returns `None` if shift amount is negative or >= 64.
    #[inline]
    pub fn checked_shl(self, rhs: Self) -> Option<Self> {
        let shift = u32::try_from(rhs.0).ok()?;
        if shift >= 64 {
            return None;
        }
        self.0.checked_shl(shift).map(Self)
    }

    /// Checked right shift. Returns `None` if shift amount is negative or >= 64.
    #[inline]
    pub fn checked_shr(self, rhs: Self) -> Option<Self> {
        let shift = u32::try_from(rhs.0).ok()?;
        if shift >= 64 {
            return None;
        }
        self.0.checked_shr(shift).map(Self)
    }
}

// Bitwise Traits (cannot overflow)

impl BitAnd for ScalarInt {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for ScalarInt {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitXor for ScalarInt {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl Not for ScalarInt {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

// Conversions

impl From<i64> for ScalarInt {
    #[inline]
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<ScalarInt> for i64 {
    #[inline]
    fn from(value: ScalarInt) -> Self {
        value.0
    }
}

// Formatting

impl fmt::Debug for ScalarInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ScalarInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Hashing

impl Hash for ScalarInt {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
