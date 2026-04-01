//! Transfer functions for value range analysis.
//!
//! Maps each ARC IR instruction (`ArcInstr`) to the `ValueRange` of its
//! destination variable. The top-level [`transfer()`] dispatcher handles
//! all `ArcInstr` variants; arithmetic and bitwise operations are delegated
//! to individual transfer functions via [`transfer_primop()`].
//!
//! # Soundness Contract
//!
//! For any concrete values `x ∈ input_range`, the concrete result of
//! the operation must be contained in the returned `ValueRange`. Returning
//! `Top` is always safe (conservative); returning a range that excludes
//! a possible concrete result is unsound.
//!
//! # Module Structure
//!
//! - `arithmetic.rs`: add, sub, mul, div, mod, `floor_div`, neg, abs
//! - `bitwise.rs`: bitand, bitor, bitxor, shl, shr, bitnot

mod arithmetic;
mod bitwise;

use ori_arc::ir::{ArcInstr, ArcValue, LitValue, PrimOp};
use ori_arc::ArcVarId;
use ori_ir::{BinaryOp, UnaryOp};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::{is_int_typed, ValueRange};
use ValueRange::{Bottom, Bounded, Top};

pub use arithmetic::{
    range_abs, range_add, range_div, range_floordiv, range_mod, range_mul, range_neg, range_sub,
};
pub use bitwise::{range_bitand, range_bitnot, range_bitor, range_bitxor, range_shl, range_shr};

/// Convert a validated shift amount (0..63) from `i64` to `u32`.
///
/// Callers must validate `0 <= v < 64` before calling. The `unwrap_or(0)`
/// is unreachable but satisfies clippy's `cast_sign_loss`/`cast_possible_truncation`.
fn shift_amount(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(0)
}

/// Apply a binary operation to two ranges, handling Bottom/Top propagation.
///
/// The closure receives `(a_lo, a_hi, b_lo, b_hi)` for the `Bounded × Bounded` case.
/// Bottom propagates (any Bottom input → Bottom), and any `Top` or mixed
/// `Top × Bounded` input → Top.
pub(crate) fn binary_transfer(
    a: ValueRange,
    b: ValueRange,
    f: impl FnOnce(i64, i64, i64, i64) -> ValueRange,
) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => f(al, ah, bl, bh),
        _ => Top,
    }
}

/// Evaluate all four corners of a binary operation and take min/max.
///
/// Given ranges `[al, ah]` and `[bl, bh]`, computes `op(a, b)` for all
/// four `(al|ah, bl|bh)` combinations. Returns the tightest enclosing
/// `Bounded` range, or `Top` if any corner returns `None` (overflow).
///
/// Used by `range_mul`, `range_div`, `range_floordiv`, and shift operations
/// that share the 4-corner evaluation pattern.
pub(crate) fn four_corner_fold(
    al: i64,
    ah: i64,
    bl: i64,
    bh: i64,
    op: impl Fn(i64, i64) -> Option<i64>,
) -> ValueRange {
    let corners = [op(al, bl), op(al, bh), op(ah, bl), op(ah, bh)];
    let mut lo = i64::MAX;
    let mut hi = i64::MIN;
    for c in corners {
        match c {
            Some(v) => {
                lo = lo.min(v);
                hi = hi.max(v);
            }
            None => return Top,
        }
    }
    Bounded { lo, hi }
}

/// Context for the transfer function — bundles per-function state.
///
/// Avoids >4 parameters per the ">3 params → config struct" guideline.
pub struct TransferContext<'a> {
    /// Current variable ranges (fixpoint state).
    pub ranges: &'a FxHashMap<ArcVarId, ValueRange>,
    /// Type pool for `is_int_typed` checks.
    pub pool: &'a Pool,
    /// Per-variable types from `ArcFunction::var_types`.
    /// Used to resolve the struct/tuple `Idx` for `Project` field lookups.
    pub var_types: &'a [Idx],
    /// Field-summary table: `(struct_idx, field_index)` → joined range.
    /// Populated by `Construct` instructions, queried by `Project`.
    pub field_summaries: &'a FxHashMap<(Idx, u32), ValueRange>,
    /// Pre-interned builtin names for `transfer_known_call`.
    pub known_builtins: &'a super::KnownBuiltins,
}

