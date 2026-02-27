//! List builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `count`, `is_empty`, `concat`, `add`, `push`,
//! `first`, `last`, `pop`, `contains`, `reverse`, and `iter` for the `list` type.

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `list.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_list_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "list.len")
    }

    /// Emit `list.is_empty()` — `len == 0`.
    pub(crate) fn emit_list_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "list.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
    }

    /// Extract list data pointer (field 2) and len (field 0) from receiver.
    fn extract_list_data_and_len(&mut self, receiver: ValueId) -> (ValueId, ValueId) {
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

    /// Emit `list.concat(other)` / `list.add(other)` — concatenate two lists.
    ///
    /// Calls `ori_list_concat(data1, len1, data2, len2, elem_size, out_ptr)`.
    /// Returns a new `{i64, i64, ptr}` list struct.
    pub(crate) fn emit_list_concat(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_concat");

        let (data1, len1) = self.extract_list_data_and_len(receiver);
        let (data2, len2) = self.extract_list_data_and_len(other);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "concat.out", list_ty);

        self.builder.call(
            func_id,
            &[data1, len1, data2, len2, elem_size_val, out_alloca],
            "concat",
        );

        Some(self.builder.load(list_ty, out_alloca, "concat.val"))
    }

    /// Emit `list.push(x)` — functional push returning a new list.
    ///
    /// Calls `ori_list_push_new(data, len, elem_ptr, elem_size, out_ptr)`.
    /// The result is a new `{i64, i64, ptr}` list struct.
    pub(crate) fn emit_list_push_new(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_push_new");

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "push.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_ptr, elem_size_val, out_alloca],
            "push",
        );

        Some(self.builder.load(list_ty, out_alloca, "push.val"))
    }

    /// Emit `list.first()` — returns `Option<T>` as `{i64 tag, T value}`.
    ///
    /// Calls `ori_list_first(data, len, elem_size, out_ptr)`.
    pub(crate) fn emit_list_first(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, "ori_list_first", "first")
    }

    /// Emit `list.last()` — returns `Option<T>` as `{i64 tag, T value}`.
    ///
    /// Calls `ori_list_last(data, len, elem_size, out_ptr)`.
    pub(crate) fn emit_list_last(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, "ori_list_last", "last")
    }

    /// Shared implementation for `first()` and `last()`.
    fn emit_list_first_or_last(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        func_name: &'static str,
        label: &str,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Option<T> layout: {i64 tag, T value}
        let elem_llvm_ty = self.resolve_type(elem_ty);
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

        Some(
            self.builder
                .load(option_ty, out_alloca, &format!("{label}.val")),
        )
    }

    /// Emit `list.contains(x)` — returns `bool`.
    ///
    /// Dispatches to type-specific runtime functions:
    /// - `[int]` → `ori_list_contains_int(data, len, needle)`
    /// - `[str]` → `ori_list_contains_str(data, len, needle_ptr)`
    pub(crate) fn emit_list_contains(
        &mut self,
        receiver: ValueId,
        needle: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let (data_ptr, len) = self.extract_list_data_and_len(receiver);

        let elem_info = self.type_info.get(elem_ty);
        let (func_name, args): (&'static str, Vec<ValueId>) = match &elem_info {
            TypeInfo::Int => ("ori_list_contains_int", vec![data_ptr, len, needle]),
            TypeInfo::Str => {
                let needle_ptr = self.str_to_ptr(needle, "contains.needle");
                ("ori_list_contains_str", vec![data_ptr, len, needle_ptr])
            }
            _ => return None, // Other element types not yet supported
        };

        let func_id = self.builder.runtime_fn(func_name);
        let result = self.builder.call(func_id, &args, "contains")?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains.bool"))
    }

    /// Emit `list.reverse()` — returns a new reversed list.
    ///
    /// Calls `ori_list_reverse(data, len, elem_size, out_ptr)`.
    pub(crate) fn emit_list_reverse(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_reverse");

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "reverse.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_size_val, out_alloca],
            "reverse",
        );

        Some(self.builder.load(list_ty, out_alloca, "reverse.val"))
    }

    /// Emit `list.iter()` — call `ori_iter_from_list(data_ptr, len, elem_size)`.
    ///
    /// List layout: `{i64 len, i64 cap, ptr data}`. The runtime expects the
    /// raw element data pointer (field 2), not a pointer to the list struct.
    pub(crate) fn emit_list_iter(
        &mut self,
        receiver: ValueId,
        _receiver_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_list");

        // Extract the raw data pointer (field 2) from {i64 len, i64 cap, ptr data}
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "list.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // List length (field 0)
        let len = self
            .builder
            .extract_value(receiver, 0, "list.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Element size
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        self.builder
            .call(func_id, &[data_ptr, len, elem_size_val], "list.iter")
    }
}
