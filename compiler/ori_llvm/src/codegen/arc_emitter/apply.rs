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
//! - [`apply_resolution`](super::apply_resolution) — closed target, method,
//!   and monomorphized call resolution

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use super::{ArcIrEmitter, EmittedValue};
use crate::codegen::abi::ReturnPassing;
use crate::codegen::value_id::ValueId;

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
        let callee_name_str = self.interner.lookup(callee);
        let runtime_projection_allowed = self.runtime_projection_allowed(func, dst);

        // Internal protocol intercepts (__iter_next, __collect_set, etc.)
        if runtime_projection_allowed && self.try_emit_protocol(dst, callee, args, func) {
            return;
        }

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if runtime_projection_allowed {
            if let Some(val) = self.try_emit_format_call(callee, args, func) {
                self.def_var_repr(dst, val, func);
                return;
            }
        }

        // Prelude builtin functions (str, int, float, byte, hash_combine, etc.)
        if runtime_projection_allowed {
            if let Some(val) = super::builtins::prelude::try_emit_prelude_function(
                self,
                callee_name_str,
                args,
                func,
            ) {
                self.def_var_repr(dst, val, func);
                return;
            }
        }

        // Traceless Traceable accessors (Error-struct + Result/Option delegation)
        // must precede `resolve_callee`: a `backend_required: false` Traceable
        // method otherwise resolves to an unbacked `_ori_trace` mono decl.
        if runtime_projection_allowed {
            if let Some(val) =
                self.try_emit_traceless_traceable(callee, args, func, func.var_type(dst))
            {
                self.def_var_repr(dst, val, func);
                return;
            }
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
        } else if let Some(val) = runtime_projection_allowed
            .then(|| self.try_emit_builtin_method(callee, args, func, func.var_type(dst)))
            .flatten()
        {
            Some(val)
        } else if let Some(val) = runtime_projection_allowed
            .then(|| self.try_emit_builtin_associated(callee, args, func.var_type(dst)))
            .flatten()
        {
            Some(val)
        } else if let Some(func_id) = runtime_projection_allowed
            .then(|| self.builder.try_runtime_fn(callee_name_str))
            .flatten()
        {
            let coerced_args = self.coerce_runtime_fn_args(callee, args, &arg_vals, func);

            // Large struct returns (Str, List, Map) use sret convention.
            if crate::codegen::runtime_decl::rt_fn_needs_sret(callee_name_str) {
                let ret_ty = self.resolve_type(func.var_type(dst));
                self.call_with_sret(func_id, &coerced_args, ret_ty, "call")
            } else {
                self.emit_rt_call(func_id, &coerced_args, "call")
            }
        } else {
            let msg = self.unresolved_direct_call_message(func, dst, callee_name_str, "apply");
            tracing::warn!("{msg}");
            self.builder.record_codegen_error_with_msg(msg);
            None
        };

        // A for-yield scratch list is heap-allocated by `ori_list_new` and can
        // unwind before `ori_list_take` finalizes it. Persist the initialized
        // element count and destructor after every successful push so the
        // cleanup-only `ori_list_free` path can release heap elements as well
        // as the backing buffer. The push may reallocate, so this must happen
        // after the runtime call and reload the builder's current data pointer.
        if runtime_projection_allowed && callee == self.list_rt_names.push && args.len() == 3 {
            self.record_list_builder_element_header(arg_vals[0], func.var_type(args[1]));
        }

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

pub(super) fn closed_target_projection_message(target: &str, site: &str) -> String {
    format!(
        "LLVM did not declare closed executable target `{target}` before {site}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug"
    )
}

#[cfg(test)]
mod diagnostic_tests {
    use super::closed_target_projection_message;

    #[test]
    fn closed_target_diagnostic_states_cause_and_action() {
        let message = closed_target_projection_message("clone$derived$7", "apply");
        assert!(message.contains("did not declare closed executable target"));
        assert!(message.contains("ORI_VERIFY_ARC=1"));
        assert!(message.contains("report this compiler bug"));
        assert!(!message.contains("missing mono instance"));
    }
}
