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

mod arithmetic;

use ori_ir::canon::{
    CanArena, CanExpr, CanId, CanNode, ConstEvalProblemKind, ConstValue, ConstantPool,
};
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
        // Successful module constants are frozen to `CanExpr::Constant`
        // before executable roots are lowered. A surviving `CanExpr::Const`
        // is either a generic const awaiting an exact mono binding or an
        // error-recovery reference, so it remains runtime here.
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

/// Evaluate one canonical subtree within the representable [`ConstValue`]
/// domain.
///
/// Normal opportunistic folding returns `None` for arithmetic traps so runtime
/// expressions can keep their original behavior. Module constant initializers
/// are different: they are required to finish at compile time, so this entry
/// point preserves the reason evaluation failed.
pub(crate) fn evaluate_constant(
    arena: &CanArena,
    constants: &ConstantPool,
    id: CanId,
) -> Result<ConstValue, ConstEvalProblemKind> {
    if !id.is_valid() {
        return Ok(ConstValue::Unit);
    }
    if let Some(value) = extract_const_value(arena, constants, id) {
        return Ok(value);
    }

    match *arena.kind(id) {
        CanExpr::Binary { op, left, right } => {
            let left = evaluate_constant(arena, constants, left)?;

            // Preserve short-circuit semantics in constant contexts. An
            // unreachable RHS must not manufacture a division-by-zero error.
            match (op, &left) {
                (BinaryOp::And, ConstValue::Bool(false)) => {
                    return Ok(ConstValue::Bool(false));
                }
                (BinaryOp::Or, ConstValue::Bool(true)) => {
                    return Ok(ConstValue::Bool(true));
                }
                _ => {}
            }

            let right = evaluate_constant(arena, constants, right)?;
            fold_binary(op, &left, &right).ok_or_else(|| binary_failure(op, &left, &right))
        }
        CanExpr::Unary { op, operand } => {
            let value = evaluate_constant(arena, constants, operand)?;
            fold_unary(op, &value).ok_or_else(|| unary_failure(op, &value))
        }
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => match evaluate_constant(arena, constants, cond)? {
            ConstValue::Bool(true) => evaluate_constant(arena, constants, then_branch),
            ConstValue::Bool(false) => evaluate_constant(arena, constants, else_branch),
            _ => Err(ConstEvalProblemKind::UnsupportedExpression {
                form: "conditional with a non-boolean condition",
            }),
        },
        CanExpr::Const(reference) => Err(ConstEvalProblemKind::UnresolvedReference { reference }),
        ref expression => Err(ConstEvalProblemKind::UnsupportedExpression {
            form: unsupported_form(expression),
        }),
    }
}

fn binary_failure(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> ConstEvalProblemKind {
    if matches!(op, BinaryOp::Div | BinaryOp::Mod | BinaryOp::FloorDiv) && is_zero(right) {
        return ConstEvalProblemKind::DivisionByZero;
    }

    if arithmetic_operands(left, right) {
        ConstEvalProblemKind::Overflow
    } else {
        ConstEvalProblemKind::UnsupportedExpression {
            form: "operator outside the primitive constant-value domain",
        }
    }
}

fn unary_failure(_op: UnaryOp, value: &ConstValue) -> ConstEvalProblemKind {
    if matches!(
        value,
        ConstValue::Int(_) | ConstValue::Duration { .. } | ConstValue::Size { .. }
    ) {
        ConstEvalProblemKind::Overflow
    } else {
        ConstEvalProblemKind::UnsupportedExpression {
            form: "unary operator outside the primitive constant-value domain",
        }
    }
}

fn arithmetic_operands(left: &ConstValue, right: &ConstValue) -> bool {
    matches!(
        left,
        ConstValue::Int(_)
            | ConstValue::Float(_)
            | ConstValue::Duration { .. }
            | ConstValue::Size { .. }
    ) && matches!(
        right,
        ConstValue::Int(_)
            | ConstValue::Float(_)
            | ConstValue::Duration { .. }
            | ConstValue::Size { .. }
    )
}

fn is_zero(value: &ConstValue) -> bool {
    match value {
        ConstValue::Int(value) => *value == 0,
        ConstValue::Float(bits) => f64::from_bits(*bits) == 0.0,
        ConstValue::Duration { value, .. } | ConstValue::Size { value, .. } => *value == 0,
        _ => false,
    }
}

fn unsupported_form(expression: &CanExpr) -> &'static str {
    match expression {
        CanExpr::Ident(_) | CanExpr::SelfRef | CanExpr::HashLength => "runtime reference",
        CanExpr::FunctionRef(_) | CanExpr::TypeRef(_) | CanExpr::Call { .. } => "function call",
        CanExpr::MethodCall { .. } => "method call",
        CanExpr::List(_) | CanExpr::Tuple(_) | CanExpr::Map(_) | CanExpr::Struct { .. } => {
            "composite value"
        }
        CanExpr::Field { .. } | CanExpr::Index { .. } => "runtime projection",
        CanExpr::Block { .. } | CanExpr::Let { .. } | CanExpr::Assign { .. } => {
            "binding or mutation"
        }
        CanExpr::Match { .. } | CanExpr::For { .. } | CanExpr::Loop { .. } => "control flow",
        CanExpr::WithCapability { .. } => "capability use",
        CanExpr::Try(_) | CanExpr::Await(_) => "effectful error or async operation",
        _ => "expression outside the primitive constant-value domain",
    }
}

#[cfg(test)]
mod tests;
