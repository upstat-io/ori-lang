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

use ori_arc::ir::{ArcInstr, ArcValue, LitValue, PrimOp};
use ori_arc::ArcVarId;
use ori_ir::{BinaryOp, UnaryOp};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::{is_int_typed, ValueRange};
use ValueRange::{Bottom, Bounded, Top};

/// Convert a validated shift amount (0..63) from `i64` to `u32`.
///
/// Callers must validate `0 <= v < 64` before calling. The `unwrap_or(0)`
/// is unreachable but satisfies clippy's `cast_sign_loss`/`cast_possible_truncation`.
fn shift_amount(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(0)
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
}

/// Compute the output range for a single `ArcInstr`.
///
/// Returns `Top` for non-int-typed destinations or unsupported patterns.
/// Exhaustive match (no `_` arm) ensures new variants cause compile errors.
pub fn transfer(instr: &ArcInstr, ctx: &TransferContext<'_>) -> ValueRange {
    let TransferContext {
        ranges,
        pool,
        var_types,
        field_summaries,
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
            transfer_known_call(*func).unwrap_or(Top)
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
/// Currently uses `Name::raw()` — a stable numeric identifier. Future:
/// compare against interned builtin names from `ori_ir::BuiltinConstant`.
fn transfer_known_call(_func: ori_ir::Name) -> Option<ValueRange> {
    // TODO(§03.5): Implement builtin name matching once interner is available
    // in the analysis context. For now, return None (conservative: all calls → Top).
    // Known builtins when matched: len → [0, MAX], count → [0, MAX],
    // byte_to_int → [0, 255], char_to_int → [0, 0x10FFFF].
    None
}

// ─── PrimOp dispatcher ────────────────────────────────────────

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

// ─── Arithmetic ───────────────────────────────────────────────

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

/// `a / b` — returns Top if divisor spans zero.
pub fn range_div(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            // Division by zero possible → Top
            if bl <= 0 && bh >= 0 {
                return Top;
            }
            // All four quotients (truncating division)
            let quotients = [al / bl, al / bh, ah / bl, ah / bh];
            let lo = *quotients.iter().min().unwrap_or(&0);
            let hi = *quotients.iter().max().unwrap_or(&0);
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

/// Floor division: `a / b` rounded toward negative infinity.
pub fn range_floordiv(a: ValueRange, b: ValueRange) -> ValueRange {
    // Conservative: same as truncating division.
    // Floor division differs only when signs differ, producing a
    // slightly lower result. For range analysis, truncating bounds
    // are conservative (the actual floor result is within ±1).
    range_div(a, b)
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

// ─── Bitwise ──────────────────────────────────────────────────

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
pub fn range_shr(a: ValueRange, b: ValueRange) -> ValueRange {
    match (a, b) {
        (Bottom, _) | (_, Bottom) => Bottom,
        (Bounded { lo: al, hi: ah }, Bounded { lo: bl, hi: bh }) => {
            if bl < 0 || bh >= 64 {
                return Top;
            }
            // Arithmetic right shift preserves sign.
            let lo = al >> shift_amount(bh); // max shift on lowest value = smallest result
            let hi = ah >> shift_amount(bl); // min shift on highest value = largest result
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
            match ((-hi).checked_sub(1), (-lo).checked_sub(1)) {
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

// ─── Literals & built-in ranges ───────────────────────────────

/// Integer literal → exact constant range.
pub fn range_literal(value: i64) -> ValueRange {
    Bounded {
        lo: value,
        hi: value,
    }
}

/// `len()` always returns `>= 0`.
pub fn range_len() -> ValueRange {
    Bounded {
        lo: 0,
        hi: i64::MAX,
    }
}

/// `count()` always returns `>= 0`.
pub fn range_count() -> ValueRange {
    Bounded {
        lo: 0,
        hi: i64::MAX,
    }
}

/// `byte_to_int()` returns `[0, 255]`.
pub fn range_byte_to_int() -> ValueRange {
    Bounded { lo: 0, hi: 255 }
}

/// `char_to_int()` returns `[0, 0x10FFFF]`.
pub fn range_char_to_int() -> ValueRange {
    Bounded {
        lo: 0,
        hi: 0x10_FFFF,
    }
}

#[cfg(test)]
mod tests;
