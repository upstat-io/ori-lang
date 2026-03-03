//! Arithmetic constant folding for binary and unary operations.
//!
//! Handles int, float, duration, size, boolean, string, and bitwise
//! operations on compile-time constant values.

use ori_ir::canon::ConstValue;
use ori_ir::{BinaryOp, DurationUnit, SizeUnit, UnaryOp};

// Binary Folding

/// Validate that a Size result fits in i64 (LLVM codegen casts bytes to i64).
///
/// Returns `Some(ConstValue::Size)` if valid, `None` if the value exceeds
/// `i64::MAX` — deferring the overflow to runtime.
fn checked_size_bytes(bytes: u64) -> Option<ConstValue> {
    if i64::try_from(bytes).is_ok() {
        Some(ConstValue::Size {
            value: bytes,
            unit: SizeUnit::Bytes,
        })
    } else {
        None
    }
}

/// Evaluate a binary operation on two constant values.
///
/// Returns `None` if the operation would cause undefined behavior
/// (division by zero, integer overflow) — these are deferred to runtime.
pub(super) fn fold_binary(
    op: BinaryOp,
    left: &ConstValue,
    right: &ConstValue,
) -> Option<ConstValue> {
    // Match by reference — all inner data is Copy, so no cloning needed.
    match (op, left, right) {
        // Integer arithmetic (with overflow detection).
        (BinaryOp::Add, ConstValue::Int(a), ConstValue::Int(b)) => {
            a.checked_add(*b).map(ConstValue::Int)
        }
        (BinaryOp::Sub, ConstValue::Int(a), ConstValue::Int(b)) => {
            a.checked_sub(*b).map(ConstValue::Int)
        }
        (BinaryOp::Mul, ConstValue::Int(a), ConstValue::Int(b)) => {
            a.checked_mul(*b).map(ConstValue::Int)
        }
        // Division-by-zero: defer to runtime.
        (
            BinaryOp::Div | BinaryOp::Mod | BinaryOp::FloorDiv,
            ConstValue::Int(_),
            ConstValue::Int(0),
        ) => None,
        (BinaryOp::Div, ConstValue::Int(a), ConstValue::Int(b)) => {
            a.checked_div(*b).map(ConstValue::Int)
        }
        (BinaryOp::Mod, ConstValue::Int(a), ConstValue::Int(b)) => {
            a.checked_rem(*b).map(ConstValue::Int)
        }
        (BinaryOp::FloorDiv, ConstValue::Int(a), ConstValue::Int(b)) => {
            // Floor division: round towards negative infinity.
            a.checked_div(*b).map(|q| {
                if (a ^ b) < 0 && a % b != 0 {
                    ConstValue::Int(q - 1)
                } else {
                    ConstValue::Int(q)
                }
            })
        }

        // Float arithmetic.
        (BinaryOp::Add, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(
            (f64::from_bits(*a) + f64::from_bits(*b)).to_bits(),
        )),
        (BinaryOp::Sub, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(
            (f64::from_bits(*a) - f64::from_bits(*b)).to_bits(),
        )),
        (BinaryOp::Mul, ConstValue::Float(a), ConstValue::Float(b)) => Some(ConstValue::Float(
            (f64::from_bits(*a) * f64::from_bits(*b)).to_bits(),
        )),
        (BinaryOp::Div, ConstValue::Float(a), ConstValue::Float(b)) => {
            let bv = f64::from_bits(*b);
            if bv == 0.0 {
                None // Defer div-by-zero to runtime.
            } else {
                Some(ConstValue::Float((f64::from_bits(*a) / bv).to_bits()))
            }
        }

        // Integer comparisons.
        (BinaryOp::Eq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a == b)),
        (BinaryOp::NotEq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a != b)),
        (BinaryOp::Lt, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a < b)),
        (BinaryOp::LtEq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a <= b)),
        (BinaryOp::Gt, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a > b)),
        (BinaryOp::GtEq, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Bool(a >= b)),

        // Boolean comparisons.
        (BinaryOp::Eq, ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a == b)),
        (BinaryOp::NotEq, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Some(ConstValue::Bool(a != b))
        }

        // String comparisons.
        (BinaryOp::Eq, ConstValue::Str(a), ConstValue::Str(b)) => Some(ConstValue::Bool(a == b)),
        (BinaryOp::NotEq, ConstValue::Str(a), ConstValue::Str(b)) => Some(ConstValue::Bool(a != b)),

        // Boolean logic.
        (BinaryOp::And, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Some(ConstValue::Bool(*a && *b))
        }
        (BinaryOp::Or, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Some(ConstValue::Bool(*a || *b))
        }

        // Bitwise operations (integers).
        (BinaryOp::BitAnd, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a & b)),
        (BinaryOp::BitOr, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a | b)),
        (BinaryOp::BitXor, ConstValue::Int(a), ConstValue::Int(b)) => Some(ConstValue::Int(a ^ b)),
        (BinaryOp::Shl, ConstValue::Int(a), ConstValue::Int(b)) => {
            let shift = u32::try_from(*b).ok().filter(|&s| s < 64)?;
            let result = a.wrapping_shl(shift);
            // Round-trip check: if shifting back recovers the original, no overflow.
            if result.wrapping_shr(shift) == *a {
                Some(ConstValue::Int(result))
            } else {
                None
            }
        }
        (BinaryOp::Shr, ConstValue::Int(a), ConstValue::Int(b)) => {
            let shift = u32::try_from(*b).ok().filter(|&s| s < 64)?;
            Some(ConstValue::Int(a >> shift))
        }

        // Duration and Size — delegated to focused helpers to keep complexity bounded.
        (_, ConstValue::Duration { .. }, ConstValue::Duration { .. })
        | (_, ConstValue::Duration { .. }, ConstValue::Int(_))
        | (_, ConstValue::Int(_), ConstValue::Duration { .. }) => {
            fold_duration_binary(op, left, right)
        }
        (_, ConstValue::Size { .. }, ConstValue::Size { .. })
        | (_, ConstValue::Size { .. }, ConstValue::Int(_))
        | (_, ConstValue::Int(_), ConstValue::Size { .. }) => fold_size_binary(op, left, right),

        // Unmatched type combinations — can't fold.
        _ => None,
    }
}

