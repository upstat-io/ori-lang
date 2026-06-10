//! Call and lambda lowering.
//!
//! Lowers function calls (direct, method) and lambda expressions.
//! Lambda bodies become separate [`ArcFunction`]s with captures as
//! leading parameters, and the call site emits `PartialApply` to
//! pack the captured variables into a closure.
//!
//! Named-argument call variants (`CallNamed`, `MethodCallNamed`) are
//! eliminated during canonicalization — all calls here use positional args.

mod lambda;

use ori_ir::canon::{CanExpr, CanId, CanRange, MonoInstanceId};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind};

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // Nounwind classification

    /// Check if a function name refers to a nounwind call.
    ///
    /// Runtime functions (`ori_*`) and compiler-internal helpers (`__*`)
    /// are known to never unwind. User-defined functions may panic, so
    /// they require `Invoke` terminators for cleanup.
    ///
    /// Exception: `ori_panic` and `ori_assert_*` raise exceptions via
    /// `_Unwind_RaiseException` — they are `noreturn` but NOT nounwind.
    /// Classifying them as nounwind prevents the ARC pipeline from
    /// generating cleanup landing pads, causing RC leaks on unwind.
    ///
    /// `__index` on a list receiver is also may-unwind (OOB panics via
    /// `ori_list_get`), but it never flows through this helper — it is
    /// emitted directly by `lower_index`.
    fn is_nounwind_call(&self, name: Name) -> bool {
        let s = self.interner.lookup(name);
        if s.starts_with("ori_panic") || s.starts_with("ori_assert") {
            return false;
        }
        s.starts_with("ori_") || s.starts_with("__")
    }

    /// Look up the abstract dispatch index for a generic-instantiated call.
    ///
    /// Returns `Some(id)` when the canon-side `mono_dispatch_map_can` carries
    /// an entry for `call_expr_id` (populated during canon lowering by
    /// `Lowerer::record_mono_dispatch_if_present`); `None` otherwise.
    /// The map is sorted by `CanId.raw` per `lower/mod.rs:367`, enabling
    /// O(log n) binary search lookup.
    fn lookup_mono_dispatch(&self, call_expr_id: CanId) -> Option<MonoInstanceId> {
        let key = call_expr_id.raw();
        self.canon
            .mono_dispatch_map_can
            .binary_search_by_key(&key, |(c, _)| c.raw())
            .ok()
            .map(|idx| self.canon.mono_dispatch_map_can[idx].1)
    }

    /// Emit either Apply (nounwind) or Invoke (may-unwind) for a direct call.
    ///
    /// `mono_instance_id` is the abstract dispatch index threaded onto the
    /// emitted carrier; sourced via `lookup_mono_dispatch` at the call site
    /// (`lower_call` / `lower_method_call`). Built-in calls emitted from
    /// other lowering helpers go directly through `emit_apply`/`emit_invoke`
    /// with `None` and do not flow through this helper.
    fn emit_call_or_invoke(
        &mut self,
        ty: Idx,
        name: Name,
        args: Vec<ArcVarId>,
        span: Span,
        mono_instance_id: Option<MonoInstanceId>,
    ) -> ArcVarId {
        if self.is_nounwind_call(name) {
            self.builder
                .emit_apply(ty, name, args, Some(span), mono_instance_id)
        } else {
            self.builder
                .emit_invoke(ty, name, args, Some(span), mono_instance_id)
        }
    }

    /// Emit `ApplyIndirect` or `InvokeIndirect` depending on catch context.
    fn emit_indirect_call(
        &mut self,
        ty: Idx,
        closure_var: ArcVarId,
        arg_vars: Vec<ArcVarId>,
        span: Span,
    ) -> ArcVarId {
        if self.builder.catch_unwind_target.is_some() {
            self.builder
                .emit_invoke_indirect(ty, closure_var, arg_vars, Some(span))
        } else {
            self.builder
                .emit_apply_indirect(ty, closure_var, arg_vars, Some(span))
        }
    }

    /// Resolve an `Ident` callee to a function name, handling `self` → enclosing fn.
    fn resolve_ident_callee(&self, name: Name) -> Name {
        let self_name = self.interner.intern("self");
        if name == self_name && self.scope.lookup(self_name).is_none() {
            self.func_name
        } else {
            name
        }
    }

    /// Try to emit a variant constructor. Returns `None` if `name` is not a variant.
    fn try_emit_variant_ctor(
        &mut self,
        name: Name,
        ty: Idx,
        arg_vars: Vec<ArcVarId>,
        span: Span,
    ) -> Option<ArcVarId> {
        let &(enum_name, variant_idx, _) = self.variant_ctors.get(&name)?;
        tracing::trace!(
            variant = self.name_str(name),
            enum_name = self.name_str(enum_name),
            "call: enum variant constructor"
        );
        Some(self.builder.emit_construct(
            ty,
            CtorKind::EnumVariant {
                enum_name,
                variant: variant_idx,
            },
            arg_vars,
            Some(span),
        ))
    }

    /// Try to emit a newtype constructor as a transparent wrap. Returns `None`
    /// if `name` is not a registered newtype constructor.
    ///
    /// Newtypes are layout-transparent per — `N(value)`
    /// produces the same runtime bytes as `value`. The wrap is purely
    /// type-level (the type stamp changes from the inner type to the newtype),
    /// so the IR emits `Let { Var(arg) }` with no additional storage or
    /// allocation. This dispatch fires before the indirect-call wildcard so
    /// newtype constructor names never reach `lower_ident`'s `Tag::Function`
    /// arm, which would emit unresolvable `PartialApply` (plan).
    fn try_emit_newtype_ctor(
        &mut self,
        name: Name,
        ty: Idx,
        arg_vars: &[ArcVarId],
        span: Span,
    ) -> Option<ArcVarId> {
        if !self.pool.is_newtype_ctor(name) {
            return None;
        }
        if arg_vars.len() != 1 {
            // Arity mismatch — newtype constructors take exactly one argument.
            // Fall through (return None) so the typechecker's existing arity
            // diagnostic is the user-visible error rather than a downstream
            // codegen confusion. Emit a warning so the codegen path is
            // observable in trace output.
            tracing::warn!(
                ctor = self.name_str(name),
                arity = arg_vars.len(),
                "newtype constructor arity mismatch — expected 1 argument"
            );
            return None;
        }
        tracing::trace!(
            ctor = self.name_str(name),
            "call: newtype constructor (transparent wrap)"
        );
        Some(
            self.builder
                .emit_let(ty, ArcValue::Var(arg_vars[0]), Some(span)),
        )
    }

    // Call (positional -- named args already desugared)

    /// Lower a function call expression to ARC IR.
    ///
    /// `call_expr_id` is the `CanId` of the call expression itself (the
    /// `CanExpr::Call` node), used as the lookup key into
    /// `CanonResult.mono_dispatch_map_can` to recover the abstract dispatch
    /// index for generic-instantiated calls.
    pub(crate) fn lower_call(
        &mut self,
        call_expr_id: CanId,
        func: CanId,
        args: CanRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let func_kind = *self.arena.kind(func);
        let mono_instance_id = self.lookup_mono_dispatch(call_expr_id);

        // Lower all arguments first.
        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();
        let arg_vars: Vec<_> = arg_ids.iter().map(|&id| self.lower_expr(id)).collect();

        match func_kind {
            CanExpr::FunctionRef(name) => {
                if let Some(var) = self.try_emit_variant_ctor(name, ty, arg_vars.clone(), span) {
                    return var;
                }
                if let Some(var) = self.try_emit_newtype_ctor(name, ty, &arg_vars, span) {
                    return var;
                }
                tracing::trace!(
                    func = self.name_str(name),
                    args = arg_vars.len(),
                    "call: direct (FunctionRef)"
                );
                self.emit_call_or_invoke(ty, name, arg_vars, span, mono_instance_id)
            }
            CanExpr::SelfRef => {
                tracing::trace!(
                    func = self.name_str(self.func_name),
                    args = arg_vars.len(),
                    "call: self-recursive (SelfRef)"
                );
                self.emit_call_or_invoke(ty, self.func_name, arg_vars, span, mono_instance_id)
            }
            CanExpr::Ident(name) if self.scope.lookup(name).is_some() => {
                let closure_var = self.lower_expr(func);
                tracing::trace!(
                    func = self.name_str(name),
                    closure = closure_var.raw(),
                    args = arg_vars.len(),
                    "call: indirect (local closure)"
                );
                self.emit_indirect_call(ty, closure_var, arg_vars, span)
            }
            CanExpr::Ident(name) => {
                if let Some(var) = self.try_emit_variant_ctor(name, ty, arg_vars.clone(), span) {
                    return var;
                }
                if let Some(var) = self.try_emit_newtype_ctor(name, ty, &arg_vars, span) {
                    return var;
                }
                let resolved = self.resolve_ident_callee(name);
                tracing::trace!(
                    func = self.name_str(resolved),
                    args = arg_vars.len(),
                    "call: direct (Ident)"
                );
                if arg_vars.len() == 1 {
                    let arg_ty = self.expr_type(arg_ids[0]);
                    if let Some(var) = self.emit_tag_check(resolved, arg_vars[0], arg_ty, span) {
                        return var;
                    }
                }
                self.emit_call_or_invoke(ty, resolved, arg_vars, span, mono_instance_id)
            }
            CanExpr::TypeRef(name) => {
                // `TypeRef` callees are how the canonicalizer represents
                // type-name references in call position (e.g., `UserId(x)` for
                // a newtype). The newtype ctor dispatch must fire here too —
                // otherwise the wildcard arm below would lower the `TypeRef`
                // via `lower_ident`, which routes through the `Tag::Function`
                // arm and emits unresolvable `PartialApply` (plan root
                // cause).
                if let Some(var) = self.try_emit_newtype_ctor(name, ty, &arg_vars, span) {
                    return var;
                }
                let closure_var = self.lower_expr(func);
                tracing::trace!(
                    name = self.name_str(name),
                    args = arg_vars.len(),
                    "call: indirect (TypeRef fallthrough)"
                );
                self.emit_indirect_call(ty, closure_var, arg_vars, span)
            }
            _ => {
                let closure_var = self.lower_expr(func);
                tracing::trace!(
                    closure = closure_var.raw(),
                    args = arg_vars.len(),
                    "call: indirect"
                );
                self.emit_indirect_call(ty, closure_var, arg_vars, span)
            }
        }
    }

    // Method call (positional -- named args already desugared)

    /// Lower a method call expression to ARC IR.
    ///
    /// `call_expr_id` is the `CanId` of the method-call expression itself
    /// (the `CanExpr::MethodCall` node), used as the lookup key into
    /// `CanonResult.mono_dispatch_map_can` to recover the abstract dispatch
    /// index for generic-instantiated method calls.
    ///
    /// For type-qualified calls (receiver is `TypeRef`, e.g. `Point.default`),
    /// the receiver is NOT passed as an argument — these are static/associated
    /// methods with no `self` parameter.
    pub(crate) fn lower_method_call(
        &mut self,
        call_expr_id: CanId,
        receiver: CanId,
        method: Name,
        args: CanRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let receiver_kind = *self.arena.kind(receiver);
        let is_type_qualified = matches!(receiver_kind, CanExpr::TypeRef(_));
        let mono_instance_id = self.lookup_mono_dispatch(call_expr_id);

        // Inline lowering for tag-check builtins (is_err, is_ok, is_some,
        // is_none). These are compiled inline by LLVM codegen and don't go
        // through the ARC pipeline, so emitting them as Invoke would cause
        // the Perceus algorithm to transfer ownership to a callee that never
        // Dec's the receiver. Lower as Project + PrimOp instead.
        if !is_type_qualified {
            if let Some(var) = self.try_lower_tag_check(receiver, method, span) {
                return var;
            }
            // Newtype `unwrap` is layout-transparent — emit
            // identity wrap. Without this, `id.unwrap` lowers as `Apply`
            // with method name `unwrap`, which the codegen treats as a
            // monomorphization lookup miss (`unresolved function 'unwrap' in
            // apply — missing mono instance?` per plan). Newtype
            // accessors have no compiled function — they are pure type-level
            // erasure of the newtype tag.
            if let Some(var) = self.try_lower_newtype_unwrap(receiver, method, ty, span) {
                return var;
            }
        }

        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();

        let mut all_args = if is_type_qualified {
            // Type-qualified call (e.g., `Point.default`) — no self argument.
            Vec::with_capacity(arg_ids.len())
        } else {
            let recv_var = self.lower_expr(receiver);
            let mut v = Vec::with_capacity(arg_ids.len() + 1);
            v.push(recv_var);
            v
        };

        for &id in &arg_ids {
            all_args.push(self.lower_expr(id));
        }
        tracing::trace!(
            method = self.name_str(method),
            type_qualified = is_type_qualified,
            args = all_args.len(),
            "method_call: dispatch"
        );
        self.emit_call_or_invoke(ty, method, all_args, span, mono_instance_id)
    }

    /// Emit an inline tag comparison for a Result/Option type.
    ///
    /// Returns `Some(bool_var)` if `method` is a recognized tag-check
    /// builtin (`is_err`, `is_ok`, `is_some`, `is_none`) on a matching type.
    /// The receiver must already be lowered to an `ArcVarId`.
    ///
    /// These builtins are lowered as `Project(tag) == constant` instead
    /// of an Invoke, because they're compiled inline by LLVM codegen and
    /// don't participate in Perceus ownership.
    fn emit_tag_check(
        &mut self,
        method: Name,
        recv_var: ArcVarId,
        recv_ty: Idx,
        span: Span,
    ) -> Option<ArcVarId> {
        let method_str = self.name_str(method);
        let resolved = self.pool.resolve_fully(recv_ty);
        let tag = self.pool.tag(resolved);

        // Result: Ok=0, Err=1. Option: Some=0, None=1.
        let target_tag = match (method_str, tag) {
            ("is_ok", Tag::Result) | ("is_some", Tag::Option) => 0,
            ("is_err", Tag::Result) | ("is_none", Tag::Option) => 1,
            _ => return None,
        };

        let tag_var = self.builder.emit_project(Idx::INT, recv_var, 0, Some(span));
        let tag_const = self.builder.emit_let(
            Idx::INT,
            crate::ir::ArcValue::Literal(crate::ir::LitValue::Int(target_tag)),
            None,
        );
        Some(self.builder.emit_let(
            Idx::BOOL,
            crate::ir::ArcValue::PrimOp {
                op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag_var, tag_const],
            },
            Some(span),
        ))
    }

    /// Try to lower a tag-check method call inline.
    ///
    /// Handles the method call path where the receiver hasn't been
    /// lowered yet. Delegates to [`emit_tag_check`] after lowering.
    fn try_lower_tag_check(
        &mut self,
        receiver: CanId,
        method: Name,
        span: Span,
    ) -> Option<ArcVarId> {
        // Quick pre-check: bail early for non-tag-check methods.
        let s = self.name_str(method);
        if !matches!(s, "is_ok" | "is_err" | "is_some" | "is_none") {
            return None;
        }
        let recv_var = self.lower_expr(receiver);
        let recv_ty = self.expr_type(receiver);
        self.emit_tag_check(method, recv_var, recv_ty, span)
    }

    /// Try to lower a newtype `.unwrap` method call as a transparent wrap.
    ///
    /// Returns `Some(var)` iff the receiver type is a registered newtype
    /// (per `Pool::is_newtype_ctor`) AND the method is `unwrap`. The wrap
    /// emits `Let { Var(receiver_var) }` because newtypes are layout-identical
    /// to their inner type. Without this dispatch, the
    /// method call path emits `Apply { func: Name("unwrap") }` which the
    /// codegen cannot resolve (no compiled `unwrap` function exists for
    /// newtype receivers — newtype accessors are type-level erasure, not
    /// runtime function dispatch).
    fn try_lower_newtype_unwrap(
        &mut self,
        receiver: CanId,
        method: Name,
        ty: Idx,
        span: Span,
    ) -> Option<ArcVarId> {
        // Quick pre-check: bail early for non-unwrap methods.
        if self.name_str(method) != "unwrap" {
            return None;
        }
        // Check the receiver's UNRESOLVED type — newtype `Tag::Named` entries
        // now register a pool resolution to their underlying type, which means
        // `pool.resolve_fully` would transparently unwrap the newtype to its
        // underlying primitive/struct/etc. and `pool.tag(resolved) ==
        // Tag::Named` would never match. Read the surface type and chase
        // `Tag::Var` links manually so we still see the `Tag::Named` layer.
        let mut recv_ty = self.expr_type(receiver);
        for _ in 0..16 {
            if self.pool.tag(recv_ty) != Tag::Var {
                break;
            }
            // Step through Var links without crossing the Named→underlying
            // resolution boundary.
            let next = self.pool.resolve_fully(recv_ty);
            if next == recv_ty {
                break;
            }
            // If `resolve_fully` jumped past the Named layer (e.g., returned
            // a primitive), we cannot identify the newtype here — bail.
            if self.pool.tag(next) != Tag::Var && self.pool.tag(next) != Tag::Named {
                return None;
            }
            recv_ty = next;
        }
        if self.pool.tag(recv_ty) != Tag::Named {
            return None;
        }
        let recv_name = self.pool.named_name(recv_ty);
        if !self.pool.is_newtype_ctor(recv_name) {
            return None;
        }
        let recv_var = self.lower_expr(receiver);
        tracing::trace!(
            newtype = self.name_str(recv_name),
            "method_call: newtype unwrap (transparent wrap)"
        );
        Some(
            self.builder
                .emit_let(ty, ArcValue::Var(recv_var), Some(span)),
        )
    }
}

// Tests

#[cfg(test)]
mod tests;
