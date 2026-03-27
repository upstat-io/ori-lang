//! Arithmetic transfer functions for value range analysis.
//!
//! Handles: add, sub, mul, div, mod, `floor_div`, neg, abs.
//! Each function computes the output `ValueRange` for the corresponding
//! arithmetic operation on integer intervals.

use super::ValueRange;
use ValueRange::{Bottom, Bounded, Top};

/// `a + b` with checked overflow.
pub fn range_add(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            match (al.checked_add(bl), ah.checked_add(bh)) {
                (Some(lo), Some(hi)) => Bounded { lo, hi },
                _ => Top,
            }
        }
        _ => Top,
    }
}

/// `a - b` with checked overflow.
pub fn range_sub(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            // a - b: lo = a.lo - b.hi, hi = a.hi - b.lo
            match (al.checked_sub(bh), ah.checked_sub(bl)) {
                (Some(lo), Some(hi)) => Bounded { lo, hi },
                _ => Top,
            }
        }
        _ => Top,
    }
}

/// `a * b` — must consider all four quadrant combinations.
pub fn range_mul(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            // All four products may be the min or max.
            let products = [
                al.checked_mul(bl),
                al.checked_mul(bh),
                ah.checked_mul(bl),
                ah.checked_mul(bh),
            ];
            let mut lo = i64::MAX;
            let mut hi = i64::MIN;
            for p in &products {
                match p {
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

/// `a / b` — returns Top if divisor spans zero or any corner overflows.
pub fn range_div(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            // Division by zero possible → Top
            if bl <= 0 && bh >= 0 {
                return Top;
            }
            // All four quotients — checked to handle i64::MIN / -1 overflow.
            let quotients = [
                al.checked_div(bl),
                al.checked_div(bh),
                ah.checked_div(bl),
                ah.checked_div(bh),
            ];
            let mut lo = i64::MAX;
            let mut hi = i64::MIN;
            for q in &quotients {
                match q {
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

/// `a % b` — result bounded by `|b| - 1`.
pub fn range_mod(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (_, Bounded { lo: bl, hi: bh }) => {
            if bl <= 0 && bh >= 0 {
                return Top; // divisor spans zero
            }
            // |result| < max(|bl|, |bh|)
            let max_abs = bl.unsigned_abs().max(bh.unsigned_abs());
            if let Ok(bound) = i64::try_from(max_abs - 1) {
                Bounded {
                    lo: -bound,
                    hi: bound,
                }
            } else {
                Top
            }
        }
        _ => Top,
    }
}

/// Checked floor division for `i64`: `a / b` rounded toward negative infinity.
/// Returns `None` on division by zero or `i64::MIN / -1` overflow.
fn checked_floor_div(a: i64, b: i64) -> Option<i64> {
    let div = a.checked_div(b)?;
    let rem = a.checked_rem(b)?;
    if rem != 0 && (a < 0) != (b < 0) {
        div.checked_sub(1)
    } else {
        Some(div)
    }
}

/// Floor division: `a div b` rounded toward negative infinity.
///
/// Ori's `div` operator uses floor division, which differs from truncating
/// division for mixed-sign operands: `-7 div 2 == -4` (not `-3`).
/// Computes all 4 corners using floor semantics, then takes min/max.
pub fn range_floordiv(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            // Division by zero possible → Top
            if bl <= 0 && bh >= 0 {
                return Top;
            }
            // All four corners — checked for overflow (i64::MIN div -1).
            let quotients = [
                checked_floor_div(al, bl),
                checked_floor_div(al, bh),
                checked_floor_div(ah, bl),
                checked_floor_div(ah, bh),
            ];
            let mut lo = i64::MAX;
            let mut hi = i64::MIN;
            for q in &quotients {
                match q {
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

/// Unary negation: `-a`.
pub fn range_neg(a: ValueRange) -> ValueRange {
    match a {
        Bottom => Bottom,
        Bounded { lo, hi } => {
            // i64::MIN.checked_neg() = None
            match (hi.checked_neg(), lo.checked_neg()) {
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

/// `abs(a)` — non-negative, but `i64::MIN.abs()` overflows.
pub fn range_abs(a: ValueRange) -> ValueRange {
    match a {
        Bottom => Bottom,
        Bounded { lo, hi } => {
            if lo == i64::MIN {
                return Top; // i64::MIN.abs() overflows
            }
            if lo >= 0 {
                // All positive — identity
                Bounded { lo, hi }
            } else if hi < 0 {
                // All negative — flip
                Bounded {
                    lo: hi.abs(),
                    hi: lo.abs(),
                }
            } else {
                // Spans zero
                Bounded {
                    lo: 0,
                    hi: lo.abs().max(hi),
                }
            }
        }
        Top => Top,
    }
}
