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
        // Literals, already-folded constants, and $-bindings are compile-time.
        CanExpr::Int(_)
        | CanExpr::Float(_)
        | CanExpr::Bool(_)
        | CanExpr::Str(_)
        | CanExpr::Char(_)
        | CanExpr::Unit
        | CanExpr::Duration { .. }
        | CanExpr::Size { .. }
        | CanExpr::Constant(_)
        | CanExpr::Const(_) => Constness::Const,

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
        CanExpr::Constant(cid) => Some(constants.get(*cid).clone()),
        _ => None,
    }
}

// Binary Folding

/// Evaluate a binary operation on two constant values.
///
/// Returns `None` if the operation would cause undefined behavior
/// (division by zero, integer overflow) — these are deferred to runtime.
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
        (UnaryOp::Not, ConstValue::Bool(v)) => Some(ConstValue::Bool(!v)),
        (UnaryOp::BitNot, ConstValue::Int(v)) => Some(ConstValue::Int(!v)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::ast::Expr;
    use ori_ir::canon::{CanExpr, ConstValue};
    use ori_ir::{BinaryOp, ExprArena, ExprKind, SharedInterner, Span, UnaryOp};
    use ori_types::{Idx, TypeCheckResult, TypedModule};

    use crate::lower;

    fn test_type_result(expr_types: Vec<Idx>) -> TypeCheckResult {
        let mut typed = TypedModule::new();
        for idx in expr_types {
            typed.expr_types.push(idx);
        }
        TypeCheckResult::ok(typed)
    }

    fn test_interner() -> SharedInterner {
        SharedInterner::new()
    }

    // ── Binary folding ─────────────────────────────────────────

    #[test]
    fn fold_int_addition() {
        // 1 + 2 → Constant(3)
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
            Span::new(0, 5),
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);

        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Int(3));
            }
            other => panic!("expected Constant(3), got {other:?}"),
        }
    }

    #[test]
    fn fold_int_subtraction() {
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Sub,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Int(7));
            }
            other => panic!("expected Constant(7), got {other:?}"),
        }
    }

    #[test]
    fn fold_int_multiplication() {
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(6), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(7), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Mul,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Int(42));
            }
            other => panic!("expected Constant(42), got {other:?}"),
        }
    }

    #[test]
    fn no_fold_division_by_zero() {
        // 10 / 0 should NOT be folded (runtime error).
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Div,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        // Should remain a Binary node (not folded).
        assert!(
            matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
            "division by zero should not be folded"
        );
    }

    #[test]
    fn no_fold_integer_overflow() {
        // i64::MAX + 1 should NOT be folded.
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(i64::MAX), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(
            matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
            "integer overflow should not be folded"
        );
    }

    #[test]
    fn fold_bool_and() {
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Bool(false), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::And,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::BOOL, Idx::BOOL, Idx::BOOL]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Bool(false));
            }
            other => panic!("expected Constant(false), got {other:?}"),
        }
    }

    #[test]
    fn fold_int_comparison() {
        // 3 < 5 → Constant(true)
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Lt,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::BOOL]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Bool(true));
            }
            other => panic!("expected Constant(true), got {other:?}"),
        }
    }

    // ── Unary folding ──────────────────────────────────────────

    #[test]
    fn fold_negation() {
        // -42 → Constant(-42)
        let mut arena = ExprArena::new();
        let operand = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Int(-42));
            }
            other => panic!("expected Constant(-42), got {other:?}"),
        }
    }

    #[test]
    fn fold_not() {
        // !true → Constant(false)
        let mut arena = ExprArena::new();
        let operand = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::BOOL, Idx::BOOL]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Bool(false));
            }
            other => panic!("expected Constant(false), got {other:?}"),
        }
    }

    // ── Dead branch elimination ────────────────────────────────

    #[test]
    fn dead_branch_if_true() {
        // if true { 42 } else { 99 } → 42
        let mut arena = ExprArena::new();
        let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
        let then_br = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
        let else_br = arena.alloc_expr(Expr::new(ExprKind::Int(99), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::If {
                cond,
                then_branch: then_br,
                else_branch: else_br,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        // Should be the then-branch value directly (Int(42)).
        assert_eq!(
            *result.arena.kind(result.root),
            CanExpr::Int(42),
            "if true should eliminate to then-branch"
        );
    }

    #[test]
    fn dead_branch_if_false() {
        // if false { 42 } else { 99 } → 99
        let mut arena = ExprArena::new();
        let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(false), Span::DUMMY));
        let then_br = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
        let else_br = arena.alloc_expr(Expr::new(ExprKind::Int(99), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::If {
                cond,
                then_branch: then_br,
                else_branch: else_br,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert_eq!(
            *result.arena.kind(result.root),
            CanExpr::Int(99),
            "if false should eliminate to else-branch"
        );
    }

    // ── Bitwise folding ────────────────────────────────────────

    #[test]
    fn fold_bitwise_and() {
        // 0xFF & 0x0F → 0x0F
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(0xFF), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(0x0F), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::BitAnd,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Int(0x0F));
            }
            other => panic!("expected Constant(0x0F), got {other:?}"),
        }
    }

    #[test]
    fn fold_shift_left() {
        // 1 << 4 → 16
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(4), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Shl,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();
        let interner = test_interner();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(*result.constants.get(*cid), ConstValue::Int(16));
            }
            other => panic!("expected Constant(16), got {other:?}"),
        }
    }

    // ── Runtime expressions not folded ─────────────────────────

    #[test]
    fn no_fold_runtime_binary() {
        // x + 1 should NOT be folded (x is a runtime variable).
        let mut arena = ExprArena::new();
        let interner = test_interner();
        let name_x = interner.intern("x");

        let left = arena.alloc_expr(Expr::new(ExprKind::Ident(name_x), Span::DUMMY));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
            Span::DUMMY,
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let pool = ori_types::Pool::new();

        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(
            matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
            "runtime binary should not be folded"
        );
    }
}
