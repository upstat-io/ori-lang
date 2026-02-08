//! Expression lowering — the core dispatch for AST → ARC IR.
//!
//! [`ArcLowerer`] walks the typed expression tree and emits ARC IR
//! instructions via [`ArcIrBuilder`]. Each expression lowers to an
//! [`ArcVarId`] (the SSA variable holding the result).

use ori_ir::ast::ExprKind;
use ori_ir::{ExprArena, ExprId, Name, Span, StringInterner};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::{ArcFunction, ArcValue, ArcVarId, LitValue, PrimOp};

use super::scope::ArcScope;
use super::{ArcIrBuilder, ArcProblem};

// ── Loop context ───────────────────────────────────────────────────

/// Context for the enclosing loop (used by `break`/`continue`).
pub(crate) struct LoopContext {
    /// Block to jump to on `break`.
    pub exit_block: crate::ir::ArcBlockId,
    /// Block to jump to on `continue`.
    pub continue_block: crate::ir::ArcBlockId,
    /// Mutable variable types for SSA merge at loop header.
    pub mutable_var_types: rustc_hash::FxHashMap<Name, Idx>,
}

// ── ArcLowerer ─────────────────────────────────────────────────────

/// Expression lowerer that walks the typed AST and emits ARC IR.
///
/// Borrows the `ArcIrBuilder` and contextual data (arena, types, interner,
/// pool) needed to lower each expression variant.
pub struct ArcLowerer<'a> {
    pub(crate) builder: &'a mut ArcIrBuilder,
    pub(crate) arena: &'a ExprArena,
    pub(crate) expr_types: &'a [Idx],
    pub(crate) interner: &'a StringInterner,
    pub(crate) pool: &'a Pool,
    pub(crate) scope: ArcScope,
    pub(crate) loop_ctx: Option<LoopContext>,
    pub(crate) problems: &'a mut Vec<ArcProblem>,
    pub(crate) lambdas: &'a mut Vec<ArcFunction>,
}

