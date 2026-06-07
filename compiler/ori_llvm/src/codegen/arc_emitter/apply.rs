//! Function call emission for ARC IR → LLVM IR.
//!
//! Handles direct calls (`Apply`), indirect calls (`ApplyIndirect`), and method
//! dispatch resolution. This is the call-site half of the emission pipeline;
//! the callee declarations live in `function_compiler`.
//!
//! # Submodules
//!
//! - [`apply_protocols`](super::apply_protocols) — internal protocol intercepts
//!   (`__iter_next`, `__collect_set`, `ori_list_take`, `__index`)
//! - [`apply_casts`](super::apply_casts) — `__cast` conversions and
//!   `ori_format_*` intercepts
//! - [`apply_helpers`](super::apply_helpers) — ABI parameter passing, sret,
//!   and aggregate-to-pointer coercion

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use super::{ArcIrEmitter, EmittedValue};
use crate::codegen::abi::{FunctionAbi, ReturnPassing};
use crate::codegen::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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
                let result = if let Some((pad, _kind)) = self.current_funclet_pad {
                    self.builder.call_with_funclet(func_id, args, pad, name)
                } else {
                    self.builder.call(func_id, args, name)
                };
                self.br_exiting_catchpad(normal);
                result
            }
            super::context::InvokeMode::Invoke { normal, unwind } => {
                if let Some((pad, _kind)) = self.current_funclet_pad {
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
    pub(super) fn lookup_method_fallback(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        let exists = self
            .ctx
            .method_functions
            .iter()
            .any(|((_, method_name), _)| *method_name == name);
        if exists {
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
        let matched = entries.iter().find(|(params, _)| {
            params.len() == arg_types.len()
                && params
                    .iter()
                    .zip(&arg_types)
                    .all(|(p, a)| self.pool.resolve_fully(*p) == *a)
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
        self.lookup_method_by_receiver(callee, args, func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, args, func, mono_instance_id))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi))
    }

    /// Emit an `Apply` instruction (ABI-aware direct call).
    pub(super) fn emit_apply(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
        mono_instance_id: Option<MonoInstanceId>,
    ) {
        let callee_name_str = self.interner.lookup(callee);

        // Internal protocol intercepts (__iter_next, __collect_set, etc.)
        if self.try_emit_protocol(dst, callee, args, func) {
            return;
        }

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if let Some(val) = self.try_emit_format_call(callee, args, func) {
            self.def_var_repr(dst, val, func);
            return;
        }

        // Prelude builtin functions (str, int, float, byte, hash_combine, etc.)
        if let Some(val) =
            super::builtins::prelude::try_emit_prelude_function(self, callee_name_str, args, func)
        {
            self.def_var_repr(dst, val, func);
            return;
        }

        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        let resolved = self.resolve_callee(callee, args, dst, func, mono_instance_id);

        let result = if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing(&arg_vals, Some(args), &params);
            match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    self.call_with_sret(func_id, &passed_args, ret_ty, "call")
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.emit_rt_call(func_id, &passed_args, "call")
                }
            }
        } else if let Some(val) =
            self.try_emit_builtin_method(callee, args, func, func.var_type(dst))
        {
            Some(val)
        } else if let Some(func_id) = self.builder.try_runtime_fn(callee_name_str) {
            let coerced_args = self.coerce_runtime_fn_args(callee, args, &arg_vals, func);

            // Large struct returns (Str, List, Map) use sret convention.
            if crate::codegen::runtime_decl::rt_fn_needs_sret(callee_name_str) {
                let ret_ty = self.resolve_type(func.var_type(dst));
                self.call_with_sret(func_id, &coerced_args, ret_ty, "call")
            } else {
                self.emit_rt_call(func_id, &coerced_args, "call")
            }
        } else {
            let msg = format!(
                "unresolved function `{callee_name_str}` in apply — missing mono instance?"
            );
            tracing::warn!("{msg}");
            self.builder.record_codegen_error_with_msg(msg);
            None
        };

        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        } else if !self.builder.has_codegen_errors() {
            // Void-returning call: ARC IR still expects dst to be defined
            // (uniform SSA — every Apply produces a variable). Bind to a
            // unit constant so successor blocks can reference it.
            // Same pattern as emit_abi_resolved_call() for Invoke terminators.
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
        }
    }

    /// Emit an `ApplyIndirect` instruction (indirect call through closure).
    pub(super) fn emit_apply_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let closure_val = self.var(closure);
        tracing::trace!(
            ?ty,
            tag = ?self.pool.tag(ty),
            closure_var = closure.raw(),
            args = args.len(),
            "emit_apply_indirect"
        );
        let fn_ptr = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_FN, "closure.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_ENV, "closure.env_ptr");

        if let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) {
            let ptr_ty = self.builder.ptr_type();
            let mut arg_vals = Vec::with_capacity(1 + args.len());
            let mut param_types = Vec::with_capacity(1 + args.len());
            arg_vals.push(env_ptr);
            param_types.push(ptr_ty);

            for &a in args {
                let arg_ty = func.var_type(a);
                let passing = crate::codegen::abi::compute_param_passing(
                    arg_ty,
                    self.type_info,
                    self.repr_plan,
                );
                match passing {
                    crate::codegen::abi::ParamPassing::Indirect { .. }
                    | crate::codegen::abi::ParamPassing::Reference => {
                        // Large struct: alloca, store, pass pointer
                        let llvm_ty = self.resolve_type(arg_ty);
                        let alloca = self.builder.alloca(llvm_ty, "icall.arg.tmp");
                        self.builder.store(self.var(a), alloca);
                        arg_vals.push(alloca);
                        param_types.push(ptr_ty);
                    }
                    crate::codegen::abi::ParamPassing::Void => {}
                    crate::codegen::abi::ParamPassing::Direct => {
                        arg_vals.push(self.var(a));
                        param_types.push(self.resolve_type(arg_ty));
                    }
                }
            }

            let ret_ty = self.resolve_type(ty);
            let ret_is_indirect =
                crate::codegen::abi::abi_size(ty, self.type_info, self.repr_plan) > 16;
            tracing::trace!(
                ?ty,
                resolved_llvm_ty = ?self.builder.arena.get_type(ret_ty),
                ret_is_indirect,
                "emit_apply_indirect: resolved return type"
            );

            if ret_is_indirect {
                // Large return type — closure uses sret. Allocate a buffer,
                // call with sret, and load the result. On ARM64, sret goes
                // in X8 via the sret attribute (not as a regular parameter).
                let sret_alloca = self.builder.alloca(ret_ty, "icall.sret");
                if let Some((pad, _kind)) = self.current_funclet_pad {
                    // Inside a SEH funclet — must carry funclet operand bundle.
                    self.builder.call_indirect_with_sret_and_funclet(
                        ret_ty,
                        &param_types,
                        fn_ptr,
                        sret_alloca,
                        &arg_vals,
                        pad,
                    );
                } else {
                    self.builder.call_indirect_with_sret(
                        ret_ty,
                        &param_types,
                        fn_ptr,
                        sret_alloca,
                        &arg_vals,
                    );
                }
                let loaded = self.builder.load(ret_ty, sret_alloca, "icall.sret.load");
                self.def_var_repr(dst, loaded, func);
            } else if let Some((pad, _kind)) = self.current_funclet_pad {
                let result = self.builder.call_indirect_with_funclet(
                    ret_ty,
                    &param_types,
                    fn_ptr,
                    &arg_vals,
                    pad,
                    "icall",
                );
                if let Some(val) = result {
                    self.def_var_repr(dst, val, func);
                }
            } else {
                let result =
                    self.builder
                        .call_indirect(ret_ty, &param_types, fn_ptr, &arg_vals, "icall");
                if let Some(val) = result {
                    self.def_var_repr(dst, val, func);
                }
            }
        } else {
            tracing::error!(
                closure_var = closure.raw(),
                "emit_apply_indirect: extract_value failed — fn_ptr or env_ptr is None"
            );
        }
    }

    // String runtime call helpers

    /// Call a string runtime function: `ori_str_concat`, `ori_str_eq`, `ori_str_ne`.
    ///
    /// String values are `{ i64, i64, ptr }` structs passed by pointer to the runtime.
    /// `returns_str` controls the return type: `true` → sret `{ i64, i64, ptr }`, `false` → `i1`.
    pub(super) fn emit_str_runtime_call(
        &mut self,
        func_name: &'static str,
        lhs: ValueId,
        rhs: ValueId,
        returns_str: bool,
    ) -> ValueId {
        let func_id = self.builder.runtime_fn(func_name);

        // Alloca + store both operands (runtime takes pointers to string structs)
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        if returns_str {
            // ori_str_concat uses sret convention (24-byte return)
            self.builder
                .call_with_sret(func_id, &[lhs_ptr, rhs_ptr], str_ty, func_name)
                .expect("str-returning runtime call uses sret; builder yields the loaded value")
        } else {
            // ori_str_eq / ori_str_ne return i1 (bool) — direct return
            let result = self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], func_name);
            result.expect("str comparison runtime fn is non-void; builder.call returns Some")
        }
    }
}
