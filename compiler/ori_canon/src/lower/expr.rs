//! Expression dispatch — the main `lower_expr` function.
//!
//! Contains the central match dispatch that maps each `ExprKind` variant
//! to its `CanExpr` equivalent, plus `lower_expr_range` and `lower_stmt_range`
//! helpers for lowering expression/statement lists.

use ori_ir::canon::{CanExpr, CanId, CanRange};
use ori_ir::{ExprId, ExprKind, ExprRange, Name, Span, TypeId, UnaryOp};
use tracing::trace;

use super::Lowerer;

impl Lowerer<'_> {
    // Expression Lowering

    /// Lower a single expression from `ExprId` to `CanId`.
    ///
    /// This is the main dispatch function. It copies the [`ExprKind`] out of
    /// the source arena (`ExprKind` is `Copy`), then matches on it to produce
    /// a `CanExpr`. This avoids borrow conflicts — we don't hold a reference
    /// to `self.src` while mutating `self.arena`.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind → CanExpr lowering dispatch"
    )]
    pub(crate) fn lower_expr(&mut self, id: ExprId) -> CanId {
        // Index/field-assignment hoisting: while an assignment desugar is in
        // flight, a source index `ExprId` resolves to its hoisted temporary
        // (`let $__assign_idx_N`) instead of being re-lowered, so a
        // side-effecting index (`arr[f()] += 1`) runs `f()` exactly once
        // across the parser-shared read-copy and write-copy.
        if !self.index_temp_overrides.is_empty() {
            if let Some(&temp) = self.index_temp_overrides.get(&id) {
                return temp;
            }
        }

        let kind = *self.src.expr_kind(id);
        let span = self.src.expr_span(id);
        let ty = self.expr_type(id);
        trace!(?id, ?kind, "lower_expr");

        match kind {
            // Leaf nodes — direct mapping
            ExprKind::Int(v) => self.push(CanExpr::Int(v), span, ty),
            ExprKind::Float(bits) => self.push(CanExpr::Float(bits), span, ty),
            ExprKind::Bool(v) => self.push(CanExpr::Bool(v), span, ty),
            ExprKind::String(name) => self.push(CanExpr::Str(name), span, ty),
            ExprKind::Char(c) => self.push(CanExpr::Char(c), span, ty),
            ExprKind::Duration { value, unit } => {
                self.push(CanExpr::Duration { value, unit }, span, ty)
            }
            ExprKind::Size { value, unit } => self.push(CanExpr::Size { value, unit }, span, ty),
            ExprKind::Unit => self.push(CanExpr::Unit, span, ty),
            ExprKind::None => self.push(CanExpr::None, span, ty),
            ExprKind::Ident(name) => {
                if self.is_type_reference(name, ori_types::Idx::from_raw(ty.raw())) {
                    self.push(CanExpr::TypeRef(name), span, ty)
                } else {
                    self.push(CanExpr::Ident(name), span, ty)
                }
            }
            ExprKind::Const(name) => {
                if let Some(value) = self.named_constants.get(&name).cloned() {
                    let constant = self.constants.intern(value);
                    self.push(CanExpr::Constant(constant), span, ty)
                } else {
                    // Generic const parameters and error-recovery references
                    // retain their name. Module constants are populated before
                    // executable roots and therefore never take this branch in
                    // a successful module.
                    self.push(CanExpr::Const(name), span, ty)
                }
            }
            ExprKind::SelfRef => self.push(CanExpr::SelfRef, span, ty),
            ExprKind::FunctionRef(name) => self.push(CanExpr::FunctionRef(name), span, ty),
            ExprKind::HashLength => self.push(CanExpr::HashLength, span, ty),
            ExprKind::Error => self.push(CanExpr::Error, span, ty),

            // Unary nodes — lower child
            ExprKind::Unary { op, operand } => {
                let operand = self.lower_expr(operand);
                let id = self.push(CanExpr::Unary { op, operand }, span, ty);
                if let Some(folded) =
                    crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id)
                {
                    folded
                } else {
                    id
                }
            }
            ExprKind::Ok(inner) => {
                let inner = self.lower_optional(inner);
                self.push(CanExpr::Ok(inner), span, ty)
            }
            ExprKind::Err(inner) => {
                let inner = self.lower_optional(inner);
                self.push(CanExpr::Err(inner), span, ty)
            }
            ExprKind::Some(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Some(inner), span, ty)
            }
            ExprKind::Break { label, value } => {
                let val = self.lower_optional(value);
                self.push(CanExpr::Break { label, value: val }, span, ty)
            }
            ExprKind::Continue { label, value } => {
                let val = self.lower_optional(value);
                self.push(CanExpr::Continue { label, value: val }, span, ty)
            }
            ExprKind::Unsafe(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Unsafe(inner), span, ty)
            }
            ExprKind::Await(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Await(inner), span, ty)
            }
            ExprKind::Try(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Try(inner), span, ty)
            }
            ExprKind::Loop { label, body } => {
                let body = self.lower_expr(body);
                self.push(CanExpr::Loop { label, body }, span, ty)
            }
            ExprKind::While { label, cond, body } => self.desugar_while(label, cond, body, span),

            // Binary nodes — lower both children
            ExprKind::Binary { op, left, right } => {
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let id = self.push(CanExpr::Binary { op, left, right }, span, ty);
                if let Some(folded) =
                    crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id)
                {
                    folded
                } else {
                    id
                }
            }
            ExprKind::Cast {
                expr,
                ty: cast_ty,
                fallible,
            } => {
                let expr = self.lower_expr(expr);
                let target = self.extract_cast_target_name(cast_ty);
                self.push(
                    CanExpr::Cast {
                        expr,
                        target,
                        fallible,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver);
                self.push(CanExpr::Field { receiver, field }, span, ty)
            }
            ExprKind::Index { receiver, index } => {
                let receiver = self.lower_expr(receiver);
                let index = self.lower_expr(index);
                let producer = self.typed.index_dispatch_map.get(id).copied();
                self.push(
                    CanExpr::Index {
                        receiver,
                        index,
                        producer,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Assign { target, value } => {
                // Type-directed index/field-assignment desugar: when `target`
                // is an `AssignTarget` the type checker planned, rewrite to the
                // pure-reassignment form (`root = root.updated(...)` /
                // `{ ...root, f: v }`) so AIMS never sees a
                // `CanExpr::Assign { target: Index/Field }`. The plan carries
                // the resolved per-level types (the type checker's
                // type-direction decision); synthesis happens here because the
                // source arena is immutable during type checking.
                if let ExprKind::AssignTarget { root, steps } = *self.src.expr_kind(target) {
                    if self.typed.resolve_assign_desugar(target).is_some() {
                        return self.desugar_assign_target(target, root, steps, value, span, ty);
                    }
                }
                let target = self.lower_expr(target);
                let value = self.lower_expr(value);
                self.push(CanExpr::Assign { target, value }, span, ty)
            }
            // AssignTarget is eliminated by the type-directed desugar (a later
            // phase); until then it lowers to a left-associated Index/Field
            // chain so it mirrors the index/field-assignment path exactly — the
            // evaluator raises the same clean "not yet supported" diagnostic.
            ExprKind::AssignTarget { root, steps } => {
                self.lower_raw_access_chain(root, steps, span, ty)
            }

            // Control flow
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(cond);
                let then_branch = self.lower_expr(then_branch);
                let else_branch = self.lower_optional(else_branch);
                let id = self.push(
                    CanExpr::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    span,
                    ty,
                );
                if let Some(folded) =
                    crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id)
                {
                    folded
                } else {
                    id
                }
            }
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => {
                let can_pattern = self.lower_binding_pattern(pattern);
                let iter = self.lower_expr(iter);
                let guard = self.lower_optional(guard);
                let body = self.lower_expr(body);
                self.push(
                    CanExpr::For {
                        label,
                        pattern: can_pattern,
                        iter,
                        guard,
                        body,
                        is_yield,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => {
                let provider = self.lower_expr(provider);
                let body = self.lower_expr(body);
                self.push(
                    CanExpr::WithCapability {
                        capability,
                        provider,
                        body,
                    },
                    span,
                    ty,
                )
            }

            // Special forms
            ExprKind::FunctionSeq(seq_id) => self.lower_function_seq(seq_id, span, ty),
            ExprKind::FunctionExp(exp_id) => self.lower_function_exp(exp_id, span, ty),

            // Containers
            ExprKind::Call { func, args } => self.lower_call(id, func, args, span, ty),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(id, receiver, method, args, span, ty),
            ExprKind::Block { stmts, result } => self.lower_block(stmts, result, span, ty),
            ExprKind::Let {
                pattern,
                ty: _let_ty,
                init,
                mutable,
            } => {
                let init = self.lower_expr(init);
                let pattern = self.lower_binding_pattern(pattern);
                self.push(
                    CanExpr::Let {
                        pattern,
                        init,
                        mutable,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Lambda {
                params,
                ret_ty: _,
                body,
            } => {
                let body = self.lower_expr(body);
                let params = self.lower_params(params);
                self.push(CanExpr::Lambda { params, body }, span, ty)
            }
            ExprKind::List(exprs) => self.lower_list(exprs, span, ty),
            ExprKind::Tuple(exprs) => self.lower_tuple(exprs, span, ty),
            ExprKind::Map(entries) => self.lower_map(entries, span, ty),
            ExprKind::Struct { name, fields } => self.lower_struct(name, fields, span, ty),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => {
                let start = self.lower_optional(start);
                let end = self.lower_optional(end);
                let step = self.lower_optional(step);
                self.push(
                    CanExpr::Range {
                        start,
                        end,
                        step,
                        inclusive,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Match { scrutinee, arms } => self.lower_match(scrutinee, arms, span, ty),

            // Sugar variants
            ExprKind::TemplateFull(name) => {
                // Trivial desugaring: template without interpolation is just a string.
                self.push(CanExpr::Str(name), span, ty)
            }
            ExprKind::TemplateLiteral { head, parts } => {
                self.desugar_template_literal(head, parts, span, ty)
            }
            ExprKind::CallNamed { func, args } => self.desugar_call_named(id, func, args, span, ty),
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.desugar_method_call_named(id, receiver, method, args, span, ty),
            ExprKind::ListWithSpread(elements) => self.desugar_list_with_spread(elements, span, ty),
            ExprKind::MapWithSpread(elements) => self.desugar_map_with_spread(elements, span, ty),
            ExprKind::StructWithSpread { name, fields } => {
                self.desugar_struct_with_spread(name, fields, span, ty)
            }
        }
    }

    /// Desugar `while[:label] cond do body` into `loop[:label] { if !cond then break[:label]; body }`.
    ///
    /// Every synthetic node carries the source `while` span. The loop body is a
    /// `Block` whose single statement is the `if !cond then break` guard and whose
    /// result is the lowered body. The loop, block, and guard are all `void`; the
    /// negated condition is `bool`; the break is `Never`. Desugaring here means the
    /// evaluator and LLVM backend need zero new code — both already lower `Loop`,
    /// `If`, `Unary`, `Break`, and `Block`.
    fn desugar_while(&mut self, label: Name, cond: ExprId, body: ExprId, span: Span) -> CanId {
        let cond_can = self.lower_expr(cond);
        let not_cond = self.push(
            CanExpr::Unary {
                op: UnaryOp::Not,
                operand: cond_can,
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
                cond: not_cond,
                then_branch: break_node,
                else_branch: CanId::INVALID,
            },
            span,
            TypeId::UNIT,
        );

        let body_can = self.lower_expr(body);

        let stmts = self.arena.push_expr_list(&[guard]);
        let loop_body = self.push(
            CanExpr::Block {
                stmts,
                result: body_can,
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

    // Range Lowering Helpers

    /// Lower an `ExprRange` (expression list) to a `CanRange`.
    pub(super) fn lower_expr_range(&mut self, range: ExprRange) -> CanRange {
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
    /// - `StmtKind::Expr(id)` → lower the expression
    /// - `StmtKind::Let { .. }` → emit a `CanExpr::Let` node
    pub(super) fn lower_stmt_range(&mut self, range: ori_ir::StmtRange) -> CanRange {
        let stmts = self.src.get_stmt_range(range);
        if stmts.is_empty() {
            return CanRange::EMPTY;
        }

        // Copy stmts out to avoid borrow conflict.
        let stmts: Vec<ori_ir::Stmt> = stmts.to_vec();

        // Lower all statements BEFORE building the expr list. lower_expr may
        // recursively lower nested match/block expressions whose own
        // start/push/finish cycles would corrupt our range.
        let lowered: Vec<CanId> = stmts
            .iter()
            .map(|stmt| match &stmt.kind {
                ori_ir::StmtKind::Expr(expr_id) => self.lower_expr(*expr_id),
                ori_ir::StmtKind::Let {
                    pattern,
                    ty: _,
                    init,
                    mutable,
                } => {
                    let init = self.lower_expr(*init);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init,
                            mutable: *mutable,
                        },
                        stmt.span,
                        TypeId::UNIT,
                    )
                }
            })
            .collect();

        self.arena.push_expr_list(&lowered)
    }
}
