//! Expression lowering — the core dispatch for canonical IR → ARC IR.
//!
//! [`ArcLowerer`] walks the canonical expression tree and emits ARC IR
//! instructions via [`ArcIrBuilder`]. Each expression lowers to an
//! [`ArcVarId`] (the SSA variable holding the result).

use ori_ir::canon::{CanArena, CanExpr, CanId, CanonResult};
use ori_ir::{Name, Span, StringInterner};
use ori_types::{Idx, Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};

use super::scope::ArcScope;
use super::{ArcIrBuilder, ArcProblem, VariantCtors};

// Loop context

/// Context for the enclosing loop (used by `break`/`continue`).
pub(crate) struct LoopContext {
    /// Block to jump to on `break`.
    pub exit_block: crate::ir::ArcBlockId,
    /// Block to jump to on `continue`.
    pub continue_block: crate::ir::ArcBlockId,
    /// Mutable variable names in block-parameter order for SSA merge.
    /// MUST be `Vec` (not `HashMap`) — order must match `add_block_param` order.
    pub mutable_vars: Vec<Name>,
}

// ArcLowerer

/// Expression lowerer that walks the canonical IR and emits ARC IR.
///
/// Borrows the `ArcIrBuilder` and contextual data (arena, canon result,
/// interner, pool) needed to lower each expression variant.
pub(crate) struct ArcLowerer<'a> {
    pub(crate) builder: &'a mut ArcIrBuilder,
    pub(crate) arena: &'a CanArena,
    pub(crate) canon: &'a CanonResult,
    pub(crate) interner: &'a StringInterner,
    pub(crate) pool: &'a Pool,
    pub(crate) scope: ArcScope,
    pub(crate) loop_ctx: Option<LoopContext>,
    pub(crate) problems: &'a mut Vec<ArcProblem>,
    pub(crate) lambdas: &'a mut Vec<ArcFunction>,
    /// Resolved `#` (hash length) value for the current index expression.
    ///
    /// Set by `lower_index` before lowering the index sub-expression,
    /// so that `CanExpr::HashLength` resolves to the collection's length.
    /// Mirrors the interpreter's `eval_can_with_hash_length`.
    pub(crate) hash_length: Option<ArcVarId>,
    /// Names freshly `let`-bound in the current block.
    ///
    /// Tracks names introduced by `let` (via `bind_pattern`) to distinguish
    /// shadows from reassignments during block-exit propagation. A `let x = 20`
    /// in an inner block creates a shadow that must NOT propagate to the parent
    /// scope, whereas `x = 20` (reassignment) MUST propagate.
    ///
    /// Saved/restored around each `lower_block` so nesting works correctly.
    pub(crate) block_let_names: FxHashSet<Name>,
    /// The name of the function currently being lowered.
    ///
    /// Used by `lower_exp_recurse()` to emit `Apply @func_name(...)` instead
    /// of a sentinel. This enables TCO detection (which checks
    /// `Apply.func == arc_func.name`) and fixes AOT compilation of
    /// `recurse()` patterns.
    pub(crate) func_name: Name,
    /// Reverse lookup from variant name to enum constructor info.
    ///
    /// Shared by reference from [`lower_function_can`](super::lower_function_can).
    /// Used to intercept variant constructor calls and emit `Construct`
    /// instructions instead of function calls.
    pub(crate) variant_ctors: &'a VariantCtors,
    /// Optional type substitution map for monomorphized generic functions.
    ///
    /// Maps generic `Idx` → concrete `Idx`. When `Some`, `expr_type()` applies
    /// the substitution so the ARC lowerer emits type-specific RC operations.
    /// `None` for non-generic functions (zero overhead).
    pub(crate) type_subst: Option<&'a FxHashMap<Idx, Idx>>,
    /// Counter for generating unique `__for_coll_N` phantom names.
    ///
    /// Prevents name collisions in nested for-loops where each level
    /// needs its own collection phantom binding.
    pub(crate) for_coll_counter: u32,
}

