//! Canonical-expression to ARC-expression dispatch.

use super::{ArcLowerer, ArcValue, ArcVarId, CanExpr, CanId, CtorKind, ForLoop, LitValue};

impl ArcLowerer<'_> {
    // Main dispatch

    /// Lower a single canonical expression, returning the `ArcVarId` of the result.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive CanExpr → ARC lowering router"
    )]
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
            // Literals
            CanExpr::Int(n) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Int(n)), Some(span))
            }
            CanExpr::Float(bits) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Float(bits)), Some(span))
            }
            CanExpr::Bool(b) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Bool(b)), Some(span))
            }
            CanExpr::Str(name) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::String(name)), Some(span))
            }
            CanExpr::Char(c) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Char(c)), Some(span))
            }
            CanExpr::Duration { value, unit } => self.builder.emit_let(
                ty,
                ArcValue::Literal(LitValue::Duration { value, unit }),
                Some(span),
            ),
            CanExpr::Size { value, unit } => self.builder.emit_let(
                ty,
                ArcValue::Literal(LitValue::Size { value, unit }),
                Some(span),
            ),
            CanExpr::Unit => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
            }
            CanExpr::HashLength => {
                if let Some(len) = self.hash_length {
                    self.builder.emit_let(ty, ArcValue::Var(len), Some(span))
                } else {
                    tracing::warn!("HashLength (#) used outside index expression");
                    self.emit_unit()
                }
            }
            CanExpr::FunctionRef(name) => {
                // Unit variant used as value (e.g., `let x = None` or `let c = Red`)
                if let Some(&(enum_name, variant_idx, field_count)) = self.variant_ctors.get(&name)
                {
                    if field_count == 0 {
                        return self.builder.emit_construct(
                            ty,
                            CtorKind::EnumVariant {
                                enum_name,
                                variant: variant_idx,
                            },
                            vec![],
                            Some(span),
                        );
                    }
                }
                // Zero-capture closure: PartialApply with empty captures
                self.builder
                    .emit_partial_apply(ty, name, vec![], Some(span))
            }

            // Compile-time constants
            CanExpr::Constant(const_id) => self.lower_constant(const_id, ty, span),

            // Identifiers
            CanExpr::Ident(name) | CanExpr::Const(name) | CanExpr::TypeRef(name) => {
                self.lower_ident(name, ty, span)
            }
            CanExpr::SelfRef => {
                // In impl methods, `self` is a parameter — look it up in scope.
                // In recurse() patterns, `self` means the enclosing function.
                let self_name = self.interner.intern("self");
                if self.scope.lookup(self_name).is_some() {
                    self.lower_ident(self_name, ty, span)
                } else {
                    self.lower_ident(self.func_name, ty, span)
                }
            }

            // Binary / Unary operators
            CanExpr::Binary { op, left, right } => self.lower_binary(op, left, right, ty, span),
            CanExpr::Unary { op, operand } => self.lower_unary(op, operand, ty, span),

            // Control flow
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

            // Collections & constructors
            CanExpr::Tuple(exprs) => self.lower_tuple(exprs, ty, span),
            CanExpr::List(exprs) => self.lower_list(exprs, ty, span),
            CanExpr::Map(entries) => self.lower_map(entries, ty, span),
            CanExpr::Struct { name, fields } => self.lower_struct(name, fields, ty, span),
            CanExpr::Ok(inner) => self.lower_ok(inner, ty, span),
            CanExpr::Err(inner) => self.lower_err(inner, ty, span),
            CanExpr::Some(inner) => self.lower_some(inner, ty, span),
            CanExpr::None => self.lower_none(ty, span),
            CanExpr::Field { receiver, field } => self.lower_field(receiver, field, ty, span),
            CanExpr::Index { receiver, index } => self.lower_index(receiver, index, ty, span),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range(start, end, step, inclusive, ty, span),
            // Transparent wrappers (sync runtime — just evaluate inner expression)
            CanExpr::Unsafe(inner) | CanExpr::Await(inner) => self.lower_expr(inner),
            // `with Cap = provider in body` — bind the capability name to the
            // lowered provider for the body so the body's `Cap` references
            // resolve, mirroring the evaluator's `with_binding`. The provider
            // is bound via the ordinary lowering path (emit_let); its drop is
            // the downstream ARC realization's job — no hand-rolled RcDec here.
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => {
                let provider_var = self.lower_expr(provider);
                // Surgical save/restore of the capability slot only: a nested
                // same-named `with` restores the outer binding on exit, and
                // body-internal reassignments to other (outer) vars survive.
                // Capture the prior binding's mutability so a shadowed mutable
                // outer var restores with its SSA-merge tracking intact.
                let prior = self.scope.lookup(capability);
                let prior_mutable = prior.is_some() && self.scope.is_mutable(capability);
                self.scope.bind(capability, provider_var);
                let result = self.lower_expr(body);
                match prior {
                    Some(p) if prior_mutable => self.scope.bind_mutable(capability, p),
                    Some(p) => self.scope.bind(capability, p),
                    None => {
                        self.scope.remove(capability);
                    }
                }
                result
            }

            CanExpr::Try(inner) => self.lower_try(inner, ty, span),
            CanExpr::Cast {
                expr,
                target: _,
                fallible,
            } => self.lower_cast(expr, fallible, ty, span),

            // Calls — `id` is the call expression's own CanId, used as the
            // key into `CanonResult.mono_dispatch_map_can` to recover the
            // abstract dispatch index for generic-instantiated calls.
            CanExpr::Call { func, args } => self.lower_call(id, func, args, ty, span),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(id, receiver, method, args, ty, span),
            CanExpr::Lambda { params, body } => self.lower_lambda(params, body, ty, span),

            // Special forms
            CanExpr::FunctionExp { kind, props } => self.lower_function_exp(kind, props, ty, span),

            // Formatting — dispatches to type-specific ori_format_* runtime functions
            CanExpr::FormatWith { expr, spec } => self.lower_format_with(expr, spec, ty, span),

            // Error recovery
            CanExpr::Error => self.emit_unit(),
        }
    }
}
