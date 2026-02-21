//! Constant folding during lowering.
//!
//! Integrated into the lowering pass — not a separate traversal. After
//! lowering children, the lowerer checks if the result is compile-time
//! constant. If so, evaluates it immediately and stores the result in
//! `ConstantPool` as `CanExpr::Constant(id)`.
//!
//! # Scope
//!
//! Simple constant folding only:
//! - Literal values and pure arithmetic
//! - Boolean logic and comparisons
//! - Dead branch elimination (`if true`/`if false`)
//!
//! Does NOT cover:
//! - CTFE (compile-time function evaluation)
//! - Algebraic simplification
//! - Function call memoization
//!
//! See `eval_v2` Section 04 for the full constant folding specification.

use ori_ir::canon::{CanArena, CanExpr, CanId, CanNode, ConstValue, ConstantPool};
use ori_ir::{BinaryOp, UnaryOp};
use ori_ir::{DurationUnit, SizeUnit};

// Constness Classification

/// Whether an expression can be evaluated at compile time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Constness {
    /// The expression is a compile-time constant.
    Const,
    /// The expression depends on runtime values.
    Runtime,
}

/// Classify whether a canonical expression is compile-time constant.
fn classify(arena: &CanArena, id: CanId) -> Constness {
    if !id.is_valid() {
        return Constness::Runtime;
    }

    match arena.kind(id) {
        // Literals and already-folded constants are compile-time.
        // Note: `CanExpr::Const(_)` (named `$` constants like `$PI`) is NOT
        // included — their values aren't resolved at the canon level, so
        // `extract_const_value()` can't extract them. They stay Runtime.
        CanExpr::Int(_)
        | CanExpr::Float(_)
        | CanExpr::Bool(_)
        | CanExpr::Str(_)
        | CanExpr::Char(_)
        | CanExpr::Unit
        | CanExpr::Duration { .. }
        | CanExpr::Size { .. }
        | CanExpr::Constant(_) => Constness::Const,

        // Binary: const if both children const AND operator is pure.
        CanExpr::Binary { op, left, right } => {
            if is_pure_binary(*op)
                && classify(arena, *left) == Constness::Const
                && classify(arena, *right) == Constness::Const
            {
                Constness::Const
            } else {
                Constness::Runtime
            }
        }

        // Unary: const if operand const AND operator is pure.
        CanExpr::Unary { op, operand } => {
            if is_pure_unary(*op) && classify(arena, *operand) == Constness::Const {
                Constness::Const
            } else {
                Constness::Runtime
            }
        }

        // If: const only if condition is const (for dead branch elimination).
        CanExpr::If { cond, .. } => {
            if classify(arena, *cond) == Constness::Const {
                Constness::Const
            } else {
                Constness::Runtime
            }
        }

        // Everything else is runtime.
        _ => Constness::Runtime,
    }
}

/// Returns `true` if the binary operator is pure (no side effects,
/// always produces the same result for the same inputs).
fn is_pure_binary(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::FloorDiv
            | BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::LtEq
            | BinaryOp::Gt
            | BinaryOp::GtEq
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
    )
}

/// Returns `true` if the unary operator is pure.
fn is_pure_unary(op: UnaryOp) -> bool {
    matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot)
}

// Constant Folding

/// Try to fold a canonical expression to a constant.
///
/// Returns `Some(new_id)` if the expression was folded to a `Constant`,
/// `None` if it cannot be folded (runtime expression). The folded node
/// is pushed into the arena and the constant is interned in the pool.
///
/// Called by the lowerer after constructing `Binary`, `Unary`, and `If` nodes.
pub(crate) fn try_fold(
    arena: &mut CanArena,
    constants: &mut ConstantPool,
    id: CanId,
) -> Option<CanId> {
    if classify(arena, id) != Constness::Const {
        return None;
    }

    let span = arena.span(id);
    let ty = arena.ty(id);

    match *arena.kind(id) {
        // Dead branch elimination.
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => try_fold_if(arena, cond, then_branch, else_branch),

        // Binary operations.
        CanExpr::Binary { op, left, right } => {
            let lval = extract_const_value(arena, constants, left)?;
            let rval = extract_const_value(arena, constants, right)?;
            let result = fold_binary(op, &lval, &rval)?;
            let const_id = constants.intern(result);
            Some(arena.push(CanNode::new(CanExpr::Constant(const_id), span, ty)))
        }

        // Unary operations.
        CanExpr::Unary { op, operand } => {
            let val = extract_const_value(arena, constants, operand)?;
            let result = fold_unary(op, &val)?;
            let const_id = constants.intern(result);
            Some(arena.push(CanNode::new(CanExpr::Constant(const_id), span, ty)))
        }

        _ => None,
    }
}