impl ArcLowerer<'_> {
    /// Get the type of an expression by its ID.
    #[inline]
    pub(crate) fn expr_type(&self, id: ExprId) -> Idx {
        let idx = id.index();
        if idx < self.expr_types.len() {
            self.expr_types[idx]
        } else {
            Idx::ERROR
        }
    }

    /// Emit a unit literal.
    pub(crate) fn emit_unit(&mut self) -> ArcVarId {
        self.builder
            .emit_let(Idx::UNIT, ArcValue::Literal(LitValue::Unit), None)
    }

    // ── Main dispatch ──────────────────────────────────────────

    /// Lower a single expression, returning the `ArcVarId` of the result.
    pub(crate) fn lower_expr(&mut self, expr_id: ExprId) -> ArcVarId {
        if !expr_id.is_valid() {
            return self.emit_unit();
        }

        let expr = self.arena.get_expr(expr_id);
        let span = expr.span;
        let ty = self.expr_type(expr_id);

        match expr.kind {
            // ── Literals ───────────────────────────────────────
            ExprKind::Int(n) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Int(n)), Some(span))
            }
            ExprKind::Float(bits) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Float(bits)), Some(span))
            }
            ExprKind::Bool(b) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Bool(b)), Some(span))
            }
            ExprKind::String(name) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::String(name)), Some(span))
            }
            ExprKind::Char(c) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Char(c)), Some(span))
            }
            ExprKind::Duration { value, unit } => self.builder.emit_let(
                ty,
                ArcValue::Literal(LitValue::Duration { value, unit }),
                Some(span),
            ),
            ExprKind::Size { value, unit } => self.builder.emit_let(
                ty,
                ArcValue::Literal(LitValue::Size { value, unit }),
                Some(span),
            ),
            ExprKind::Unit => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
            }

            // ── Identifiers ───────────────────────────────────
            ExprKind::Ident(name) | ExprKind::Const(name) => self.lower_ident(name, ty, span),
            ExprKind::SelfRef => {
                // `self` is pre-bound as a parameter — look it up.
                let self_name = self.interner.intern("self");
                self.lower_ident(self_name, ty, span)
            }
            ExprKind::FunctionRef(_name) => {
                // Function references lower to a closure construct with no captures.
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
            }
            ExprKind::HashLength => {
                // `#` in index context — placeholder for collection length.
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
            }

            // ── Binary / Unary operators ──────────────────────
            ExprKind::Binary { op, left, right } => self.lower_binary(op, left, right, ty, span),
            ExprKind::Unary { op, operand } => self.lower_unary(op, operand, ty, span),

            // ── Control flow ──────────────────────────────────
            ExprKind::Block { stmts, result } => self.lower_block(stmts, result, ty),
            ExprKind::Let {
                pattern,
                ty: _parsed_ty,
                init,
                mutable,
            } => self.lower_let(pattern, init, mutable),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if(cond, then_branch, else_branch, ty, span),
            ExprKind::Match { scrutinee, arms } => self.lower_match(scrutinee, arms, ty, span),
            ExprKind::Loop { body } => self.lower_loop(body, ty),
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield: _,
            } => self.lower_for(binding, iter, guard, body, ty),
            ExprKind::Break(value) => self.lower_break(value),
            ExprKind::Continue(value) => self.lower_continue(value),
            ExprKind::Assign { target, value } => self.lower_assign(target, value, span),

            // ── Collections & constructors ────────────────────
            ExprKind::Tuple(exprs) => self.lower_tuple(exprs, ty, span),
            ExprKind::List(exprs) => self.lower_list(exprs, ty, span),
            ExprKind::Map(entries) => self.lower_map(entries, ty, span),
            ExprKind::Struct { name, fields } => self.lower_struct(name, fields, ty, span),
            ExprKind::Ok(inner) => self.lower_ok(inner, ty, span),
            ExprKind::Err(inner) => self.lower_err(inner, ty, span),
            ExprKind::Some(inner) => self.lower_some(inner, ty, span),
            ExprKind::None => self.lower_none(ty, span),
            ExprKind::Field { receiver, field } => self.lower_field(receiver, field, ty, span),
            ExprKind::Index { receiver, index } => self.lower_index(receiver, index, ty, span),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range(start, end, step, inclusive, ty, span),
            ExprKind::Try(inner) => self.lower_try(inner, ty, span),
            ExprKind::Cast {
                expr,
                ty: _parsed_ty,
                fallible,
            } => self.lower_cast(expr, fallible, ty, span),
            ExprKind::ListWithSpread(elements) => self.lower_list_with_spread(elements, ty, span),
            ExprKind::MapWithSpread(elements) => self.lower_map_with_spread(elements, ty, span),
            ExprKind::StructWithSpread { name, fields } => {
                self.lower_struct_with_spread(name, fields, ty, span)
            }
            ExprKind::TemplateFull(name) => self.lower_template_full(name, ty, span),
            ExprKind::TemplateLiteral { head, parts } => {
                self.lower_template_literal(head, parts, ty, span)
            }

            // ── Calls ─────────────────────────────────────────
            ExprKind::Call { func, args } => self.lower_call(func, args, ty, span),
            ExprKind::CallNamed { func, args } => self.lower_call_named(func, args, ty, span),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(receiver, method, args, ty, span),
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.lower_method_call_named(receiver, method, args, ty, span),
            ExprKind::Lambda {
                params,
                ret_ty: _,
                body,
            } => self.lower_lambda(params, body, ty, span),

            // ── Unsupported (post-0.1-alpha) ──────────────────
            ExprKind::Await(_) => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "Await",
                    span,
                });
                self.emit_unit()
            }
            ExprKind::WithCapability { .. } => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "WithCapability",
                    span,
                });
                self.emit_unit()
            }
            ExprKind::FunctionSeq(_) => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "FunctionSeq",
                    span,
                });
                self.emit_unit()
            }
            ExprKind::FunctionExp(_) => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "FunctionExp",
                    span,
                });
                self.emit_unit()
            }

            // ── Error recovery ────────────────────────────────
            ExprKind::Error => self.emit_unit(),
        }
    }

    // ── Identifier lowering ────────────────────────────────────

    fn lower_ident(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        if let Some(var) = self.scope.lookup(name) {
            // Emit a Var reference so the use is tracked.
            self.builder.emit_let(ty, ArcValue::Var(var), Some(span))
        } else {
            // Unbound identifier — might be a global or an error.
            // Emit a placeholder for now.
            tracing::debug!(
                name = ?name,
                "unbound identifier in ARC IR lowering"
            );
            self.builder
                .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
        }
    }

    // ── Binary / Unary operators ───────────────────────────────

    fn lower_binary(
        &mut self,
        op: ori_ir::BinaryOp,
        left: ExprId,
        right: ExprId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let lhs = self.lower_expr(left);
        let rhs = self.lower_expr(right);
        self.builder.emit_let(
            ty,
            ArcValue::PrimOp {
                op: PrimOp::Binary(op),
                args: vec![lhs, rhs],
            },
            Some(span),
        )
    }

    fn lower_unary(
        &mut self,
        op: ori_ir::UnaryOp,
        operand: ExprId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let arg = self.lower_expr(operand);
        self.builder.emit_let(
            ty,
            ArcValue::PrimOp {
                op: PrimOp::Unary(op),
                args: vec![arg],
            },
            Some(span),
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::ast::{Expr, ExprKind};
    use ori_ir::{BinaryOp, ExprArena, Name, Span, StringInterner, UnaryOp};
    use ori_types::Idx;
    use ori_types::Pool;

    use crate::ir::{ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};
    use crate::lower::ArcProblem;

    /// Helper: create a lowerer with a single expression body.
    fn lower_single_expr(kind: ExprKind, ty: Idx) -> crate::ir::ArcFunction {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let body_id = arena.alloc_expr(Expr::new(kind, Span::new(0, 10)));
        let expr_types = {
            let mut types = vec![Idx::ERROR; body_id.index() + 1];
            types[body_id.index()] = ty;
            types
        };

        let mut problems = Vec::new();
        let name = Name::from_raw(1);
        let (func, _lambdas) = super::super::lower_function(
            name,
            &[],
            ty,
            body_id,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );
        assert!(problems.is_empty(), "unexpected problems: {problems:?}");
        func
    }

    #[test]
    fn lower_int_literal() {
        let func = lower_single_expr(ExprKind::Int(42), Idx::INT);
        assert_eq!(func.blocks.len(), 1);
        assert_eq!(func.blocks[0].body.len(), 1);

        if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
            assert_eq!(*value, ArcValue::Literal(LitValue::Int(42)));
        } else {
            panic!("expected Let instruction");
        }
        assert!(matches!(
            func.blocks[0].terminator,
            ArcTerminator::Return { .. }
        ));
    }

    #[test]
    fn lower_bool_literal() {
        let func = lower_single_expr(ExprKind::Bool(true), Idx::BOOL);
        if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
            assert_eq!(*value, ArcValue::Literal(LitValue::Bool(true)));
        } else {
            panic!("expected Let");
        }
    }

    #[test]
    fn lower_unit_literal() {
        let func = lower_single_expr(ExprKind::Unit, Idx::UNIT);
        if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
            assert_eq!(*value, ArcValue::Literal(LitValue::Unit));
        } else {
            panic!("expected Let");
        }
    }

    #[test]
    fn lower_binary_op() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let left_id = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let right_id = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
        let add_id = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left: left_id,
                right: right_id,
            },
            Span::new(0, 5),
        ));

        let mut expr_types = vec![Idx::ERROR; add_id.index() + 1];
        expr_types[left_id.index()] = Idx::INT;
        expr_types[right_id.index()] = Idx::INT;
        expr_types[add_id.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            add_id,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        // Should have: let v0 = 1, let v1 = 2, let v2 = Add(v0, v1), return v2
        assert_eq!(func.blocks[0].body.len(), 3);
        if let ArcInstr::Let { value, .. } = &func.blocks[0].body[2] {
            assert!(matches!(
                value,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    ..
                }
            ));
        } else {
            panic!("expected PrimOp");
        }
    }

    #[test]
    fn lower_unary_op() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let operand_id = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::new(1, 2)));
        let neg_id = arena.alloc_expr(Expr::new(
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand: operand_id,
            },
            Span::new(0, 2),
        ));

        let mut expr_types = vec![Idx::ERROR; neg_id.index() + 1];
        expr_types[operand_id.index()] = Idx::INT;
        expr_types[neg_id.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            neg_id,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert_eq!(func.blocks[0].body.len(), 2);
        if let ArcInstr::Let { value, .. } = &func.blocks[0].body[1] {
            assert!(matches!(
                value,
                ArcValue::PrimOp {
                    op: PrimOp::Unary(UnaryOp::Neg),
                    ..
                }
            ));
        } else {
            panic!("expected PrimOp");
        }
    }

    #[test]
    fn lower_unsupported_expr_produces_problem() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        // Await is unsupported.
        let inner_id = arena.alloc_expr(Expr::new(ExprKind::Unit, Span::new(6, 10)));
        let await_id = arena.alloc_expr(Expr::new(ExprKind::Await(inner_id), Span::new(0, 10)));

        let mut expr_types = vec![Idx::ERROR; await_id.index() + 1];
        expr_types[inner_id.index()] = Idx::UNIT;
        expr_types[await_id.index()] = Idx::UNIT;

        let mut problems = Vec::new();
        let (_func, _) = super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::UNIT,
            await_id,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert_eq!(problems.len(), 1);
        assert!(matches!(
            &problems[0],
            ArcProblem::UnsupportedExpr { kind: "Await", .. }
        ));
    }

    #[test]
    fn lower_function_with_params() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let param_name = Name::from_raw(100);
        let body_id = arena.alloc_expr(Expr::new(ExprKind::Ident(param_name), Span::new(0, 1)));

        let mut expr_types = vec![Idx::ERROR; body_id.index() + 1];
        expr_types[body_id.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::lower_function(
            Name::from_raw(1),
            &[(param_name, Idx::INT)],
            Idx::INT,
            body_id,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert_eq!(func.params.len(), 1);
        assert_eq!(func.params[0].ty, Idx::INT);
        // The body should reference the param via ArcValue::Var.
        assert!(!func.blocks[0].body.is_empty());
    }
}
