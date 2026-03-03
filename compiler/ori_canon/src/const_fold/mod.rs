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

mod arithmetic;

use ori_ir::canon::{CanArena, CanExpr, CanId, CanNode, ConstValue, ConstantPool};
use ori_ir::{BinaryOp, UnaryOp};

use self::arithmetic::{fold_binary, fold_unary};

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

#[cfg(test)]
mod tests;
