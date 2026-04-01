//! Overflow guard insertion for narrowed integers.
//!
//! When a value is narrowed from i64 to a smaller type (i8/i16/i32),
//! arithmetic operations might overflow the narrow type even though they
//! wouldn't overflow the canonical i64. This module determines whether
//! overflow is possible and recommends a strategy to handle it.
//!
//! # Strategies
//!
//! - **(c) Proven safe**: Range analysis proves the result fits in the
//!   target width. No runtime cost — the narrowed operation is used directly.
//! - **(a) Widen before operation**: Promote operands to a wider type,
//!   compute, then truncate the result back to the narrow type. Adds a
//!   few `sext`/`trunc` instructions but preserves the narrow storage.
//! - **(b) Compute at canonical width**: If overflow is common or the
//!   wider intermediate still overflows, use i64 for the entire expression.
//!   Gives up the narrowing benefit for this expression.
//!
//! # Usage
//!
//! Phase B (local variable narrowing) will call `recommend_strategy()` for
//! each arithmetic operation on narrowed variables. The LLVM emitter will
//! use the strategy to decide how to emit the operation.

use ori_ir::BinaryOp;

use crate::range::transfer::{
    range_add, range_bitand, range_bitor, range_bitxor, range_div, range_floordiv, range_mod,
    range_mul, range_shl, range_shr, range_sub,
};
use crate::range::ValueRange;
use crate::repr::IntWidth;

/// Whether an arithmetic operation on narrowed operands can overflow
/// the target width.
///
/// Returns `true` when the computed result range does NOT fit in `target`,
/// meaning overflow is possible and a guard or widening is needed.
///
/// Returns `false` when range analysis proves the result fits — the
/// narrowed operation can be used directly without any guard.
///
/// For non-arithmetic operations (comparisons, logical, range construction),
/// conservatively returns `true` (uses `ValueRange::Top`).
#[must_use]
pub fn can_overflow(op: BinaryOp, lhs: ValueRange, rhs: ValueRange, target: IntWidth) -> bool {
    let result_range = compute_result_range(op, lhs, rhs);
    !result_range.fits_in(target)
}

/// Recommended strategy for handling potential overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStrategy {
    /// **(c)** Range analysis proves no overflow is possible.
    /// Use the narrowed operation directly — zero runtime cost.
    ProvenSafe,

    /// **(a)** Widen operands to `intermediate_width` before computing,
    /// then truncate the result back to the target width.
    ///
    /// Emits:
    /// ```llvm
    /// %wide_a = sext i8 %a to i16
    /// %wide_b = sext i8 %b to i16
    /// %result = add i16 %wide_a, %wide_b
    /// %narrow = trunc i16 %result to i8
    /// ```
    WidenCompute {
        /// The intermediate width to compute at (always wider than target).
        intermediate_width: IntWidth,
    },

    /// **(b)** Use canonical i64 for this entire expression.
    /// The narrowing benefit is lost for this operation, but correctness
    /// is preserved. Used when even the next-wider type overflows.
    UseCanonical,
}

/// Recommend a strategy for handling overflow at a narrowed arithmetic site.
///
/// Priority:
/// 1. **Proven safe** — if `can_overflow()` returns false, the narrowed
///    operation is safe as-is. Zero cost.
/// 2. **Widen to next-wider** — if the result fits in the next wider
///    integer type (e.g., i8 → i16), compute at that width. Low cost.
/// 3. **Use canonical** — fall back to i64 when even the next-wider
///    type isn't sufficient.
#[must_use]
pub fn recommend_strategy(
    op: BinaryOp,
    lhs: ValueRange,
    rhs: ValueRange,
    target: IntWidth,
) -> OverflowStrategy {
    // Strategy (c): proven safe — no overflow possible.
    if !can_overflow(op, lhs, rhs, target) {
        return OverflowStrategy::ProvenSafe;
    }

    // Strategy (a): widen to next-wider type if result fits there.
    if let Some(wider) = next_wider(target) {
        let result_range = compute_result_range(op, lhs, rhs);
        if result_range.fits_in(wider) {
            return OverflowStrategy::WidenCompute {
                intermediate_width: wider,
            };
        }
    }

    // Strategy (b): use canonical i64.
    OverflowStrategy::UseCanonical
}

/// Compute the result range of a binary operation given operand ranges.
///
/// Delegates to the transfer functions from range analysis.
/// Non-arithmetic operations conservatively return `Top` — overflow
/// detection is only meaningful for operations that produce integer
/// results (not booleans or ranges).
fn compute_result_range(op: BinaryOp, lhs: ValueRange, rhs: ValueRange) -> ValueRange {
    match op {
        // Arithmetic — use exact transfer functions.
        BinaryOp::Add => range_add(lhs, rhs),
        BinaryOp::Sub => range_sub(lhs, rhs),
        BinaryOp::Mul => range_mul(lhs, rhs),
        BinaryOp::Div => range_div(lhs, rhs),
        BinaryOp::Mod => range_mod(lhs, rhs),
        BinaryOp::FloorDiv => range_floordiv(lhs, rhs),

        // Bitwise — can produce arbitrary bit patterns.
        BinaryOp::Shl => range_shl(lhs, rhs),
        BinaryOp::Shr => range_shr(lhs, rhs),
        BinaryOp::BitAnd => range_bitand(lhs, rhs),
        BinaryOp::BitOr => range_bitor(lhs, rhs),
        BinaryOp::BitXor => range_bitxor(lhs, rhs),

        // Comparison/logical → boolean result (not overflow concern).
        // Range/coalesce/matmul → non-int or unsupported.
        // Conservative Top means `can_overflow()` returns true.
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::MatMul
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::Coalesce => ValueRange::Top,
    }
}

/// Returns the next wider integer type, or `None` for I64 (canonical).
fn next_wider(width: IntWidth) -> Option<IntWidth> {
    match width {
        IntWidth::I8 => Some(IntWidth::I16),
        IntWidth::I16 => Some(IntWidth::I32),
        IntWidth::I32 => Some(IntWidth::I64),
        IntWidth::I64 => None,
    }
}
