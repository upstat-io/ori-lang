//! Bitwise transfer functions for value range analysis.
//!
//! Handles: bitand, bitor, bitxor, shl, shr, bitnot.
//! Each function computes the output `ValueRange` for the corresponding
//! bitwise operation on integer intervals.

use super::{binary_transfer, four_corner_fold, shift_amount, ValueRange};
use ValueRange::{Bottom, Bounded, Top};

/// `a & b` — conservative for non-trivial ranges.
pub fn range_bitand(a: ValueRange, b: ValueRange) -> ValueRange {
    binary_transfer(a, b, |al, ah, bl, bh| {
        // If both non-negative, result in [0, min(ah, bh)]
        if al >= 0 && bl >= 0 {
            Bounded {
                lo: 0,
                hi: ah.min(bh),
            }
        } else {
            Top
        }
    })
}

/// `a | b` — conservative.
pub fn range_bitor(a: ValueRange, b: ValueRange) -> ValueRange {
    binary_transfer(a, b, |_al, _ah, _bl, _bh| {
        // Non-negative: result in [max(al, bl), next_power_of_two - 1]
        // Conservative approximation: just return Top for now.
        Top
    })
}

/// `a ^ b` — conservative.
pub fn range_bitxor(a: ValueRange, b: ValueRange) -> ValueRange {
    binary_transfer(a, b, |_al, _ah, _bl, _bh| Top)
}

/// `a << b` — returns Top if shift is negative or >= 64.
pub fn range_shl(a: ValueRange, b: ValueRange) -> ValueRange {
    binary_transfer(a, b, |al, ah, bl, bh| {
        if bl < 0 || bh >= 64 {
            return Top; // negative shift or shift >= bit width
        }
        four_corner_fold(al, ah, bl, bh, |a, b| a.checked_shl(shift_amount(b)))
    })
}

/// `a >> b` — arithmetic right shift.
///
/// Monotonicity is sign-dependent: for positive values, more shift = smaller
/// result; for negative values, more shift = larger result (closer to -1).
/// Computes all 4 corners for soundness.
pub fn range_shr(a: ValueRange, b: ValueRange) -> ValueRange {
    binary_transfer(a, b, |al, ah, bl, bh| {
        if bl < 0 || bh >= 64 {
            return Top;
        }
        // Arithmetic right shift never overflows for valid shift amounts,
        // so we use an infallible `Some(a >> shift)` wrapper.
        four_corner_fold(al, ah, bl, bh, |a, b| Some(a >> shift_amount(b)))
    })
}

/// `~a` — bitwise complement.
pub fn range_bitnot(a: ValueRange) -> ValueRange {
    match a {
        Bottom => Bottom,
        Bounded { lo, hi } => {
            // ~x = -x - 1, so ~[lo, hi] = [-hi-1, -lo-1]
            // Use checked_neg() to avoid overflow on i64::MIN (same pattern as range_neg).
            let new_lo = hi.checked_neg().and_then(|v| v.checked_sub(1));
            let new_hi = lo.checked_neg().and_then(|v| v.checked_sub(1));
            match (new_lo, new_hi) {
                (Some(new_lo), Some(new_hi)) => Bounded {
                    lo: new_lo,
                    hi: new_hi,
                },
                _ => Top,
            }
        }
        Top => Top,
    }
}
