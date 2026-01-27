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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_and_extraction() {
        let n = ScalarInt::new(42);
        assert_eq!(n.raw(), 42);
    }

    #[test]
    fn constants() {
        assert_eq!(ScalarInt::ZERO.raw(), 0);
        assert_eq!(ScalarInt::ONE.raw(), 1);
        assert_eq!(ScalarInt::MIN.raw(), i64::MIN);
        assert_eq!(ScalarInt::MAX.raw(), i64::MAX);
    }

    #[test]
    fn is_zero() {
        assert!(ScalarInt::ZERO.is_zero());
        assert!(!ScalarInt::ONE.is_zero());
        assert!(!ScalarInt::new(-1).is_zero());
    }

    #[test]
    fn checked_add_basic() {
        assert_eq!(
            ScalarInt::new(2).checked_add(ScalarInt::new(3)),
            Some(ScalarInt::new(5))
        );
    }

    #[test]
    fn checked_add_overflow() {
        assert_eq!(ScalarInt::MAX.checked_add(ScalarInt::ONE), None);
    }

    #[test]
    fn checked_sub_basic() {
        assert_eq!(
            ScalarInt::new(5).checked_sub(ScalarInt::new(3)),
            Some(ScalarInt::new(2))
        );
    }

    #[test]
    fn checked_sub_overflow() {
        assert_eq!(ScalarInt::MIN.checked_sub(ScalarInt::ONE), None);
    }

    #[test]
    fn checked_mul_basic() {
        assert_eq!(
            ScalarInt::new(3).checked_mul(ScalarInt::new(4)),
            Some(ScalarInt::new(12))
        );
    }

    #[test]
    fn checked_mul_overflow() {
        assert_eq!(ScalarInt::MAX.checked_mul(ScalarInt::new(2)), None);
    }

    #[test]
    fn checked_div_basic() {
        assert_eq!(
            ScalarInt::new(10).checked_div(ScalarInt::new(3)),
            Some(ScalarInt::new(3))
        );
    }

    #[test]
    fn checked_div_by_zero() {
        assert_eq!(ScalarInt::new(10).checked_div(ScalarInt::ZERO), None);
    }

    #[test]
    fn checked_div_min_neg_one() {
        assert_eq!(ScalarInt::MIN.checked_div(ScalarInt::new(-1)), None);
    }

    #[test]
    fn checked_rem_basic() {
        assert_eq!(
            ScalarInt::new(10).checked_rem(ScalarInt::new(3)),
            Some(ScalarInt::new(1))
        );
    }

    #[test]
    fn checked_rem_by_zero() {
        assert_eq!(ScalarInt::new(10).checked_rem(ScalarInt::ZERO), None);
    }

    #[test]
    fn checked_neg_basic() {
        assert_eq!(ScalarInt::new(5).checked_neg(), Some(ScalarInt::new(-5)));
        assert_eq!(ScalarInt::new(-5).checked_neg(), Some(ScalarInt::new(5)));
        assert_eq!(ScalarInt::ZERO.checked_neg(), Some(ScalarInt::ZERO));
    }

    #[test]
    fn checked_neg_min_overflow() {
        assert_eq!(ScalarInt::MIN.checked_neg(), None);
    }

    #[test]
    fn checked_floor_div_positive() {
        assert_eq!(
            ScalarInt::new(7).checked_floor_div(ScalarInt::new(2)),
            Some(ScalarInt::new(3))
        );
    }

    #[test]
    fn checked_floor_div_negative_numerator() {
        assert_eq!(
            ScalarInt::new(-7).checked_floor_div(ScalarInt::new(2)),
            Some(ScalarInt::new(-4))
        );
    }

    #[test]
    fn checked_floor_div_negative_denominator() {
        assert_eq!(
            ScalarInt::new(7).checked_floor_div(ScalarInt::new(-2)),
            Some(ScalarInt::new(-4))
        );
    }

    #[test]
    fn checked_floor_div_both_negative() {
        assert_eq!(
            ScalarInt::new(-7).checked_floor_div(ScalarInt::new(-2)),
            Some(ScalarInt::new(3))
        );
    }

    #[test]
    fn checked_floor_div_exact() {
        assert_eq!(
            ScalarInt::new(6).checked_floor_div(ScalarInt::new(2)),
            Some(ScalarInt::new(3))
        );
    }

    #[test]
    fn checked_floor_div_by_zero() {
        assert_eq!(
            ScalarInt::new(7).checked_floor_div(ScalarInt::ZERO),
            None
        );
    }

    #[test]
    fn checked_floor_div_min_neg_one() {
        assert_eq!(
            ScalarInt::MIN.checked_floor_div(ScalarInt::new(-1)),
            None
        );
    }

    #[test]
    fn checked_shl_basic() {
        assert_eq!(
            ScalarInt::new(1).checked_shl(ScalarInt::new(3)),
            Some(ScalarInt::new(8))
        );
    }

    #[test]
    fn checked_shl_negative_shift() {
        assert_eq!(ScalarInt::new(1).checked_shl(ScalarInt::new(-1)), None);
    }

    #[test]
    fn checked_shl_too_large() {
        assert_eq!(ScalarInt::new(1).checked_shl(ScalarInt::new(64)), None);
    }

    #[test]
    fn checked_shr_basic() {
        assert_eq!(
            ScalarInt::new(8).checked_shr(ScalarInt::new(3)),
            Some(ScalarInt::new(1))
        );
    }

    #[test]
    fn checked_shr_negative_shift() {
        assert_eq!(ScalarInt::new(8).checked_shr(ScalarInt::new(-1)), None);
    }

    #[test]
    fn bitwise_and() {
        assert_eq!(
            ScalarInt::new(0b1010) & ScalarInt::new(0b1100),
            ScalarInt::new(0b1000)
        );
    }

    #[test]
    fn bitwise_or() {
        assert_eq!(
            ScalarInt::new(0b1010) | ScalarInt::new(0b1100),
            ScalarInt::new(0b1110)
        );
    }

    #[test]
    fn bitwise_xor() {
        assert_eq!(
            ScalarInt::new(0b1010) ^ ScalarInt::new(0b1100),
            ScalarInt::new(0b0110)
        );
    }

    #[test]
    fn bitwise_not() {
        assert_eq!(!ScalarInt::new(0), ScalarInt::new(-1));
        assert_eq!(!ScalarInt::new(-1), ScalarInt::new(0));
    }

    #[test]
    fn from_i64() {
        let n: ScalarInt = 42i64.into();
        assert_eq!(n.raw(), 42);
    }

    #[test]
    fn into_i64() {
        let n = ScalarInt::new(42);
        let raw: i64 = n.into();
        assert_eq!(raw, 42);
    }

    #[test]
    fn display_formatting() {
        assert_eq!(format!("{}", ScalarInt::new(42)), "42");
        assert_eq!(format!("{}", ScalarInt::new(-5)), "-5");
        assert_eq!(format!("{}", ScalarInt::ZERO), "0");
    }

    #[test]
    fn debug_formatting() {
        assert_eq!(format!("{:?}", ScalarInt::new(42)), "42");
    }

    #[test]
    fn hash_consistency() {
        use std::collections::hash_map::DefaultHasher;

        fn hash_val(n: ScalarInt) -> u64 {
            let mut h = DefaultHasher::new();
            n.hash(&mut h);
            h.finish()
        }

        assert_eq!(hash_val(ScalarInt::new(42)), hash_val(ScalarInt::new(42)));
        assert_ne!(hash_val(ScalarInt::new(42)), hash_val(ScalarInt::new(43)));
    }

    #[test]
    fn ordering() {
        assert!(ScalarInt::new(1) < ScalarInt::new(2));
        assert!(ScalarInt::new(-1) < ScalarInt::ZERO);
        assert!(ScalarInt::MIN < ScalarInt::MAX);
    }

    #[test]
    fn memory_size() {
        assert_eq!(
            std::mem::size_of::<ScalarInt>(),
            std::mem::size_of::<i64>()
        );
    }

    // =========================================================================
    // Addition boundary tests
    // =========================================================================

    #[test]
    fn checked_add_max_plus_max_overflows() {
        assert_eq!(ScalarInt::MAX.checked_add(ScalarInt::MAX), None);
    }

    #[test]
    fn checked_add_min_plus_neg1_overflows() {
        assert_eq!(ScalarInt::MIN.checked_add(ScalarInt::new(-1)), None);
    }

    #[test]
    fn checked_add_min_plus_min_overflows() {
        assert_eq!(ScalarInt::MIN.checked_add(ScalarInt::MIN), None);
    }

    #[test]
    fn checked_add_min_plus_max() {
        assert_eq!(
            ScalarInt::MIN.checked_add(ScalarInt::MAX),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_add_identity_at_max() {
        assert_eq!(
            ScalarInt::MAX.checked_add(ScalarInt::ZERO),
            Some(ScalarInt::MAX)
        );
    }

    #[test]
    fn checked_add_identity_at_min() {
        assert_eq!(
            ScalarInt::MIN.checked_add(ScalarInt::ZERO),
            Some(ScalarInt::MIN)
        );
    }

    #[test]
    fn checked_add_near_boundary_valid() {
        // MAX - 1 + 1 = MAX (valid)
        assert_eq!(
            ScalarInt::new(i64::MAX - 1).checked_add(ScalarInt::ONE),
            Some(ScalarInt::MAX)
        );
    }

    #[test]
    fn checked_add_near_boundary_overflow() {
        // MAX - 1 + 2 overflows
        assert_eq!(
            ScalarInt::new(i64::MAX - 1).checked_add(ScalarInt::new(2)),
            None
        );
    }

    #[test]
    fn checked_add_commutativity() {
        let a = ScalarInt::new(123);
        let b = ScalarInt::new(-456);
        assert_eq!(a.checked_add(b), b.checked_add(a));
    }

    // =========================================================================
    // Subtraction boundary tests
    // =========================================================================

    #[test]
    fn checked_sub_max_minus_neg1_overflows() {
        assert_eq!(ScalarInt::MAX.checked_sub(ScalarInt::new(-1)), None);
    }

    #[test]
    fn checked_sub_zero_minus_min_overflows() {
        assert_eq!(ScalarInt::ZERO.checked_sub(ScalarInt::MIN), None);
    }

    #[test]
    fn checked_sub_min_minus_max() {
        // MIN - MAX overflows (would be -2^63 - (2^63-1) = -(2^64-1))
        assert_eq!(ScalarInt::MIN.checked_sub(ScalarInt::MAX), None);
    }

    #[test]
    fn checked_sub_self_cancellation_at_max() {
        assert_eq!(
            ScalarInt::MAX.checked_sub(ScalarInt::MAX),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_sub_self_cancellation_at_min() {
        assert_eq!(
            ScalarInt::MIN.checked_sub(ScalarInt::MIN),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_sub_near_boundary_valid() {
        // MIN + 1 - 1 = MIN (valid)
        assert_eq!(
            ScalarInt::new(i64::MIN + 1).checked_sub(ScalarInt::ONE),
            Some(ScalarInt::MIN)
        );
    }

    #[test]
    fn checked_sub_near_boundary_overflow() {
        // MIN + 1 - 2 overflows
        assert_eq!(
            ScalarInt::new(i64::MIN + 1).checked_sub(ScalarInt::new(2)),
            None
        );
    }

    // =========================================================================
    // Multiplication boundary tests
    // =========================================================================

    #[test]
    fn checked_mul_min_times_neg1_overflows() {
        assert_eq!(ScalarInt::MIN.checked_mul(ScalarInt::new(-1)), None);
    }

    #[test]
    fn checked_mul_min_times_2_overflows() {
        assert_eq!(ScalarInt::MIN.checked_mul(ScalarInt::new(2)), None);
    }

    #[test]
    fn checked_mul_max_times_max_overflows() {
        assert_eq!(ScalarInt::MAX.checked_mul(ScalarInt::MAX), None);
    }

    #[test]
    fn checked_mul_min_times_zero() {
        assert_eq!(
            ScalarInt::MIN.checked_mul(ScalarInt::ZERO),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_mul_max_times_zero() {
        assert_eq!(
            ScalarInt::MAX.checked_mul(ScalarInt::ZERO),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_mul_identity_at_max() {
        assert_eq!(
            ScalarInt::MAX.checked_mul(ScalarInt::ONE),
            Some(ScalarInt::MAX)
        );
    }

    #[test]
    fn checked_mul_identity_at_min() {
        assert_eq!(
            ScalarInt::MIN.checked_mul(ScalarInt::ONE),
            Some(ScalarInt::MIN)
        );
    }

    #[test]
    fn checked_mul_commutativity() {
        let a = ScalarInt::new(123);
        let b = ScalarInt::new(-7);
        assert_eq!(a.checked_mul(b), b.checked_mul(a));
    }

    #[test]
    fn checked_mul_neg1_times_max() {
        assert_eq!(
            ScalarInt::new(-1).checked_mul(ScalarInt::MAX),
            Some(ScalarInt::new(-i64::MAX))
        );
    }

    // =========================================================================
    // Division boundary tests
    // =========================================================================

    #[test]
    fn checked_div_zero_divided_by_anything() {
        assert_eq!(
            ScalarInt::ZERO.checked_div(ScalarInt::new(42)),
            Some(ScalarInt::ZERO)
        );
        assert_eq!(
            ScalarInt::ZERO.checked_div(ScalarInt::new(-42)),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_div_self_divide() {
        assert_eq!(
            ScalarInt::new(42).checked_div(ScalarInt::new(42)),
            Some(ScalarInt::ONE)
        );
        assert_eq!(
            ScalarInt::MAX.checked_div(ScalarInt::MAX),
            Some(ScalarInt::ONE)
        );
        assert_eq!(
            ScalarInt::MIN.checked_div(ScalarInt::MIN),
            Some(ScalarInt::ONE)
        );
    }

    #[test]
    fn checked_div_one_divided_by_max() {
        assert_eq!(
            ScalarInt::ONE.checked_div(ScalarInt::MAX),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_div_max_divided_by_neg1() {
        assert_eq!(
            ScalarInt::MAX.checked_div(ScalarInt::new(-1)),
            Some(ScalarInt::new(-i64::MAX))
        );
    }

    // =========================================================================
    // Remainder boundary tests
    // =========================================================================

    #[test]
    fn checked_rem_min_mod_neg1_overflows() {
        // MIN % -1 overflows because division MIN / -1 overflows
        assert_eq!(ScalarInt::MIN.checked_rem(ScalarInt::new(-1)), None);
    }

    #[test]
    fn checked_rem_negative_numerator() {
        // -7 % 3 = -1 (sign follows numerator)
        assert_eq!(
            ScalarInt::new(-7).checked_rem(ScalarInt::new(3)),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_rem_negative_denominator() {
        // 7 % -3 = 1 (sign follows numerator)
        assert_eq!(
            ScalarInt::new(7).checked_rem(ScalarInt::new(-3)),
            Some(ScalarInt::new(1))
        );
    }

    #[test]
    fn checked_rem_both_negative() {
        // -7 % -3 = -1
        assert_eq!(
            ScalarInt::new(-7).checked_rem(ScalarInt::new(-3)),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_rem_exact_division() {
        assert_eq!(
            ScalarInt::new(6).checked_rem(ScalarInt::new(3)),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_rem_max_mod_2() {
        // MAX is odd → MAX % 2 = 1
        assert_eq!(
            ScalarInt::MAX.checked_rem(ScalarInt::new(2)),
            Some(ScalarInt::ONE)
        );
    }

    // =========================================================================
    // Floor division boundary tests
    // =========================================================================

    #[test]
    fn checked_floor_div_neg1_div_2() {
        // -1 div 2 = -1 (floor toward negative infinity)
        assert_eq!(
            ScalarInt::new(-1).checked_floor_div(ScalarInt::new(2)),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_floor_div_1_div_2() {
        assert_eq!(
            ScalarInt::new(1).checked_floor_div(ScalarInt::new(2)),
            Some(ScalarInt::ZERO)
        );
    }

    #[test]
    fn checked_floor_div_min_div_max() {
        // MIN / MAX: truncating = -1, MIN % MAX = -1 (mixed signs) → floor = -2
        assert_eq!(
            ScalarInt::MIN.checked_floor_div(ScalarInt::MAX),
            Some(ScalarInt::new(-2))
        );
    }

    #[test]
    fn checked_floor_div_max_div_min() {
        // MAX / MIN: truncating = 0, MAX % MIN = MAX (mixed signs) → floor = -1
        assert_eq!(
            ScalarInt::MAX.checked_floor_div(ScalarInt::MIN),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_floor_div_exact_negative() {
        // -6 div 3 = -2 (exact, no adjustment needed)
        assert_eq!(
            ScalarInt::new(-6).checked_floor_div(ScalarInt::new(3)),
            Some(ScalarInt::new(-2))
        );
    }

    // =========================================================================
    // Shift boundary tests
    // =========================================================================

    #[test]
    fn checked_shl_zero_shift() {
        assert_eq!(
            ScalarInt::new(42).checked_shl(ScalarInt::ZERO),
            Some(ScalarInt::new(42))
        );
    }

    #[test]
    fn checked_shl_max_shift() {
        // shift by 63 is valid
        assert_eq!(
            ScalarInt::ONE.checked_shl(ScalarInt::new(63)),
            Some(ScalarInt::MIN) // 1 << 63 = MIN (sign bit set)
        );
    }

    #[test]
    fn checked_shr_zero_shift() {
        assert_eq!(
            ScalarInt::new(42).checked_shr(ScalarInt::ZERO),
            Some(ScalarInt::new(42))
        );
    }

    #[test]
    fn checked_shr_max_shift() {
        // MIN >> 63 = -1 (arithmetic shift, sign-extends)
        assert_eq!(
            ScalarInt::MIN.checked_shr(ScalarInt::new(63)),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_shr_neg1_any_shift() {
        // -1 >> anything = -1 (all bits set, arithmetic shift)
        assert_eq!(
            ScalarInt::new(-1).checked_shr(ScalarInt::new(1)),
            Some(ScalarInt::new(-1))
        );
        assert_eq!(
            ScalarInt::new(-1).checked_shr(ScalarInt::new(32)),
            Some(ScalarInt::new(-1))
        );
        assert_eq!(
            ScalarInt::new(-1).checked_shr(ScalarInt::new(63)),
            Some(ScalarInt::new(-1))
        );
    }

    #[test]
    fn checked_shl_max_wraps() {
        // MAX << 1 wraps (checked_shl only guards shift amount, not bit loss)
        // 0x7FFF...FFFE = -2
        assert_eq!(
            ScalarInt::MAX.checked_shl(ScalarInt::ONE),
            Some(ScalarInt::new(-2))
        );
    }

    #[test]
    fn checked_shr_too_large() {
        assert_eq!(ScalarInt::new(8).checked_shr(ScalarInt::new(64)), None);
    }

    // =========================================================================
    // Bitwise boundary tests
    // =========================================================================

    #[test]
    fn bitwise_and_max_min() {
        assert_eq!(ScalarInt::MAX & ScalarInt::MIN, ScalarInt::ZERO);
    }

    #[test]
    fn bitwise_or_max_min() {
        assert_eq!(ScalarInt::MAX | ScalarInt::MIN, ScalarInt::new(-1));
    }

    #[test]
    fn bitwise_xor_self() {
        assert_eq!(ScalarInt::MAX ^ ScalarInt::MAX, ScalarInt::ZERO);
        assert_eq!(ScalarInt::MIN ^ ScalarInt::MIN, ScalarInt::ZERO);
    }

    #[test]
    fn bitwise_not_max() {
        assert_eq!(!ScalarInt::MAX, ScalarInt::MIN);
    }

    #[test]
    fn bitwise_not_min() {
        assert_eq!(!ScalarInt::MIN, ScalarInt::MAX);
    }

    // =========================================================================
    // Negation boundary tests
    // =========================================================================

    #[test]
    fn checked_neg_max() {
        assert_eq!(
            ScalarInt::MAX.checked_neg(),
            Some(ScalarInt::new(-i64::MAX))
        );
    }

    #[test]
    fn checked_neg_near_min() {
        // -(MIN + 1) = MAX
        assert_eq!(
            ScalarInt::new(i64::MIN + 1).checked_neg(),
            Some(ScalarInt::MAX)
        );
    }

    #[test]
    #[expect(clippy::unwrap_used, reason = "test verifies double negation roundtrip")]
    fn checked_neg_double_negation() {
        let n = ScalarInt::new(42);
        assert_eq!(n.checked_neg().unwrap().checked_neg(), Some(n));
    }

    #[test]
    #[expect(clippy::unwrap_used, reason = "test verifies double negation roundtrip at MAX")]
    fn checked_neg_double_negation_at_max() {
        assert_eq!(
            ScalarInt::MAX.checked_neg().unwrap().checked_neg(),
            Some(ScalarInt::MAX)
        );
    }

    // =========================================================================
    // Property tests
    // =========================================================================

    #[test]
    #[expect(clippy::unwrap_used, reason = "test verifies add/sub inverse property")]
    fn add_sub_inverse() {
        let a = ScalarInt::new(100);
        let b = ScalarInt::new(50);
        assert_eq!(
            a.checked_add(b).unwrap().checked_sub(b),
            Some(a)
        );
    }

    #[test]
    #[expect(clippy::unwrap_used, reason = "test verifies mul/div inverse property (a == q*b + r)")]
    fn mul_div_inverse() {
        let a = ScalarInt::new(100);
        let b = ScalarInt::new(7);
        let q = a.checked_div(b).unwrap();
        let r = a.checked_rem(b).unwrap();
        // a == q*b + r
        assert_eq!(
            q.checked_mul(b).unwrap().checked_add(r),
            Some(a)
        );
    }

    #[test]
    fn zero_identity_add() {
        for val in [ScalarInt::MIN, ScalarInt::new(-1), ScalarInt::ZERO, ScalarInt::ONE, ScalarInt::MAX] {
            assert_eq!(val.checked_add(ScalarInt::ZERO), Some(val));
        }
    }

    #[test]
    fn one_identity_mul() {
        for val in [ScalarInt::MIN, ScalarInt::new(-1), ScalarInt::ZERO, ScalarInt::ONE, ScalarInt::MAX] {
            assert_eq!(val.checked_mul(ScalarInt::ONE), Some(val));
        }
    }

    #[test]
    fn multiply_by_zero() {
        for val in [ScalarInt::MIN, ScalarInt::new(-1), ScalarInt::ONE, ScalarInt::MAX] {
            assert_eq!(val.checked_mul(ScalarInt::ZERO), Some(ScalarInt::ZERO));
        }
    }

    #[test]
    fn self_subtraction() {
        for val in [ScalarInt::MIN, ScalarInt::new(-1), ScalarInt::ZERO, ScalarInt::ONE, ScalarInt::MAX] {
            assert_eq!(val.checked_sub(val), Some(ScalarInt::ZERO));
        }
    }
}
