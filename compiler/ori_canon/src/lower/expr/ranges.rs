//! Expression and statement range lowering.

use ori_ir::canon::{CanId, CanRange};
use ori_ir::{ExprId, ExprRange, TypeId};

use super::super::Lowerer;

impl Lowerer<'_> {
    /// Lower an `ExprRange` (expression list) to a `CanRange`.
    pub(in crate::lower) fn lower_expr_range(&mut self, range: ExprRange) -> CanRange {
        let src_ids = self.src.get_expr_list(range);
        if src_ids.is_empty() {
            return CanRange::EMPTY;
        }

        // Copy IDs out to avoid holding a borrow on src while mutating arena.
        let src_ids: Vec<ExprId> = src_ids.to_vec();
        let mut lowered = Vec::with_capacity(src_ids.len());
        for id in src_ids {
            lowered.push(self.lower_expr(id));
        }
        self.arena.push_expr_list(&lowered)
    }

    /// Lower a `StmtRange` (block statements) to a `CanRange`.
    ///
    /// Each statement is lowered to a canonical node:
    /// - `StmtKind::Expr(id)` -> lower the expression
    /// - `StmtKind::Let { .. }` -> emit a `CanExpr::Let` node
    pub(in crate::lower) fn lower_stmt_range(&mut self, range: ori_ir::StmtRange) -> CanRange {
        let stmts = self.src.get_stmt_range(range);
        if stmts.is_empty() {
            return CanRange::EMPTY;
        }

        // Copy stmts out to avoid borrow conflict.
        let stmts: Vec<ori_ir::Stmt> = stmts.to_vec();

        // Nested lowering must finish before range construction because nested
        // start/push/finish cycles would corrupt the outer range.
        let lowered: Vec<CanId> = stmts
            .iter()
            .map(|stmt| match &stmt.kind {
                ori_ir::StmtKind::Expr(expr_id) => self.lower_expr(*expr_id),
                ori_ir::StmtKind::Let {
                    pattern,
                    ty: _,
                    init,
                    mutable,
                } => self.lower_let_kind(*pattern, *init, *mutable, stmt.span, TypeId::UNIT),
            })
            .collect();

        self.arena.push_expr_list(&lowered)
    }
}
