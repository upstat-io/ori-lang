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
//!    (See [`super::catch_thunk_gen`].)
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
    #[expect(
        clippy::too_many_lines,
        reason = "SEH catch/invoke emits thunk + try_call + branch"
    )]
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
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi));

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
            match &param_abi.passing {
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
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

        // Allocate context struct
        let ctx_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "catch.ctx", ctx_struct_ty);

        // Store args into context fields
        self.store_args_to_context(
            &params,
            &passed_args,
            &ctx_field_types,
            ctx_struct_ty,
            ctx_alloca,
        );

        // Call ori_try_call, branch, and load result
        self.emit_try_call_and_branch(
            thunk_id,
            ctx_alloca,
            ctx_struct_ty,
            result_field_idx,
            result_ty,
            normal_block,
            unwind_block,
            dst,
            arc_func,
        );
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

        let mut ctx_fields_inkwell: Vec<_> = coerced_args
            .iter()
            .map(|_| self.builder.arena.get_type(ptr_ty))
            .collect();
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

        // Call ori_try_call, branch, and load result
        self.emit_try_call_and_branch(
            thunk_id,
            ctx_alloca,
            ctx_struct_ty,
            result_field_idx,
            Some(i64_ty),
            normal_block,
            unwind_block,
            dst,
            arc_func,
        );
    }

    // -- Shared helpers --

    /// Store ABI-passed args into context struct fields.
    fn store_args_to_context(
        &mut self,
        params: &[crate::codegen::abi::ParamAbi],
        passed_args: &[ValueId],
        ctx_field_types: &[LLVMTypeId],
        ctx_struct_ty: LLVMTypeId,
        ctx_alloca: ValueId,
    ) {
        let mut field_idx: u32 = 0;
        let mut passed_idx = 0;
        for param_abi in params {
            if matches!(param_abi.passing, ParamPassing::Void) {
                continue;
            }
            match &param_abi.passing {
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
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
    }

    /// Call `ori_try_call`, branch on result, and define the destination variable.
    ///
    /// Shared by both the compiled-function and runtime-function catch paths.
    fn emit_try_call_and_branch(
        &mut self,
        thunk_id: FunctionId,
        ctx_alloca: ValueId,
        ctx_struct_ty: LLVMTypeId,
        result_field_idx: u32,
        result_ty: Option<LLVMTypeId>,
        normal_block: BlockId,
        unwind_block: BlockId,
        dst: ArcVarId,
        arc_func: &ArcFunction,
    ) {
        let thunk_ptr = self.builder.get_function_ptr(thunk_id);
        let try_call_fn = self.builder.runtime_fn("ori_try_call");
        let result = self
            .builder
            .call(try_call_fn, &[thunk_ptr, ctx_alloca], "try.result")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let one = self.builder.const_i64(1);
        let is_ok = self.builder.icmp_eq(result, one, "try.ok");
        self.builder.cond_br(is_ok, normal_block, unwind_block);

        // Success path: load result from context
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
}
