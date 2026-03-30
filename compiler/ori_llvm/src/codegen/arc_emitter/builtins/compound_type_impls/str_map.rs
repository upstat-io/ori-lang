//! String and Map trait helper codegen.
//!
//! String helpers are exposed for use by compound trait dispatch.
//! Map equals delegates to the `ori_map_eq` runtime function with
//! thunk callbacks for key/value comparison.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Call `ori_str_compare(ptr, ptr) -> i8` for string comparison.
    pub(in super::super) fn emit_str_compare_call(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_compare");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], "str_cmp")
    }

    /// Call `ori_str_hash(ptr) -> i64` for string hashing.
    pub(in super::super) fn emit_str_hash_call(&mut self, val: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_hash");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_hash.arg", str_ty);
        self.builder.store(val, ptr);

        self.emit_rt_call(func_id, &[ptr], "str_hash")
    }

    /// `{K: V}.equals(other) -> bool`
    ///
    /// Calls `ori_map_eq(a, b, key_size, val_size, key_eq, key_hash, val_eq)`.
    /// Uses the existing thunk infrastructure for key/value comparison callbacks.
    pub(crate) fn emit_map_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_eq");

        // Store both maps to stack (ori_map_eq expects ptr to OriMap)
        let map_ty = self.resolve_type(ori_types::Idx::STR); // {i64, i64, ptr} — same layout
        let lhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "meq.lhs", map_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "meq.rhs", map_ty);
        self.builder.store(rhs, rhs_ptr);

        let key_size = self.element_store_size(key_ty);
        let val_size = self.element_store_size(val_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);

        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;
        let val_eq = self.get_or_create_eq_thunk(val_ty)?;

        self.emit_rt_call(
            func_id,
            &[
                lhs_ptr,
                rhs_ptr,
                key_size_val,
                val_size_val,
                key_eq,
                key_hash,
                val_eq,
            ],
            "map_eq",
        )
    }
}
