//! Bitwise transfer functions for value range analysis.
//!
//! Handles: bitand, bitor, bitxor, shl, shr, bitnot.
//! Each function computes the output `ValueRange` for the corresponding
//! bitwise operation on integer intervals.

use super::shift_amount;
use super::ValueRange;
use ValueRange::{Bottom, Bounded, Top};

/// `a & b` — conservative for non-trivial ranges.
pub fn range_bitand(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            // If both non-negative, result ∈ [0, min(ah, bh)]
            if al >= 0 && bl >= 0 {
                Bounded {
                    lo: 0,
                    hi: ah.min(bh),
                }
            } else {
                Top
            }
        }
        _ => Top,
    }
}

/// `a | b` — conservative.
pub fn range_bitor(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        // Non-negative: result ∈ [max(al, bl), next_power_of_two - 1]
        // Conservative approximation: just return Top for now.
        _ => Top,
    }
}

/// `a ^ b` — conservative.
pub fn range_bitxor(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        _ => Top,
    }
}

/// `a << b` — returns Top if shift is negative or >= 64.
pub fn range_shl(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            if bl < 0 || bh >= 64 {
                return Top; // negative shift or shift >= bit width
            }
            // Conservative: check all four corner products
            let (sbl, sbh) = (shift_amount(bl), shift_amount(bh));
            let results = [
                al.checked_shl(sbl),
                al.checked_shl(sbh),
                ah.checked_shl(sbl),
                ah.checked_shl(sbh),
            ];
            let mut lo = i64::MAX;
            let mut hi = i64::MIN;
            for r in &results {
                match r {
                    Some(v) => {
                        lo = lo.min(*v);
                        hi = hi.max(*v);
                    }
                    None => return Top,
                }
            }
            Bounded { lo, hi }
        }
        _ => Top,
    }
}

/// `a >> b` — arithmetic right shift.
///
/// Monotonicity is sign-dependent: for positive values, more shift = smaller
/// result; for negative values, more shift = larger result (closer to -1).
/// Computes all 4 corners for soundness.
pub fn range_shr(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            if bl < 0 || bh >= 64 {
                return Top;
            }
            // Compute all 4 corners and take min/max for soundness.
            let sbl = shift_amount(bl);
            let sbh = shift_amount(bh);
            let corners = [al >> sbl, al >> sbh, ah >> sbl, ah >> sbh];
            let lo = corners.iter().copied().min().unwrap_or(al);
            let hi = corners.iter().copied().max().unwrap_or(ah);
            Bounded { lo, hi }
        }
        _ => Top,
    }
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
