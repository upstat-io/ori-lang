//! Map and Set builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains_key`, `keys`, `values`,
//! `get`, `insert`, `remove`, and `iter` for maps, and `length`, `len`,
//! `is_empty`, `contains`, `insert`, `remove`, `union`, `intersection`,
//! `difference`, `to_list`, `into`, and `iter` for sets.
//!
//! Also includes `range.iter()` since Range is a simple collection type.

use ori_types::Idx;

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Map methods
    // -----------------------------------------------------------------------

    /// Emit `map.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr keys, ptr vals}`.
    pub(crate) fn emit_map_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, 0, "map.len")
    }

    /// Emit `map.is_empty()` — `len == 0`.
    pub(crate) fn emit_map_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, 0, "map.len")?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, "map.is_empty"))
    }

    /// Emit `map.contains_key(key)` — linear scan through string keys.
    ///
    /// Calls `ori_map_contains_key(keys_ptr, len, needle_ptr)`.
    /// Map layout: `{i64 len, i64 cap, ptr keys, ptr vals}`.
    pub(crate) fn emit_map_contains_key(
        &mut self,
        receiver: ValueId,
        key: ValueId,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_contains_key");

        let keys_ptr = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let needle_ptr = self.str_to_ptr(key, "contains_key.needle");
        let result = self
            .builder
            .call(func_id, &[keys_ptr, len, needle_ptr], "contains_key")?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "contains_key.bool"))
    }

    /// Emit `map.keys()` — extract keys as a new list.
    ///
    /// Calls `ori_map_keys_to_list(keys_ptr, len, key_size, out_ptr)`.
    /// Returns `{i64 len, i64 cap, ptr data}` (list struct).
    pub(crate) fn emit_map_keys(&mut self, receiver: ValueId, key_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_keys_to_list");

        let keys_ptr = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
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
            .call(func_id, &[keys_ptr, len, key_size_val, out_alloca], "keys");

        Some(self.builder.load(list_ty, out_alloca, "keys.val"))
    }

    /// Emit `map.values()` — extract values as a new list.
    ///
    /// Calls `ori_map_values_to_list(vals_ptr, len, val_size, out_ptr)`.
    /// Returns `{i64 len, i64 cap, ptr data}` (list struct).
    pub(crate) fn emit_map_values(&mut self, receiver: ValueId, val_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_values_to_list");

        let vals_ptr = self
            .builder
            .extract_value(receiver, 3, "map.vals")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let val_size = self.element_store_size(val_ty);
        let val_size_val = self.builder.const_i64(val_size as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "values.out", list_ty);

        self.builder.call(
            func_id,
            &[vals_ptr, len, val_size_val, out_alloca],
            "values",
        );

        Some(self.builder.load(list_ty, out_alloca, "values.val"))
    }

    /// Emit `map.get(key)` — returns `Option<V>` via sret.
    ///
    /// Calls `ori_map_get(keys, vals, len, needle_ptr, val_size, out_ptr)`.
    /// Returns `{i64 tag, V value}` — tag 0=Some, 1=None.
    pub(crate) fn emit_map_get(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_get");

        let keys = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let vals = self
            .builder
            .extract_value(receiver, 3, "map.vals")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let needle_ptr = self.str_to_ptr(key, "get.needle");
        let val_size = self.element_store_size(val_ty);
        let val_size_val = self.builder.const_i64(val_size as i64);

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
            &[keys, vals, len, needle_ptr, val_size_val, out_alloca],
            "get",
        );

        Some(self.builder.load(option_ty, out_alloca, "get.val"))
    }

    /// Extract map keys, vals, len and compute key/val sizes from type info.
    fn extract_map_components(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> (ValueId, ValueId, ValueId, ValueId, ValueId) {
        let keys = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let vals = self
            .builder
            .extract_value(receiver, 3, "map.vals")
            .unwrap_or_else(|| self.builder.const_null_ptr());
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
        (keys, vals, len, key_size_val, val_size_val)
    }

    /// Build the LLVM struct type `{i64, i64, ptr, ptr}` for map sret returns.
    fn map_struct_type(&mut self) -> LLVMTypeId {
        self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        )
    }

    /// Emit `map.insert(key, value)` — returns a new map via sret.
    ///
    /// Calls `ori_map_insert(keys, vals, len, key_ptr, val_ptr, key_size, val_size, out_ptr)`.
    pub(crate) fn emit_map_insert(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        value: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_insert");

        let (keys, vals, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let key_ptr = self.str_to_ptr(key, "insert.key");
        let val_ptr = self.elem_to_ptr(value, val_ty, "insert.val");

        let map_ty = self.map_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "insert.out", map_ty);

        self.builder.call(
            func_id,
            &[
                keys,
                vals,
                len,
                key_ptr,
                val_ptr,
                key_size_val,
                val_size_val,
                out_alloca,
            ],
            "insert",
        );

        Some(self.builder.load(map_ty, out_alloca, "insert.val"))
    }

    /// Emit `map.remove(key)` — returns a new map via sret.
    ///
    /// Calls `ori_map_remove(keys, vals, len, needle_ptr, key_size, val_size, out_ptr)`.
    pub(crate) fn emit_map_remove(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_remove");

        let (keys, vals, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty);

        let needle_ptr = self.str_to_ptr(key, "remove.needle");

        let map_ty = self.map_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "remove.out", map_ty);

        self.builder.call(
            func_id,
            &[
                keys,
                vals,
                len,
                needle_ptr,
                key_size_val,
                val_size_val,
                out_alloca,
            ],
            "remove",
        );

        Some(self.builder.load(map_ty, out_alloca, "remove.val"))
    }

    /// Emit `map.iter()` — call `ori_iter_from_map(keys, vals, len, ks, vs, owns, k_dec, v_dec)`.
    ///
    /// Map layout: `{i64 len, i64 cap, ptr keys, ptr vals}`.
    /// Yields `(K, V)` tuples as concatenated key+value bytes.
    /// The iterator takes ownership of one RC reference to each of the keys
    /// and values buffers, releasing them via `ori_rc_dec` when dropped.
    pub(crate) fn emit_map_iter(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_map");

        let keys = self
            .builder
            .extract_value(receiver, 2, "map.keys")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let vals = self
            .builder
            .extract_value(receiver, 3, "map.vals")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let key_size = self.element_store_size(key_ty);
        let val_size = self.element_store_size(val_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);
        let owns_data = self.builder.const_bool(true);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_ty);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_ty);

        self.builder.call(
            func_id,
            &[
                keys,
                vals,
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

    /// Extract set data pointer and length from `{i64 len, i64 cap, ptr data}`.
    fn extract_set_components(&mut self, receiver: ValueId) -> (ValueId, ValueId) {
        let data_ptr = self
            .builder
            .extract_value(receiver, 2, "set.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let len = self
            .builder
            .extract_value(receiver, 0, "set.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        (data_ptr, len)
    }

    /// Emit `set.contains(elem)` — calls `ori_set_contains(data, len, elem_ptr, elem_size)`.
    pub(crate) fn emit_set_contains(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_contains");

        let (data_ptr, len) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "contains.elem");
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let result = self.builder.call(
            func_id,
            &[data_ptr, len, elem_ptr, elem_size],
            "set.contains",
        )?;

        // Convert i64 (0/1) to i1 (bool)
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_ne(result, zero, "set.contains.bool"))
    }

    /// Emit `set.insert(elem)` — returns a new set via sret.
    pub(crate) fn emit_set_insert(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_insert");

        let (data_ptr, len) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let set_ty = self.list_struct_type(); // Same layout as list: {i64, i64, ptr}
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "set.insert.out", set_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_ptr, elem_size, out_alloca],
            "set.insert",
        );

        Some(self.builder.load(set_ty, out_alloca, "set.insert.val"))
    }

    /// Emit `set.remove(elem)` — returns a new set via sret.
    pub(crate) fn emit_set_remove(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_remove");

        let (data_ptr, len) = self.extract_set_components(receiver);
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "remove.elem");
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let set_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "set.remove.out", set_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_ptr, elem_size, out_alloca],
            "set.remove",
        );

        Some(self.builder.load(set_ty, out_alloca, "set.remove.val"))
    }

    /// Emit a two-set operation (union/intersection/difference) via sret.
    fn emit_set_binary_op(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        func_name: &'static str,
        label: &str,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(func_name);

        let (d1, l1) = self.extract_set_components(receiver);
        let (d2, l2) = self.extract_set_components(other);
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let set_ty = self.list_struct_type();
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            &format!("set.{label}.out"),
            set_ty,
        );

        self.builder.call(
            func_id,
            &[d1, l1, d2, l2, elem_size, out_alloca],
            &format!("set.{label}"),
        );

        Some(
            self.builder
                .load(set_ty, out_alloca, &format!("set.{label}.val")),
        )
    }

    /// Emit `set.union(other)`.
    pub(crate) fn emit_set_union(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(receiver, other, elem_ty, "ori_set_union", "union")
    }

    /// Emit `set.intersection(other)`.
    pub(crate) fn emit_set_intersection(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(
            receiver,
            other,
            elem_ty,
            "ori_set_intersection",
            "intersection",
        )
    }

    /// Emit `set.difference(other)`.
    pub(crate) fn emit_set_difference(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_set_binary_op(receiver, other, elem_ty, "ori_set_difference", "difference")
    }

    /// Emit `set.to_list()` / `set.into()` — copies set data into a new list via sret.
    pub(crate) fn emit_set_to_list(&mut self, receiver: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_set_to_list");

        let (data_ptr, len) = self.extract_set_components(receiver);
        let elem_size = self
            .builder
            .const_i64(self.element_store_size(elem_ty) as i64);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "set.to_list.out", list_ty);

        self.builder.call(
            func_id,
            &[data_ptr, len, elem_size, out_alloca],
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
}
