//! List builtin method codegen for LLVM.
//!
//! Handles read-only accessors (`length`, `len`, `count`, `is_empty`,
//! `first`, `last`, `contains`, `iter`) and helpers for the `list` type.
//!
//! COW mutation methods (push, pop, concat, reverse, set, insert, remove,
//! sort) are in the sibling `list_cow` module.

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

// Read-only accessors

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
        let result = self.emit_rt_call(func_id, &args, "contains")?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains.bool"))
    }

    /// Emit `list[index]` — bounds-checked element access, returns `T` directly.
    ///
    /// Calls `ori_list_get(data, len, index, elem_size, out_ptr)`.
    /// Panics on out-of-bounds (no Option wrapper — direct element return).
    pub(crate) fn emit_list_index(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_get");

        let (data_ptr, len) = self.extract_list_data_and_len(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        // Alloca for the element (T, not Option<T>)
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "index.out", elem_llvm_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, index, elem_size_val, out_alloca],
            "index",
        );

        Some(self.builder.load(elem_llvm_ty, out_alloca, "index.val"))
    }

    /// Emit `list.iter()` — call `ori_iter_from_list(data, len, cap, elem_size, elem_dec_fn)`.
    ///
    /// The iterator takes ownership of one RC reference to the list data buffer.
    /// This function explicitly emits `ori_list_rc_inc` to give the iterator its
    /// own reference — the ARC pipeline may not always do so (e.g., when the list
    /// is a borrowed function parameter). When the iterator is consumed/dropped,
    /// `Drop for IterState` calls `ori_buffer_rc_dec` to release this reference.
    ///
    /// Element cleanup contract: the iterator does NOT manage element-level RC.
    /// Elements are owned by the collection — the list's drop function handles
    /// all element cleanup when the buffer's refcount reaches zero. Passing null
    /// for `elem_dec_fn`: the iterator borrows elements, it does not own them.
    /// Element cleanup is handled by the list's explicit `RcDec` (from the ARC
    /// pipeline's AIMS emission) which carries the real `elem_dec_fn`. The
    /// phantom `__for_coll` binding in `lower_for` ensures the explicit `RcDec`
    /// happens AFTER `ori_iter_drop`, so the explicit dec is the one that
    /// reaches zero and performs element cleanup.
    pub(crate) fn emit_list_iter(
        &mut self,
        receiver: ValueId,
        _receiver_ty: Idx,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_list");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        // Pass null for elem_dec_fn: the iterator does not own elements.
        // The list's explicit RcDec (placed after ori_iter_drop by the
        // __for_coll phantom binding) handles element cleanup when RC → 0.
        let _ = elem_ty; // elem_ty used only for elem_size above
        let elem_dec_fn_null = self.builder.const_null_ptr();

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, elem_size_val, elem_dec_fn_null],
            "list.iter",
        )
    }
    /// Emit `list.slice(start, end)` — zero-copy seamless slice.
    ///
    /// Creates a view into the original buffer. No elements are copied.
    /// The original buffer's RC is incremented (the slice references it).
    pub(crate) fn emit_list_slice(
        &mut self,
        receiver: ValueId,
        start: ValueId,
        end: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_slice");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "slice.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, start, end, elem_size_val, out],
            "slice",
        );

        Some(self.builder.load(list_ty, out, "slice.val"))
    }

    /// Emit `list.take(n)` — zero-copy first-N elements slice.
    ///
    /// Equivalent to `list.slice(0, n)`. No elements are copied.
    pub(crate) fn emit_list_take_slice(
        &mut self,
        receiver: ValueId,
        n: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_slice_take");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "take.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, n, elem_size_val, out],
            "take",
        );

        Some(self.builder.load(list_ty, out, "take.val"))
    }

    /// Emit `list.drop(n)` — zero-copy skip-first-N elements slice.
    ///
    /// Equivalent to `list.slice(n, len)`. No elements are copied.
    pub(crate) fn emit_list_drop_slice(
        &mut self,
        receiver: ValueId,
        n: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_slice_drop");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver);
        let elem_size_val = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "drop.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[data_ptr, len, cap, n, elem_size_val, out],
            "drop",
        );

        Some(self.builder.load(list_ty, out, "drop.val"))
    }
}

