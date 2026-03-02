//! Map and Set builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains_key`, `keys`, `values`,
//! `get`, `insert`, `remove`, and `iter` for maps, and `length`, `len`,
//! `is_empty`, `contains`, `insert`, `remove`, `union`, `intersection`,
//! `difference`, `to_list`, `into`, and `iter` for sets.
//!
//! Map and set mutations use COW semantics: when the collection is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each mutating method returns a `{i64 len, i64 cap, ptr data}` struct.
//!
//! Also includes `range.iter()` since Range is a simple collection type.

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Map methods
    // -----------------------------------------------------------------------

    /// Emit `map.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_map_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "map.len")
    }

    /// Emit `map.is_empty()` — `len == 0`.
    pub(crate) fn emit_map_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "map.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "map.is_empty"))
    }

    /// Emit `map.contains_key(key)` — hash table lookup with type-specific equality.
    ///
    /// Calls `ori_map_contains_key(data, cap, len, needle, key_size, key_eq, key_hash)`.
    /// Map layout: `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_map_contains_key(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        key_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_contains_key");

        let data = self
            .builder
            .extract_value(receiver, 2, "map.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let cap = self
            .builder
            .extract_value(receiver, 1, "map.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let needle_ptr = self.elem_to_ptr(key, key_ty, "contains_key.needle");
        let key_size = self.element_store_size(key_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;

        let result = self.builder.call(
            func_id,
            &[data, cap, len, needle_ptr, key_size_val, key_eq, key_hash],
            "contains_key",
        )?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains_key.bool"))
    }

    /// Emit `map.keys()` — extract keys as a new list.
    ///
    /// Calls `ori_map_keys_to_list(data, cap, len, key_size, out_ptr)`.
    /// Keys start at `data + 0` so data is directly the keys array.
    pub(crate) fn emit_map_keys(&mut self, receiver: ValueId, key_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_keys_to_list");

        let data = self
            .builder
            .extract_value(receiver, 2, "map.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let cap = self
            .builder
            .extract_value(receiver, 1, "map.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let key_size = self.element_store_size(key_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "keys.out", list_ty);

        self.builder
            .call(func_id, &[data, cap, len, key_size_val, out_alloca], "keys");

        Some(self.builder.load(list_ty, out_alloca, "keys.val"))
    }

    /// Emit `map.values()` — extract values as a new list.
    ///
    /// Calls `ori_map_values_to_list(data, cap, len, key_size, val_size, out_ptr)`.
    /// Values start at `data + cap * key_size`.
    pub(crate) fn emit_map_values(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_values_to_list");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "values.out", list_ty);

        self.builder.call(
            func_id,
            &[data, cap, len, key_size_val, val_size_val, out_alloca],
            "values",
        );

        Some(self.builder.load(list_ty, out_alloca, "values.val"))
    }

    /// Emit `map.get(key)` — returns `Option<V>` via sret.
    ///
    /// Calls `ori_map_get(data, cap, len, needle, key_size, val_size, key_eq, key_hash, out_ptr)`.
    /// Returns `{i64 tag, V value}` — tag 0=Some, 1=None.
    pub(crate) fn emit_map_get(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_get");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let needle_ptr = self.elem_to_ptr(key, key_ty, "get.needle");
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;

        // Option<V> layout: {i64 tag, V value}
        let val_llvm_ty = self.resolve_type(val_ty);
        let raw_val_ty = self.builder.raw_type(val_llvm_ty);
        let option_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(&[self.builder.scx().type_i64().into(), raw_val_ty], false)
                .into(),
        );
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "get.out", option_ty);

        self.builder.call(
            func_id,
            &[
                data,
                cap,
                len,
                needle_ptr,
                key_size_val,
                val_size_val,
                key_eq,
                key_hash,
                out_alloca,
            ],
            "get",
        );

        Some(self.builder.load(option_ty, out_alloca, "get.val"))
    }

    /// Extract map data, cap, len and compute key/val sizes from type info.
    fn extract_map_components(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> (ValueId, ValueId, ValueId, ValueId, ValueId) {
        let data = self
            .builder
            .extract_value(receiver, 2, "map.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let cap = self
            .builder
            .extract_value(receiver, 1, "map.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let key_size_val = self
            .builder
            .const_i64(self.element_store_size(key_ty) as i64);
        let val_size_val = self
            .builder
            .const_i64(self.element_store_size(val_ty) as i64);
        (data, cap, len, key_size_val, val_size_val)
    }

    /// Build the LLVM struct type `{i64, i64, ptr}` for map sret returns.
    fn map_struct_type(&mut self) -> LLVMTypeId {
        // Same as list/set — {i64 len, i64 cap, ptr data}
        self.list_struct_type()
    }

    /// Emit `map.insert(key, value)` — COW insert returning the (possibly mutated) map.
    ///
    /// Fast path (unique + key exists): overwrites value in place.
    /// Fast path (unique + new key + capacity): appends in place.
    /// Slow path (shared): copies to new buffer.
    ///
    /// Calls `ori_map_insert_cow(data, len, cap, key, value, key_size, val_size,
    ///         key_eq, key_hash, key_inc, val_inc, cow_mode, out_ptr)`.
    pub(crate) fn emit_map_insert(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        value: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_insert_cow");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let key_ptr = self.elem_to_ptr(key, key_ty, "insert.key");
        let val_ptr = self.elem_to_ptr(value, val_ty, "insert.val");
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;
        let key_inc = self.get_or_generate_elem_inc_fn(key_ty);
        let val_inc = self.get_or_generate_elem_inc_fn(val_ty);

        let map_ty = self.map_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "insert.out", map_ty);

        self.builder.call(
            func_id,
            &[
                data,
                len,
                cap,
                key_ptr,
                val_ptr,
                key_size_val,
                val_size_val,
                key_eq,
                key_hash,
                key_inc,
                val_inc,
                cow_mode,
                out,
            ],
            "insert",
        );

        Some(self.builder.load(map_ty, out, "insert.val"))
    }

    /// Emit `map.remove(key)` — COW remove returning the (possibly mutated) map.
    ///
    /// Fast path (unique + key found): shifts entries left in place.
    /// Slow path (shared): copies all entries except the removed one.
    ///
    /// Calls `ori_map_remove_cow(data, len, cap, key, key_size, val_size,
    ///         key_eq, key_hash, key_inc, val_inc, cow_mode, out_ptr)`.
    pub(crate) fn emit_map_remove(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_remove_cow");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let key_ptr = self.elem_to_ptr(key, key_ty, "remove.key");
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;
        let key_inc = self.get_or_generate_elem_inc_fn(key_ty);
        let val_inc = self.get_or_generate_elem_inc_fn(val_ty);

        let map_ty = self.map_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "remove.out", map_ty);

        self.builder.call(
            func_id,
            &[
                data,
                len,
                cap,
                key_ptr,
                key_size_val,
                val_size_val,
                key_eq,
                key_hash,
                key_inc,
                val_inc,
                cow_mode,
                out,
            ],
            "remove",
        );

        Some(self.builder.load(map_ty, out, "remove.val"))
    }

    /// Emit `map.iter()` — call `ori_iter_from_map(data, cap, len, ks, vs, owns, k_dec, v_dec)`.
    ///
    /// Map layout: `{i64 len, i64 cap, ptr data}`.
    /// Yields `(K, V)` tuples as concatenated key+value bytes.
    /// The iterator takes ownership of one RC reference to the data buffer,
    /// releasing it via `ori_map_buffer_rc_dec` when dropped.
    pub(crate) fn emit_map_iter(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_map");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let owns_data = self.builder.const_bool(true);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_ty);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_ty);

        self.builder.call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                val_size_val,
                owns_data,
                key_dec_fn,
                val_dec_fn,
            ],
            "map.iter",
        )
    }

    // -----------------------------------------------------------------------
    // Set methods
    // -----------------------------------------------------------------------

    /// Emit `set.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_set_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "set.len")
    }

    /// Emit `set.is_empty()` — `len == 0`.
    pub(crate) fn emit_set_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "set.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "set.is_empty"))
    }

    /// Extract set data, len, cap from `{i64 len, i64 cap, ptr data}`.
    fn extract_set_components(&mut self, receiver: ValueId) -> (ValueId, ValueId, ValueId) {
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "set.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "set.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let cap = self
            .builder
            .extract_value(receiver, 1, "set.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        (data_ptr, len, cap)
    }

    /// Emit `set.contains(elem)` — hash table lookup with type-specific equality.
    ///
    /// Calls `ori_set_contains(data, cap, len, elem_ptr, elem_size, elem_eq, elem_hash)`.
    pub(crate) fn emit_set_contains(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_contains");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "contains.elem");
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;

        let result = self.builder.call(
            func_id,
            &[data_ptr, cap, len, elem_ptr, elem_size, elem_eq, elem_hash],
            "set.contains",
        )?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "set.contains.bool"))
    }

    /// Emit `set.insert(elem)` — COW insert returning the (possibly mutated) set.
    ///
    /// No-op if element exists. Fast path (unique): appends in place.
    /// Slow path (shared): copies to new buffer.
    ///
    /// Calls `ori_set_insert_cow(data, len, cap, elem, elem_size, elem_align,
    ///         elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)`.
    pub(crate) fn emit_set_insert(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_insert_cow");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let (elem_size, elem_align) = self.elem_size_and_align(elem_ty);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let set_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "set.insert.out", set_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr, len, cap, elem_ptr, elem_size, elem_align, elem_eq, elem_hash, inc_fn,
                cow_mode, out,
            ],
            "set.insert",
        );

        Some(self.builder.load(set_ty, out, "set.insert.val"))
    }

    /// Emit `set.remove(elem)` — COW remove returning the (possibly mutated) set.
    ///
    /// No-op if element not found. Fast path (unique): shifts left in place.
    /// Slow path (shared): copies all except removed.
    ///
    /// Calls `ori_set_remove_cow(data, len, cap, elem, elem_size, elem_align,
    ///         elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)`.
    pub(crate) fn emit_set_remove(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_remove_cow");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "remove.elem");
        let (elem_size, elem_align) = self.elem_size_and_align(elem_ty);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let set_ty = self.list_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "set.remove.out", set_ty);

        self.builder.call(
            func_id,
            &[
                data_ptr, len, cap, elem_ptr, elem_size, elem_align, elem_eq, elem_hash, inc_fn,
                cow_mode, out,
            ],
            "set.remove",
        );

        Some(self.builder.load(set_ty, out, "set.remove.val"))
    }

    /// Emit a two-set COW operation (union/intersection/difference) via sret.
    ///
    /// The receiver (set1) is consumed; the other (set2) is borrowed.
    ///
    /// Calls `ori_set_{op}_cow(d1, l1, c1, d2, l2, c2, elem_size, elem_align,
    ///         elem_eq, elem_hash, inc_fn, cow_mode, out_ptr)`.
    fn emit_set_binary_op(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        func_name: &'static str,
        label: &str,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);

        let (d1, l1, c1) = self.extract_set_components(receiver);
        // Second set: need data, len, and cap for hash table lookups
        let d2 = self
            .builder
            .extract_value(other, 2, "set2.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let l2 = self
            .builder
            .extract_value(other, 0, "set2.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let c2 = self
            .builder
            .extract_value(other, 1, "set2.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let (elem_size, elem_align) = self.elem_size_and_align(elem_ty);
        let elem_eq = self.get_or_create_eq_thunk(elem_ty)?;
        let elem_hash = self.get_or_create_hash_thunk(elem_ty)?;
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let set_ty = self.list_struct_type();
        let out = self.builder.create_entry_alloca(
            self.current_function,
            &format!("set.{label}.out"),
            set_ty,
        );

        self.builder.call(
            func_id,
            &[
                d1, l1, c1, d2, l2, c2, elem_size, elem_align, elem_eq, elem_hash, inc_fn,
                cow_mode, out,
            ],
            &format!("set.{label}"),
        );

        Some(self.builder.load(set_ty, out, &format!("set.{label}.val")))
    }

    /// Emit `set.union(other)` — COW union.
    pub(crate) fn emit_set_union(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_union_cow",
            "union",
            cow_mode,
        )
    }

    /// Emit `set.intersection(other)` — COW intersection.
    pub(crate) fn emit_set_intersection(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_intersection_cow",
            "intersection",
            cow_mode,
        )
    }

    /// Emit `set.difference(other)` — COW difference.
    pub(crate) fn emit_set_difference(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_difference_cow",
            "difference",
            cow_mode,
        )
    }

    /// Emit `set.to_list()` / `set.into()` — copies set data into a new list via sret.
    ///
    /// Calls `ori_set_to_list(data, cap, len, elem_size, out_ptr)`.
    pub(crate) fn emit_set_to_list(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_to_list");

        let (data_ptr, len, cap) = self.extract_set_components(receiver);
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "set.to_list.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, cap, len, elem_size, out_alloca],
            "set.to_list",
        );

        Some(self.builder.load(list_ty, out_alloca, "set.to_list.val"))
    }

    // -----------------------------------------------------------------------
    // Range methods
    // -----------------------------------------------------------------------

    /// Emit `range.iter()` — call `ori_iter_from_range(start, end, step, inclusive)`.
    ///
    /// Range is lowered as a 4-element Tuple `{i64 start, i64 end, i64 step,
    /// i64 inclusive}` by `lower_range`. The inclusive flag (field 3) is
    /// stored as i64 (0 or 1) and truncated to i1 for the runtime call.
    pub(crate) fn emit_range_iter(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_range");

        let start = self
            .builder
            .extract_value(receiver, 0, "range.start")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let end = self
            .builder
            .extract_value(receiver, 1, "range.end")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let step = self
            .builder
            .extract_value(receiver, 2, "range.step")
            .unwrap_or_else(|| self.builder.const_i64(1));
        let incl_i64 = self
            .builder
            .extract_value(receiver, 3, "range.incl.raw")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Truncate inclusive flag from i64 to i1 for the runtime
        let bool_ty = self.builder.bool_type();
        let inclusive = self.builder.trunc(incl_i64, bool_ty, "range.inclusive");

        self.builder
            .call(func_id, &[start, end, step, inclusive], "range.iter")
    }

    // -----------------------------------------------------------------------
    // Equality thunk generation
    // -----------------------------------------------------------------------

    /// Get or generate an LLVM equality thunk for comparing elements of `elem_ty`.
    ///
    /// The thunk has signature `fn(*const u8, *const u8) -> bool` (i1). Each
    /// element type gets a specialized function that loads elements by their
    /// native LLVM type before comparing.
    ///
    /// For strings, returns a pointer to `ori_str_eq` directly (same signature).
    /// For primitives, generates an inline comparison function.
    ///
    /// Returns a function pointer `ValueId`, or `None` if the element type
    /// is not yet supported for equality comparison.
    pub(in crate::codegen::arc_emitter) fn get_or_create_eq_thunk(
        &mut self,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        // Check cache first
        if let Some(&func_id) = self.eq_thunk_cache.get(&elem_ty) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let elem_info = self.type_info.get(elem_ty);

        // String equality: `ori_str_eq` has signature fn(ptr, ptr) -> i1 — use directly
        if matches!(&elem_info, TypeInfo::Str) {
            let func_id = self.builder.runtime_fn("ori_str_eq");
            self.eq_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        let type_suffix = match &elem_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
            TypeInfo::Float => "float",
            TypeInfo::Bool => "bool",
            TypeInfo::Char => "char",
            TypeInfo::Byte => "byte",
            _ => return None,
        };

        let func_name = format!("_ori_eq_{type_suffix}");

        // Check if already declared in the module (shared across emitters)
        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

        // If the function already has a body, just cache and return
        if self.builder.function_has_body(func_id) {
            self.eq_thunk_cache.insert(elem_ty, func_id);
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

        // Generate the equality comparison body
        let result = self.gen_primitive_eq(a_ptr, b_ptr, elem_ty, &elem_info);

        self.builder.ret(result);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.eq_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
    }

    /// Generate equality comparison body for primitive types.
    ///
    /// Loads values from pointers, compares with `icmp eq` (integers) or
    /// `fcmp oeq` (floats), and returns `i1`.
    fn gen_primitive_eq(
        &mut self,
        a_ptr: ValueId,
        b_ptr: ValueId,
        elem_ty: Idx,
        elem_info: &TypeInfo,
    ) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let a_val = self.builder.load(llvm_ty, a_ptr, "a");
        let b_val = self.builder.load(llvm_ty, b_ptr, "b");

        match elem_info {
            // Integer equality (signed: int, Duration, Size; unsigned: bool, char, byte)
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte => self.builder.icmp_eq(a_val, b_val, "eq"),
            // Float equality (ordered — NaN != NaN)
            TypeInfo::Float => self.builder.fcmp_oeq(a_val, b_val, "eq"),
            _ => unreachable!("non-primitive passed to gen_primitive_eq"),
        }
    }

    // -----------------------------------------------------------------------
    // Hash thunk generation
    // -----------------------------------------------------------------------

    /// Get or generate an LLVM hash thunk for hashing elements of `elem_ty`.
    ///
    /// The thunk has signature `fn(*const u8) -> i64`. Each element type gets
    /// a specialized function that loads the element by its native LLVM type
    /// before computing the hash.
    ///
    /// For strings, returns a pointer to `ori_str_hash` directly (same signature).
    /// For primitives, generates an inline hash function.
    ///
    /// Returns a function pointer `ValueId`, or `None` if the element type
    /// is not yet supported for hashing.
    pub(in crate::codegen::arc_emitter) fn get_or_create_hash_thunk(
        &mut self,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        // Check cache first
        if let Some(&func_id) = self.hash_thunk_cache.get(&elem_ty) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let elem_info = self.type_info.get(elem_ty);

        // String hash: `ori_str_hash` has signature fn(ptr) -> i64 — use directly
        if matches!(&elem_info, TypeInfo::Str) {
            let func_id = self.builder.runtime_fn("ori_str_hash");
            self.hash_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        let type_suffix = match &elem_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
            TypeInfo::Float => "float",
            TypeInfo::Bool => "bool",
            TypeInfo::Char => "char",
            TypeInfo::Byte => "byte",
            _ => return None,
        };

        let func_name = format!("_ori_hash_{type_suffix}");

        // Check if already declared in the module (shared across emitters)
        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty], i64_ty);

        // If the function already has a body, just cache and return
        if self.builder.function_has_body(func_id) {
            self.hash_thunk_cache.insert(elem_ty, func_id);
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

        // Generate the hash body
        let result = self.gen_primitive_hash(a_ptr, elem_ty, &elem_info);

        self.builder.ret(result);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.hash_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
    }

    /// Generate hash body for primitive types.
    ///
    /// Loads the value from the pointer and converts to i64:
    /// - `int`/`Duration`/`Size`: identity (already i64)
    /// - `bool`/`byte`: load i8, zero-extend to i64
    /// - `char`: load i32, zero-extend to i64
    /// - `float`: load f64, bitcast to i64
    fn gen_primitive_hash(
        &mut self,
        a_ptr: ValueId,
        elem_ty: Idx,
        elem_info: &TypeInfo,
    ) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let a_val = self.builder.load(llvm_ty, a_ptr, "a");
        let i64_ty = self.builder.i64_type();

        match elem_info {
            // i64 types — identity hash
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => a_val,
            // Sub-i64 types (i8 bool/byte, i32 char) — zero-extend to i64
            TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Char => {
                self.builder.zext(a_val, i64_ty, "hash.zext")
            }
            // f64 — bitcast to i64
            TypeInfo::Float => self.builder.bitcast(a_val, i64_ty, "hash.bits"),
            _ => unreachable!("non-primitive passed to gen_primitive_hash"),
        }
    }
}
