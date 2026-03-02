//! Function call emission for ARC IR → LLVM IR.
//!
//! Handles direct calls (`Apply`), indirect calls (`ApplyIndirect`), method
//! dispatch resolution, ABI parameter passing, sret protocol, and runtime
//! function coercion. This is the call-site half of the emission pipeline;
//! the callee declarations live in `function_compiler`.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::{Idx, Tag};

use super::context::InvokeMode;
use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ReturnPassing};
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit either LLVM `invoke` or `call` + `br` based on [`InvokeMode`].
    ///
    /// - `InvokeMode::Invoke`: emits `invoke` with normal + unwind continuations
    /// - `InvokeMode::Call`: emits `call` + unconditional `br` to normal block
    pub(super) fn call_or_invoke_llvm(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        mode: InvokeMode,
        name: &str,
    ) -> Option<ValueId> {
        match mode {
            InvokeMode::Call { normal } => {
                let result = self.builder.call(func_id, args, name);
                self.builder.br(normal);
                result
            }
            InvokeMode::Invoke { normal, unwind } => {
                self.builder.invoke(func_id, args, normal, unwind, name)
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

        // Internal protocol: __iter_next(iter, elem_ty_marker).
        // args[0] = iterator pointer, args[1] = zero marker carrying elem_ty.
        // Result type is INT (no RC management); actual element type comes
        // from the marker argument.
        if callee_name_str == "__iter_next" && args.len() >= 2 {
            let iter_ptr = self.var(args[0]);
            let elem_ty = func.var_type(args[1]);
            if let Some(val) = self.emit_iter_next(iter_ptr, elem_ty) {
                self.def_var_repr(dst, val, func);
            }
            return;
        }

        // Internal protocol: __collect_set(iter).
        // Type-directed rewrite from `collect()` when target type is Set<T>.
        // Uses sret pattern like emit_iter_collect but calls ori_iter_collect_set.
        if callee_name_str == "__collect_set" && !args.is_empty() {
            let iter_ptr = self.var(args[0]);
            let iter_ty = func.var_type(args[0]);
            let elem_ty = self.pool.iterator_elem(iter_ty);
            if let Some(val) = self.emit_iter_collect_set(iter_ptr, elem_ty) {
                self.def_var_repr(dst, val, func);
            }
            return;
        }

        // ori_list_take uses explicit sret pattern: void(list_ptr, out_ptr).
        // The ARC IR emits Apply "ori_list_take"(list_ptr) expecting a struct return.
        // We handle the sret plumbing here: alloca result struct, call, load.
        if callee_name_str == "ori_list_take" && !args.is_empty() {
            if let Some(val) = self.emit_list_take(args[0], func) {
                self.def_var_repr(dst, val, func);
            }
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
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi.clone()));

        let result = if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing(&arg_vals, &params);
            match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    self.call_with_sret(func_id, &passed_args, ret_ty, "call")
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.builder.call(func_id, &passed_args, "call")
                }
            }
        } else if let Some(val) = self.try_emit_builtin_method(callee, args, func) {
            Some(val)
        } else if let Some(func_id) = self.builder.try_runtime_fn(callee_name_str) {
            // Runtime function fallback: coerce aggregate args to pointers.
            // Runtime functions (ori_print, ori_str_*, etc.) take ptr params,
            // but ARC IR passes aggregate structs (Str, List, etc.) by value.
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
                self.builder.call(func_id, &coerced_args, "call")
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
            tracing::trace!(
                ?ty,
                resolved_llvm_ty = ?self.builder.arena.get_type(ret_ty),
                "emit_apply_indirect: resolved return type"
            );
            if let Some(val) =
                self.builder
                    .call_indirect(ret_ty, &param_types, fn_ptr, &arg_vals, "icall")
            {
                self.def_var_repr(dst, val, func);
            }
        } else {
            tracing::error!(
                closure_var = closure.raw(),
                "emit_apply_indirect: extract_value failed — fn_ptr or env_ptr is None"
            );
        }
    }

    // -----------------------------------------------------------------------
    // ABI helpers
    // -----------------------------------------------------------------------

    /// Apply parameter passing modes to argument values.
    ///
    /// Apply param passing: `Indirect`/`Reference` (alloca+store+pass ptr),
    /// `Direct` (pass through), `Void` (skip).
    pub(super) fn apply_param_passing(
        &mut self,
        args: &[ValueId],
        params: &[crate::codegen::abi::ParamAbi],
    ) -> Vec<ValueId> {
        let mut result = Vec::with_capacity(args.len());
        let mut arg_idx = 0;

        for param_abi in params {
            if arg_idx >= args.len() {
                break;
            }

            match &param_abi.passing {
                crate::codegen::abi::ParamPassing::Indirect { .. }
                | crate::codegen::abi::ParamPassing::Reference => {
                    let param_ty = self.resolve_type(param_abi.ty);
                    let alloca = self.builder.create_entry_alloca(
                        self.current_function,
                        "ref_arg",
                        param_ty,
                    );
                    self.builder.store(args[arg_idx], alloca);
                    result.push(alloca);
                    arg_idx += 1;
                }
                crate::codegen::abi::ParamPassing::Direct => {
                    result.push(args[arg_idx]);
                    arg_idx += 1;
                }
                crate::codegen::abi::ParamPassing::Void => {
                    // Void params are not physically passed — skip
                }
            }
        }

        // Pass remaining args directly (shouldn't happen in well-typed code)
        while arg_idx < args.len() {
            result.push(args[arg_idx]);
            arg_idx += 1;
        }

        result
    }

    /// Call a function with sret (struct return via hidden pointer).
    ///
    /// The sret alloca is placed in the entry block (via `create_entry_alloca`)
    /// to ensure it dominates all uses, even in loop bodies or branch targets.
    pub(super) fn call_with_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        ret_ty: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let sret_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "sret.tmp", ret_ty);
        let mut full_args = Vec::with_capacity(1 + args.len());
        full_args.push(sret_alloca);
        full_args.extend_from_slice(args);
        self.builder.call(func_id, &full_args, name);
        Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
    }

    // -----------------------------------------------------------------------
    // List take (sret helper for for-yield finalization)
    // -----------------------------------------------------------------------

    /// Emit `ori_list_take(list_ptr, out_ptr)` with manual sret handling.
    ///
    /// `ori_list_take` uses an explicit sret pattern: `void(ptr list, ptr out)`.
    /// We alloca a `{i64, i64, ptr}` result, call the function, then load.
    fn emit_list_take(&mut self, list_var: ArcVarId, _func: &ArcFunction) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_take");
        let list_ptr = self.var(list_var);

        // Alloca {i64, i64, ptr} for the result
        let list_struct_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            "list_take.out",
            list_struct_ty,
        );

        // Call ori_list_take(list_ptr, out_alloca) — void return
        self.builder
            .call(func_id, &[list_ptr, out_alloca], "list_take");

        // Load the result struct from the alloca
        Some(
            self.builder
                .load(list_struct_ty, out_alloca, "list_take.val"),
        )
    }

    // -----------------------------------------------------------------------
    // Aggregate-to-pointer coercion
    // -----------------------------------------------------------------------

    /// Coerce an aggregate value to a pointer for runtime function calls.
    ///
    /// Runtime functions like `ori_print` expect `ptr` arguments (pointers to
    /// structs), but ARC IR passes aggregate values directly. When we detect
    /// that a call arg is an aggregate but the callee expects `ptr`, we
    /// alloca+store+pass the pointer.
    pub(super) fn coerce_aggregate_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let tag = self.pool.tag(ty);
        match tag {
            Tag::Str | Tag::List | Tag::Set | Tag::Map => {
                let llvm_ty = self.resolve_type(ty);
                let alloca =
                    self.builder
                        .create_entry_alloca(self.current_function, "rt_arg", llvm_ty);
                self.builder.store(val, alloca);
                alloca
            }
            _ => val,
        }
    }

    /// Coerce any value (including scalars) to a pointer via alloca+store.
    ///
    /// Unlike `coerce_aggregate_to_ptr` which only handles struct types,
    /// this works for ALL types. Used by `ori_list_push` which needs a
    /// `*const u8` pointer to any element's bytes.
    pub(super) fn coerce_any_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let llvm_ty = self.resolve_type(ty);
        let alloca = self
            .builder
            .create_entry_alloca(self.current_function, "elem_arg", llvm_ty);
        self.builder.store(val, alloca);
        alloca
    }

    // -----------------------------------------------------------------------
    // Format call decomposition
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // String runtime call helpers
    // -----------------------------------------------------------------------

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
            let result = self.builder.call(func_id, &[lhs_ptr, rhs_ptr], func_name);
            result.unwrap_or_else(|| self.builder.const_bool(false))
        }
    }
}
