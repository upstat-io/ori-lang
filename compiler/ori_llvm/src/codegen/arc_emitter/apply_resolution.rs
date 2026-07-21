//! Closed-target, method, and monomorphized call resolution.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::codegen::abi::FunctionAbi;
use crate::codegen::value_id::{FunctionId, ValueId};

use super::apply::closed_target_projection_message;
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Resolve one receiver-qualified method through the closed artifact census.
    pub(super) fn lookup_exact_method_target(
        &self,
        receiver: Idx,
        method: Name,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let semantic = self.pool.method_receiver_key(receiver);
        if let Some(target) = self.ctx.exact_method_functions.get(&(semantic, method)) {
            return Some(target);
        }
        let resolved = self.pool.resolve_fully(receiver);
        let target = self
            .ctx
            .exact_method_functions
            .get(&(resolved, method))
            .or_else(|| {
                self.ctx
                    .exact_method_functions
                    .get(&(self.pool.method_receiver_key(resolved), method))
            });
        target
    }

    /// Whether a closed call site is allowed to use spelling-based runtime projection.
    ///
    /// Once executable facts are bound, only a `CallableTarget::Runtime` may
    /// enter builtin/runtime emission. Exact function and external targets must
    /// resolve through their declared artifact identity and fail closed if that
    /// declaration is absent.
    pub(super) fn runtime_projection_allowed(&self, func: &ArcFunction, dst: ArcVarId) -> bool {
        if !self.ctx.executable_facts_bound {
            return true;
        }
        matches!(
            self.ctx.executable_call_targets.get(&(func.name, dst)),
            Some(ori_repr::executable::CallableTarget::Runtime(_))
        )
    }

    /// Explain an unresolved direct call using the closed target identity.
    pub(super) fn unresolved_direct_call_message(
        &self,
        func: &ArcFunction,
        dst: ArcVarId,
        fallback_name: &str,
        site: &str,
    ) -> String {
        if !self.ctx.executable_facts_bound {
            return format!(
                "unresolved function `{fallback_name}` in {site}; ensure the call has a typed declaration or concrete monomorphized instance"
            );
        }

        match self.ctx.executable_call_targets.get(&(func.name, dst)) {
            Some(ori_repr::executable::CallableTarget::Function(function)) => {
                let target = self
                    .ctx
                    .executable_function_names
                    .get(function.index())
                    .map_or(fallback_name, |name| self.interner.lookup(*name));
                closed_target_projection_message(target, site)
            }
            Some(ori_repr::executable::CallableTarget::External(function)) => {
                let target = self
                    .ctx
                    .executable_external_names
                    .get(function.index())
                    .map_or(fallback_name, |name| self.interner.lookup(*name));
                closed_target_projection_message(target, site)
            }
            Some(ori_repr::executable::CallableTarget::Runtime(operation)) => format!(
                "LLVM has no physical projection for closed runtime operation {operation:?} in {site}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug"
            ),
            None => format!(
                "closed executable call to `{fallback_name}` has no frozen target in {site}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug"
            ),
        }
    }

    /// Emit either LLVM `invoke` or `call` + `br` based on [`InvokeMode`].
    ///
    /// - `InvokeMode::Invoke`: emits `invoke` with normal + unwind continuations
    /// - `InvokeMode::Call`: emits `call` + unconditional `br` to normal block
    pub(super) fn call_or_invoke_llvm(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        mode: super::context::InvokeMode,
        name: &str,
    ) -> Option<ValueId> {
        match mode {
            super::context::InvokeMode::Call { normal } => {
                let result = if let Some(pad) = self.current_cleanup_pad {
                    self.builder.call_with_funclet(func_id, args, pad, name)
                } else {
                    self.builder.call(func_id, args, name)
                };
                self.br_outside_cleanup_pad(normal);
                result
            }
            super::context::InvokeMode::Invoke { normal, unwind } => {
                if let Some(pad) = self.current_cleanup_pad {
                    self.builder
                        .invoke_with_funclet(func_id, args, pad, normal, unwind, name)
                } else {
                    self.builder.invoke(func_id, args, normal, unwind, name)
                }
            }
        }
    }

    /// Look up a method function using the first arg's type as a receiver.
    ///
    /// Derived methods (e.g., `compare`, `eq`, `clone`) in ARC IR use unqualified
    /// names. When two types derive the same trait, the unqualified lookup is
    /// ambiguous. This method uses the first arg's type index to resolve the
    /// correct type-qualified entry in `method_functions`.
    pub(super) fn lookup_method_by_receiver(
        &self,
        name: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let &first_arg = args.first()?;
        let receiver_ty = func.var_type(first_arg);
        if self.ctx.executable_facts_bound {
            return self.lookup_exact_method_target(receiver_ty, name);
        }
        // A generic-composite receiver resolves to its materialized
        // concrete body; prefer the per-instantiation derived method keyed on
        // that Idx before the type-name-keyed map.
        let resolved = self.pool.resolve_fully(receiver_ty);
        if let Some(hit) = self.ctx.mono_derive_functions.get(&(resolved, name)) {
            return Some(hit);
        }
        let type_name = self.ctx.type_idx_to_name.get(&receiver_ty)?;
        self.ctx.method_functions.get(&(*type_name, name))
    }

    /// Look up a static/associated method by its return type.
    ///
    /// Type-qualified calls with no receiver (e.g., `Point.default()`) have an
    /// empty `args` list in ARC IR, so `lookup_method_by_receiver` fails.
    /// For factory methods like `default()`, the return type IS the owning type,
    /// so we can use `func.var_type(dst)` to find the correct type-qualified
    /// entry in `method_functions`.
    pub(super) fn lookup_method_by_return_type(
        &self,
        name: Name,
        dst: ArcVarId,
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let return_ty = func.var_type(dst);
        if self.ctx.executable_facts_bound {
            return self.lookup_exact_method_target(return_ty, name);
        }
        // A generic-composite return type (e.g. `Default` on
        // `P3Pair<int,str>`) resolves to its materialized concrete body; prefer
        // the per-instantiation derived method keyed on that Idx before the
        // last-instantiation-wins type-name-keyed map.
        let resolved = self.pool.resolve_fully(return_ty);
        if let Some(hit) = self.ctx.mono_derive_functions.get(&(resolved, name)) {
            return Some(hit);
        }
        let type_name = self.ctx.type_idx_to_name.get(&return_ty)?;
        self.ctx.method_functions.get(&(*type_name, name))
    }

    /// Diagnostic check for method lookup when all typed dispatches miss.
    ///
    /// Always returns `None` — this function only logs diagnostics.
    /// If a method exists in `method_functions` but wasn't found through
    /// normal dispatch, it means the receiver's type wasn't registered in
    /// `type_idx_to_name` (e.g., enum types whose derives aren't compiled yet).
    /// Returning `None` ensures the caller falls through to the "unresolved
    /// function" error path instead of silently calling the wrong method.
    ///
    /// The warning fires only when `receiver_ty` resolves to a type
    /// `type_idx_to_name` is meant to cover (`Tag::Applied` generic
    /// composites, `Tag::Struct`, `Tag::Enum` — mirrors the registration
    /// idiom at `derive_codegen/instantiation.rs`). A builtin receiver
    /// (str, bool, ...) legitimately falls through every typed dispatch
    /// step to `try_emit_builtin_method`; gating on the receiver's tag
    /// stops the warning firing whenever ANY unrelated type happens to
    /// share the method name.
    pub(super) fn lookup_method_fallback(
        &self,
        name: Name,
        receiver_ty: Option<Idx>,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let exists = self
            .ctx
            .method_functions
            .iter()
            .any(|((_, method_name), _)| *method_name == name);
        if exists
            && receiver_ty.is_some_and(|ty| {
                matches!(self.pool.tag(ty), Tag::Applied | Tag::Struct | Tag::Enum)
            })
        {
            tracing::warn!(
                method = %self.interner.lookup(name),
                "method exists for another type but receiver type not registered — \
                 likely missing enum derive codegen"
            );
        }
        None
    }

    /// Resolve a generic function call to its monomorphized variant.
    ///
    /// The ARC IR uses the original generic name (e.g., `identity`) while
    /// the LLVM function was declared under the mangled name
    /// (`identity$m$3_int`). Two paths:
    ///
    /// 1. Abstract-index fast path (sub-step 1e/1f canon-side-table +
    ///    sub-step 1b-deferred deferred-resolution publication): when the
    ///    ARC carrier supplies `mono_instance_id`, look up the mangled
    ///    name directly from `ctx.mono_dispatch_by_id`. This is the
    ///    canonical post-1f path for paths covered by the typeck
    ///    publication pipeline.
    /// 2. Argument-type fallback: kept live for ARC `Invoke` terminators
    ///    in tail position and `apply`-pattern invocations whose carrier
    ///    still has `mono_instance_id = None`. When wired through, the
    ///    fallback becomes dead and can be removed; until then it
    ///    matches concrete argument types against
    ///    `ctx.mono_dispatch[callee]` to pick the correct specialization.
    pub(super) fn lookup_mono_dispatch(
        &self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
        mono_instance_id: Option<MonoInstanceId>,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        if let Some(id) = mono_instance_id {
            let by_id_hit = self.ctx.mono_dispatch_by_id.get(&id);
            tracing::debug!(
                callee = %self.interner.lookup(callee),
                ?id,
                by_id_hit = by_id_hit.is_some(),
                "lookup_mono_dispatch id fast-path"
            );
            if let Some(mangled) = by_id_hit {
                return self.ctx.functions.get(mangled);
            }
        }

        let Some(entries) = self.ctx.mono_dispatch.get(&callee) else {
            tracing::debug!(
                callee = %self.interner.lookup(callee),
                had_instance_id = mono_instance_id.is_some(),
                "lookup_mono_dispatch arg-type fallback: no named entries"
            );
            return None;
        };
        let arg_types: Vec<Idx> = args
            .iter()
            .map(|a| self.pool.resolve_fully(func.var_type(*a)))
            .collect();
        // Match by structural type equality across the merged/imported pool
        // boundary: a param `[int]` and an arg `[int]` interned to distinct Idx
        // denote the same type, so `Pool::structural_eq` is required, not raw
        // Idx identity (the SSOT both fallback sites call).
        //
        // The provenance-erased fallback must not bind a builtin method call to
        // a same-named free-function specialization.
        let skip_self_target = ori_repr::monomorphize::callee_shadows_builtin_method(
            self.pool,
            self.interner,
            callee,
            args,
            func,
        );
        let matched = entries.iter().find(|(params, mangled)| {
            (!skip_self_target || *mangled != func.name)
                && params.len() == arg_types.len()
                && params
                    .iter()
                    .zip(&arg_types)
                    .all(|(p, a)| self.pool.structural_eq(*p, *a))
        });
        if matched.is_none() {
            for (params, _) in entries {
                for (i, (p, a)) in params.iter().zip(&arg_types).enumerate() {
                    let rp = self.pool.resolve_fully(*p);
                    tracing::debug!(
                        callee = %self.interner.lookup(callee),
                        idx = i,
                        param = ?rp,
                        param_tag = ?self.pool.tag(rp),
                        arg = ?*a,
                        arg_tag = ?self.pool.tag(*a),
                        eq = (rp == *a),
                        "lookup_mono_dispatch arg-mismatch detail"
                    );
                }
            }
        }
        tracing::debug!(
            callee = %self.interner.lookup(callee),
            n_entries = entries.len(),
            n_args = arg_types.len(),
            matched = matched.is_some(),
            "lookup_mono_dispatch arg-type fallback result"
        );
        matched.and_then(|(_, mangled)| self.ctx.functions.get(mangled))
    }

    /// Resolve a callee via the 5-step dispatch chain (shared by `Apply`
    /// emission and `Invoke` terminator emission):
    ///
    /// 1. Receiver-based: use first arg's type (instance methods)
    /// 2. Return-type-based: use dst's type (static methods like default)
    /// 3. Unqualified: bare function name (free functions)
    /// 4. Monomorphized generic: abstract-index fast path via
    ///    `mono_instance_id`, degrading to argument-type matching
    /// 5. Diagnostic fallback: logs warning, returns None
    pub(super) fn resolve_callee(
        &self,
        callee: Name,
        args: &[ArcVarId],
        dst: ArcVarId,
        func: &ArcFunction,
        mono_instance_id: Option<MonoInstanceId>,
    ) -> Option<(
        FunctionId,
        Vec<crate::codegen::abi::ParamAbi>,
        crate::codegen::abi::ReturnAbi,
    )> {
        if self.ctx.executable_facts_bound {
            if let Some(clone) = self
                .ctx
                .length_projection_call_targets
                .get(&(func.name, dst))
            {
                return self
                    .ctx
                    .functions
                    .get(clone)
                    .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi));
            }
            let target = self.ctx.executable_call_targets.get(&(func.name, dst))?;
            return match target {
                ori_repr::executable::CallableTarget::Function(function) => {
                    let name = *self.ctx.executable_function_names.get(function.index())?;
                    self.ctx
                        .functions
                        .get(&name)
                        .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi))
                }
                ori_repr::executable::CallableTarget::External(function) => {
                    let name = *self.ctx.executable_external_names.get(function.index())?;
                    self.ctx
                        .functions
                        .get(&name)
                        .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi))
                }
                ori_repr::executable::CallableTarget::Runtime(_) => None,
            };
        }

        // Receiver's resolved type, computed once for the diagnostic-only
        // fallback step — mirrors `lookup_method_by_receiver`'s own
        // `args.first()` + `func.var_type()` + `resolve_fully()` derivation.
        let resolved_receiver_ty = args
            .first()
            .map(|&a| self.pool.resolve_fully(func.var_type(a)));
        self.lookup_method_by_receiver(callee, args, func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, args, func, mono_instance_id))
            .or_else(|| self.lookup_method_fallback(callee, resolved_receiver_ty))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi))
    }
}