// Private helpers

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
    pub(in crate::codegen::arc_emitter::builtins::collections) fn elem_size_and_align(
        &mut self,
        elem_ty: Idx,
    ) -> (ValueId, ValueId) {
        let size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let align = self
            .builder
            .const_i64(self.element_store_align(elem_ty) as i64);
        (size, align)
    }

    /// Get or generate an LLVM comparison thunk for sorting elements of `elem_ty`.
    ///
    /// The thunk has signature `fn(*const u8, *const u8) -> i32` and returns
    /// -1 (less), 0 (equal), or 1 (greater). Each element type gets a
    /// specialized function that loads elements by their native LLVM type
    /// before comparing.
    ///
    /// Returns a function pointer `ValueId`, or `None` if the element type
    /// is not yet supported for sorting.
    pub(in crate::codegen::arc_emitter::builtins::collections) fn get_or_create_compare_thunk(
        &mut self,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        // Check cache first
        if let Some(&func_id) = self.compare_thunk_cache.get(&elem_ty) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let elem_info = self.type_info.get(elem_ty);
        let type_suffix = match &elem_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
            TypeInfo::Float => "float",
            TypeInfo::Bool => "bool",
            TypeInfo::Char => "char",
            TypeInfo::Byte => "byte",
            TypeInfo::Str => "str",
            _ => return None,
        };

        let func_name = format!("_ori_cmp_{type_suffix}");

        // Check if already declared in the module (shared across emitters)
        let ptr_ty = self.builder.ptr_type();
        let i32_ty = self.builder.i32_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], i32_ty);

        // If the function already has a body, just cache and return
        if self.builder.function_has_body(func_id) {
            self.compare_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Set up the function
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let a_ptr = self.builder.get_param(func_id, 0);
        let b_ptr = self.builder.get_param(func_id, 1);

        // Generate the comparison body based on element type
        let result = match &elem_info {
            TypeInfo::Str => self.gen_str_compare(a_ptr, b_ptr),
            _ => self.gen_primitive_compare(a_ptr, b_ptr, elem_ty, &elem_info),
        };

        self.builder.ret(result);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.compare_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
    }

    /// Generate comparison body for primitive types (int, float, bool, char, byte).
    ///
    /// Loads values from pointers, compares, and returns i32 (-1/0/1).
    fn gen_primitive_compare(
        &mut self,
        a_ptr: ValueId,
        b_ptr: ValueId,
        elem_ty: Idx,
        elem_info: &TypeInfo,
    ) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let a_val = self.builder.load(llvm_ty, a_ptr, "a");
        let b_val = self.builder.load(llvm_ty, b_ptr, "b");

        let neg1 = self.builder.const_i32(-1);
        let zero = self.builder.const_i32(0);
        let pos1 = self.builder.const_i32(1);

        match elem_info {
            // Signed integer comparison (int, Duration, Size)
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                let lt = self.builder.icmp_slt(a_val, b_val, "lt");
                let gt = self.builder.icmp_sgt(a_val, b_val, "gt");
                let gt_or_eq = self.builder.select(gt, pos1, zero, "gt_or_eq");
                self.builder.select(lt, neg1, gt_or_eq, "cmp")
            }
            // Float comparison (ordered)
            TypeInfo::Float => {
                let lt = self.builder.fcmp_olt(a_val, b_val, "lt");
                let gt = self.builder.fcmp_ogt(a_val, b_val, "gt");
                let gt_or_eq = self.builder.select(gt, pos1, zero, "gt_or_eq");
                self.builder.select(lt, neg1, gt_or_eq, "cmp")
            }
            // Unsigned comparison (bool, char, byte) — zext to i32 first
            TypeInfo::Bool | TypeInfo::Char | TypeInfo::Byte => {
                let i32_ty = self.builder.i32_type();
                let a_ext = self.builder.zext(a_val, i32_ty, "a.ext");
                let b_ext = self.builder.zext(b_val, i32_ty, "b.ext");
                let lt = self.builder.icmp_ult(a_ext, b_ext, "lt");
                let gt = self.builder.icmp_ugt(a_ext, b_ext, "gt");
                let gt_or_eq = self.builder.select(gt, pos1, zero, "gt_or_eq");
                self.builder.select(lt, neg1, gt_or_eq, "cmp")
            }
            _ => unreachable!("non-primitive passed to gen_primitive_compare"),
        }
    }

    /// Generate comparison body for strings.
    ///
    /// Calls `ori_str_compare(a, b) -> i8` (returns Ori Ordering: 0=Less,
    /// 1=Equal, 2=Greater) and converts to i32 (-1/0/1) via `result - 1`.
    fn gen_str_compare(&mut self, a_ptr: ValueId, b_ptr: ValueId) -> ValueId {
        let cmp_fn = self.builder.runtime_fn("ori_str_compare");
        let ord = self
            .builder
            .call(cmp_fn, &[a_ptr, b_ptr], "ord")
            .unwrap_or_else(|| self.builder.const_i8(1)); // Equal fallback

        // Convert Ordering (0,1,2) to sort convention (-1,0,1): subtract 1
        let one = self.builder.const_i8(1);
        let shifted = self.builder.sub(ord, one, "shifted");

        // Sign-extend i8 → i32
        let i32_ty = self.builder.i32_type();
        self.builder.sext(shifted, i32_ty, "cmp")
    }
}
