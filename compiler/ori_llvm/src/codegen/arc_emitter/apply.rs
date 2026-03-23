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
//! - [`apply_helpers`](super::apply_helpers) — ABI parameter passing, sret,
//!   and aggregate-to-pointer coercion

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::{Idx, Tag};

use super::ArcIrEmitter;
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
    /// The ARC IR uses the **original** generic name (e.g., `"identity"`),
    /// but the LLVM function was declared under the **mangled** name
    /// (e.g., `"identity$m$int"`). This method matches the concrete argument
    /// types at the call site to find the correct monomorphization.
    pub(super) fn lookup_mono_dispatch(
        &self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let entries = self.ctx.mono_dispatch.get(&callee)?;
        let arg_types: Vec<Idx> = args
            .iter()
            .map(|a| self.pool.resolve_fully(func.var_type(*a)))
            .collect();
        entries
            .iter()
            .find(|(params, _)| {
                params.len() == arg_types.len()
                    && params
                        .iter()
                        .zip(&arg_types)
                        .all(|(p, a)| self.pool.resolve_fully(*p) == *a)
            })
            .and_then(|(_, mangled)| self.ctx.functions.get(mangled))
    }

    /// Emit an `Apply` instruction (ABI-aware direct call).
    pub(super) fn emit_apply(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let callee_name_str = self.interner.lookup(callee);

        // Internal protocol intercepts (__iter_next, __collect_set, etc.)
        if self.try_emit_protocol(dst, callee_name_str, args, func) {
            return;
        }

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if let Some(val) = self.try_emit_format_call(callee_name_str, args, func) {
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

        // Method dispatch chain (same as emit_invoke):
        // 1. Receiver-based: use first arg's type (instance methods)
        // 2. Return-type-based: use dst's type (static methods like default)
        // 3. Unqualified: bare function name (free functions)
        // 4. Monomorphized generic: match arg types → mangled specialization
        // 5. Diagnostic fallback: logs warning, returns None
        let resolved = self
            .lookup_method_by_receiver(callee, args, func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, args, func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi));

        let result = if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing_with_forwarding(&arg_vals, args, &params);
            match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    self.call_with_sret(func_id, &passed_args, ret_ty, "call")
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.emit_rt_call(func_id, &passed_args, "call")
                }
            }
        } else if let Some(val) = self.try_emit_builtin_method(callee, args, func) {
            Some(val)
        } else if let Some(func_id) = self.builder.try_runtime_fn(callee_name_str) {
            // Runtime function fallback: coerce aggregate args to pointers.
            // Runtime functions (ori_print, ori_str_*, etc.) take ptr params,
            // but ARC IR passes aggregate structs (Str, List, etc.) by value.
            // When a variable has a known source pointer (borrowed parameter),
            // forward it directly instead of alloca+store.
            let is_list_push = callee_name_str == "ori_list_push";
            let coerced_args: Vec<ValueId> = args
                .iter()
                .zip(arg_vals.iter())
                .enumerate()
                .map(|(i, (arc_var, &val))| {
                    let arg_ty = func.var_type(*arc_var);
                    if is_list_push && i == 1 {
                        // ori_list_push(list_ptr, elem_ptr, elem_size):
                        // arg[1] is the element value that must be coerced
                        // to a pointer regardless of its type (even scalars).
                        self.coerce_any_to_ptr(val, arg_ty)
                    } else if let Some(&src_ptr) = self.borrowed_param_ptrs.get(arc_var) {
                        // Borrowed parameter forwarding: forward the original
                        // pointer directly to the runtime function.
                        let tag = self.pool.tag(arg_ty);
                        if matches!(tag, Tag::Str | Tag::List | Tag::Set | Tag::Map) {
                            src_ptr
                        } else {
                            self.coerce_aggregate_to_ptr(val, arg_ty)
                        }
                    } else {
                        self.coerce_aggregate_to_ptr(val, arg_ty)
                    }
                })
                .collect();
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
        let fn_ptr = self.builder.extract_value(closure_val, 0, "closure.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "closure.env_ptr");

        if let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) {
            let ptr_ty = self.builder.ptr_type();
            let mut arg_vals = Vec::with_capacity(1 + args.len());
            let mut param_types = Vec::with_capacity(1 + args.len());
            arg_vals.push(env_ptr);
            param_types.push(ptr_ty);

            for &a in args {
                let arg_ty = func.var_type(a);
                let passing = crate::codegen::abi::compute_param_passing(arg_ty, self.type_info);
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
            let ret_is_indirect = crate::codegen::abi::abi_size(ty, self.type_info) > 16;
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

    // Format call decomposition

    /// Intercept `ori_format_*` calls and decompose the string spec argument.
    ///
    /// ARC IR emits `Apply("ori_format_int", [val, spec_str])` with 2 args.
    /// Runtime expects `ori_format_int(val, spec_ptr, spec_len)` — 3 args.
    /// The `spec_str` is `{i64 len, ptr data}` that needs decomposition.
    pub(super) fn try_emit_format_call(
        &mut self,
        callee_name: &str,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.len() < 2 {
            return None;
        }

        let func_id = match callee_name {
            "ori_format_int" => self.builder.runtime_fn("ori_format_int"),
            "ori_format_float" => self.builder.runtime_fn("ori_format_float"),
            "ori_format_str" => self.builder.runtime_fn("ori_format_str"),
            "ori_format_bool" => self.builder.runtime_fn("ori_format_bool"),
            "ori_format_char" => self.builder.runtime_fn("ori_format_char"),
            _ => return None,
        };

        // args[0] = the value to format
        let value = self.var(args[0]);
        // args[1] = spec string {i64 len, ptr data}
        let spec_str = self.var(args[1]);

        // For ori_format_str, the value arg is also a string struct — coerce to ptr.
        let value_arg = if callee_name == "ori_format_str" {
            let val_ty = func.var_type(args[0]);
            self.coerce_aggregate_to_ptr(value, val_ty)
        } else {
            value
        };

        // Decompose spec string via SSO-safe runtime helpers.
        // Field extraction is WRONG for SSO strings (field 0 = inline bytes, not len).
        let spec_str_ptr = self.str_to_ptr(spec_str, "fmt.spec");
        let len_fn = self.builder.runtime_fn("ori_str_len");
        let spec_len = self
            .builder
            .call(len_fn, &[spec_str_ptr], "fmt.spec_len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let data_fn = self.builder.runtime_fn("ori_str_data");
        let spec_ptr = self
            .builder
            .call(data_fn, &[spec_str_ptr], "fmt.spec_ptr")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // Call runtime: ori_format_*(value, spec_ptr, spec_len) → Str via sret
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[value_arg, spec_ptr, spec_len], str_ty, "fmt")
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
                .unwrap_or_else(|| {
                    tracing::warn!("ArcIrEmitter: string runtime call returned no value");
                    self.builder.const_i64(0)
                })
        } else {
            // ori_str_eq / ori_str_ne return i1 (bool) — direct return
            let result = self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], func_name);
            result.unwrap_or_else(|| self.builder.const_bool(false))
        }
    }
}
