//! AOT entry point generation: C-compatible `main()` wrapper.
//!
//! Panic trampoline generation lives in [`super::panic_trampoline`].

use ori_ir::Name;
use ori_types::{FunctionSig, Idx};
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use crate::codegen::eh_model::EhModel;
use crate::codegen::value_id::{FunctionId, ValueId};

/// Cleanup state for the C main wrapper's `ori_args_cleanup` call.
///
/// Tracks whether the wrapper or the callee owns the args buffer on
/// normal return, so the wrapper avoids double-freeing when the callee
/// takes ownership via Indirect ABI.
pub(super) struct MainArgsCleanup {
    pub(super) cleanup_fn: FunctionId,
    pub(super) data: ValueId,
    pub(super) len: ValueId,
    /// When `true`, the wrapper retains ownership of the args buffer on
    /// normal return (callee borrows via `ParamPassing::Reference`).
    /// When `false`, the callee takes ownership (Indirect/Direct ABI) and
    /// frees the buffer via its ARC dec — the wrapper must NOT double-free.
    ///
    /// On the unwind path, the wrapper always cleans up regardless of this
    /// flag, because the callee's ARC dec doesn't execute when it unwinds.
    pub(super) wrapper_owns_on_normal: bool,
    /// The list struct type (`{ i64, i64, ptr }`), needed by SEH thunk.
    pub(super) list_ty: crate::codegen::value_id::LLVMTypeId,
    /// How the param is passed to `_ori_main`, needed by SEH thunk.
    pub(super) param_passing: ParamPassing,
}

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Generate a C-compatible `main()` wrapper that calls the Ori `@main` function.
    ///
    /// The wrapper bridges the C calling convention (`ccc`) to Ori's internal
    /// calling convention (`fastcc`). Four `@main` signatures are supported:
    ///
    /// | Ori signature               | C wrapper                                    |
    /// |-----------------------------|----------------------------------------------|
    /// | `@main () -> void`          | `define i32 @main() { call @_ori_main(); ret 0 }` |
    /// | `@main () -> int`           | `define i32 @main() { trunc call @_ori_main() }` |
    /// | `@main (args) -> void`      | `define i32 @main(i32, ptr) { ... }`         |
    /// | `@main (args) -> int`       | `define i32 @main(i32, ptr) { ... }`         |
    ///
    /// When `_ori_main` can unwind and args exist, the wrapper uses
    /// `invoke`+`landingpad` (Itanium) or `ori_try_call` thunk (SEH) to
    /// catch Ori panics, clean up the args buffer, and exit with code 1.
    /// On SEH/MSVC, Ori panics use `RaiseException` with a custom code
    /// that LLVM's `catchpad` does not catch — `ori_try_call` uses C-level
    /// `__try`/`__except` instead.
    ///
    /// Must be called after `declare_all()` + `emit_prepared_functions()` so
    /// the `@main` function is already compiled. Returns `false` if no `@main`
    /// was found.
    pub fn generate_main_wrapper(
        &mut self,
        main_name: Name,
        main_sig: &FunctionSig,
        panic_name: Option<Name>,
    ) -> bool {
        let Some(&(ori_main_id, ref abi)) = self.codegen_ctx.functions.get(&main_name) else {
            debug!("no @main function declared — skipping entry point wrapper");
            return false;
        };
        let abi = abi.clone();

        // Generate panic trampoline if @panic handler exists
        let panic_trampoline = panic_name.and_then(|name| self.generate_panic_trampoline(name));

        let has_args = !main_sig.param_types.is_empty();
        let returns_int = main_sig.return_type == Idx::INT;

        debug!(
            has_args,
            returns_int,
            has_panic = panic_trampoline.is_some(),
            "generating C main() entry point wrapper"
        );

        // C main signature: i32 @main() or i32 @main(i32 %argc, ptr %argv)
        let i32_ty = self.builder.i32_type();
        let c_main_params = if has_args {
            let ptr_ty = self.builder.ptr_type();
            vec![i32_ty, ptr_ty]
        } else {
            vec![]
        };

        let c_main_id = self
            .builder
            .declare_function("main", &c_main_params, i32_ty);
        self.builder.set_ccc(c_main_id);
        self.builder.add_uwtable_attribute(c_main_id);
        self.builder.add_noundef_return_attribute(c_main_id);

        // The C main wrapper is nounwind if the Ori @main is nounwind —
        // all calls it makes (including _ori_main) are proven non-unwinding.
        if self.codegen_ctx.nounwind_functions.contains(&main_name) {
            self.builder.add_nounwind_attribute(c_main_id);
        }

        let entry = self.builder.append_block(c_main_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(c_main_id);

        // Register panic handler trampoline if present
        if let Some(trampoline_id) = panic_trampoline {
            let register_fn = self.builder.runtime_fn("ori_register_panic_handler");
            let trampoline_ptr = self.builder.get_function_ptr(trampoline_id);
            self.builder.call(register_fn, &[trampoline_ptr], "");
        }

        // Build args and prepare cleanup info
        let (call_args, args_cleanup) = self.build_main_args(has_args, c_main_id, &abi);

        // Determine if _ori_main can unwind. When it can AND args exist,
        // use invoke+landingpad to clean up the args buffer on the unwind path.
        // Even when the callee takes ownership (Indirect ABI), we need invoke
        // for unwind cleanup — the callee's ARC dec hasn't run if it unwinds.
        let can_unwind = !self.codegen_ctx.nounwind_functions.contains(&main_name);
        let use_invoke = can_unwind && args_cleanup.is_some();

        if use_invoke {
            let args = args_cleanup.unwrap();
            match self.builder.eh_model() {
                EhModel::Itanium => self.emit_main_call_with_invoke(
                    c_main_id,
                    ori_main_id,
                    &call_args,
                    abi.return_abi.passing,
                    returns_int,
                    i32_ty,
                    &args,
                ),
                EhModel::Seh => self.emit_main_call_with_seh_try(
                    c_main_id,
                    ori_main_id,
                    &call_args,
                    abi.return_abi.passing,
                    returns_int,
                    i32_ty,
                    &args,
                ),
            }
        } else {
            self.emit_main_call_direct(
                ori_main_id,
                &call_args,
                abi.return_abi.passing,
                returns_int,
                i32_ty,
                args_cleanup,
            );
        }

        true
    }

    /// Build args for calling the Ori `@main` function.
    ///
    /// Returns `(call_args, args_cleanup)` where `args_cleanup` contains
    /// the cleanup function, data pointer, length, and ownership flag.
    fn build_main_args(
        &mut self,
        has_args: bool,
        c_main_id: FunctionId,
        abi: &FunctionAbi,
    ) -> (Vec<ValueId>, Option<MainArgsCleanup>) {
        if !has_args {
            return (vec![], None);
        }

        let arg_count = self.builder.get_param(c_main_id, 0);
        let arg_values = self.builder.get_param(c_main_id, 1);

        let args_fn = self.builder.runtime_fn("ori_args_from_argv");
        let list_ty = self.builder.register_type(
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
        let args_val =
            self.builder
                .call_with_sret(args_fn, &[arg_count, arg_values], list_ty, "args");

        let Some(val) = args_val else {
            return (vec![], None);
        };

        // Determine callee ownership: Reference means callee borrows (wrapper
        // retains ownership), Indirect/Direct means callee takes ownership.
        let param_passing = abi.params.first().map(|p| &p.passing);
        let wrapper_owns_on_normal = matches!(param_passing, Some(ParamPassing::Reference));

        // Check callee's param ABI: Indirect/Reference means _ori_main
        // expects a pointer, not the struct value directly.
        let call_arg = if matches!(
            param_passing,
            Some(ParamPassing::Indirect { .. } | ParamPassing::Reference)
        ) {
            let alloca = self
                .builder
                .create_entry_alloca(c_main_id, "args.indirect", list_ty);
            self.builder.store(val, alloca);
            alloca
        } else {
            val
        };

        // Pre-extract data/len for cleanup — available in both normal and
        // unwind paths since they dominate both.
        let cleanup_fn = self.builder.runtime_fn("ori_args_cleanup");
        let data = self
            .builder
            .extract_value(val, 2, "args.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(val, 0, "args.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let cleanup = MainArgsCleanup {
            cleanup_fn,
            data,
            len,
            wrapper_owns_on_normal,
            list_ty,
            param_passing: param_passing.copied().unwrap_or(ParamPassing::Direct),
        };
        (vec![call_arg], Some(cleanup))
    }

    /// Emit `_ori_main` call via `invoke` + catch-all landingpad (Itanium).
    ///
    /// Since `main` is the top of the call stack, we use catch-all semantics
    /// (not cleanup+resume) — this guarantees the unwinder finds a handler in
    /// Phase 1, runs all cleanup pads in Phase 2, then our catch-all frees
    /// the args buffer, frees the exception object, and returns exit code 1.
    fn emit_main_call_with_invoke(
        &mut self,
        c_main_id: FunctionId,
        ori_main_id: FunctionId,
        call_args: &[ValueId],
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
            ..
        } = *args_cleanup;

        let personality_id = self.builder.runtime_fn("ori_eh_personality");
        self.builder.set_personality(c_main_id, personality_id);

        let normal_bb = self.builder.append_block(c_main_id, "normal");
        let catch_bb = self.builder.append_block(c_main_id, "catch");

        // Invoke _ori_main — normal return → normal_bb, unwind → catch_bb
        let main_result = match &return_passing {
            ReturnPassing::Direct => self.builder.invoke(
                ori_main_id,
                call_args,
                normal_bb,
                catch_bb,
                "ori_main_result",
            ),
            ReturnPassing::Void | ReturnPassing::Sret { .. } => {
                self.builder
                    .invoke(ori_main_id, call_args, normal_bb, catch_bb, "");
                None
            }
        };

        // Normal path: conditional cleanup + leak check + return
        self.builder.position_at_end(normal_bb);
        let main_exit_code = match (returns_int, main_result) {
            (true, Some(val)) => self.builder.trunc(val, i32_ty, "exit_code"),
            _ => self.builder.const_i32(0),
        };
        if wrapper_owns_on_normal {
            self.builder.call(cleanup_fn, &[data, len], "");
        }
        self.emit_leak_check_and_ret(main_exit_code);

        // Catch path: landingpad catch-all + cleanup + exit(1)
        self.builder.position_at_end(catch_bb);
        let lp = self
            .builder
            .landingpad_catch_all(personality_id, "lp.catch");
        if let Some(exc_ptr) = self.builder.extract_value(lp, 0, "exc.ptr") {
            let catch_cleanup = self.builder.runtime_fn("ori_catch_cleanup");
            self.builder.call(catch_cleanup, &[exc_ptr], "");
        }
        self.builder.call(cleanup_fn, &[data, len], "");
        let panic_exit = self.builder.const_i32(1);
        self.builder.ret(panic_exit);
    }

    /// Emit `_ori_main` call via direct `call` (no unwind handling needed).
    ///
    /// Used when `_ori_main` is nounwind or has no args. When args exist
    /// but the callee takes ownership (Indirect ABI), cleanup is skipped
    /// to avoid double-freeing the args buffer.
    fn emit_main_call_direct(
        &mut self,
        ori_main_id: FunctionId,
        call_args: &[ValueId],
        return_passing: ReturnPassing,
        returns_int: bool,
        i32_ty: crate::codegen::value_id::LLVMTypeId,
        args_cleanup: Option<MainArgsCleanup>,
    ) {
        let main_exit_code = match &return_passing {
            ReturnPassing::Direct => {
                let result = self.builder.call(ori_main_id, call_args, "ori_main_result");
                match (returns_int, result) {
                    (true, Some(val)) => self.builder.trunc(val, i32_ty, "exit_code"),
                    _ => self.builder.const_i32(0),
                }
            }
            ReturnPassing::Void | ReturnPassing::Sret { .. } => {
                self.builder.call(ori_main_id, call_args, "");
                self.builder.const_i32(0)
            }
        };

        if let Some(cleanup) = args_cleanup {
            if cleanup.wrapper_owns_on_normal {
                self.builder
                    .call(cleanup.cleanup_fn, &[cleanup.data, cleanup.len], "");
            }
        }

        self.emit_leak_check_and_ret(main_exit_code);
    }

    /// Emit `ori_check_leaks` call, select final exit code, and return.
    pub(super) fn emit_leak_check_and_ret(
        &mut self,
        main_exit_code: crate::codegen::value_id::ValueId,
    ) {
        // Check for RC leaks (ORI_CHECK_LEAKS=1). Returns 0 if clean, 2 if leaks.
        // Exit code precedence: leak code (2) overrides main's exit code when both
        // are non-zero. This is intentional — in testing, leaked memory is surfaced
        // over application-level failures so the test harness can distinguish leaks
        // from runtime errors. When ORI_CHECK_LEAKS is unset, ori_check_leaks()
        // returns 0, so main's exit code passes through unchanged.
        let check_leaks_fn = self.builder.runtime_fn("ori_check_leaks");
        let leak_code = self
            .builder
            .call(check_leaks_fn, &[], "leak_check")
            .unwrap_or(main_exit_code);

        let zero = self.builder.const_i32(0);
        let has_leak = self.builder.icmp_ne(leak_code, zero, "has_leak");
        let final_exit = self
            .builder
            .select(has_leak, leak_code, main_exit_code, "final_exit");
        self.builder.ret(final_exit);
    }
}
