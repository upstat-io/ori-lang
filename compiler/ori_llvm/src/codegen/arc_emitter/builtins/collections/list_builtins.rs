//! List builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `count`, `is_empty`, `concat`, `add`, `push`,
//! `first`, `last`, `pop`, `contains`, `reverse`, `set`, `insert`, `remove`,
//! and `iter` for the `list` type.
//!
//! Mutation methods (push, pop, concat, reverse, set, insert, remove) use
//! COW (Copy-on-Write) runtime functions: when the list is uniquely owned
//! (RC == 1), mutation happens in-place; when shared, a copy is made first.

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

// ---------------------------------------------------------------------------
// Read-only accessors
// ---------------------------------------------------------------------------

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

    /// Emit `list.first()` — returns `Option<T>` as `{i64 tag, T value}`.
    pub(crate) fn emit_list_first(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, "ori_list_first", "first")
    }

    /// Emit `list.last()` — returns `Option<T>` as `{i64 tag, T value}`.
    pub(crate) fn emit_list_last(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        self.emit_list_first_or_last(receiver, elem_ty, "ori_list_last", "last")
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

    /// Emit `list.iter()` — call `ori_iter_from_list(data_ptr, len, elem_size)`.
    pub(crate) fn emit_list_iter(
        &mut self,
        receiver: ValueId,
        _receiver_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_list");

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        self.builder
            .call(func_id, &[data_ptr, len, elem_size_val], "list.iter")
    }
}

// ---------------------------------------------------------------------------
// COW mutation methods
// ---------------------------------------------------------------------------

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `list.push(x)` — COW push returning the (possibly mutated) list.
    ///
    /// Fast path (unique + capacity): appends in place, O(1).
    /// Slow path (shared): copies to new buffer, O(n).
    pub(crate) fn emit_list_push_cow(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_push_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "push.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                out,
            ],
            "push",
        );

        Some(self.builder.load(list_ty, out, "push.val"))
    }

    /// Emit `list.pop()` — COW pop returning the list with last element removed.
    ///
    /// Fast path (unique): decrements len in place, O(1).
    /// Slow path (shared): copies to new buffer with len-1 elements.
    #[expect(
        dead_code,
        reason = "pop() returns Option<T> in typeck; COW pop needs ARC pipeline dual-return"
    )]
    pub(crate) fn emit_list_pop_cow(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_pop_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "pop.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, cap, elem_size_val, elem_align_val, out],
            "pop",
        );

        Some(self.builder.load(list_ty, out, "pop.val"))
    }

    /// Emit `list.set(index, value)` — COW index assignment returning modified list.
    ///
    /// Fast path (unique): overwrites element at index in place.
    /// Slow path (shared): copies buffer, overwrites target index.
    #[expect(
        dead_code,
        reason = "ready for type checker support — pending TYPECK_BUILTIN_METHODS entry"
    )]
    pub(crate) fn emit_list_set_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_set_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "set.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "set.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                out,
            ],
            "set",
        );

        Some(self.builder.load(list_ty, out, "set.val"))
    }

    /// Emit `list.insert(index, value)` — COW insert returning modified list.
    ///
    /// Fast path (unique + capacity): memmove + write in place.
    /// Slow path (shared): new allocation with element inserted.
    #[expect(
        dead_code,
        reason = "ready for type checker support — pending TYPECK_BUILTIN_METHODS entry"
    )]
    pub(crate) fn emit_list_insert_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_insert_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "insert.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                out,
            ],
            "insert",
        );

        Some(self.builder.load(list_ty, out, "insert.val"))
    }

    /// Emit `list.remove(index)` — COW remove returning modified list.
    ///
    /// Fast path (unique): memmove shift left in place.
    /// Slow path (shared): new allocation without the removed element.
    #[expect(
        dead_code,
        reason = "ready for type checker support — pending TYPECK_BUILTIN_METHODS entry"
    )]
    pub(crate) fn emit_list_remove_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_remove_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "remove.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr,
                len,
                cap,
                index,
                elem_size_val,
                elem_align_val,
                out,
            ],
            "remove",
        );

        Some(self.builder.load(list_ty, out, "remove.val"))
    }

    /// Emit `list.concat(other)` / `list.add(other)` — COW concatenation.
    ///
    /// Fast path (list1 unique + capacity): copies list2 elements after list1.
    /// Slow path (shared): new allocation with both lists.
    pub(crate) fn emit_list_concat_cow(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_concat_cow");

        let (data1, len1, cap1) = self.extract_list_fields(receiver);
        let (data2, len2) = self.extract_list_data_and_len(other);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "concat.out", list_ty);

        self.builder.call(
            func_id,
            &[
                data1,
                len1,
                cap1,
                data2,
                len2,
                elem_size_val,
                elem_align_val,
                out,
            ],
            "concat",
        );

        Some(self.builder.load(list_ty, out, "concat.val"))
    }

    /// Emit `list.reverse()` — COW reverse returning the reversed list.
    ///
    /// Fast path (unique): swaps pairs from both ends inward, O(n), no allocation.
    /// Slow path (shared): new allocation with elements in reverse order.
    pub(crate) fn emit_list_reverse_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_reverse_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "reverse.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, cap, elem_size_val, elem_align_val, out],
            "reverse",
        );

        Some(self.builder.load(list_ty, out, "reverse.val"))
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Extract list data pointer (field 2) and len (field 0) from receiver.
    ///
    /// Used by read-only methods that don't need capacity.
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

    /// Extract all three list fields: data (field 2), len (field 0), cap (field 1).
    ///
    /// Used by COW mutation methods that need capacity for the uniqueness fast path.
    fn extract_list_fields(&mut self, receiver: ValueId) -> (ValueId, ValueId, ValueId) {
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
    fn emit_list_first_or_last(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        func_name: &'static str,
        label: &str,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

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

    /// Build `(elem_size, elem_align)` constant pair for COW runtime calls.
    fn elem_size_and_align(&mut self, elem_ty: Idx) -> (ValueId, ValueId) {
        let size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let align = self
            .builder
            .const_i64(self.element_store_align(elem_ty) as i64);
        (size, align)
    }
}