/// Duration binary operations — arithmetic and comparisons normalized to nanoseconds.
///
/// Signed i64 results are stored as u64 via `cast_unsigned()`. The round-trip is safe:
/// readers call `to_nanos()` on the stored value, which uses `cast_signed()` to recover
/// the original i64 (multiplier = 1 for Nanoseconds).
fn fold_duration_binary(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
    match (op, left, right) {
        // Duration ± Duration, Duration % Duration.
        (
            BinaryOp::Add,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => {
            let (a_ns, b_ns) = (au.to_nanos(*a)?, bu.to_nanos(*b)?);
            a_ns.checked_add(b_ns).map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            })
        }
        (
            BinaryOp::Sub,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => {
            let (a_ns, b_ns) = (au.to_nanos(*a)?, bu.to_nanos(*b)?);
            a_ns.checked_sub(b_ns).map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            })
        }
        (
            BinaryOp::Mod,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => {
            let (a_ns, b_ns) = (au.to_nanos(*a)?, bu.to_nanos(*b)?);
            a_ns.checked_rem(b_ns).map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            })
        }

        // Duration comparisons.
        (
            BinaryOp::Eq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a)? == bu.to_nanos(*b)?)),
        (
            BinaryOp::NotEq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a)? != bu.to_nanos(*b)?)),
        (
            BinaryOp::Lt,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a)? < bu.to_nanos(*b)?)),
        (
            BinaryOp::LtEq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a)? <= bu.to_nanos(*b)?)),
        (
            BinaryOp::Gt,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a)? > bu.to_nanos(*b)?)),
        (
            BinaryOp::GtEq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a)? >= bu.to_nanos(*b)?)),

        // Duration * int, int * Duration, Duration / int.
        (BinaryOp::Mul, ConstValue::Duration { value: a, unit: au }, ConstValue::Int(b)) => au
            .to_nanos(*a)?
            .checked_mul(*b)
            .map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            }),
        (BinaryOp::Mul, ConstValue::Int(a), ConstValue::Duration { value: b, unit: bu }) => a
            .checked_mul(bu.to_nanos(*b)?)
            .map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            }),
        (BinaryOp::Div, ConstValue::Duration { value: a, unit: au }, ConstValue::Int(b)) => au
            .to_nanos(*a)?
            .checked_div(*b)
            .map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            }),

        _ => None,
    }
}

