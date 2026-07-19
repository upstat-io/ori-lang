//! Function sequence desugaring — `FunctionSeq` variants to canonical IR.
//!
//! Handles lowering of `FunctionSeq` variants (Try, Match, `ForPattern`)
//! into primitive `CanExpr` nodes.

use ori_ir::canon::{CanBindingPattern, CanExpr, CanId};
use ori_ir::{Mutability, Name, Span, TypeId};

use super::Lowerer;

impl Lowerer<'_> {
    // FunctionSeq Desugaring

    /// Lower a `FunctionSeq` (`ExprArena` side-table) into primitive `CanExpr` nodes.
    ///
    /// Each `FunctionSeq` variant is desugared:
    /// - `Try { stmts, result }` → carrier-wrapped `Block` over a normal statement range
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
                // Why: a source-level `?` is already `CanExpr::Try` (from `lower_expr`);
                // the statements therefore lower as a normal range. The try boundary
                // still constructs the successful carrier around a bare block value.
                // Spec: Clause 16.7.3.
                let lowered_stmts = self.lower_stmt_range(stmts);
                // Why: tail-less try block (`result == ExprId::INVALID`) uses the
                // optional-child helper (like `lower_block`) so it lowers to
                // `CanExpr::Block { result: CanId::INVALID }`, not a sentinel index.
                let lowered_result = self.lower_optional(result);
                let result_ty = if result.is_valid() {
                    self.expr_type(result)
                } else {
                    TypeId::UNIT
                };
                let outer = self.pool.resolve_fully(ori_types::Idx::from_raw(ty.raw()));
                let outer_tag = self.pool.tag(outer);
                let result_tag = self.pool.tag(
                    self.pool
                        .resolve_fully(ori_types::Idx::from_raw(result_ty.raw())),
                );
                let tail_is_wrapped =
                    matches!(outer_tag, ori_types::Tag::Result | ori_types::Tag::Option)
                        && result_tag == outer_tag;

                let block_ty = if tail_is_wrapped { ty } else { result_ty };
                let block = self.push(
                    CanExpr::Block {
                        stmts: lowered_stmts,
                        result: lowered_result,
                    },
                    span,
                    block_ty,
                );

                if tail_is_wrapped {
                    block
                } else {
                    match outer_tag {
                        ori_types::Tag::Result => self.push(CanExpr::Ok(block), span, ty),
                        ori_types::Tag::Option => self.push(CanExpr::Some(block), span, ty),
                        _ => block,
                    }
                }
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

                // Why: a `map` transform emits Error — ForPattern map semantics
                // are unspecified; a wrong method name (e.g. "concat") would
                // misdispatch in the backends.
                let iter = if map.is_some() {
                    self.push(CanExpr::Error, span, ty)
                } else {
                    iter
                };

                // Why: the `default` expr is NOT lowered — ForPattern default
                // handling is deferred, and lowering it would allocate orphaned
                // arena nodes.
                let body = self.lower_expr(arm.body);

                // Use a simple for with the binding from the arm pattern.
                let binding_name = match &arm.pattern {
                    ori_ir::MatchPattern::Binding(name) => *name,
                    // Wildcard and complex patterns: simplified for now
                    _ => Name::EMPTY,
                };
                let pattern = self.arena.push_binding_pattern(CanBindingPattern::Name {
                    name: binding_name,
                    mutable: Mutability::Immutable,
                });

                let guard = arm.guard.map_or(CanId::INVALID, |g| self.lower_expr(g));

                self.push(
                    CanExpr::For {
                        label: Name::EMPTY,
                        pattern,
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
}
