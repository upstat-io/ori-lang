//! Panic handler trampoline generation for AOT.
//!
//! Generates `_ori_panic_trampoline`, a C-callable function that bridges
//! the runtime's flat C values (msg ptr/len, file ptr/len, line, col) to
//! the user's `@panic(info: PanicInfo) -> void` Ori function by constructing
//! the `PanicInfo` struct in LLVM IR.
//!
//! IMPORTANT: `PanicInfo` fields are reordered by the compiler's layout optimizer
//! (descending alignment, then descending size). The trampoline must use the
//! compiler's `ReprPlan` to map declaration-order field indices to memory-order
//! indices, or the struct will be misaligned when the user's @panic function
//! reads it.

use ori_ir::Name;
use ori_types::Idx;
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Generate a panic handler trampoline.
    ///
    /// The trampoline bridges the C runtime to the user's `@panic` function:
    /// 1. Receives flat C values from the runtime (msg ptr/len, file ptr/len, line, col)
    /// 2. Constructs the Ori `PanicInfo` struct in LLVM IR
    /// 3. Calls the user's compiled `@panic` function
    ///
    /// Uses `struct_gep` with memory-order field indices from the `ReprPlan`
    /// to handle struct field reordering correctly.
    ///
    /// Returns `Some(FunctionId)` of the trampoline, or `None` if the `@panic`
    /// function was not declared.
    pub(super) fn generate_panic_trampoline(&mut self, panic_name: Name) -> Option<FunctionId> {
        let Some(&(user_panic_id, ref abi)) = self.codegen_ctx.functions.get(&panic_name) else {
            debug!("no @panic function declared — skipping trampoline");
            return None;
        };

        debug!("generating panic handler trampoline");

        // Get the PanicInfo type from the @panic function's first parameter.
        // This is the compiler-resolved type with correct field ordering.
        //
        // PC-2 upstream guarantor: `panic_info_idx` is the user `@panic`
        // FunctionSig.param_types[0], fully covered by `validate_body_types`
        // in `ori_types::check::validators` before reaching codegen. No
        // additional `assert_no_unresolved_idx` guard is needed here.
        let panic_info_idx = abi.params.first().map(|p| p.ty);

        let [msg_idx, loc_idx, trace_idx, tid_idx] = self.panic_info_field_indices(panic_info_idx);

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

        let [msg_data, msg_len, file_data, file_len, line, col] =
            self.panic_trampoline_params(trampoline_id);

        // Build sub-struct types (these have no field reordering — they match
        // the compiler's layout because primitive fields have equal alignment).
        let scx = self.builder.scx();

        // str type: { i64 len, i64 cap, ptr data }
        let str_struct_ty = scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        );
        let str_ty_id = self.builder.register_type(str_struct_ty.into());

        // Build strings via ori_str_from_raw to get proper SSO/RC-managed layout.
        let from_raw = self.builder.runtime_fn("ori_str_from_raw");

        let zero_i64 = self.builder.const_i64(0);
        let null_ptr = self.builder.const_null_ptr();

        // Build message string
        let message =
            self.build_panic_runtime_string(from_raw, msg_data, msg_len, str_ty_id, "message");

        // Build empty function name (ori_str_from_raw with null → SSO empty)
        let empty_str =
            self.build_panic_runtime_string(from_raw, null_ptr, zero_i64, str_ty_id, "empty_fn");

        // Build file name string
        let file_str =
            self.build_panic_runtime_string(from_raw, file_data, file_len, str_ty_id, "file");

        // Build location: TraceEntry = { empty_fn, file, line, col }
        // TraceEntry fields are all 8-byte aligned → no reordering needed.
        let trace_entry_ty = scx.type_struct(
            &[
                str_struct_ty.into(),
                str_struct_ty.into(),
                scx.type_i64().into(),
                scx.type_i64().into(),
            ],
            false,
        );
        let trace_entry_ty_id = self.builder.register_type(trace_entry_ty.into());
        let location = self.builder.build_struct(
            trace_entry_ty_id,
            &[empty_str, file_str, line, col],
            "location",
        );

        // Build empty stack_trace: [TraceEntry] = { 0, 0, null }
        let list_ty = scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        );
        let list_ty_id = self.builder.register_type(list_ty.into());
        let stack_trace =
            self.builder
                .build_struct(list_ty_id, &[zero_i64, zero_i64, null_ptr], "stack_trace");

        // Build thread_id: Option<int> = { 0 (None tag), 0 }
        let option_int_ty = scx.type_struct(&[scx.type_i64().into(), scx.type_i64().into()], false);
        let option_int_ty_id = self.builder.register_type(option_int_ty.into());
        let thread_id =
            self.builder
                .build_struct(option_int_ty_id, &[zero_i64, zero_i64], "thread_id");

        // Build PanicInfo using the compiler's field ordering via struct_gep.
        // The compiler may reorder fields by alignment/size, so we use the
        // remapped indices from the ReprPlan.
        let panic_info_ty_id = if let Some(idx) = panic_info_idx {
            let bte = self.type_resolver.resolve(idx);
            self.builder.register_type(bte)
        } else {
            // Fallback: construct manually (declaration order — may be wrong
            // if the compiler reorders, but this path is only hit when the
            // @panic function has no resolved type).
            let panic_info_ty = scx.type_struct(
                &[
                    str_struct_ty.into(),
                    trace_entry_ty.into(),
                    list_ty.into(),
                    option_int_ty.into(),
                ],
                false,
            );
            self.builder.register_type(panic_info_ty.into())
        };

        let alloca =
            self.builder
                .create_entry_alloca(trampoline_id, "panic_info.ptr", panic_info_ty_id);

        self.store_panic_info_fields(
            panic_info_ty_id,
            alloca,
            [msg_idx, loc_idx, trace_idx, tid_idx],
            [message, location, stack_trace, thread_id],
        );

        // Call the user's @panic function with pointer to PanicInfo
        self.builder.call(user_panic_id, &[alloca], "");

        // Emit ret void (handler returns normally → runtime proceeds with default)
        self.builder.ret_void();

        self.verify_panic_trampoline(trampoline_id);

        Some(trampoline_id)
    }

    fn panic_trampoline_params(&mut self, trampoline_id: FunctionId) -> [ValueId; 6] {
        [
            self.builder.get_param(trampoline_id, 0),
            self.builder.get_param(trampoline_id, 1),
            self.builder.get_param(trampoline_id, 2),
            self.builder.get_param(trampoline_id, 3),
            self.builder.get_param(trampoline_id, 4),
            self.builder.get_param(trampoline_id, 5),
        ]
    }

    fn store_panic_info_fields(
        &mut self,
        panic_info_type: LLVMTypeId,
        panic_info: ValueId,
        indices: [u32; 4],
        values: [ValueId; 4],
    ) {
        let names = [
            "pi.message",
            "pi.location",
            "pi.stack_trace",
            "pi.thread_id",
        ];
        for ((index, value), name) in indices.into_iter().zip(values).zip(names) {
            let pointer = self
                .builder
                .struct_gep(panic_info_type, panic_info, index, name);
            self.builder.store(value, pointer);
        }
    }

    fn panic_info_field_indices(&self, panic_info_idx: Option<Idx>) -> [u32; 4] {
        let Some(repr) = panic_info_idx.and_then(|idx| {
            self.repr_plan().and_then(|plan| match plan.get_repr(idx)? {
                ori_repr::MachineRepr::Struct(repr) => Some(repr),
                _ => None,
            })
        }) else {
            return [0, 1, 2, 3];
        };

        [
            repr.memory_index(0).unwrap_or(0) as u32,
            repr.memory_index(1).unwrap_or(1) as u32,
            repr.memory_index(2).unwrap_or(2) as u32,
            repr.memory_index(3).unwrap_or(3) as u32,
        ]
    }

    fn build_panic_runtime_string(
        &mut self,
        from_raw: FunctionId,
        data: ValueId,
        len: ValueId,
        string_type: LLVMTypeId,
        name: &str,
    ) -> ValueId {
        self.builder
            .call_with_sret(from_raw, &[data, len], string_type, name)
            .unwrap_or_else(|| {
                self.builder
                    .build_struct(string_type, &[len, len, data], name)
            })
    }

    fn verify_panic_trampoline(&mut self, trampoline_id: FunctionId) {
        if !self.verify_arc {
            return;
        }

        let function = self.builder.get_function_value(trampoline_id);
        if !function.verify(true) {
            tracing::error!("LLVM IR verification failed (generate_panic_trampoline)");
            self.builder.record_codegen_error();
        }
    }
}
