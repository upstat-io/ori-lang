//! Closure wrapper function generation for [`ArcIrEmitter`].
//!
//! Generates `_ori_partial_N` wrapper functions that bridge the closure
//! calling convention `(env_ptr, user_args...)` to the lambda's flat
//! calling convention `(captures..., user_args...)`.

use ori_arc::ownership::Ownership;
use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ParamAbi, ParamPassing, ReturnPassing};
use crate::codegen::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Generate a wrapper function for a closure.
    ///
    /// The wrapper bridges the closure calling convention `(env_ptr, user_args...)`
    /// to the lambda's flat calling convention `(captures..., user_args...)`.
    ///
    /// ```text
    /// define ccc ret_type @_ori_partial_N(ptr %env, <user_param_types...>) {
    ///   %cap.0 = gep env_struct, %env, 0, 1 → load
    ///   ...
    ///   %result = call fastcc ret_type @callee(%cap.0, ..., %user_param_0, ...)
    ///   ret ret_type %result
    /// }
    /// ```
    #[expect(
        clippy::too_many_lines,
        reason = "closure wrapper emits sequential LLVM IR setup"
    )]
    pub(super) fn generate_closure_wrapper(
        &mut self,
        callee_func_id: FunctionId,
        callee_abi: &FunctionAbi,
        capture_types: &[Idx],
        capture_ownership: &[Ownership],
        remaining_params: &[ParamAbi],
        target_is_nounwind: bool,
    ) -> ValueId {
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let wrapper_name = format!("_ori_partial_{partial_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Determine return type and sret mode
        let ptr_ty = self.builder.ptr_type();
        let ret_ty = self.resolve_type(callee_abi.return_abi.ty);
        let has_sret = matches!(callee_abi.return_abi.passing, ReturnPassing::Sret { .. });
        let is_void = matches!(callee_abi.return_abi.passing, ReturnPassing::Void);

        // Build wrapper parameter types.
        // When has_sret: [ptr sret_out, ptr env, user_params...]
        // Otherwise:     [ptr env, user_params...]
        let mut wrapper_param_types =
            Vec::with_capacity(usize::from(has_sret) + 1 + remaining_params.len());
        if has_sret {
            wrapper_param_types.push(ptr_ty); // sret output pointer
        }
        wrapper_param_types.push(ptr_ty); // env_ptr
        for param in remaining_params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = self.resolve_type(param.ty);
                    wrapper_param_types.push(ty);
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    wrapper_param_types.push(ptr_ty);
                }
                ParamPassing::Void => {}
            }
        }

        // Declare wrapper function.
        // When has_sret, the wrapper uses explicit sret on its first parameter
        // (matching the lambda's ABI). This is critical on ARM64 where sret
        // goes in X8 — the trampoline's indirect call must agree with both the
        // wrapper and the lambda on sret placement.
        let wrapper_func_id = if is_void || has_sret {
            self.builder
                .declare_void_function(&wrapper_name, &wrapper_param_types)
        } else {
            self.builder
                .declare_function(&wrapper_name, &wrapper_param_types, ret_ty)
        };
        self.builder.set_ccc(wrapper_func_id);
        self.builder.add_uwtable_attribute(wrapper_func_id);
        if target_is_nounwind {
            self.builder.add_nounwind_attribute(wrapper_func_id);
        }

        // Add sret + noalias attributes on the sret parameter.
        if has_sret {
            self.builder.add_sret_attribute(wrapper_func_id, 0, ret_ty);
            self.builder.add_noalias_attribute(wrapper_func_id, 0);
        }

        // noundef on return value — Ori values are always defined.
        if !is_void && !has_sret {
            self.builder.add_noundef_return_attribute(wrapper_func_id);
        }

        // noundef on env pointer and user params — skip the hidden sret
        // pointer (param 0 when has_sret) because sret is a compiler-managed
        // ABI parameter, not a user value. Matches function_compiler.rs which
        // uses sret_offset for the same purpose.
        let sret_offset = u32::from(has_sret);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "wrapper params bounded by lambda arity, well within u32 range"
        )]
        for i in sret_offset..wrapper_param_types.len() as u32 {
            self.builder.add_noundef_param_attribute(wrapper_func_id, i);
        }

        // Generate wrapper body
        let entry = self.builder.append_block(wrapper_func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(wrapper_func_id);

        // When has_sret: param 0 = sret_out, param 1 = env_ptr
        // Otherwise:     param 0 = env_ptr
        let env_param_idx: u32 = u32::from(has_sret);
        let env_ptr_val = self.builder.get_param(wrapper_func_id, env_param_idx);

        // Set current_function to wrapper so inc_value_rc creates blocks
        // in the right function (it uses self.current_function for append_block).
        let saved_current_function = self.current_function;
        self.current_function = wrapper_func_id;

        // Build env struct type for GEP (same layout as build_closure_env)
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let mut env_fields: Vec<inkwell::types::BasicTypeEnum<'_>> = vec![ptr_llvm];
        for &cap_ty in capture_types {
            env_fields.push(self.type_resolver.resolve(cap_ty));
        }
        let env_struct = self.builder.scx().type_struct(&env_fields, false);
        let env_struct_ty_id = self.builder.register_type(env_struct.into());

        // Unpack captures from env struct (fields 1..N)
        let mut callee_args = Vec::with_capacity(callee_abi.params.len());

        // Handle sret: pass the wrapper's sret parameter through to the callee.
        if has_sret {
            let sret_out = self.builder.get_param(wrapper_func_id, 0);
            callee_args.push(sret_out);
        }

        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_ty) in capture_types.iter().enumerate() {
            let field_ty = self.resolve_type(cap_ty);
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                env_ptr_val,
                (i + 1) as u32,
                &format!("cap.{i}.ptr"),
            );
            // Check callee ABI: if this capture param is Indirect/Reference,
            // pass the pointer directly; otherwise load and pass by value.
            // Note: callee_abi.params does NOT include sret (sret is in return_abi),
            // so params[i] directly maps to the i-th capture parameter.
            let param_passing = callee_abi.params.get(i).map(|p| &p.passing);
            // When the callee takes this capture as OWNED and the capture
            // is RC-tracked, emit RcInc. The env drop function will dec
            // the env's copy, and the lambda (or its downstream closures)
            // will eventually dec the passed copy. Without this inc, nested
            // closures cause double-free: both the outer env drop and the
            // inner env drop dec the same refcount.
            let ownership = capture_ownership
                .get(i)
                .copied()
                .unwrap_or(Ownership::Owned);
            let needs_inc = ownership == Ownership::Owned && self.classifier.needs_rc(cap_ty);

            if matches!(
                param_passing,
                Some(ParamPassing::Indirect { .. } | ParamPassing::Reference)
            ) {
                // Reference passing: load value for RcInc, then pass pointer.
                if needs_inc {
                    let loaded = self
                        .builder
                        .load(field_ty, field_ptr, &format!("cap.{i}.inc"));
                    self.inc_value_rc(loaded, cap_ty, 1);
                }
                callee_args.push(field_ptr);
            } else {
                let cap_val = self.builder.load(field_ty, field_ptr, &format!("cap.{i}"));
                if needs_inc {
                    self.inc_value_rc(cap_val, cap_ty, 1);
                }
                callee_args.push(cap_val);
            }
        }

        // Forward remaining user params.
        // When has_sret: params start at 2 (0=sret, 1=env)
        // Otherwise: params start at 1 (0=env)
        let mut wrapper_param_idx: u32 = env_param_idx + 1;
        for param in remaining_params {
            if param.passing != ParamPassing::Void {
                let user_val = self.builder.get_param(wrapper_func_id, wrapper_param_idx);
                callee_args.push(user_val);
                wrapper_param_idx += 1;
            }
        }

        // Call the actual lambda function
        let result = self.builder.call(callee_func_id, &callee_args, "result");

        // Emit return
        if has_sret {
            // Result was written through sret pointer — return void.
            self.builder.ret_void();
        } else if is_void {
            self.builder.ret_void();
        } else if let Some(val) = result {
            self.builder.ret(val);
        } else {
            let zero = self.builder.const_i64(0);
            self.builder.ret(zero);
        }

        // Restore builder position and emitter's current_function
        self.current_function = saved_current_function;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(wrapper_func_id)
    }
}
