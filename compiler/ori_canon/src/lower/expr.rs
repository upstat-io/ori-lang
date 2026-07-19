//! Expression dispatch — the main `lower_expr` function.
//!
//! Contains the central match dispatch that maps each `ExprKind` variant
//! to its `CanExpr` equivalent, plus `lower_expr_range` and `lower_stmt_range`
//! helpers for lowering expression/statement lists.

mod basic;
mod ranges;

use ori_ir::canon::{CanExpr, CanId};
use ori_ir::{ExprId, ExprKind, TypeId};
use tracing::trace;

use super::Lowerer;

impl Lowerer<'_> {
    // Expression Lowering

    /// Lower a single expression from `ExprId` to `CanId`.
    ///
    /// This is the main dispatch function. It copies the [`ExprKind`] out of
    /// the source arena (`ExprKind` is `Copy`), then matches on it to produce
    /// a `CanExpr`. Copying releases the `self.src` borrow before `self.arena`
    /// is mutated.
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
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::None
            | ExprKind::Ident(_)
            | ExprKind::Const(_)
            | ExprKind::SelfRef
            | ExprKind::FunctionRef(_)
            | ExprKind::HashLength
            | ExprKind::Error => self.lower_leaf_kind(kind, span, ty),

            ExprKind::Unary { .. }
            | ExprKind::Ok(_)
            | ExprKind::Err(_)
            | ExprKind::Some(_)
            | ExprKind::Break { .. }
            | ExprKind::Continue { .. }
            | ExprKind::Unsafe(_)
            | ExprKind::Await(_)
            | ExprKind::Try(_)
            | ExprKind::Loop { .. }
            | ExprKind::While { .. } => self.lower_unary_kind(kind, span, ty),

            ExprKind::Binary { .. }
            | ExprKind::Cast { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index { .. }
            | ExprKind::Assign { .. }
            | ExprKind::AssignTarget { .. } => self.lower_access_kind(id, kind, span, ty),

            ExprKind::If { .. }
            | ExprKind::For { .. }
            | ExprKind::WithCapability { .. }
            | ExprKind::FunctionSeq(_)
            | ExprKind::FunctionExp(_) => self.lower_control_kind(kind, span, ty),

            ExprKind::Call { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::Block { .. }
            | ExprKind::Let { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::List(_)
            | ExprKind::Tuple(_)
            | ExprKind::Map(_)
            | ExprKind::Struct { .. }
            | ExprKind::Range { .. }
            | ExprKind::Match { .. } => self.lower_container_kind(id, kind, span, ty),

            ExprKind::TemplateFull(_)
            | ExprKind::TemplateLiteral { .. }
            | ExprKind::CallNamed { .. }
            | ExprKind::MethodCallNamed { .. }
            | ExprKind::ListWithSpread(_)
            | ExprKind::MapWithSpread(_)
            | ExprKind::StructWithSpread { .. } => self.lower_sugar_kind(id, kind, span, ty),
        }
    }

    fn lower_access_kind(
        &mut self,
        id: ExprId,
        kind: ExprKind,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        match kind {
            ExprKind::Binary { op, left, right } => {
                self.lower_binary_kind(op, left, right, span, ty)
            }
            ExprKind::Cast {
                expr,
                ty: cast_ty,
                fallible,
            } => self.lower_cast_kind(expr, cast_ty, fallible, span, ty),
            projection @ (ExprKind::Field { .. } | ExprKind::Index { .. }) => {
                self.lower_projection_kind(id, projection, span, ty)
            }
            ExprKind::Assign { target, value } => self.lower_assign_kind(target, value, span, ty),
            ExprKind::AssignTarget { root, steps } => {
                self.lower_raw_access_chain(root, steps, span, ty)
            }
            _ => unreachable!("lower_access_kind called with non-access expression"),
        }
    }

    fn lower_binary_kind(
        &mut self,
        op: ori_ir::BinaryOp,
        left: ExprId,
        right: ExprId,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        let left = self.lower_expr(left);
        let right = self.lower_expr(right);
        self.push_foldable(CanExpr::Binary { op, left, right }, span, ty)
    }

    fn lower_cast_kind(
        &mut self,
        expr: ExprId,
        cast_ty: ori_ir::ParsedTypeId,
        fallible: bool,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
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

    fn lower_projection_kind(
        &mut self,
        id: ExprId,
        projection: ExprKind,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        match projection {
            ExprKind::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver);
                self.push(CanExpr::Field { receiver, field }, span, ty)
            }
            ExprKind::Index { receiver, index } => {
                let receiver = self.lower_expr(receiver);
                let index = self.lower_expr(index);
                let dispatch = self
                    .typed
                    .index_dispatch_map
                    .get(id)
                    .copied()
                    .unwrap_or(ori_ir::canon::IndexDispatch::Error);
                self.push(
                    CanExpr::Index {
                        receiver,
                        index,
                        dispatch,
                    },
                    span,
                    ty,
                )
            }
            _ => unreachable!("lower_projection_kind called with non-projection expression"),
        }
    }

    fn lower_assign_kind(
        &mut self,
        target: ExprId,
        value: ExprId,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        if let ExprKind::AssignTarget { root, steps } = *self.src.expr_kind(target) {
            if self.typed.resolve_assign_desugar(target).is_some() {
                return self.desugar_assign_target(target, root, steps, value, span, ty);
            }
        }
        let target = self.lower_expr(target);
        let value = self.lower_expr(value);
        self.push(CanExpr::Assign { target, value }, span, ty)
    }

    fn lower_control_kind(&mut self, kind: ExprKind, span: ori_ir::Span, ty: TypeId) -> CanId {
        match kind {
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if_kind(cond, then_branch, else_branch, span, ty),
            for_expr @ ExprKind::For { .. } => self.lower_for_kind(for_expr, span, ty),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => self.lower_with_capability_kind(capability, provider, body, span, ty),
            ExprKind::FunctionSeq(sequence) => self.lower_function_seq(sequence, span, ty),
            ExprKind::FunctionExp(expression) => self.lower_function_exp(expression, span, ty),
            _ => unreachable!("lower_control_kind called with non-control expression"),
        }
    }

    fn lower_if_kind(
        &mut self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        let cond = self.lower_expr(cond);
        let then_branch = self.lower_expr(then_branch);
        let else_branch = self.lower_optional(else_branch);
        self.push_foldable(
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            },
            span,
            ty,
        )
    }

    fn lower_for_kind(&mut self, kind: ExprKind, span: ori_ir::Span, ty: TypeId) -> CanId {
        let ExprKind::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } = kind
        else {
            unreachable!("lower_for_kind called with non-for expression");
        };
        let pattern = self.lower_binding_pattern(pattern);
        let iter = self.lower_expr(iter);
        let guard = self.lower_optional(guard);
        let body = self.lower_expr(body);
        self.push(
            CanExpr::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            },
            span,
            ty,
        )
    }

    fn lower_with_capability_kind(
        &mut self,
        capability: ori_ir::Name,
        provider: ExprId,
        body: ExprId,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
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

    fn lower_container_kind(
        &mut self,
        id: ExprId,
        kind: ExprKind,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        match kind {
            ExprKind::Call { func, args } => self.lower_call(id, func, args, span, ty),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(id, receiver, method, args, span, ty),
            ExprKind::Block { stmts, result } => self.lower_block(stmts, result, span, ty),
            ExprKind::Let {
                pattern,
                init,
                mutable,
                ..
            } => self.lower_let_kind(pattern, init, mutable, span, ty),
            ExprKind::Lambda { params, body, .. } => self.lower_lambda_kind(params, body, span, ty),
            ExprKind::List(exprs) => self.lower_list(exprs, span, ty),
            ExprKind::Tuple(exprs) => self.lower_tuple(exprs, span, ty),
            ExprKind::Map(entries) => self.lower_map(entries, span, ty),
            ExprKind::Struct { name, fields } => self.lower_struct(name, fields, span, ty),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range_kind(start, end, step, inclusive, span, ty),
            ExprKind::Match { scrutinee, arms } => self.lower_match(scrutinee, arms, span, ty),
            _ => unreachable!("lower_container_kind called with non-container expression"),
        }
    }

    fn lower_let_kind(
        &mut self,
        pattern: ori_ir::BindingPatternId,
        init: ExprId,
        mutable: ori_ir::Mutability,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
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

    fn lower_lambda_kind(
        &mut self,
        params: ori_ir::ParamRange,
        body: ExprId,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        let body = self.lower_expr(body);
        let params = self.lower_params(params);
        self.push(CanExpr::Lambda { params, body }, span, ty)
    }

    fn lower_range_kind(
        &mut self,
        start: ExprId,
        end: ExprId,
        step: ExprId,
        inclusive: bool,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
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

    fn lower_sugar_kind(
        &mut self,
        id: ExprId,
        kind: ExprKind,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        match kind {
            ExprKind::TemplateFull(name) => self.push(CanExpr::Str(name), span, ty),
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
            _ => unreachable!("lower_sugar_kind called with non-sugar expression"),
        }
    }

    fn push_foldable(&mut self, kind: CanExpr, span: ori_ir::Span, ty: TypeId) -> CanId {
        let id = self.push(kind, span, ty);
        crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id).unwrap_or(id)
    }
}