/// Size binary operations — arithmetic and comparisons normalized to bytes.
///
/// Results are bounded by `checked_size_bytes` to ensure values fit `i64::MAX`,
/// matching LLVM codegen's `bytes as i64` cast.
fn fold_size_binary(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
    match (op, left, right) {
        // Size ± Size, Size % Size.
        (
            BinaryOp::Add,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => au
            .to_bytes(*a)?
            .checked_add(bu.to_bytes(*b)?)
            .and_then(checked_size_bytes),
        (
            BinaryOp::Sub,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => au
            .to_bytes(*a)?
            .checked_sub(bu.to_bytes(*b)?)
            .and_then(checked_size_bytes),
        (
            BinaryOp::Mod,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => {
            let (a_b, b_b) = (au.to_bytes(*a)?, bu.to_bytes(*b)?);
            a_b.checked_rem(b_b).and_then(checked_size_bytes)
        }

        // Size comparisons.
        (
            BinaryOp::Eq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a)? == bu.to_bytes(*b)?)),
        (
            BinaryOp::NotEq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a)? != bu.to_bytes(*b)?)),
        (
            BinaryOp::Lt,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a)? < bu.to_bytes(*b)?)),
        (
            BinaryOp::LtEq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a)? <= bu.to_bytes(*b)?)),
        (
            BinaryOp::Gt,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a)? > bu.to_bytes(*b)?)),
        (
            BinaryOp::GtEq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a)? >= bu.to_bytes(*b)?)),

        // Size * int, int * Size, Size / int (reject negative int).
        (BinaryOp::Mul, ConstValue::Size { value: a, unit: au }, ConstValue::Int(b)) if *b >= 0 => {
            au.to_bytes(*a)?
                .checked_mul(b.cast_unsigned())
                .and_then(checked_size_bytes)
        }
        (BinaryOp::Mul, ConstValue::Int(a), ConstValue::Size { value: b, unit: bu }) if *a >= 0 => {
            a.cast_unsigned()
                .checked_mul(bu.to_bytes(*b)?)
                .and_then(checked_size_bytes)
        }
        (BinaryOp::Div, ConstValue::Size { value: a, unit: au }, ConstValue::Int(b)) => {
            if *b <= 0 {
                None // Negative or zero divisor for Size — defer to runtime.
            } else {
                au.to_bytes(*a)?
                    .checked_div(b.cast_unsigned())
                    .and_then(checked_size_bytes)
            }
        }

        _ => None,
    }
}

// Unary Folding

/// Evaluate a unary operation on a constant value.
pub(super) fn fold_unary(op: UnaryOp, val: &ConstValue) -> Option<ConstValue> {
    // Match by reference — all inner data is Copy, so no cloning needed.
    match (op, val) {
        (UnaryOp::Neg, ConstValue::Int(v)) => v.checked_neg().map(ConstValue::Int),
        (UnaryOp::Neg, ConstValue::Float(bits)) => {
            Some(ConstValue::Float((-f64::from_bits(*bits)).to_bits()))
        }
        (UnaryOp::Neg, ConstValue::Duration { value, unit }) => {
            let nanos = unit.to_nanos(*value)?;
            nanos.checked_neg().map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            })
        }
        (UnaryOp::Not, ConstValue::Bool(v)) => Some(ConstValue::Bool(!v)),
        (UnaryOp::BitNot, ConstValue::Int(v)) => Some(ConstValue::Int(!v)),
        _ => None,
    }
}
