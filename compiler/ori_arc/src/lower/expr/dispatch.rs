//! Canonical-expression to ARC-expression dispatch.

use ori_ir::canon::{CanExpr, CanId};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue};

use super::{ArcLowerer, ForLoop};

impl ArcLowerer<'_> {
    // Main dispatch

    /// Lower a single canonical expression, returning the `ArcVarId` of the result.
    pub(crate) fn lower_expr(&mut self, id: CanId) -> ArcVarId {
        if !id.is_valid() {
            return self.emit_unit();
        }

        let kind = *self.arena.kind(id);
        let span = self.arena.span(id);
        let ty = self.expr_type(id);
        tracing::trace!(
            id = id.raw(),
            bb = self.builder.current_block().index(),
            "lower_expr"
        );

        match kind {
            CanExpr::Int(_)
            | CanExpr::Float(_)
            | CanExpr::Bool(_)
            | CanExpr::Str(_)
            | CanExpr::Char(_)
            | CanExpr::Duration { .. }
            | CanExpr::Size { .. }
            | CanExpr::Unit
            | CanExpr::HashLength
            | CanExpr::FunctionRef(_)
            | CanExpr::Constant(_)
            | CanExpr::Ident(_)
            | CanExpr::TypeRef(_)
            | CanExpr::Const(_)
            | CanExpr::SelfRef => self.lower_value_expr(kind, ty, span),

            CanExpr::Binary { op, left, right } => self.lower_binary(op, left, right, ty, span),
            CanExpr::Unary { op, operand } => self.lower_unary(op, operand, ty, span),

            CanExpr::Block { .. }
            | CanExpr::Let { .. }
            | CanExpr::If { .. }
            | CanExpr::Match { .. }
            | CanExpr::Loop { .. }
            | CanExpr::For { .. }
            | CanExpr::Break { .. }
            | CanExpr::Continue { .. }
            | CanExpr::Assign { .. } => self.lower_control_expr(kind, ty, span),

            CanExpr::Tuple(_)
            | CanExpr::List(_)
            | CanExpr::Map(_)
            | CanExpr::Struct { .. }
            | CanExpr::Ok(_)
            | CanExpr::Err(_)
            | CanExpr::Some(_)
            | CanExpr::None
            | CanExpr::Field { .. }
            | CanExpr::Index { .. }
            | CanExpr::Range { .. } => self.lower_collection_expr(kind, ty, span),

            CanExpr::Unsafe(inner) | CanExpr::Await(inner) => self.lower_expr(inner),
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => self.lower_with_capability(capability, provider, body),

            CanExpr::Try(inner) => self.lower_try(inner, ty, span),
            CanExpr::Cast {
                expr,
                target: _,
                fallible,
            } => self.lower_cast(expr, fallible, ty, span),

            CanExpr::Call { func, args } => self.lower_call(id, func, args, ty, span),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(id, receiver, method, args, ty, span),
            CanExpr::Lambda { params, body } => self.lower_lambda(params, body, ty, span),

            CanExpr::FunctionExp { kind, props } => self.lower_function_exp(kind, props, ty, span),
            CanExpr::FormatWith { expr, spec } => self.lower_format_with(expr, spec, ty, span),
            CanExpr::Error => self.emit_unit(),
        }
    }

    fn lower_value_expr(
        &mut self,
        kind: CanExpr,
        ty: ori_types::Idx,
        span: ori_ir::Span,
    ) -> ArcVarId {
        let literal = match kind {
            CanExpr::Int(value) => Some(LitValue::Int(value)),
            CanExpr::Float(value) => Some(LitValue::Float(value)),
            CanExpr::Bool(value) => Some(LitValue::Bool(value)),
            CanExpr::Str(value) => Some(LitValue::String(value)),
            CanExpr::Char(value) => Some(LitValue::Char(value)),
            CanExpr::Duration { value, unit } => Some(LitValue::Duration { value, unit }),
            CanExpr::Size { value, unit } => Some(LitValue::Size { value, unit }),
            CanExpr::Unit => Some(LitValue::Unit),
            _ => None,
        };
        if let Some(literal) = literal {
            return self
                .builder
                .emit_let(ty, ArcValue::Literal(literal), Some(span));
        }

        match kind {
            CanExpr::HashLength => self.lower_hash_length(ty, span),
            CanExpr::FunctionRef(name) => self.lower_function_reference(name, ty, span),
            CanExpr::Constant(const_id) => self.lower_constant(const_id, ty, span),
            CanExpr::Ident(name) | CanExpr::TypeRef(name) => self.lower_ident(name, ty, span),
            CanExpr::Const(name) => self.lower_const_reference(name, ty, span),
            CanExpr::SelfRef => self.lower_self_reference(ty, span),
            _ => unreachable!("lower_value_expr called with non-value expression"),
        }
    }

    fn lower_hash_length(&mut self, ty: ori_types::Idx, span: ori_ir::Span) -> ArcVarId {
        if let Some(len) = self.hash_length {
            self.builder.emit_let(ty, ArcValue::Var(len), Some(span))
        } else {
            tracing::warn!("HashLength (#) used outside index expression");
            self.emit_unit()
        }
    }

    fn lower_self_reference(&mut self, ty: ori_types::Idx, span: ori_ir::Span) -> ArcVarId {
        let self_name = self.interner.intern("self");
        let name = if self.scope.lookup(self_name).is_some() {
            self_name
        } else {
            self.func_name
        };
        self.lower_ident(name, ty, span)
    }

    fn lower_function_reference(
        &mut self,
        name: ori_ir::Name,
        ty: ori_types::Idx,
        span: ori_ir::Span,
    ) -> ArcVarId {
        if let Some(&(enum_name, variant, field_count)) = self.variant_ctors.get(&name) {
            if field_count == 0 {
                return self.builder.emit_construct(
                    ty,
                    CtorKind::EnumVariant { enum_name, variant },
                    vec![],
                    Some(span),
                );
            }
        }
        self.builder
            .emit_partial_apply(ty, name, vec![], Some(span))
    }

    fn lower_control_expr(
        &mut self,
        kind: CanExpr,
        ty: ori_types::Idx,
        span: ori_ir::Span,
    ) -> ArcVarId {
        match kind {
            CanExpr::Block { stmts, result } => self.lower_block(stmts, result, ty),
            CanExpr::Let { pattern, init, .. } => self.lower_let(pattern, init),
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if(cond, then_branch, else_branch, ty, span),
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => self.lower_match(scrutinee, decision_tree, arms, ty, span),
            CanExpr::Loop { body, label, .. } => self.lower_loop(body, ty, label),
            CanExpr::For {
                pattern,
                iter,
                guard,
                body,
                is_yield,
                label,
                ..
            } => self.lower_for(ForLoop {
                pattern,
                iter,
                guard,
                body,
                ty,
                is_yield,
                label,
            }),
            CanExpr::Break { value, label, .. } => self.lower_break(value, label),
            CanExpr::Continue { value, label, .. } => self.lower_continue(value, label),
            CanExpr::Assign { target, value } => self.lower_assign(target, value, span),
            _ => unreachable!("lower_control_expr called with non-control expression"),
        }
    }

    fn lower_collection_expr(
        &mut self,
        kind: CanExpr,
        ty: ori_types::Idx,
        span: ori_ir::Span,
    ) -> ArcVarId {
        match kind {
            CanExpr::Tuple(exprs) => self.lower_tuple(exprs, ty, span),
            CanExpr::List(exprs) => self.lower_list(exprs, ty, span),
            CanExpr::Map(entries) => self.lower_map(entries, ty, span),
            CanExpr::Struct { name, fields } => self.lower_struct(name, fields, ty, span),
            CanExpr::Ok(inner) => self.lower_ok(inner, ty, span),
            CanExpr::Err(inner) => self.lower_err(inner, ty, span),
            CanExpr::Some(inner) => self.lower_some(inner, ty, span),
            CanExpr::None => self.lower_none(ty, span),
            CanExpr::Field { receiver, field } => self.lower_field(receiver, field, ty, span),
            CanExpr::Index {
                receiver,
                index,
                dispatch,
            } => self.lower_index(receiver, index, dispatch, ty, span),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range(start, end, step, inclusive, ty, span),
            _ => unreachable!("lower_collection_expr called with non-collection expression"),
        }
    }

    fn lower_with_capability(
        &mut self,
        capability: ori_ir::Name,
        provider: CanId,
        body: CanId,
    ) -> ArcVarId {
        let provider_var = self.lower_expr(provider);
        let prior = self.scope.lookup(capability);
        let prior_mutable = prior.is_some() && self.scope.is_mutable(capability);
        self.scope.bind(capability, provider_var);
        let result = self.lower_expr(body);
        match prior {
            Some(value) if prior_mutable => self.scope.bind_mutable(capability, value),
            Some(value) => self.scope.bind(capability, value),
            None => {
                self.scope.remove(capability);
            }
        }
        result
    }
}
