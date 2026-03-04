//! SEH catch trampoline for `catch(expr:)` on Windows MSVC.
//!
//! On MSVC, Ori panics raise a custom SEH exception via `RaiseException`
//! (implemented in `eh_personality.c`). The `ori_try_call` C function
//! catches this with `__try`/`__except`, avoiding LLVM `catchpad` entirely.
//!
//! # Architecture
//!
//! For each catch-type `Invoke` on SEH:
//!
//! 1. **Thunk function**: `void @_ori_catch_thunk$N(ptr %ctx)` — loads args
//!    from a context struct, calls the real function, stores the result back.
//!
//! 2. **Call site**: allocates context, stores args, calls `ori_try_call`
//!    (C, `__try`/`__except`), branches on the result.
//!
//! 3. **Catch block**: now a regular block (no catchpad), reached via the
//!    failure branch of `ori_try_call`.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;

use super::context::EmittedValue;
use super::ArcIrEmitter;
use crate::codegen::abi::{ParamPassing, ReturnPassing};
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a catch-type Invoke using the `ori_try_call` trampoline.
    ///
    /// Instead of LLVM `invoke` + `catchpad` (which Rust's runtime rejects),
    /// this generates:
    /// 1. A thunk function that loads args from a context struct, calls the
    ///    real function, and stores the result.
    /// 2. A call to `ori_try_call(thunk_ptr, ctx_ptr)` → i64 (1=ok, 0=caught).
    /// 3. A conditional branch: success → normal block, caught → unwind block.
    pub(super) fn emit_seh_catch_invoke(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        arc_args: &[ArcVarId],
        normal: ori_arc::ir::ArcBlockId,
        unwind: ori_arc::ir::ArcBlockId,
        arc_func: &ArcFunction,
    ) {
        let func_name_str = self.interner.lookup(callee);
        let normal_block = self.block(normal);
        let unwind_block = self.block(unwind);

        // Collect arg values before we start generating the thunk
        let arg_vals: Vec<ValueId> = arc_args.iter().map(|a| self.var(*a)).collect();

        // Resolve the callee function and its ABI
        let resolved = self
            .lookup_method_by_receiver(callee, arc_args, arc_func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, arc_func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, arc_args, arc_func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi.clone()));

        let Some((callee_func_id, params, ret_abi)) = resolved else {
            // Fallback: try runtime function
            if let Some(func_id) = self.builder.try_runtime_fn(func_name_str) {
                self.emit_seh_catch_rt_call(
                    dst,
                    func_id,
                    arc_args,
                    &arg_vals,
                    normal_block,
                    unwind_block,
                    arc_func,
                );
            } else {
                let msg = format!("unresolved function `{func_name_str}` in SEH catch invoke");
                tracing::warn!("{msg}");
                self.builder.br(normal_block);
                self.builder.position_at_end(normal_block);
                let unit = self.builder.const_i64(0);
                self.def_var(dst, EmittedValue::Immediate(unit));
                self.builder.record_codegen_error_with_msg(msg);
            }
            return;
        };

        // Apply ABI param passing to get the LLVM-level args
        let passed_args = self.apply_param_passing(&arg_vals, &params);

        // Build context struct type: [passed_args..., result]
        let has_result = !matches!(ret_abi.passing, ReturnPassing::Void);

        // Collect LLVM types for each passed arg
        let mut ctx_field_types: Vec<LLVMTypeId> = Vec::with_capacity(passed_args.len() + 1);
        for (i, param_abi) in params.iter().enumerate() {
            if matches!(param_abi.passing, ParamPassing::Void) {
                continue;
            }
            // Indirect/Reference args are passed as pointers at call site,
            // but in the context struct we store the original value (not the ptr).
            // The thunk will take the address of the field.
            match &param_abi.passing {
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    // Store the value itself, thunk will pass &field
                    let field_ty = self.resolve_type(param_abi.ty);
                    ctx_field_types.push(field_ty);
                }
                ParamPassing::Direct => {
                    if i < arg_vals.len() {
                        let field_ty = self.resolve_type(param_abi.ty);
                        ctx_field_types.push(field_ty);
                    }
                }
                ParamPassing::Void => unreachable!(),
            }
        }

        // Result field
        let result_ty = if has_result {
            let ty = match &ret_abi.passing {
                ReturnPassing::Sret { .. } | ReturnPassing::Direct => self.resolve_type(ret_abi.ty),
                ReturnPassing::Void => unreachable!(),
            };
            ctx_field_types.push(ty);
            Some(ty)
        } else {
            None
        };

        // Create the context struct type
        let ctx_field_inkwell: Vec<_> = ctx_field_types
            .iter()
            .map(|&ty_id| self.builder.arena.get_type(ty_id))
            .collect();
        let ctx_struct = self.builder.scx().type_struct(&ctx_field_inkwell, false);
        let ctx_struct_ty = self.builder.register_type(ctx_struct.into());

        let result_field_idx = if has_result {
            ctx_field_types.len() as u32 - 1
        } else {
            0
        };

        // Generate the thunk function
        let thunk_id = self.generate_catch_thunk(
            callee_func_id,
            &params,
            &ret_abi,
            ctx_struct_ty,
            result_field_idx,
        );

        // === Call site emission ===

        // Allocate context struct
        let ctx_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "catch.ctx", ctx_struct_ty);

        // Store args into context fields
        let mut field_idx: u32 = 0;
        let mut passed_idx = 0;
        for param_abi in &params {
            if matches!(param_abi.passing, ParamPassing::Void) {
                continue;
            }
            match &param_abi.passing {
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    // The passed_args[passed_idx] is already a pointer (alloca).
                    // We need to store the *value* into the context field.
                    // Load from the alloca, then store into context.
                    if passed_idx < passed_args.len() {
                        let field_ty = ctx_field_types[field_idx as usize];
                        let val = self
                            .builder
                            .load(field_ty, passed_args[passed_idx], "ctx.val");
                        let field_ptr = self.builder.struct_gep(
                            ctx_struct_ty,
                            ctx_alloca,
                            field_idx,
                            &format!("ctx.arg.{field_idx}"),
                        );
                        self.builder.store(val, field_ptr);
                    }
                    field_idx += 1;
                    passed_idx += 1;
                }
                ParamPassing::Direct => {
                    if passed_idx < passed_args.len() {
                        let field_ptr = self.builder.struct_gep(
                            ctx_struct_ty,
                            ctx_alloca,
                            field_idx,
                            &format!("ctx.arg.{field_idx}"),
                        );
                        self.builder.store(passed_args[passed_idx], field_ptr);
                    }
                    field_idx += 1;
                    passed_idx += 1;
                }
                ParamPassing::Void => unreachable!(),
            }
        }

        // Get thunk function pointer
        let thunk_ptr = self.builder.get_function_ptr(thunk_id);

        // Call ori_try_call(thunk_ptr, ctx_ptr)
        let try_call_fn = self.builder.runtime_fn("ori_try_call");
        let result = self
            .builder
            .call(try_call_fn, &[thunk_ptr, ctx_alloca], "try.result")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Branch: result == 1 → success, result == 0 → caught
        let one = self.builder.const_i64(1);
        let is_ok = self.builder.icmp_eq(result, one, "try.ok");
        self.builder.cond_br(is_ok, normal_block, unwind_block);

        // === Success path: load result from context ===
        self.builder.position_at_end(normal_block);
        if let Some(rty) = result_ty {
            let result_ptr =
                self.builder
                    .struct_gep(ctx_struct_ty, ctx_alloca, result_field_idx, "ctx.result");
            let result_val = self.builder.load(rty, result_ptr, "catch.result");
            self.def_var_repr(dst, result_val, arc_func);
        } else {
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
        }
    }

    /// Generate a catch thunk function: `void @_ori_catch_thunk$N(ptr %ctx)`.
    ///
    /// The thunk loads arguments from the context struct, calls the real
    /// function with proper ABI, and stores the result back.
    fn generate_catch_thunk(
        &mut self,
        callee_id: FunctionId,
        params: &[crate::codegen::abi::ParamAbi],
        ret_abi: &crate::codegen::abi::ReturnAbi,
        ctx_struct_ty: LLVMTypeId,
        result_field_idx: u32,
    ) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();
        let counter = self.catch_thunk_counter;
        self.catch_thunk_counter += 1;

        let name = format!("_ori_catch_thunk${counter}");
        let thunk_id = self.builder.declare_void_function(&name, &[ptr_ty]);
        self.builder.set_ccc(thunk_id);
        // NOT nounwind — the callee may panic, and the unwind must propagate
        // through this thunk so that ori_try_call's catch_unwind can catch it.
        // Add uwtable so the SEH unwinder can walk through the thunk's frame.
        self.builder.add_uwtable_attribute(thunk_id);

        // Save builder state
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        // Create entry block
        let entry = self.builder.append_block(thunk_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(thunk_id);
        self.current_function = thunk_id;

        // Get context pointer param
        let ctx_ptr = self.builder.get_param(thunk_id, 0);

        // Load args from context and build call args
        let mut call_args: Vec<ValueId> = Vec::new();
        let has_sret = matches!(ret_abi.passing, ReturnPassing::Sret { .. });

        // If sret, allocate local for the return value
        let sret_alloca = if has_sret {
            let ret_ty = self.resolve_type(ret_abi.ty);
            let alloca = self.builder.alloca(ret_ty, "thunk.sret");
            call_args.push(alloca);
            Some(alloca)
        } else {
            None
        };

        let mut field_idx: u32 = 0;
        for param_abi in params {
            if matches!(param_abi.passing, ParamPassing::Void) {
                continue;
            }
            let field_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_ptr,
                field_idx,
                &format!("thunk.arg.{field_idx}"),
            );

            match &param_abi.passing {
                ParamPassing::Direct => {
                    let field_ty = self.resolve_type(param_abi.ty);
                    let val = self.builder.load(field_ty, field_ptr, "thunk.load");
                    call_args.push(val);
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    // Callee expects a pointer — pass the address of the field
                    call_args.push(field_ptr);
                }
                ParamPassing::Void => unreachable!(),
            }
            field_idx += 1;
        }

        // Call the real function
        let result = self.builder.call(callee_id, &call_args, "thunk.call");

        // Store result into context
        match &ret_abi.passing {
            ReturnPassing::Direct => {
                if let Some(val) = result {
                    let result_ptr = self.builder.struct_gep(
                        ctx_struct_ty,
                        ctx_ptr,
                        result_field_idx,
                        "thunk.result.ptr",
                    );
                    self.builder.store(val, result_ptr);
                }
            }
            ReturnPassing::Sret { .. } => {
                // Result is in sret alloca — copy to context
                if let Some(sret) = sret_alloca {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    let val = self.builder.load(ret_ty, sret, "thunk.sret.load");
                    let result_ptr = self.builder.struct_gep(
                        ctx_struct_ty,
                        ctx_ptr,
                        result_field_idx,
                        "thunk.result.ptr",
                    );
                    self.builder.store(val, result_ptr);
                }
            }
            ReturnPassing::Void => {
                // Nothing to store
            }
        }

        self.builder.ret_void();

        // Restore builder state
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        thunk_id
    }

    /// Emit a catch-type Invoke for a runtime function via `ori_try_call`.
    ///
    /// Simpler path for runtime functions (which use `ccc` and take `ptr` args).
    fn emit_seh_catch_rt_call(
        &mut self,
        dst: ArcVarId,
        func_id: FunctionId,
        arc_args: &[ArcVarId],
        arg_vals: &[ValueId],
        normal_block: BlockId,
        unwind_block: BlockId,
        arc_func: &ArcFunction,
    ) {
        // Coerce aggregate args to pointers (same as emit_invoke runtime path)
        let coerced_args: Vec<ValueId> = arc_args
            .iter()
            .zip(arg_vals.iter())
            .map(|(arc_var, &val)| {
                let arg_ty = arc_func.var_type(*arc_var);
                self.coerce_aggregate_to_ptr(val, arg_ty)
            })
            .collect();

        // Build context: [coerced_args..., result(i64)]
        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();

        // All runtime fn args are ptr type
        let mut ctx_fields_inkwell: Vec<_> = coerced_args
            .iter()
            .map(|_| self.builder.arena.get_type(ptr_ty))
            .collect();
        // Result field (i64 — runtime functions return i64 or void)
        ctx_fields_inkwell.push(self.builder.arena.get_type(i64_ty));

        let ctx_struct = self.builder.scx().type_struct(&ctx_fields_inkwell, false);
        let ctx_struct_ty = self.builder.register_type(ctx_struct.into());

        let result_field_idx = coerced_args.len() as u32;

        // Generate minimal thunk for runtime call
        let thunk_id = self.generate_rt_catch_thunk(
            func_id,
            coerced_args.len(),
            ctx_struct_ty,
            result_field_idx,
        );

        // Allocate and populate context
        let ctx_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "catch.ctx", ctx_struct_ty);

        for (i, &arg) in coerced_args.iter().enumerate() {
            let field_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_alloca,
                i as u32,
                &format!("ctx.arg.{i}"),
            );
            self.builder.store(arg, field_ptr);
        }

        // Call ori_try_call
        let thunk_ptr = self.builder.get_function_ptr(thunk_id);
        let try_call_fn = self.builder.runtime_fn("ori_try_call");
        let result = self
            .builder
            .call(try_call_fn, &[thunk_ptr, ctx_alloca], "try.result")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let one = self.builder.const_i64(1);
        let is_ok = self.builder.icmp_eq(result, one, "try.ok");
        self.builder.cond_br(is_ok, normal_block, unwind_block);

        // Success path
        self.builder.position_at_end(normal_block);
        let result_ptr =
            self.builder
                .struct_gep(ctx_struct_ty, ctx_alloca, result_field_idx, "ctx.result");
        let result_val = self.builder.load(i64_ty, result_ptr, "catch.result");
        self.def_var_repr(dst, result_val, arc_func);
    }

    /// Generate a catch thunk for a runtime function call.
    ///
    /// All runtime function args are `ptr` type (after coercion).
    fn generate_rt_catch_thunk(
        &mut self,
        callee_id: FunctionId,
        num_args: usize,
        ctx_struct_ty: LLVMTypeId,
        result_field_idx: u32,
    ) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();
        let counter = self.catch_thunk_counter;
        self.catch_thunk_counter += 1;

        let name = format!("_ori_catch_thunk${counter}");
        let thunk_id = self.builder.declare_void_function(&name, &[ptr_ty]);
        self.builder.set_ccc(thunk_id);
        // NOT nounwind — callee may panic; uwtable for SEH frame info
        self.builder.add_uwtable_attribute(thunk_id);

        // Save builder state
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        let entry = self.builder.append_block(thunk_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(thunk_id);
        self.current_function = thunk_id;

        let ctx_ptr = self.builder.get_param(thunk_id, 0);

        // Load args from context
        let mut call_args: Vec<ValueId> = Vec::with_capacity(num_args);
        for i in 0..num_args {
            let field_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_ptr,
                i as u32,
                &format!("thunk.arg.{i}"),
            );
            let val = self.builder.load(ptr_ty, field_ptr, "thunk.load");
            call_args.push(val);
        }

        // Call the runtime function
        let result = self.builder.call(callee_id, &call_args, "thunk.call");

        // Store result
        if let Some(val) = result {
            let result_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_ptr,
                result_field_idx,
                "thunk.result.ptr",
            );
            self.builder.store(val, result_ptr);
        }

        self.builder.ret_void();

        // Restore builder state
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        thunk_id
    }
}