impl ArcLowerer<'_> {
    /// Resolve an interned `Name` to its string for diagnostic/tracing output.
    #[inline]
    pub(crate) fn name_str(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    /// Get the type of a canonical expression by its ID.
    ///
    /// When `type_subst` is `Some` (monomorphized function), applies the
    /// substitution to return the concrete type instead of the generic one.
    #[inline]
    pub(crate) fn expr_type(&self, id: CanId) -> Idx {
        if !id.is_valid() {
            return Idx::ERROR;
        }
        let ty = self.arena.ty(id);
        let idx = Idx::from_raw(ty.raw());
        self.resolve_body_type(idx)
    }

    /// Apply the type substitution map if present, returning the concrete type.
    ///
    /// For non-generic functions (`type_subst` is `None`), returns `ty` unchanged.
    #[inline]
    pub(crate) fn resolve_body_type(&self, ty: Idx) -> Idx {
        match self.type_subst {
            Some(map) => map.get(&ty).copied().unwrap_or(ty),
            None => ty,
        }
    }

    /// Emit a unit literal.
    pub(crate) fn emit_unit(&mut self) -> ArcVarId {
        self.builder
            .emit_let(Idx::UNIT, ArcValue::Literal(LitValue::Unit), None)
    }

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
            CanExpr::Loop { body, .. } => self.lower_loop(body, ty),
            CanExpr::For {
                pattern,
                iter,
                guard,
                body,
                is_yield,
                ..
            } => self.lower_for(pattern, iter, guard, body, ty, is_yield),
            CanExpr::Break { value, .. } => self.lower_break(value),
            CanExpr::Continue { value, .. } => self.lower_continue(value),
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
            CanExpr::WithCapability { body, .. } => self.lower_expr(body),

            CanExpr::Try(inner) => self.lower_try(inner, ty, span),
            CanExpr::Cast {
                expr,
                target: _,
                fallible,
            } => self.lower_cast(expr, fallible, ty, span),

            // Calls
            CanExpr::Call { func, args } => self.lower_call(func, args, ty, span),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(receiver, method, args, ty, span),
            CanExpr::Lambda { params, body } => self.lower_lambda(params, body, ty, span),

            // Special forms
            CanExpr::FunctionExp { kind, props } => self.lower_function_exp(kind, props, ty, span),

            // Formatting — dispatches to type-specific ori_format_* runtime functions
            CanExpr::FormatWith { expr, spec } => self.lower_format_with(expr, spec, ty, span),

            // Error recovery
            CanExpr::Error => self.emit_unit(),
        }
    }

    // Identifier lowering

    fn lower_ident(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        if let Some(var) = self.scope.lookup(name) {
            self.builder.emit_let(ty, ArcValue::Var(var), Some(span))
        } else if let Some(&(enum_name, variant_idx, field_count)) = self.variant_ctors.get(&name) {
            if field_count == 0 {
                // Unit variant as identifier (e.g., `Red` in `let x = Red`)
                self.builder.emit_construct(
                    ty,
                    CtorKind::EnumVariant {
                        enum_name,
                        variant: variant_idx,
                    },
                    vec![],
                    Some(span),
                )
            } else {
                // Tuple variant used as value — fn→closure coercion not yet supported
                tracing::warn!(
                    variant = self.name_str(name),
                    "tuple variant used as first-class value (not yet supported)"
                );
                self.emit_unit()
            }
        } else if self.pool.tag(self.pool.resolve_fully(ty)) == Tag::Function {
            // Named function used as a value — emit zero-capture closure.
            // This handles `CanExpr::Ident` for top-level functions that weren't
            // rewritten to `CanExpr::FunctionRef` by the canonicalizer (e.g.,
            // `apply(f: double, x: 21)` where `double` is a named function).
            self.builder
                .emit_partial_apply(ty, name, vec![], Some(span))
        } else {
            tracing::debug!(
                name = ?name,
                "unbound identifier in ARC IR lowering"
            );
            self.builder
                .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
        }
    }

    // Constant lowering

    /// Lower a compile-time constant from the `ConstantPool`.
    fn lower_constant(
        &mut self,
        const_id: ori_ir::canon::ConstantId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        use ori_ir::canon::ConstValue;
        let value = self.canon.constants.get(const_id);
        let lit = match value {
            ConstValue::Int(n) => LitValue::Int(*n),
            ConstValue::Float(bits) => LitValue::Float(*bits),
            ConstValue::Bool(b) => LitValue::Bool(*b),
            ConstValue::Str(name) => LitValue::String(*name),
            ConstValue::Char(c) => LitValue::Char(*c),
            ConstValue::Unit => LitValue::Unit,
            ConstValue::Duration { value, unit } => LitValue::Duration {
                value: *value,
                unit: *unit,
            },
            ConstValue::Size { value, unit } => LitValue::Size {
                value: *value,
                unit: *unit,
            },
        };
        self.builder
            .emit_let(ty, ArcValue::Literal(lit), Some(span))
    }

    // Binary / Unary operators

    fn lower_binary(
        &mut self,
        op: ori_ir::BinaryOp,
        left: CanId,
        right: CanId,
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
        operand: CanId,
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

// Tests

#[cfg(test)]
mod tests;
