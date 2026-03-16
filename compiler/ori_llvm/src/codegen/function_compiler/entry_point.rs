//! AOT entry point generation: `main()` wrapper and panic trampoline.

use ori_ir::Name;
use ori_types::{FunctionSig, Idx};
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::abi::ReturnPassing;
use crate::codegen::value_id::FunctionId;

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

        // Build args for calling the Ori @main function
        let call_args = if has_args {
            // Call ori_args_from_argv(arg_count, arg_values) → Ori [str] via sret
            let arg_count = self.builder.get_param(c_main_id, 0);
            let arg_values = self.builder.get_param(c_main_id, 1);

            let args_fn = self.builder.runtime_fn("ori_args_from_argv");
            // List type: {i64 len, i64 cap, ptr data} — same struct as Str
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
            if let Some(val) = args_val {
                vec![val]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Call the Ori @main function
        let main_exit_code = match &abi.return_abi.passing {
            ReturnPassing::Direct => {
                let result = self
                    .builder
                    .call(ori_main_id, &call_args, "ori_main_result");
                if returns_int {
                    if let Some(val) = result {
                        self.builder.trunc(val, i32_ty, "exit_code")
                    } else {
                        self.builder.const_i32(0)
                    }
                } else {
                    self.builder.const_i32(0)
                }
            }
            ReturnPassing::Void | ReturnPassing::Sret { .. } => {
                self.builder.call(ori_main_id, &call_args, "");
                self.builder.const_i32(0)
            }
        };

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

        // If leak check found issues (non-zero), use that exit code; otherwise use main's.
        let zero = self.builder.const_i32(0);
        let has_leak = self.builder.icmp_ne(leak_code, zero, "has_leak");
        let final_exit = self
            .builder
            .select(has_leak, leak_code, main_exit_code, "final_exit");
        self.builder.ret(final_exit);

        true
    }

    /// Generate a panic handler trampoline.
    ///
    /// The trampoline bridges the C runtime to the user's `@panic` function:
    /// 1. Receives flat C values from the runtime (msg ptr/len, file ptr/len, line, col)
    /// 2. Constructs the Ori `PanicInfo` struct in LLVM IR
    /// 3. Calls the user's compiled `@panic` function
    ///
    /// Returns `Some(FunctionId)` of the trampoline, or `None` if the `@panic`
    /// function was not declared.
    #[expect(
        clippy::too_many_lines,
        reason = "panic trampoline emits sequential LLVM IR for PanicInfo"
    )]
    fn generate_panic_trampoline(&mut self, panic_name: Name) -> Option<FunctionId> {
        let Some(&(user_panic_id, _)) = self.codegen_ctx.functions.get(&panic_name) else {
            debug!("no @panic function declared — skipping trampoline");
            return None;
        };

        debug!("generating panic handler trampoline");

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();

        // Trampoline signature: (ptr msg_data, i64 msg_len, ptr file_data, i64 file_len, i64 line, i64 col) -> void
        let trampoline_id = self.builder.declare_void_function(
            "_ori_panic_trampoline",
            &[ptr_ty, i64_ty, ptr_ty, i64_ty, i64_ty, i64_ty],
        );
        self.builder.set_ccc(trampoline_id);

        let entry = self.builder.append_block(trampoline_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(trampoline_id);

        // Extract parameters
        let msg_data = self.builder.get_param(trampoline_id, 0);
        let msg_len = self.builder.get_param(trampoline_id, 1);
        let file_data = self.builder.get_param(trampoline_id, 2);
        let file_len = self.builder.get_param(trampoline_id, 3);
        let line = self.builder.get_param(trampoline_id, 4);
        let col = self.builder.get_param(trampoline_id, 5);

        // Construct PanicInfo struct:
        //   PanicInfo = { str message, TraceEntry location, [TraceEntry] stack_trace, Option<int> thread_id }
        //
        // Where:
        //   str         = { i64 len, i64 cap, ptr data }
        //   TraceEntry  = { str function, str file, int line, int column }
        //                = { {i64, i64, ptr}, {i64, i64, ptr}, i64, i64 }
        //   [TraceEntry] = { i64 len, i64 cap, ptr data }
        //   Option<int>  = { i8 tag, i64 value }

        let scx = self.builder.scx();

        // str type: { i64, i64, ptr }
        let str_struct_ty = scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        );

        // TraceEntry type: { str, str, i64, i64 }
        let trace_entry_ty = scx.type_struct(
            &[
                str_struct_ty.into(),
                str_struct_ty.into(),
                scx.type_i64().into(),
                scx.type_i64().into(),
            ],
            false,
        );

        // [TraceEntry] type: { i64, i64, ptr }
        let list_ty = scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        );

        // Option<int> type: { i8, i64 }
        let option_int_ty = scx.type_struct(&[scx.type_i8().into(), scx.type_i64().into()], false);

        // PanicInfo type: { str, TraceEntry, [TraceEntry], Option<int> }
        let panic_info_ty = scx.type_struct(
            &[
                str_struct_ty.into(),
                trace_entry_ty.into(),
                list_ty.into(),
                option_int_ty.into(),
            ],
            false,
        );

        // Register all types
        let str_ty_id = self.builder.register_type(str_struct_ty.into());
        let trace_entry_ty_id = self.builder.register_type(trace_entry_ty.into());
        let list_ty_id = self.builder.register_type(list_ty.into());
        let option_int_ty_id = self.builder.register_type(option_int_ty.into());
        let panic_info_ty_id = self.builder.register_type(panic_info_ty.into());

        // Build strings via ori_str_from_raw to get proper SSO/RC-managed layout.
        // Inline {len, cap, global_ptr} would be UB if passed to COW operations
        // (global pointers lack RC headers).
        let from_raw = self.builder.runtime_fn("ori_str_from_raw");

        let zero_i64 = self.builder.const_i64(0);
        let null_ptr = self.builder.const_null_ptr();

        // Build message string
        let message = self
            .builder
            .call_with_sret(from_raw, &[msg_data, msg_len], str_ty_id, "message")
            .unwrap_or_else(|| {
                let msg_cap = msg_len;
                self.builder
                    .build_struct(str_ty_id, &[msg_len, msg_cap, msg_data], "message")
            });

        // Build empty function name (ori_str_from_raw with null → SSO empty)
        let empty_str = self
            .builder
            .call_with_sret(from_raw, &[null_ptr, zero_i64], str_ty_id, "empty_fn")
            .unwrap_or_else(|| {
                self.builder
                    .build_struct(str_ty_id, &[zero_i64, zero_i64, null_ptr], "empty_fn")
            });

        // Build file name string
        let file_str = self
            .builder
            .call_with_sret(from_raw, &[file_data, file_len], str_ty_id, "file")
            .unwrap_or_else(|| {
                let file_cap = file_len;
                self.builder
                    .build_struct(str_ty_id, &[file_len, file_cap, file_data], "file")
            });

        // Build location: TraceEntry = { empty_fn, file, line, col }
        let location = self.builder.build_struct(
            trace_entry_ty_id,
            &[empty_str, file_str, line, col],
            "location",
        );

        // Build empty stack_trace: [TraceEntry] = { 0, 0, null }
        let stack_trace =
            self.builder
                .build_struct(list_ty_id, &[zero_i64, zero_i64, null_ptr], "stack_trace");

        // Build thread_id: Option<int> = { 0 (None tag), 0 }
        let zero_i8 = self.builder.const_i8(0);
        let thread_id =
            self.builder
                .build_struct(option_int_ty_id, &[zero_i8, zero_i64], "thread_id");

        // Build PanicInfo = { message, location, stack_trace, thread_id }
        let panic_info = self.builder.build_struct(
            panic_info_ty_id,
            &[message, location, stack_trace, thread_id],
            "panic_info",
        );

        // The user's @panic function receives PanicInfo via Indirect passing
        // (struct >16 bytes → passed by pointer). Allocate on the stack and
        // pass the pointer.
        let alloca = self.builder.alloca(panic_info_ty_id, "panic_info.ptr");
        self.builder.store(panic_info, alloca);

        // Call the user's @panic function with pointer to PanicInfo
        self.builder.call(user_panic_id, &[alloca], "");

        // Emit ret void (handler returns normally → runtime proceeds with default)
        self.builder.ret_void();

        Some(trampoline_id)
    }
}