/// Dead branch elimination: `if true { A } else { B }` → `A`.
fn try_fold_if(
    arena: &CanArena,
    cond: CanId,
    then_branch: CanId,
    else_branch: CanId,
) -> Option<CanId> {
    match arena.kind(cond) {
        CanExpr::Bool(true) => Some(then_branch),
        CanExpr::Bool(false) => {
            if else_branch.is_valid() {
                Some(else_branch)
            } else {
                None // `if false { A }` with no else — can't eliminate
            }
        }
        _ => None,
    }
}

// Value Extraction

/// Extract a `ConstValue` from a canonical expression node.
///
/// Works for both literal `CanExpr` variants and already-folded `Constant` nodes.
fn extract_const_value(
    arena: &CanArena,
    constants: &ConstantPool,
    id: CanId,
) -> Option<ConstValue> {
    match arena.kind(id) {
        CanExpr::Int(v) => Some(ConstValue::Int(*v)),
        CanExpr::Float(bits) => Some(ConstValue::Float(*bits)),
        CanExpr::Bool(v) => Some(ConstValue::Bool(*v)),
        CanExpr::Str(name) => Some(ConstValue::Str(*name)),
        CanExpr::Char(c) => Some(ConstValue::Char(*c)),
        CanExpr::Unit => Some(ConstValue::Unit),
        CanExpr::Duration { value, unit } => Some(ConstValue::Duration {
            value: *value,
            unit: *unit,
        }),
        CanExpr::Size { value, unit } => Some(ConstValue::Size {
            value: *value,
            unit: *unit,
        }),
        CanExpr::Constant(cid) => Some(constants.get(*cid).clone()),
        _ => None,
    }
}

// Binary Folding

