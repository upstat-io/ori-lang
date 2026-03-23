//! SEH main wrapper: `ori_try_call` thunk for `@main(args: [str])` on MSVC.
//!
//! On MSVC, Ori panics use `RaiseException` with a custom exception code
//! (`ORI_SEH_EXCEPTION_CODE`). LLVM's `catchpad`/`__CxxFrameHandler3` does NOT
//! catch these — only the C-level `__try`/`__except` in `ori_try_call` does.
//!
//! This module generates a thunk function and the `ori_try_call`-based call
//! site, mirroring the [`super::super::arc_emitter::catch_thunk`] pattern
//! used by `catch(expr:)`.

use super::entry_point::MainArgsCleanup;
use super::FunctionCompiler;
use crate::codegen::abi::{ParamPassing, ReturnPassing};
use crate::codegen::value_id::FunctionId;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Emit `_ori_main` call via `ori_try_call` thunk (SEH/MSVC).
    ///
    /// Wraps `_ori_main` in a thunk callable by `ori_try_call`, which uses
    /// `__try`/`__except` to catch Ori's custom SEH exception. On success,
    /// the wrapper extracts the return value, runs conditional cleanup and
    /// leak check, then returns. On catch, it always cleans up args and
    /// returns exit code 1.
    pub(super) fn emit_main_call_with_seh_try(
        &mut self,
        c_main_id: FunctionId,
        ori_main_id: FunctionId,
        call_args: &[crate::codegen::value_id::ValueId],
        return_passing: ReturnPassing,
        returns_int: bool,
        i32_ty: crate::codegen::value_id::LLVMTypeId,
        args_cleanup: &MainArgsCleanup,
    ) {
        let MainArgsCleanup {
            cleanup_fn,
            data,
            len,
            wrapper_owns_on_normal,
            list_ty,
            param_passing,
        } = *args_cleanup;

        // Build context struct: { list_struct [, i64 result] }
        let i64_ty = self.builder.i64_type();
        let mut ctx_fields = vec![self.builder.arena.get_type(list_ty)];
        if returns_int {
            ctx_fields.push(self.builder.arena.get_type(i64_ty));
        }
        let ctx_struct = self.builder.scx().type_struct(&ctx_fields, false);
        let ctx_struct_ty = self.builder.register_type(ctx_struct.into());

        // Generate thunk and allocate context
        let thunk_id = self.generate_main_seh_thunk(
            ori_main_id,
            return_passing,
            returns_int,
            list_ty,
            ctx_struct_ty,
            param_passing,
        );
        let ctx_alloca = self
            .builder
            .create_entry_alloca(c_main_id, "main.seh.ctx", ctx_struct_ty);

        // Store args value into context field 0
        let arg_field = self
            .builder
            .struct_gep(ctx_struct_ty, ctx_alloca, 0, "ctx.arg");
        match param_passing {
            ParamPassing::Direct => {
                self.builder.store(call_args[0], arg_field);
            }
            ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                // call_args[0] is a pointer — load the struct value and store
                let val = self.builder.load(list_ty, call_args[0], "ctx.arg.load");
                self.builder.store(val, arg_field);
            }
            ParamPassing::Void => unreachable!(),
        }

        // Call ori_try_call(thunk, ctx) → i64 (1=ok, 0=caught)
        let try_call_fn = self.builder.runtime_fn("ori_try_call");
        let thunk_ptr = self.builder.get_function_ptr(thunk_id);
        let result = self
            .builder
            .call(try_call_fn, &[thunk_ptr, ctx_alloca], "try.result")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let one = self.builder.const_i64(1);
        let is_ok = self.builder.icmp_eq(result, one, "try.ok");
        let success_bb = self.builder.append_block(c_main_id, "seh.success");
        let caught_bb = self.builder.append_block(c_main_id, "seh.caught");
        self.builder.cond_br(is_ok, success_bb, caught_bb);

        // Success: extract result, conditional cleanup, leak check, return
        self.builder.position_at_end(success_bb);
        let main_exit_code = if returns_int {
            let result_ptr = self
                .builder
                .struct_gep(ctx_struct_ty, ctx_alloca, 1, "ctx.result");
            let result_val = self.builder.load(i64_ty, result_ptr, "main.result");
            self.builder.trunc(result_val, i32_ty, "exit_code")
        } else {
            self.builder.const_i32(0)
        };
        if wrapper_owns_on_normal {
            self.builder.call(cleanup_fn, &[data, len], "");
        }
        self.emit_leak_check_and_ret(main_exit_code);

        // Caught: always cleanup args + return exit code 1
        self.builder.position_at_end(caught_bb);
        self.builder.call(cleanup_fn, &[data, len], "");
        let panic_exit = self.builder.const_i32(1);
        self.builder.ret(panic_exit);
    }

    /// Generate `void @_ori_main_seh_thunk(ptr %ctx)` for `ori_try_call`.
    ///
    /// Loads `_ori_main`'s arg from context, calls it, stores the result.
    /// NOT nounwind — the callee's panic must propagate through to
    /// `ori_try_call`'s `__try`/`__except`.
    fn generate_main_seh_thunk(
        &mut self,
        ori_main_id: FunctionId,
        return_passing: ReturnPassing,
        returns_int: bool,
        list_ty: crate::codegen::value_id::LLVMTypeId,
        ctx_struct_ty: crate::codegen::value_id::LLVMTypeId,
        param_passing: ParamPassing,
    ) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();
        let thunk_id = self
            .builder
            .declare_void_function("_ori_main_seh_thunk", &[ptr_ty]);
        self.builder.set_ccc(thunk_id);
        self.builder.add_uwtable_attribute(thunk_id);

        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        let entry = self.builder.append_block(thunk_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(thunk_id);

        let ctx_ptr = self.builder.get_param(thunk_id, 0);

        // Load/address arg from context field 0
        let call_arg = match param_passing {
            ParamPassing::Direct => {
                let field_ptr = self
                    .builder
                    .struct_gep(ctx_struct_ty, ctx_ptr, 0, "thunk.arg.ptr");
                self.builder.load(list_ty, field_ptr, "thunk.arg")
            }
            ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                // Callee expects a pointer — pass address of context field
                self.builder
                    .struct_gep(ctx_struct_ty, ctx_ptr, 0, "thunk.arg.ptr")
            }
            ParamPassing::Void => unreachable!(),
        };

        // Call _ori_main
        let result = match &return_passing {
            ReturnPassing::Direct => self.builder.call(ori_main_id, &[call_arg], "thunk.result"),
            ReturnPassing::Void | ReturnPassing::Sret { .. } => {
                self.builder.call(ori_main_id, &[call_arg], "");
                None
            }
        };

        // Store return value into context field 1
        if returns_int {
            if let Some(val) = result {
                let result_ptr =
                    self.builder
                        .struct_gep(ctx_struct_ty, ctx_ptr, 1, "thunk.result.ptr");
                self.builder.store(val, result_ptr);
            }
        }

        self.builder.ret_void();

        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        thunk_id
    }
}
