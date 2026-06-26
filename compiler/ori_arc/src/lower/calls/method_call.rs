//! Method-call lowering.
//!
//! Lowers `CanExpr::MethodCall` to ARC IR, including the pre-dispatch
//! special cases that must not reach the generic named-`Apply` path:
//! tag-check builtins (`is_ok`/`is_err`/`is_some`/`is_none`), newtype
//! `unwrap`, and closure-typed struct fields invoked through the receiver.

use ori_ir::canon::{CanExpr, CanId, CanRange};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId};
use crate::lower::expr::ArcLowerer;

impl ArcLowerer<'_> {
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
            // apply — missing mono instance?`). Newtype
            // accessors have no compiled function — they are pure type-level
            // erasure of the newtype tag.
            if let Some(var) = self.try_lower_newtype_unwrap(receiver, method, ty, span) {
                return var;
            }
            // Closure-typed struct field invoked through the receiver
            // (`s.transform(21)` where `transform: (int) -> int` is a field, not
            // a method). Project the field to recover the closure value and emit
            // an INDIRECT call; the receiver is NOT threaded as a self argument.
            // Without this, the call lowers as `Apply { func: Name("transform") }`
            // which codegen cannot resolve (no compiled function named after the
            // field exists). Mirrors the evaluator's callable-struct-field path.
            if let Some(var) = self.try_lower_closure_field(receiver, method, args, ty, span) {
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

    /// Try to lower a method call that names a closure-typed struct FIELD as an
    /// indirect call through that field.
    ///
    /// Returns `Some(var)` iff the receiver's resolved type is a struct carrying
    /// a field named `method` whose type is a closure (`Tag::Function`). The
    /// closure value is recovered via `Project` and invoked through
    /// `emit_indirect_call` with the explicit call args; the receiver is NOT
    /// passed as a `self` argument (a stored closure has no implicit receiver).
    /// Mirrors the evaluator's callable-struct-field dispatch
    /// (`ori_eval` `method_dispatch`: a struct field matching the method name and
    /// holding a function value is called instead of dispatching a method).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field indices never exceed u32"
    )]
    fn try_lower_closure_field(
        &mut self,
        receiver: CanId,
        method: Name,
        args: CanRange,
        ty: Idx,
        span: Span,
    ) -> Option<ArcVarId> {
        let recv_ty = self.expr_type(receiver);
        let resolved = self.pool.resolve_fully(recv_ty);
        if self.pool.tag(resolved) != Tag::Struct {
            return None;
        }
        // Find a field named `method` whose type is a closure.
        let count = self.pool.struct_field_count(resolved);
        let mut field_idx = None;
        let mut field_ty = Idx::ERROR;
        for i in 0..count {
            let (fname, fty) = self.pool.struct_field(resolved, i);
            if fname == method {
                field_ty = fty;
                if self.pool.tag(self.pool.resolve_fully(fty)) == Tag::Function {
                    field_idx = Some(i as u32);
                }
                break;
            }
        }
        let field_idx = field_idx?;

        // Project the closure value off the receiver, then call it indirectly
        // with the explicit args (no implicit self).
        let recv_var = self.lower_expr(receiver);
        let closure_var = self
            .builder
            .emit_project(field_ty, recv_var, field_idx, Some(span));
        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();
        let arg_vars: Vec<_> = arg_ids.iter().map(|&id| self.lower_expr(id)).collect();
        tracing::trace!(
            field = self.name_str(method),
            args = arg_vars.len(),
            "method_call: closure-typed struct field (indirect call)"
        );
        Some(self.emit_indirect_call(ty, closure_var, arg_vars, span))
    }
}
