//! ABI-aware direct and indirect ARC call emission.
//! Internal protocols, casts, and method resolution precede ordinary ABI dispatch.

mod local_yield;

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::abi::{ParamAbi, ReturnAbi, ReturnPassing};
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

use super::{ArcIrEmitter, EmittedValue, StringRuntimeReturnAbi};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an `Apply` instruction (ABI-aware direct call).
    pub(super) fn emit_apply(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
        mono_instance_id: Option<MonoInstanceId>,
    ) {
        let runtime_projection_allowed = self.runtime_projection_allowed(func, dst);

        if runtime_projection_allowed && self.try_emit_local_yield_apply(dst, callee, args, func) {
            return;
        }

        if runtime_projection_allowed && self.try_emit_apply_special(dst, callee, args, func) {
            return;
        }

        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        let result = match self.resolve_callee(callee, args, dst, func, mono_instance_id) {
            Some((func_id, params, ret_abi)) => {
                self.emit_resolved_direct_call(func_id, &params, ret_abi, &arg_vals, args)
            }

            None if runtime_projection_allowed => {
                self.emit_runtime_projection_fallback(dst, callee, args, &arg_vals, func)
            }

            None => self.record_unresolved_direct_call(dst, callee, func),
        };

        // INVARIANT: Record destructor metadata after each push because reallocation can
        // change the scratch buffer before an unwind cleanup releases its elements.
        if runtime_projection_allowed && callee == self.list_rt_names.push && args.len() == 3 {
            self.record_list_builder_element_header(arg_vals[0], func.var_type(args[1]));
        }

        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        } else if !self.builder.has_codegen_errors() {
            // INVARIANT: Every Apply defines its destination, including void calls.
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

        let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) else {
            let msg = invalid_indirect_closure_message(closure);
            tracing::error!("{msg}");
            self.builder.record_codegen_error_with_msg(msg);
            return;
        };

        let (arg_vals, param_types) = self.marshal_indirect_call_args(env_ptr, args, func);

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
            self.emit_indirect_call_sret(dst, ret_ty, &param_types, fn_ptr, &arg_vals, func);
        } else {
            self.emit_indirect_call_direct(dst, ret_ty, &param_types, fn_ptr, &arg_vals, func);
        }
    }

    /// Call a string runtime function: `ori_str_concat`, `ori_str_eq`, `ori_str_ne`.
    ///
    /// String values are `{ i64, i64, ptr }` structs passed by pointer to the runtime.
    /// `return_abi` selects sret `{ i64, i64, ptr }` or direct `i1` return.
    #[expect(
        clippy::expect_used,
        reason = "registered string runtime ABIs always return a value"
    )]
    pub(super) fn emit_str_runtime_call(
        &mut self,
        func_name: &'static str,
        lhs: ValueId,
        rhs: ValueId,
        return_abi: StringRuntimeReturnAbi,
    ) -> ValueId {
        let func_id = self.builder.runtime_fn(func_name);

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        // INVARIANT: Each registered string runtime ABI returns a value in its selected mode.
        match return_abi {
            StringRuntimeReturnAbi::StringSret => self
                .builder
                .call_with_sret(func_id, &[lhs_ptr, rhs_ptr], str_ty, func_name)
                .expect("str-returning runtime call uses sret; builder yields the loaded value"),

            StringRuntimeReturnAbi::BoolDirect => {
                let result = self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], func_name);
                result.expect("str comparison runtime fn is non-void; builder.call returns Some")
            }
        }
    }

    /// Emit a special-case `Apply` that bypasses ordinary callee resolution:
    /// protocol builtins, format calls, prelude functions, and traceless
    /// `Traceable` accessors. Returns `true` when the call was fully emitted
    /// and `dst` defined.
    fn try_emit_apply_special(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        if self.try_emit_protocol(dst, callee, args, func) {
            return true;
        }

        let special = self
            .try_emit_format_call(callee, args, func)
            .or_else(|| {
                let callee_name = self.interner.lookup(callee);
                super::builtins::prelude::try_emit_prelude_function(
                    &mut *self,
                    callee_name,
                    args,
                    func,
                )
            })
            // Why: Traceless accessors have no backend declaration for normal resolution.
            .or_else(|| self.try_emit_traceless_traceable(callee, args, func, func.var_type(dst)));

        match special {
            Some(val) => {
                self.def_var_repr(dst, val, func);
                true
            }

            None => false,
        }
    }

    /// Emit a direct call to a resolved callee per its declared ABI.
    fn emit_resolved_direct_call(
        &mut self,
        func_id: FunctionId,
        params: &[ParamAbi],
        ret_abi: ReturnAbi,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
    ) -> Option<ValueId> {
        let passed_args = self.apply_param_passing(arg_vals, Some(args), params);
        match &ret_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_abi.ty);
                self.call_with_sret(func_id, &passed_args, ret_ty, "call")
            }

            ReturnPassing::Direct | ReturnPassing::Void => {
                self.emit_rt_call(func_id, &passed_args, "call")
            }
        }
    }

    /// Fallback chain for an unresolved callee when runtime projection is
    /// allowed: builtin method, builtin associated function, then a named
    /// `ori_*` runtime function; records a codegen error when nothing matches.
    fn emit_runtime_projection_fallback(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if let Some(val) = self.try_emit_builtin_method(callee, args, func, func.var_type(dst)) {
            return Some(val);
        }
        if let Some(val) = self.try_emit_builtin_associated(callee, args, func.var_type(dst)) {
            return Some(val);
        }
        let callee_name = self.interner.lookup(callee);
        if let Some(func_id) = self.builder.try_runtime_fn(callee_name) {
            return self.emit_coerced_runtime_fn_call(func_id, dst, callee, args, arg_vals, func);
        }
        self.record_unresolved_direct_call(dst, callee, func)
    }

    /// Emit a call to a declared `ori_*` runtime function with coerced
    /// arguments, via sret when the function's declaration requires it.
    fn emit_coerced_runtime_fn_call(
        &mut self,
        func_id: FunctionId,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        let coerced_args = self.coerce_runtime_fn_args(callee, args, arg_vals, func);
        let callee_name = self.interner.lookup(callee);

        if crate::codegen::runtime_decl::rt_fn_needs_sret(callee_name) {
            let ret_ty = self.resolve_type(func.var_type(dst));
            self.call_with_sret(func_id, &coerced_args, ret_ty, "call")
        } else {
            self.emit_rt_call(func_id, &coerced_args, "call")
        }
    }

    /// Record the unresolved-direct-call codegen error; always yields `None`.
    fn record_unresolved_direct_call(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        func: &ArcFunction,
    ) -> Option<ValueId> {
        let callee_name = self.interner.lookup(callee);
        let msg = self.unresolved_direct_call_message(func, dst, callee_name, "apply");
        tracing::warn!("{msg}");
        self.builder.record_codegen_error_with_msg(msg);
        None
    }

    fn record_list_builder_element_header(&mut self, list_ptr: ValueId, element_ty: Idx) {
        let list_struct_ty = self.fat_ptr_llvm_type();
        let len_ptr =
            self.builder
                .struct_gep(list_struct_ty, list_ptr, FIELD_LEN, "list_builder.len_ptr");

        let data_ptr = self.builder.struct_gep(
            list_struct_ty,
            list_ptr,
            FIELD_DATA,
            "list_builder.data_ptr",
        );
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let len = self.builder.load(i64_ty, len_ptr, "list_builder.len");
        let data = self.builder.load(ptr_ty, data_ptr, "list_builder.data");
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(element_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder.call(store_dec, &[data, elem_dec_fn], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder.call(store_count, &[data, len], "");
    }

    /// Marshal explicit closure arguments under the uniform borrowed ABI.
    pub(super) fn marshal_indirect_call_args(
        &mut self,
        env_ptr: ValueId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> (Vec<ValueId>, Vec<LLVMTypeId>) {
        let ptr_ty = self.builder.ptr_type();
        let capacity = args.len().saturating_add(1);
        let mut arg_vals = Vec::with_capacity(capacity);
        let mut param_types = Vec::with_capacity(capacity);
        arg_vals.push(env_ptr);
        param_types.push(ptr_ty);

        for &a in args {
            let arg_ty = func.var_type(a);
            let passing = crate::codegen::abi::compute_closure_param_passing(
                arg_ty,
                self.type_info,
                self.repr_plan,
                self.classifier,
            );

            match passing {
                crate::codegen::abi::ParamPassing::Indirect { .. }
                | crate::codegen::abi::ParamPassing::Reference => {
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

        (arg_vals, param_types)
    }

    /// Emit an indirect call whose return is passed via sret.
    fn emit_indirect_call_sret(
        &mut self,
        dst: ArcVarId,
        ret_ty: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) {
        // Why: ARM64 passes the closure sret pointer in X8, not as an argument.
        let sret_alloca = self.builder.alloca(ret_ty, "icall.sret");
        if let Some(pad) = self.current_cleanup_pad {
            // Why: Calls inside an SEH funclet require its operand bundle.
            self.builder.call_indirect_with_sret_and_funclet(
                ret_ty,
                param_types,
                fn_ptr,
                sret_alloca,
                arg_vals,
                pad,
            );
        } else {
            self.builder.call_indirect_with_sret(
                ret_ty,
                param_types,
                fn_ptr,
                sret_alloca,
                arg_vals,
            );
        }
        let loaded = self.builder.load(ret_ty, sret_alloca, "icall.sret.load");
        self.def_var_repr(dst, loaded, func);
    }

    /// Emit an indirect call whose return is passed directly (or is void).
    fn emit_indirect_call_direct(
        &mut self,
        dst: ArcVarId,
        ret_ty: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) {
        let result = if let Some(pad) = self.current_cleanup_pad {
            self.builder.call_indirect_with_funclet(
                ret_ty,
                param_types,
                fn_ptr,
                arg_vals,
                pad,
                "icall",
            )
        } else {
            self.builder
                .call_indirect(ret_ty, param_types, fn_ptr, arg_vals, "icall")
        };

        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        }
    }
}

/// Builds the diagnostic for a closed target missing from LLVM declarations.
pub(super) fn closed_target_projection_message(target: &str, site: &str) -> String {
    format!(
        "LLVM did not declare closed executable target `{target}` before {site}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug"
    )
}

fn invalid_indirect_closure_message(closure: ArcVarId) -> String {
    format!(
        "LLVM could not read the function and environment fields of indirect-call closure v{}; report this compiler bug",
        closure.raw()
    )
}

#[cfg(test)]
mod tests;
