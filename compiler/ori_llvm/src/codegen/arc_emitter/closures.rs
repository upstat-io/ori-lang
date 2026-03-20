//! Closure (partial application) emission for [`ArcIrEmitter`].
//!
//! Handles `PartialApply` instructions: allocating closure environments,
//! generating environment drop functions, and creating wrapper functions
//! that bridge the closure calling convention to the lambda's flat convention.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::Idx;

use super::context::EmittedValue;
use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ParamAbi, ParamPassing, ReturnPassing};
use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a `PartialApply` instruction (closure creation).
    ///
    /// For **non-capturing lambdas** (callee is in `non_capturing_lambdas`),
    /// the lambda was declared with closure-compatible ABI (`ccc` + phantom
    /// `ptr %_env`), so we can use the lambda's function pointer directly as
    /// `fn_ptr` without generating a `_ori_partial_N` trampoline. The closure
    /// is `{ lambda_fn_ptr, null }`.
    ///
    /// For **capturing lambdas**, generates a wrapper function that bridges
    /// the closure calling convention `(env_ptr, user_args...)` to the
    /// lambda's flat convention `(captures..., user_args...)`, allocates an
    /// RC-tracked environment struct, and builds `{ wrapper_fn_ptr, env_ptr }`.
    pub(super) fn emit_partial_apply(
        &mut self,
        dst: ArcVarId,
        _ty: Idx,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let callee_name_str = self.interner.lookup(callee);
        let is_non_capturing = self.ctx.non_capturing_lambdas.contains(&callee);

        tracing::debug!(
            name = callee_name_str,
            captures = args.len(),
            non_capturing = is_non_capturing,
            "ArcIrEmitter: PartialApply — closure creation"
        );

        // Look up the callee (lambda function), already compiled and registered
        let Some(&(callee_func_id, ref callee_abi)) = self.ctx.functions.get(&callee) else {
            tracing::warn!(
                name = callee_name_str,
                "emit_partial_apply: callee not found"
            );
            let closure_ty = self.builder.closure_type();
            let null_ptr = self.builder.const_null_ptr();
            let closure =
                self.builder
                    .build_struct(closure_ty, &[null_ptr, null_ptr], "partial_apply");
            self.def_var(dst, EmittedValue::Aggregate(closure));
            return;
        };

        // Non-capturing fast path: lambda already has closure-compatible ABI,
        // so use its function pointer directly — no wrapper needed.
        if is_non_capturing && args.is_empty() {
            let fn_ptr = self.builder.get_function_ptr(callee_func_id);
            let null_env = self.builder.const_null_ptr();
            let closure_ty = self.builder.closure_type();
            let closure =
                self.builder
                    .build_struct(closure_ty, &[fn_ptr, null_env], "partial_apply.direct");
            self.def_var(dst, EmittedValue::Aggregate(closure));
            return;
        }

        let callee_abi = callee_abi.clone();
        let num_captures = args.len();

        // Capture types (from ARC IR variable types)
        let capture_types: Vec<Idx> = args.iter().map(|&v| func.var_type(v)).collect();

        // Remaining user params (the closure awaits these)
        let remaining_params: Vec<ParamAbi> = callee_abi.params[num_captures..].to_vec();

        // == Allocate and pack the environment ==
        let env_ptr = if capture_types.is_empty() {
            self.builder.const_null_ptr()
        } else {
            self.build_closure_env(args, &capture_types)
        };

        // == Generate wrapper function ==
        let target_is_nounwind = self.ctx.nounwind_functions.contains(&callee);
        let wrapper_fn_ptr = self.generate_closure_wrapper(
            callee_func_id,
            &callee_abi,
            &capture_types,
            &remaining_params,
            target_is_nounwind,
        );

        // == Build fat-pointer closure { wrapper_fn_ptr, env_ptr } ==
        let closure_ty = self.builder.closure_type();
        let closure =
            self.builder
                .build_struct(closure_ty, &[wrapper_fn_ptr, env_ptr], "partial_apply");
        self.def_var(dst, EmittedValue::Aggregate(closure));
    }

    /// Allocate and pack a closure environment struct.
    ///
    /// Layout: `{ ptr drop_fn, cap_0_ty, cap_1_ty, ... }`
    /// Allocated via `ori_rc_alloc` (RC-tracked heap memory).
    fn build_closure_env(&mut self, capture_vars: &[ArcVarId], capture_types: &[Idx]) -> ValueId {
        // Build env struct type: { drop_fn: ptr, cap_0, cap_1, ... }
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let mut env_fields: Vec<inkwell::types::BasicTypeEnum<'_>> = vec![ptr_llvm];
        for &cap_ty in capture_types {
            env_fields.push(self.type_resolver.resolve(cap_ty));
        }
        let env_struct = self.builder.scx().type_struct(&env_fields, false);
        let env_struct_ty_id = self.builder.register_type(env_struct.into());

        // Compute size via LLVM's target layout, falling back to
        // summing field sizes for compound captures (str, tuple, struct).
        let env_size = env_struct
            .size_of()
            .and_then(inkwell::values::IntValue::get_zero_extended_constant)
            .unwrap_or_else(|| TypeLayoutResolver::type_store_size(env_struct.into()));

        // Allocate via ori_rc_alloc(size, align=8)
        let size_val = self.builder.const_i64(env_size as i64);
        let align_val = self.builder.const_i64(8);
        let rc_alloc_func = self.builder.runtime_fn("ori_rc_alloc");
        let data_ptr = self
            .emit_rt_call(rc_alloc_func, &[size_val, align_val], "env.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // Generate drop function for this environment
        let drop_fn_id = self.generate_env_drop_fn(env_struct_ty_id, capture_types, env_size);
        let drop_fn_ptr = self.builder.get_function_ptr(drop_fn_id);

        // Store drop_fn at field 0
        let drop_field = self
            .builder
            .struct_gep(env_struct_ty_id, data_ptr, 0, "env.drop_fn");
        self.builder.store(drop_fn_ptr, drop_field);

        // Store each capture at fields 1..N
        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_var) in capture_vars.iter().enumerate() {
            let cap_val = self.var(cap_var);
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                data_ptr,
                (i + 1) as u32,
                &format!("env.cap.{i}"),
            );
            self.builder.store(cap_val, field_ptr);
        }

        data_ptr
    }

    /// Generate a drop function for a closure environment.
    ///
    /// The drop function RC-decrements each captured variable that is
    /// reference-counted, then frees the environment via `ori_rc_free`.
    fn generate_env_drop_fn(
        &mut self,
        env_struct_ty_id: LLVMTypeId,
        capture_types: &[Idx],
        env_size: u64,
    ) -> FunctionId {
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let func_name = format!("_ori_partial_{partial_id}_drop");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Declare: void @_ori_partial_N_drop(ptr %data)
        let ptr_ty = self.builder.ptr_type();
        let func_id = self.builder.declare_void_function(&func_name, &[ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        // noundef on data pointer param — Ori never passes poison pointers.
        self.builder.add_noundef_param_attribute(func_id, 0);

        // Generate body
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let data_ptr = self.builder.get_param(func_id, 0);

        // RC dec each captured variable that needs it.
        //
        // Collections (List, Set, Map) need special handling: their drop
        // functions expect a pointer to the full `{len, cap, data}` struct,
        // but `ori_rc_dec` only passes the raw data buffer pointer. Use
        // the buffer RC dec helpers instead, which extract len/cap/data
        // from the full value and call the appropriate runtime function.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_ty) in capture_types.iter().enumerate() {
            let needs_rc = self.classifier.needs_rc(cap_ty);
            if needs_rc {
                let field_ty = self.resolve_type(cap_ty);
                let field_ptr = self.builder.struct_gep(
                    env_struct_ty_id,
                    data_ptr,
                    (i + 1) as u32, // +1: field 0 is drop_fn
                    &format!("cap.{i}.ptr"),
                );
                let field_val = self.builder.load(field_ty, field_ptr, &format!("cap.{i}"));

                let resolved = self.pool.resolve_fully(cap_ty);
                let tag = self.pool.tag(resolved);
                match tag {
                    ori_types::Tag::List | ori_types::Tag::Set => {
                        self.emit_buffer_rc_dec_list_or_set(field_val, resolved, tag);
                    }
                    ori_types::Tag::Map => {
                        self.emit_buffer_rc_dec_map(field_val, resolved);
                    }
                    ori_types::Tag::Function => {
                        // Closure: { fn_ptr, env_ptr } — extract env_ptr,
                        // null-check, load dynamic drop_fn from env header.
                        if let Some(env_ptr) =
                            self.builder
                                .extract_value(field_val, 1, &format!("cap.{i}.env"))
                        {
                            if !self.builder.is_const_null_ptr(env_ptr) {
                                let is_null =
                                    self.builder.is_null_ptr(env_ptr, &format!("cap.{i}.null"));
                                let do_dec =
                                    self.builder.append_block(func_id, &format!("cap.{i}.dec"));
                                let skip_blk =
                                    self.builder.append_block(func_id, &format!("cap.{i}.skip"));
                                self.builder.cond_br(is_null, skip_blk, do_dec);

                                self.builder.position_at_end(do_dec);
                                let ptr_ty = self.builder.ptr_type();
                                let drop_fn_val =
                                    self.builder
                                        .load(ptr_ty, env_ptr, &format!("cap.{i}.drop_fn"));
                                let rc_dec_id = self.builder.runtime_fn("ori_rc_dec");
                                self.builder.call(rc_dec_id, &[env_ptr, drop_fn_val], "");
                                self.builder.br(skip_blk);

                                self.builder.position_at_end(skip_blk);
                            }
                        }
                    }
                    _ => {
                        let data_ptrs = self.extract_rc_data_ptrs(field_val, cap_ty);
                        let drop_fn = self.get_or_generate_drop_fn(cap_ty);
                        let rc_dec_id = self.builder.runtime_fn("ori_rc_dec");
                        for data_ptr_val in data_ptrs {
                            self.builder.call(rc_dec_id, &[data_ptr_val, drop_fn], "");
                        }
                    }
                }
            }
        }

        // Free the env struct
        let size_val = self.builder.const_i64(env_size as i64);
        let align_val = self.builder.const_i64(8);
        let rc_free_id = self.builder.runtime_fn("ori_rc_free");
        self.builder
            .call(rc_free_id, &[data_ptr, size_val, align_val], "");
        self.builder.ret_void();

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        func_id
    }

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
    fn generate_closure_wrapper(
        &mut self,
        callee_func_id: FunctionId,
        callee_abi: &FunctionAbi,
        capture_types: &[Idx],
        remaining_params: &[ParamAbi],
        target_is_nounwind: bool,
    ) -> ValueId {
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let wrapper_name = format!("_ori_partial_{partial_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Build wrapper parameter types: ptr %env + remaining user params
        let ptr_ty = self.builder.ptr_type();
        let mut wrapper_param_types = Vec::with_capacity(1 + remaining_params.len());
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

        // Determine return type
        let ret_ty = self.resolve_type(callee_abi.return_abi.ty);
        let has_sret = matches!(callee_abi.return_abi.passing, ReturnPassing::Sret { .. });
        let is_void = matches!(callee_abi.return_abi.passing, ReturnPassing::Void);

        // Declare wrapper function.
        // When the callee uses sret (large return), the wrapper still returns
        // the struct directly — it bridges from the callee's sret convention
        // to a direct return for indirect callers. LLVM's codegen will lower
        // the wrapper's `ret` to sret at the ABI level if needed.
        let wrapper_func_id = if is_void {
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

        // noundef on return value — Ori values are always defined.
        if !is_void {
            self.builder.add_noundef_return_attribute(wrapper_func_id);
        }

        // noundef on all params — env pointer and user params are always defined.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "wrapper params bounded by lambda arity, well within u32 range"
        )]
        for i in 0..wrapper_param_types.len() {
            self.builder
                .add_noundef_param_attribute(wrapper_func_id, i as u32);
        }

        // Generate wrapper body
        let entry = self.builder.append_block(wrapper_func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(wrapper_func_id);

        let env_ptr_val = self.builder.get_param(wrapper_func_id, 0);

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

        // Handle sret: if callee uses sret, allocate a temp and pass it first
        let sret_alloca = if has_sret {
            let alloca = self.builder.alloca(ret_ty, "sret.tmp");
            callee_args.push(alloca);
            Some(alloca)
        } else {
            None
        };

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
            if matches!(
                param_passing,
                Some(ParamPassing::Indirect { .. } | ParamPassing::Reference)
            ) {
                callee_args.push(field_ptr);
            } else {
                let cap_val = self.builder.load(field_ty, field_ptr, &format!("cap.{i}"));
                callee_args.push(cap_val);
            }
        }

        // Forward remaining user params (wrapper params 1..N)
        let mut wrapper_param_idx: u32 = 1; // 0 = env_ptr
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
            if let Some(alloca) = sret_alloca {
                // Load from sret alloca and return... but wrapper is void for sret.
                // Actually, the wrapper itself is called indirectly via ccc.
                // ApplyIndirect doesn't use sret — it uses direct returns.
                // So the wrapper must load from sret and return directly.
                let loaded = self.builder.load(ret_ty, alloca, "sret.load");
                self.builder.ret(loaded);
            }
        } else if is_void {
            self.builder.ret_void();
        } else if let Some(val) = result {
            self.builder.ret(val);
        } else {
            let zero = self.builder.const_i64(0);
            self.builder.ret(zero);
        }

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(wrapper_func_id)
    }
}
