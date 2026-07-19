//! Canonical desugaring for `while` expressions.

use ori_ir::canon::{CanExpr, CanId};
use ori_ir::{ExprId, Name, Span, TypeId, UnaryOp};

use super::Lowerer;

impl Lowerer<'_> {
    /// Desugar `while[:label] cond do body` into a primitive labeled loop.
    ///
    /// Every synthetic node carries the source `while` span. The generated
    /// loop body checks `!cond`, breaks when true, then evaluates the body.
    pub(super) fn desugar_while(
        &mut self,
        label: Name,
        condition: ExprId,
        body: ExprId,
        span: Span,
    ) -> CanId {
        let condition = self.lower_expr(condition);
        let negated = self.push(
            CanExpr::Unary {
                op: UnaryOp::Not,
                operand: condition,
            },
            span,
            TypeId::BOOL,
        );
        let break_node = self.push(
            CanExpr::Break {
                label,
                value: CanId::INVALID,
            },
            span,
            TypeId::NEVER,
        );
        let guard = self.push(
            CanExpr::If {
                cond: negated,
                then_branch: break_node,
                else_branch: CanId::INVALID,
            },
            span,
            TypeId::UNIT,
        );
        let body = self.lower_expr(body);
        let statements = self.arena.push_expr_list(&[guard]);
        let loop_body = self.push(
            CanExpr::Block {
                stmts: statements,
                result: body,
            },
            span,
            TypeId::UNIT,
        );

        self.push(
            CanExpr::Loop {
                label,
                body: loop_body,
            },
            span,
            TypeId::UNIT,
        )
    }
}