/// Compute the output range for a single `ArcInstr`.
///
/// Returns `Top` for non-int-typed destinations or unsupported patterns.
/// Exhaustive match (no `_` arm) ensures new variants cause compile errors.
#[tracing::instrument(skip_all)]
pub fn transfer(instr: &ArcInstr, ctx: &TransferContext<'_>) -> ValueRange {
    let TransferContext {
        ranges,
        pool,
        var_types,
        field_summaries,
        known_builtins,
    } = ctx;
    match instr {
        ArcInstr::Let { ty, value, .. } => {
            if !is_int_typed(*ty, pool) {
                return Top;
            }
            match value {
                ArcValue::Literal(LitValue::Int(n)) => range_literal(*n),
                ArcValue::Var(v) => ranges.get(v).copied().unwrap_or(Top),
                ArcValue::PrimOp { op, args } => transfer_primop(*op, args, ranges),
                ArcValue::Literal(_) => Top,
            }
        }
        ArcInstr::Apply { ty, func, .. } => {
            if !is_int_typed(*ty, pool) {
                return Top;
            }
            transfer_known_call(*func, known_builtins).unwrap_or(Top)
        }
        ArcInstr::ApplyIndirect { .. }
        | ArcInstr::PartialApply { .. }
        | ArcInstr::Construct { .. }
        | ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Reset { .. }
        | ArcInstr::Reuse { .. }
        | ArcInstr::CollectionReuse { .. } => Top,
        ArcInstr::Project {
            ty, value, field, ..
        } => {
            if !is_int_typed(*ty, pool) {
                return Top;
            }
            let struct_idx = var_types.get(value.index()).copied();
            match struct_idx {
                Some(idx) => field_summaries.get(&(idx, *field)).copied().unwrap_or(Top),
                None => Top,
            }
        }
        ArcInstr::IsShared { .. } => Bounded { lo: 0, hi: 1 },
        ArcInstr::Select {
            ty,
            true_val,
            false_val,
            ..
        } => {
            if !is_int_typed(*ty, pool) {
                return Top;
            }
            let t = ranges.get(true_val).copied().unwrap_or(Top);
            let f = ranges.get(false_val).copied().unwrap_or(Top);
            t.join(f)
        }
    }
}

/// Check if a function name corresponds to a known built-in with a fixed range.
///
/// Returns `Some(range)` for recognized builtins, `None` for unknown callees.
/// Matches against pre-interned names from `KnownBuiltins`. When builtins
/// are not populated (tests without an interner), all calls return `None`.
pub fn transfer_known_call(
    func: ori_ir::Name,
    builtins: &super::KnownBuiltins,
) -> Option<ValueRange> {
    if builtins.len == Some(func) {
        return Some(range_len());
    }
    if builtins.count == Some(func) {
        return Some(range_count());
    }
    if builtins.byte_to_int == Some(func) {
        return Some(range_byte_to_int());
    }
    if builtins.char_to_int == Some(func) {
        return Some(range_char_to_int());
    }
    if builtins.abs == Some(func) {
        // abs with unknown input → return Top (can't compute range_abs without operand range).
        // The abs transfer function is applied at PrimOp level with operand ranges.
        return Some(ValueRange::Bounded {
            lo: 0,
            hi: i64::MAX,
        });
    }
    None
}

// PrimOp dispatcher

/// Dispatch a primitive operation to the appropriate transfer function.
///
/// Exhaustive match on both `BinaryOp` (23 variants) and `UnaryOp` (4 variants).
fn transfer_primop(
    op: PrimOp,
    args: &[ArcVarId],
    ranges: &FxHashMap<ArcVarId, ValueRange>,
) -> ValueRange {
    match op {
        PrimOp::Binary(bin) => {
            let a = args
                .first()
                .map_or(Top, |v| ranges.get(v).copied().unwrap_or(Top));
            let b = args
                .get(1)
                .map_or(Top, |v| ranges.get(v).copied().unwrap_or(Top));
            match bin {
                BinaryOp::Add => range_add(a, b),
                BinaryOp::Sub => range_sub(a, b),
                BinaryOp::Mul => range_mul(a, b),
                BinaryOp::Div => range_div(a, b),
                BinaryOp::Mod => range_mod(a, b),
                BinaryOp::FloorDiv => range_floordiv(a, b),
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::And
                | BinaryOp::Or => Bounded { lo: 0, hi: 1 },
                BinaryOp::BitAnd => range_bitand(a, b),
                BinaryOp::BitOr => range_bitor(a, b),
                BinaryOp::BitXor => range_bitxor(a, b),
                BinaryOp::Shl => range_shl(a, b),
                BinaryOp::Shr => range_shr(a, b),
                BinaryOp::MatMul
                | BinaryOp::Range
                | BinaryOp::RangeInclusive
                | BinaryOp::Coalesce => Top,
            }
        }
        PrimOp::Unary(un) => {
            let a = args
                .first()
                .map_or(Top, |v| ranges.get(v).copied().unwrap_or(Top));
            match un {
                UnaryOp::Neg => range_neg(a),
                UnaryOp::Not => Bounded { lo: 0, hi: 1 },
                UnaryOp::BitNot => range_bitnot(a),
                UnaryOp::Try => Top, // desugared before ARC IR
            }
        }
    }
}

// Literals and built-in ranges

/// Integer literal → exact constant range.
pub(crate) fn range_literal(value: i64) -> ValueRange {
    Bounded {
        lo: value,
        hi: value,
    }
}

/// `len()` always returns `>= 0`.
pub(crate) fn range_len() -> ValueRange {
    Bounded {
        lo: 0,
        hi: i64::MAX,
    }
}

/// `count()` always returns `>= 0`.
pub(crate) fn range_count() -> ValueRange {
    Bounded {
        lo: 0,
        hi: i64::MAX,
    }
}

/// `byte_to_int()` returns `[0, 255]`.
pub(crate) fn range_byte_to_int() -> ValueRange {
    Bounded { lo: 0, hi: 255 }
}

/// `char_to_int()` returns `[0, 0x10FFFF]`.
pub(crate) fn range_char_to_int() -> ValueRange {
    Bounded {
        lo: 0,
        hi: 0x10_FFFF,
    }
}

#[cfg(test)]
mod tests;
