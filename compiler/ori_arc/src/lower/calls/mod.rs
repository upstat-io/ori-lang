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
mod method_call;

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
    /// `__index` on a list or string receiver is also may-unwind (OOB panics),
    /// but it never flows through this helper; `lower_index` emits its Invoke
    /// carrier directly so callee-owned values are cleaned before propagation.
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
    /// The map is sorted by `CanId.raw` in `Lowerer::finish`, enabling
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
    /// arm, which would emit an unresolvable `PartialApply`.
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

    /// Try to emit the builtin `Error` struct constructor as a direct
    /// `Construct`. Returns `None` unless the callee is `Error` AND the
    /// `Error` struct is registered. Fires before the indirect-call wildcard
    /// so `Error(msg)` never reaches the `Tag::Function` arm, which would emit
    /// an unresolvable `PartialApply @Error` that AOT calls through a null fn
    /// ptr (SIGSEGV). Spec: Annex E §Built-in Type Representations.
    fn try_emit_struct_ctor(
        &mut self,
        name: Name,
        ty: Idx,
        arg_vars: Vec<ArcVarId>,
        span: Span,
    ) -> Option<ArcVarId> {
        // Both the selected name and the type-checker-selected result must be
        // the builtin Error struct. A module enum variant may also be named
        // `Error`, so spelling alone is not a constructor identity.
        let error_name = self.interner.intern("Error");
        if name != error_name || !self.pool.is_error_struct_receiver(ty) {
            return None;
        }
        if arg_vars.len() != 1 {
            // The `Error` constructor takes exactly one `str`; fall through so
            // the typechecker's existing arity diagnostic is the user-visible
            // error rather than a downstream codegen confusion (mirrors
            // `try_emit_newtype_ctor`).
            return None;
        }
        tracing::trace!(
            ctor = self.name_str(name),
            "call: Error builtin struct constructor"
        );
        let resolved = self.pool.resolve_fully(ty);
        if self.pool.tag(resolved) != Tag::Struct {
            return None;
        }
        let fields = self.pool.struct_fields(resolved);
        let trace_list_ty = fields[1].1;
        let trace_var =
            self.builder
                .emit_construct(trace_list_ty, CtorKind::ListLiteral, vec![], Some(span));
        let mut full_args = arg_vars;
        full_args.push(trace_var);

        Some(
            self.builder
                .emit_construct(ty, CtorKind::Struct(name), full_args, Some(span)),
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
                if let Some(var) = self.try_emit_struct_ctor(name, ty, arg_vars.clone(), span) {
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
                if let Some(var) = self.try_emit_struct_ctor(name, ty, arg_vars.clone(), span) {
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
                if let Some(var) = self.try_emit_variant_ctor(name, ty, arg_vars.clone(), span) {
                    return var;
                }
                if let Some(var) = self.try_emit_newtype_ctor(name, ty, &arg_vars, span) {
                    return var;
                }
                if let Some(var) = self.try_emit_struct_ctor(name, ty, arg_vars.clone(), span) {
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

    /// Emit an inline tag comparison for a Result/Option type.
    ///
    /// Returns `Some(bool_var)` if `method` is a recognized tag-check
    /// builtin (`is_err`, `is_ok`, `is_some`, `is_none`) on a matching type.
    /// The receiver must already be lowered to an `ArcVarId`.
    ///
    /// These builtins lower to the backend-neutral primitive sequence
    /// `Project(tag) == constant` rather than a call. The sequence has no
    /// callee ownership transfer, and every physical consumer implements it
    /// directly; LLVM currently emits it inline.
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
}

// Tests

#[cfg(test)]
mod tests;
