//! Builtin `Error` struct-constructor emission.
//!
//! Declares + defines `_ori_Error_ctor` so a first-class `Error` value resolves
//! to a real function pointer; called from `declare_all` in the parent module.

use super::{compute_function_abi, FunctionCompiler, FunctionSig, Idx};

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Declare + define the builtin `Error` struct constructor as a
    /// referenceable closure-ABI function so a first-class `Error` value
    /// (`let f = Error`) resolves to a real function pointer instead of an
    /// unresolvable `PartialApply @Error` (Spec: Annex E §Built-in Type
    /// Representations). `Error` layout is `{ message: str, trace: [TraceEntry] }`
    /// (48 bytes, sret-returned): the body moves the `message` str param into the
    /// sret slot per-field and zero-inits `trace` to `{ 0, 0, null }` (FastISel-safe).
    pub(super) fn declare_error_constructor(&mut self) {
        let Some(error_idx) = self.pool.error_struct_idx() else {
            return;
        };
        let error_name = self.interner.intern("Error");
        if self.codegen_ctx.functions.contains_key(&error_name) {
            return;
        }

        let msg_name = self.interner.intern("msg");
        let sig = FunctionSig::synthetic(error_name, vec![msg_name], vec![Idx::STR], error_idx);
        let abi = compute_function_abi(&sig, self.type_info, self.repr_plan());

        let ptr_ty = self.builder.ptr_type();
        let func_id =
            self.declare_function_llvm_with_extra_params("_ori_Error_ctor", &abi, &[ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.set_internal_linkage(func_id);
        self.builder.add_nounwind_attribute(func_id);

        self.codegen_ctx
            .functions
            .insert(error_name, (func_id, abi));
        self.codegen_ctx.non_capturing_lambdas.insert(error_name);

        // Body: move the 24-byte str (param 2: sret=0, phantom env=1, str=2)
        // into the Error sret slot (param 0). Per-field GEP+load+store.
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let ret_ptr = self.builder.get_param(func_id, 0);
        let str_ptr = self.builder.get_param(func_id, 2);

        let error_llvm = self.resolve_type(error_idx);
        let error_struct_ty = self.builder.register_type(error_llvm);

        let remap_field = |decl_field: u32| -> u32 {
            let Some(plan) = self.repr_plan() else {
                return decl_field;
            };
            let resolved = self.pool.resolve_fully(error_idx);
            let Some(repr) = plan.get_repr(resolved) else {
                return decl_field;
            };
            match repr {
                ori_repr::MachineRepr::Struct(s) => {
                    s.memory_index(decl_field).map_or(decl_field, |i| i as u32)
                }
                _ => decl_field,
            }
        };

        let mem_message_field = remap_field(0);
        let mem_trace_field = remap_field(1);

        // Copy message string (param 2) to field message of the Error struct
        let dst_msg_ptr =
            self.builder
                .struct_gep(error_struct_ty, ret_ptr, mem_message_field, "dst_msg_ptr");
        let str_llvm = self.type_resolver.resolve(Idx::STR);
        let str_struct_ty = self.builder.register_type(str_llvm);
        let mut idx = 0u32;
        while let Some(fty) = self.builder.struct_field_type(str_struct_ty, idx) {
            let src = self.builder.struct_gep(str_struct_ty, str_ptr, idx, "src");
            let val = self.builder.load(fty, src, "fld");
            let dst = self
                .builder
                .struct_gep(str_struct_ty, dst_msg_ptr, idx, "dst");
            self.builder.store(val, dst);
            idx += 1;
        }

        // Initialize field trace to an empty list `{ 0, 0, null }` via the
        // canonical `const_zero_ty` empty-value primitive, so this site and
        // `emit_str_into_error` share one empty-trace-init path.
        let dst_trace_ptr =
            self.builder
                .struct_gep(error_struct_ty, ret_ptr, mem_trace_field, "dst_trace_ptr");
        let list_struct_ty = str_struct_ty; // list shares the same structure layout as str
        let empty_trace = self.builder.const_zero_ty(list_struct_ty);
        self.builder.store(empty_trace, dst_trace_ptr);

        self.builder.ret_void();

        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }
    }
}