/// Evaluate a binary operation on two constant values.
///
/// Returns `None` if the operation would cause undefined behavior
/// (division by zero, integer overflow) — these are deferred to runtime.
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive (BinaryOp, ConstValue, ConstValue) fold dispatch"
)]
fn fold_binary(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
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

        // Duration arithmetic (normalized to nanoseconds).
        // Signed i64 results are stored as u64 via `cast_unsigned()`. The
        // round-trip is safe: readers call `to_nanos()` on the stored value,
        // which is a no-op (multiplier = 1 for Nanoseconds), then
        // `cast_signed()` to recover the original i64.
        (
            BinaryOp::Add,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => {
            let (a_ns, b_ns) = (au.to_nanos(*a), bu.to_nanos(*b));
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
            let (a_ns, b_ns) = (au.to_nanos(*a), bu.to_nanos(*b));
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
            let (a_ns, b_ns) = (au.to_nanos(*a), bu.to_nanos(*b));
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
        ) => Some(ConstValue::Bool(au.to_nanos(*a) == bu.to_nanos(*b))),
        (
            BinaryOp::NotEq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a) != bu.to_nanos(*b))),
        (
            BinaryOp::Lt,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a) < bu.to_nanos(*b))),
        (
            BinaryOp::LtEq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a) <= bu.to_nanos(*b))),
        (
            BinaryOp::Gt,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a) > bu.to_nanos(*b))),
        (
            BinaryOp::GtEq,
            ConstValue::Duration { value: a, unit: au },
            ConstValue::Duration { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_nanos(*a) >= bu.to_nanos(*b))),

        // Duration * int, int * Duration, Duration / int.
        (BinaryOp::Mul, ConstValue::Duration { value: a, unit: au }, ConstValue::Int(b)) => au
            .to_nanos(*a)
            .checked_mul(*b)
            .map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            }),
        (BinaryOp::Mul, ConstValue::Int(a), ConstValue::Duration { value: b, unit: bu }) => a
            .checked_mul(bu.to_nanos(*b))
            .map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            }),
        (BinaryOp::Div, ConstValue::Duration { value: a, unit: au }, ConstValue::Int(b)) => au
            .to_nanos(*a)
            .checked_div(*b)
            .map(|r| ConstValue::Duration {
                value: r.cast_unsigned(),
                unit: DurationUnit::Nanoseconds,
            }),

        // Size arithmetic (normalized to bytes).
        (
            BinaryOp::Add,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => au
            .to_bytes(*a)
            .checked_add(bu.to_bytes(*b))
            .map(|r| ConstValue::Size {
                value: r,
                unit: SizeUnit::Bytes,
            }),
        (
            BinaryOp::Sub,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => au
            .to_bytes(*a)
            .checked_sub(bu.to_bytes(*b))
            .map(|r| ConstValue::Size {
                value: r,
                unit: SizeUnit::Bytes,
            }),
        (
            BinaryOp::Mod,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => {
            let (a_b, b_b) = (au.to_bytes(*a), bu.to_bytes(*b));
            a_b.checked_rem(b_b).map(|r| ConstValue::Size {
                value: r,
                unit: SizeUnit::Bytes,
            })
        }

        // Size comparisons.
        (
            BinaryOp::Eq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a) == bu.to_bytes(*b))),
        (
            BinaryOp::NotEq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a) != bu.to_bytes(*b))),
        (
            BinaryOp::Lt,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a) < bu.to_bytes(*b))),
        (
            BinaryOp::LtEq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a) <= bu.to_bytes(*b))),
        (
            BinaryOp::Gt,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a) > bu.to_bytes(*b))),
        (
            BinaryOp::GtEq,
            ConstValue::Size { value: a, unit: au },
            ConstValue::Size { value: b, unit: bu },
        ) => Some(ConstValue::Bool(au.to_bytes(*a) >= bu.to_bytes(*b))),

        // Size * int, int * Size, Size / int (reject negative int).
        (BinaryOp::Mul, ConstValue::Size { value: a, unit: au }, ConstValue::Int(b)) if *b >= 0 => {
            au.to_bytes(*a)
                .checked_mul(b.cast_unsigned())
                .map(|r| ConstValue::Size {
                    value: r,
                    unit: SizeUnit::Bytes,
                })
        }
        (BinaryOp::Mul, ConstValue::Int(a), ConstValue::Size { value: b, unit: bu }) if *a >= 0 => {
            a.cast_unsigned()
                .checked_mul(bu.to_bytes(*b))
                .map(|r| ConstValue::Size {
                    value: r,
                    unit: SizeUnit::Bytes,
                })
        }
        (BinaryOp::Div, ConstValue::Size { value: a, unit: au }, ConstValue::Int(b)) => {
            if *b <= 0 {
                None // Negative or zero divisor for Size — defer to runtime.
            } else {
                au.to_bytes(*a)
                    .checked_div(b.cast_unsigned())
                    .map(|r| ConstValue::Size {
                        value: r,
                        unit: SizeUnit::Bytes,
                    })
            }
        }

        // Unmatched type combinations — can't fold.
        _ => None,
    }
}

// Unary Folding

/// Evaluate a unary operation on a constant value.
fn fold_unary(op: UnaryOp, val: &ConstValue) -> Option<ConstValue> {
    // Match by reference — all inner data is Copy, so no cloning needed.
    match (op, val) {
        (UnaryOp::Neg, ConstValue::Int(v)) => v.checked_neg().map(ConstValue::Int),
        (UnaryOp::Neg, ConstValue::Float(bits)) => {
            Some(ConstValue::Float((-f64::from_bits(*bits)).to_bits()))
        }
        (UnaryOp::Neg, ConstValue::Duration { value, unit }) => {
            let nanos = unit.to_nanos(*value);
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

#[cfg(test)]
mod tests;
