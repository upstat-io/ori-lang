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
    /// Representations). `Error` layout == `str` (24 bytes), so the body moves
    /// the str param into the `Error` sret slot per-field (FastISel-safe).
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
        // `Error` == `{ message: str }`; its 24 bytes ARE the `str` (message at
        // offset 0). GEP both the str param and the Error sret slot as the
        // `str` fat-pointer struct `{ i64 len, i64 cap, ptr data }`.
        let str_llvm = self.type_resolver.resolve(Idx::STR);
        let struct_ty = self.builder.register_type(str_llvm);
        // Copy each field of the resolved `str` fat-pointer struct (field types
        // derived from the LLVM type, not hardcoded) into the Error sret slot.
        let mut idx = 0u32;
        while let Some(fty) = self.builder.struct_field_type(struct_ty, idx) {
            let src = self.builder.struct_gep(struct_ty, str_ptr, idx, "src");
            let val = self.builder.load(fty, src, "fld");
            let dst = self.builder.struct_gep(struct_ty, ret_ptr, idx, "dst");
            self.builder.store(val, dst);
            idx += 1;
        }
        self.builder.ret_void();

        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }
    }
}
