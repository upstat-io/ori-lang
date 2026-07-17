//! List field/data extraction for list-builtin codegen.
//!
//! Provides shared extraction utilities (`extract_list_data_and_len`,
//! `extract_list_fields`), the `first`/`last` shared implementation, and
//! the `elem_size_and_align` helper used by COW mutation methods.

use ori_ir::OPTION_TAG_SOME;
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Extract list data pointer (field 2) and len (field 0) from receiver.
    ///
    /// Used by read-only methods that don't need capacity.
    pub(super) fn extract_list_data_and_len(
        &mut self,
        receiver: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        self.extract_collection_data_and_len(receiver, "list.data", "list.len")
    }

    /// Extract all three list fields: data (field 2), len (field 0), cap (field 1).
    ///
    /// Used by COW mutation methods that need capacity for the uniqueness fast path.
    pub(in crate::codegen::arc_emitter) fn extract_list_fields(
        &mut self,
        receiver: ValueId,
    ) -> Option<(ValueId, ValueId, ValueId)> {
        // FastISel-safe field extraction for the 24-byte fat pointer is handled
        // centrally in IrBuilder::extract_value (AB-5) under JIT.
        self.extract_collection_fields(receiver, "list.data", "list.len", "list.cap")
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

        let (data_ptr, len) = self.extract_list_data_and_len(receiver)?;
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
            "list.first_last.out",
            option_ty,
        );

        self.builder
            .call(func_id, &[data_ptr, len, elem_size_val, out_alloca], label);

        let opt_val = self
            .builder
            .load(option_ty, out_alloca, "list.first_last.val");

        // Sign-extend the narrowed value field back to canonical
        // i64 so downstream code (unwrap, comparison) sees the expected type.
        if self
            .narrowed_collection_element_width(collection_idx)
            .is_some()
        {
            let tag = self
                .builder
                .extract_value(opt_val, 0, "list.first_last.tag")?;
            let narrow_val = self
                .builder
                .extract_value(opt_val, 1, "list.first_last.narrow")?;
            let wide_val = self.sext_narrowed_collection_element(
                narrow_val,
                collection_idx,
                "list.first_last.sext",
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
                .insert_value(result, tag, 0, "list.first_last.opt.tag");
            result = self
                .builder
                .insert_value(result, wide_val, 1, "list.first_last.opt.val");
            Some(result)
        } else if crate::codegen::type_info::repr_box_oracle::payload_type_is_rc_boxed(
            self.pool, elem_ty,
        ) {
            // Recursive element: the runtime filled `opt_val` as an inline
            // `{ i64, %T }` Option, but the canonical `Option<T>` layout boxes
            // the Some payload to `{ i64, ptr }`. On the Some path, RC-alloc a
            // box, copy the inline element in, and build the boxed Option; on
            // the None path, build `{ tag, null }`.
            self.rebuild_recursive_first_option(opt_val, elem_ty)
        } else {
            // RC-retain the element payload when Some — the runtime memcpy
            // duplicates payload bytes without incrementing RC on inner fields.
            let tag = self
                .builder
                .extract_value(opt_val, 0, "list.first_last.tag")?;
            let some = self.builder.const_int_matching(tag, OPTION_TAG_SOME as u64);
            let is_some = self.builder.icmp_eq(tag, some, "list.first_last.is_some");
            let payload = self
                .builder
                .extract_value(opt_val, 1, "list.first_last.payload")?;

            let inc_bb = self
                .builder
                .append_block(self.current_function, "list.first_last.rc_inc");
            let merge_bb = self
                .builder
                .append_block(self.current_function, "list.first_last.rc_merge");
            self.builder.cond_br(is_some, inc_bb, merge_bb);
            self.builder.position_at_end(inc_bb);
            self.inc_value_rc(payload, elem_ty, 1);
            self.builder.br(merge_bb);
            self.builder.position_at_end(merge_bb);

            Some(opt_val)
        }
    }

    /// Convert an inline `{ i64 tag, %T value }` Option (as the runtime
    /// `ori_list_first`/`last` fills it) into the canonical boxed
    /// `{ i64 tag, ptr }` `Option<T>` for a recursive element type `T`. On the
    /// Some path: RC-alloc a box sized to `T`, copy the inline element in, and
    /// store the box pointer. On the None path: `{ tag, null }`.
    fn rebuild_recursive_first_option(
        &mut self,
        inline_opt: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self
            .builder
            .extract_value(inline_opt, 0, "list.first_last.tag")?;
        let inline_payload = self
            .builder
            .extract_value(inline_opt, 1, "list.first_last.inline")?;
        let some = self.builder.const_int_matching(tag, OPTION_TAG_SOME as u64);
        let is_some = self.builder.icmp_eq(tag, some, "list.first_last.is_some");

        // Boxed-layout Option<T> = { i64, ptr } (recursive Some payload boxed).
        let boxed_opt_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        );
        let null_ptr = self.builder.const_null_ptr();
        let none_opt = {
            let mut v = self.builder.const_zero_ty(boxed_opt_ty);
            v = self
                .builder
                .insert_value(v, tag, 0, "list.first_last.none.tag");
            self.builder
                .insert_value(v, null_ptr, 1, "list.first_last.none.ptr")
        };

        let some_bb = self
            .builder
            .append_block(self.current_function, "list.first_last.box.some");
        let join_bb = self
            .builder
            .append_block(self.current_function, "list.first_last.box.join");
        let entry_bb = self.builder.current_block();
        self.builder.cond_br(is_some, some_bb, join_bb);

        // Some path: box the inline element (retaining its heap sub-pointers,
        // since the list still owns its copy), store the box pointer.
        self.builder.position_at_end(some_bb);
        let size = self.element_store_size(elem_ty);
        let box_ptr = self.rc_alloc(size, 8);
        self.builder.store(inline_payload, box_ptr);
        self.inc_value_rc(inline_payload, elem_ty, 1);
        let mut some_opt = self.builder.const_zero_ty(boxed_opt_ty);
        some_opt = self
            .builder
            .insert_value(some_opt, tag, 0, "list.first_last.some.tag");
        some_opt = self
            .builder
            .insert_value(some_opt, box_ptr, 1, "list.first_last.some.ptr");
        let some_end_bb = self.builder.current_block();
        self.builder.br(join_bb);

        self.builder.position_at_end(join_bb);
        let mut incoming = Vec::new();
        if let Some(e) = entry_bb {
            incoming.push((none_opt, e));
        }
        if let Some(s) = some_end_bb {
            incoming.push((some_opt, s));
        }
        self.builder
            .phi_from_incoming(boxed_opt_ty, &incoming, "list.first_last.opt")
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
