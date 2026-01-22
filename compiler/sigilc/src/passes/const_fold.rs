// Constant folding pass for Sigil TIR
// Folds constant expressions at compile time
//
// Uses the Folder trait - only overrides methods for expressions it transforms.

use super::{Pass, PassContext, PassError, PassResult};
use crate::ast::BinaryOp;
use crate::ast::Span;
use crate::ir::{Folder, TExpr, TExprKind, TModule, Type};

/// Constant folding optimization pass
/// Folds: 1 + 2 → 3, "a" + "b" → "ab", if true then x else y → x
pub struct ConstantFoldingPass;

impl Pass for ConstantFoldingPass {
    fn name(&self) -> &'static str {
        "constant_folding"
    }

    fn run(&self, ir: &mut TModule, _ctx: &mut PassContext) -> Result<PassResult, PassError> {
        let mut folder = ConstFolder::new();

        // Fold configs
        for config in &mut ir.configs {
            config.value = folder.fold_expr(config.value.clone());
        }

        // Fold functions
        for func in &mut ir.functions {
            func.body = folder.fold_expr(func.body.clone());
        }

        // Fold tests
        for test in &mut ir.tests {
            test.body = folder.fold_expr(test.body.clone());
        }

        if folder.count > 0 {
            Ok(PassResult::changed(folder.count))
        } else {
            Ok(PassResult::unchanged())
        }
    }
}

struct ConstFolder {
    count: usize,
}

impl ConstFolder {
    fn new() -> Self {
        ConstFolder { count: 0 }
    }

    fn try_fold_binary(&self, op: BinaryOp, left: &TExpr, right: &TExpr) -> Option<TExprKind> {
        match (&left.kind, &right.kind) {
            // Integer operations
            (TExprKind::Int(a), TExprKind::Int(b)) => match op {
                BinaryOp::Add => Some(TExprKind::Int(a + b)),
                BinaryOp::Sub => Some(TExprKind::Int(a - b)),
                BinaryOp::Mul => Some(TExprKind::Int(a * b)),
                BinaryOp::Div if *b != 0 => Some(TExprKind::Int(a / b)),
                BinaryOp::IntDiv if *b != 0 => Some(TExprKind::Int(a / b)),
                BinaryOp::Mod if *b != 0 => Some(TExprKind::Int(a % b)),
                BinaryOp::Eq => Some(TExprKind::Bool(a == b)),
                BinaryOp::NotEq => Some(TExprKind::Bool(a != b)),
                BinaryOp::Lt => Some(TExprKind::Bool(a < b)),
                BinaryOp::LtEq => Some(TExprKind::Bool(a <= b)),
                BinaryOp::Gt => Some(TExprKind::Bool(a > b)),
                BinaryOp::GtEq => Some(TExprKind::Bool(a >= b)),
                _ => None,
            },

            // Float operations
            (TExprKind::Float(a), TExprKind::Float(b)) => match op {
                BinaryOp::Add => Some(TExprKind::Float(a + b)),
                BinaryOp::Sub => Some(TExprKind::Float(a - b)),
                BinaryOp::Mul => Some(TExprKind::Float(a * b)),
                BinaryOp::Div if *b != 0.0 => Some(TExprKind::Float(a / b)),
                BinaryOp::Eq => Some(TExprKind::Bool(a == b)),
                BinaryOp::NotEq => Some(TExprKind::Bool(a != b)),
                BinaryOp::Lt => Some(TExprKind::Bool(a < b)),
                BinaryOp::LtEq => Some(TExprKind::Bool(a <= b)),
                BinaryOp::Gt => Some(TExprKind::Bool(a > b)),
                BinaryOp::GtEq => Some(TExprKind::Bool(a >= b)),
                _ => None,
            },

            // Boolean operations
            (TExprKind::Bool(a), TExprKind::Bool(b)) => match op {
                BinaryOp::And => Some(TExprKind::Bool(*a && *b)),
                BinaryOp::Or => Some(TExprKind::Bool(*a || *b)),
                BinaryOp::Eq => Some(TExprKind::Bool(a == b)),
                BinaryOp::NotEq => Some(TExprKind::Bool(a != b)),
                _ => None,
            },

            // String concatenation
            (TExprKind::String(a), TExprKind::String(b)) => match op {
                BinaryOp::Add => Some(TExprKind::String(format!("{}{}", a, b))),
                BinaryOp::Eq => Some(TExprKind::Bool(a == b)),
                BinaryOp::NotEq => Some(TExprKind::Bool(a != b)),
                _ => None,
            },

            _ => None,
        }
    }
}

impl Folder for ConstFolder {
    // Only override the methods we need to transform

    fn fold_binary(
        &mut self,
        op: BinaryOp,
        left: TExpr,
        right: TExpr,
        ty: Type,
        span: Span,
    ) -> TExpr {
        // First, recursively fold children
        let left = self.fold_expr(left);
        let right = self.fold_expr(right);

        // Try to fold if both are constants
        if let Some(folded) = self.try_fold_binary(op, &left, &right) {
            self.count += 1;
            return TExpr::new(folded, ty, span);
        }

        // Return unfolded
        TExpr::new(
            TExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            ty,
            span,
        )
    }

    fn fold_if(
        &mut self,
        cond: TExpr,
        then_branch: TExpr,
        else_branch: TExpr,
        ty: Type,
        span: Span,
    ) -> TExpr {
        // First, fold the condition
        let cond = self.fold_expr(cond);

        // If condition is constant, eliminate branch
        if let TExprKind::Bool(b) = cond.kind {
            self.count += 1;
            return if b {
                self.fold_expr(then_branch)
            } else {
                self.fold_expr(else_branch)
            };
        }

        // Otherwise, fold both branches
        let then_branch = self.fold_expr(then_branch);
        let else_branch = self.fold_expr(else_branch);

        TExpr::new(
            TExprKind::If {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            },
            ty,
            span,
        )
    }

    // All other expression types use the default implementation from Folder trait
    // which handles recursion automatically!
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Type;

    fn make_int(n: i64) -> TExpr {
        TExpr::new(TExprKind::Int(n), Type::Int, 0..1)
    }

    fn make_bool(b: bool) -> TExpr {
        TExpr::new(TExprKind::Bool(b), Type::Bool, 0..1)
    }

    #[test]
    fn test_fold_int_addition() {
        let mut folder = ConstFolder::new();
        let expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(make_int(1)),
                right: Box::new(make_int(2)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Int(3)));
        assert_eq!(folder.count, 1);
    }

    #[test]
    fn test_fold_if_true() {
        let mut folder = ConstFolder::new();
        let expr = TExpr::new(
            TExprKind::If {
                cond: Box::new(make_bool(true)),
                then_branch: Box::new(make_int(1)),
                else_branch: Box::new(make_int(2)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Int(1)));
        assert_eq!(folder.count, 1);
    }

    #[test]
    fn test_fold_string_concat() {
        let mut folder = ConstFolder::new();
        let expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(TExpr::new(
                    TExprKind::String("hello ".to_string()),
                    Type::Str,
                    0..1,
                )),
                right: Box::new(TExpr::new(
                    TExprKind::String("world".to_string()),
                    Type::Str,
                    0..1,
                )),
            },
            Type::Str,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::String(s) if s == "hello world"));
    }
}
