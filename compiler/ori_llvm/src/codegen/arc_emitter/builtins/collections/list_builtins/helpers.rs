//! Private helper methods for list builtins.
//!
//! Provides shared extraction utilities (`extract_list_data_and_len`,
//! `extract_list_fields`), the `first`/`last` shared implementation, and
//! the `elem_size_and_align` helper used by COW mutation methods.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Extract list data pointer (field 2) and len (field 0) from receiver.
    ///
    /// Used by read-only methods that don't need capacity.
    pub(super) fn extract_list_data_and_len(&mut self, receiver: ValueId) -> (ValueId, ValueId) {
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "list.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "list.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        (data_ptr, len)
    }

    /// Extract all three list fields: data (field 2), len (field 0), cap (field 1).
    ///
    /// Used by COW mutation methods that need capacity for the uniqueness fast path.
    pub(in crate::codegen::arc_emitter::builtins::collections) fn extract_list_fields(
        &mut self,
        receiver: ValueId,
    ) -> (ValueId, ValueId, ValueId) {
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "list.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "list.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let cap = self
            .builder
            .extract_value(receiver, 1, "list.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        (data_ptr, len, cap)
    }

    /// Shared implementation for `first()` and `last()`.
    pub(super) fn emit_list_first_or_last(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        list_ty: Idx,
        func_name: &'static str,
        label: &str,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        // Use narrowed element size/type if available.
        let collection_idx = self.pool.resolve_fully(list_ty);
        let elem_size_val = self
            .builder
            .const_i64(self.collection_elem_size(collection_idx, elem_ty) as i64);

        // Option<T> layout: {i64 tag, T value}
        // For narrowed collections, use the narrowed element LLVM type so the
        // runtime memcpy of `elem_size` bytes fills the struct correctly.
        let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_ty);
        let raw_elem_ty = self.builder.raw_type(elem_llvm_ty);
        let option_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(&[self.builder.scx().type_i64().into(), raw_elem_ty], false)
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            &format!("{label}.out"),
            option_ty,
        );

        self.builder
            .call(func_id, &[data_ptr, len, elem_size_val, out_alloca], label);

        let opt_val = self
            .builder
            .load(option_ty, out_alloca, &format!("{label}.val"));

        // Sign-extend the narrowed value field back to canonical
        // i64 so downstream code (unwrap, comparison) sees the expected type.
        if self
            .narrowed_collection_element_width(collection_idx)
            .is_some()
        {
            let tag = self
                .builder
                .extract_value(opt_val, 0, &format!("{label}.tag"))?;
            let narrow_val = self
                .builder
                .extract_value(opt_val, 1, &format!("{label}.narrow"))?;
            let wide_val = self.sext_narrowed_collection_element(
                narrow_val,
                collection_idx,
                &format!("{label}.sext"),
            );
            // Rebuild Option with canonical element type: {i64 tag, i64 value}
            let canonical_elem_ty = self.resolve_type(elem_ty);
            let raw_canonical = self.builder.raw_type(canonical_elem_ty);
            let canonical_opt_ty = self.builder.register_type(
                self.builder
                    .scx()
                    .type_struct(
                        &[self.builder.scx().type_i64().into(), raw_canonical],
                        false,
                    )
                    .into(),
            );
            let mut result = self.builder.const_zero_ty(canonical_opt_ty);
            result = self
                .builder
                .insert_value(result, tag, 0, &format!("{label}.opt.tag"));
            result = self
                .builder
                .insert_value(result, wide_val, 1, &format!("{label}.opt.val"));
            Some(result)
        } else {
            Some(opt_val)
        }
    }

    /// Build `(elem_size, elem_align)` constant pair for COW runtime calls.
    ///
    /// When `collection_ty` is `Some`, uses `collection_elem_size` for
    /// Phase C narrowed element sizes. Otherwise falls back to canonical size.
    pub(in crate::codegen::arc_emitter::builtins::collections) fn elem_size_and_align(
        &mut self,
        elem_ty: Idx,
        collection_ty: Option<Idx>,
    ) -> (ValueId, ValueId) {
        let size_bytes = if let Some(coll_ty) = collection_ty {
            let collection_idx = self.pool.resolve_fully(coll_ty);
            self.collection_elem_size(collection_idx, elem_ty)
        } else {
            self.element_store_size(elem_ty)
        };
        let size = self.builder.const_i64(size_bytes as i64);
        let align = self
            .builder
            .const_i64(self.element_store_align(elem_ty) as i64);
        (size, align)
    }
}
