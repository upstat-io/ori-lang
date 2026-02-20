//! Function sequence desugaring — `FunctionSeq` variants to canonical IR.
//!
//! Handles lowering of `FunctionSeq` variants (Try, Match, `ForPattern`)
//! into primitive `CanExpr` nodes.

use ori_ir::canon::{CanExpr, CanId, CanRange};
use ori_ir::{Name, Span, TypeId};

use super::Lowerer;

impl Lowerer<'_> {
    // FunctionSeq Desugaring

    /// Lower a `FunctionSeq` (`ExprArena` side-table) into primitive `CanExpr` nodes.
    ///
    /// Each `FunctionSeq` variant is desugared:
    /// - `Try { stmts, result }` → `Block` with Try-wrapped statements
    /// - `Match { scrutinee, arms }` → `Match` with decision tree
    /// - `ForPattern { over, map, arm, default }` → `For` with match body
    pub(super) fn lower_function_seq(
        &mut self,
        seq_id: ori_ir::FunctionSeqId,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let seq = self.src.get_function_seq(seq_id).clone();
        match seq {
            ori_ir::FunctionSeq::Try { stmts, result, .. } => {
                let lowered_stmts = self.lower_try_stmts(stmts);
                let result = self.lower_expr(result);
                self.push(
                    CanExpr::Block {
                        stmts: lowered_stmts,
                        result,
                    },
                    span,
                    ty,
                )
            }
            ori_ir::FunctionSeq::Match {
                scrutinee, arms, ..
            } => self.lower_match(scrutinee, arms, span, ty),
            ori_ir::FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default: _,
                ..
            } => {
                // Desugar ForPattern into a For expression.
                // The arm's pattern becomes a match inside the for body.
                let iter = self.lower_expr(over);

                // If there's a map transform, emit Error — ForPattern map
                // semantics are not fully specified yet. Emitting a MethodCall
                // with a wrong method name (e.g., "concat") would cause
                // backends to dispatch incorrectly.
                let iter = if map.is_some() {
                    self.push(CanExpr::Error, span, ty)
                } else {
                    iter
                };

                // Lower the arm body directly.
                // Note: we do NOT lower the default expression — ForPattern
                // default handling is deferred. Lowering it would allocate
                // orphaned nodes in the arena.
                let body = self.lower_expr(arm.body);

                // Use a simple for with the binding from the arm pattern.
                let binding = match &arm.pattern {
                    ori_ir::MatchPattern::Binding(name) => *name,
                    // Wildcard and complex patterns: simplified for now
                    _ => Name::EMPTY,
                };

                let guard = arm.guard.map_or(CanId::INVALID, |g| self.lower_expr(g));

                self.push(
                    CanExpr::For {
                        label: Name::EMPTY,
                        binding,
                        iter,
                        guard,
                        body,
                        is_yield: true,
                    },
                    span,
                    ty,
                )
            }
        }
    }

    // Try Statement Lowering

    /// Lower try statements — each statement wrapped in Try.
    fn lower_try_stmts(&mut self, range: ori_ir::StmtRange) -> CanRange {
        let stmts = self.src.get_stmt_range(range);
        if stmts.is_empty() {
            return CanRange::EMPTY;
        }

        let stmts: Vec<_> = stmts.to_vec();

        // Lower all statements before building the expr list.
        let lowered: Vec<CanId> = stmts
            .iter()
            .map(|stmt| match &stmt.kind {
                ori_ir::StmtKind::Let {
                    pattern,
                    ty: _,
                    init,
                    mutable,
                } => {
                    let value = self.lower_expr(*init);
                    let tried_value = self.push(CanExpr::Try(value), stmt.span, TypeId::ERROR);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init: tried_value,
                            mutable: *mutable,
                        },
                        stmt.span,
                        TypeId::UNIT,
                    )
                }
                ori_ir::StmtKind::Expr(expr) => {
                    let value = self.lower_expr(*expr);
                    self.push(CanExpr::Try(value), stmt.span, TypeId::ERROR)
                }
            })
            .collect();

        let start = self.arena.start_expr_list();
        for can_id in lowered {
            self.arena.push_expr_list_item(can_id);
        }
        self.arena.finish_expr_list(start)
    }
}
