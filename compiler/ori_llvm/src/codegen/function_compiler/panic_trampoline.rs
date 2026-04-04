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
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::value_id::FunctionId;

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
    #[expect(
        clippy::too_many_lines,
        reason = "panic trampoline emits sequential LLVM IR for PanicInfo"
    )]
    pub(super) fn generate_panic_trampoline(&mut self, panic_name: Name) -> Option<FunctionId> {
        let Some(&(user_panic_id, ref abi)) = self.codegen_ctx.functions.get(&panic_name) else {
            debug!("no @panic function declared — skipping trampoline");
            return None;
        };

        debug!("generating panic handler trampoline");

        // Get the PanicInfo type from the @panic function's first parameter.
        // This is the compiler-resolved type with correct field ordering.
        let panic_info_idx = abi.params.first().map(|p| p.ty);

        // Get field index remapping from the ReprPlan.
        // Declaration order: message(0), location(1), stack_trace(2), thread_id(3)
        // Memory order: determined by descending alignment then descending size
        let field_remap = panic_info_idx.and_then(|idx| {
            self.repr_plan()
                .and_then(|plan| match plan.get_repr(idx)? {
                    ori_repr::MachineRepr::Struct(s) => Some(s),
                    _ => None,
                })
                .map(|repr| {
                    [
                        repr.memory_index(0).unwrap_or(0), // message
                        repr.memory_index(1).unwrap_or(1), // location
                        repr.memory_index(2).unwrap_or(2), // stack_trace
                        repr.memory_index(3).unwrap_or(3), // thread_id
                    ]
                })
        });
        let [msg_idx, loc_idx, trace_idx, tid_idx] = field_remap.unwrap_or([0, 1, 2, 3]);

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

        // Store each field at its memory-order position using struct_gep.
        // Declaration fields: [message, location, stack_trace, thread_id]
        // Memory indices: [msg_idx, loc_idx, trace_idx, tid_idx]
        let fields_with_idx = [
            (msg_idx as u32, message, "pi.message"),
            (loc_idx as u32, location, "pi.location"),
            (trace_idx as u32, stack_trace, "pi.stack_trace"),
            (tid_idx as u32, thread_id, "pi.thread_id"),
        ];
        for (idx, val, name) in fields_with_idx {
            let ptr = self.builder.struct_gep(panic_info_ty_id, alloca, idx, name);
            self.builder.store(val, ptr);
        }

        // Call the user's @panic function with pointer to PanicInfo
        self.builder.call(user_panic_id, &[alloca], "");

        // Emit ret void (handler returns normally → runtime proceeds with default)
        self.builder.ret_void();

        Some(trampoline_id)
    }
}
