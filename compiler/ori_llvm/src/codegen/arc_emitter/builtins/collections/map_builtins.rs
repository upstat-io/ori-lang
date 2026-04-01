//! Map builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains_key`, `keys`, `values`,
//! `get`, `insert`, `remove`, and `iter` for maps.
//!
//! Map mutations use COW semantics: when the collection is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each mutating method returns a `{i64 len, i64 cap, ptr data}` struct.

use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `map.length` — extract field 0 (len) from `{i64 len, i64 cap, ptr data}`.
    pub(crate) fn emit_map_length(&mut self, receiver: ValueId) -> Option<ValueId> {
        self.builder.extract_value(receiver, FIELD_LEN, "map.len")
    }

    /// Emit `map.is_empty()` — `len == 0`.
    pub(crate) fn emit_map_is_empty(&mut self, receiver: ValueId) -> Option<ValueId> {
        let len = self.builder.extract_value(receiver, FIELD_LEN, "map.len")?;
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
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_contains_key");

        let data = self
            .builder
            .extract_value(receiver, FIELD_DATA, "map.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let cap = self
            .builder
            .extract_value(receiver, FIELD_CAP, "map.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let len = self
            .builder
            .extract_value(receiver, FIELD_LEN, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        let needle_ptr = self.elem_to_ptr(key, key_ty, "contains_key.needle");
        // Use narrowed key size if available.
        let collection_idx = self.pool.resolve_fully(map_ty);
        let key_size = self.collection_elem_size(collection_idx, key_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;

        let result = self.emit_rt_call(
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
    /// Calls `ori_map_keys_to_list(data, cap, len, key_size, key_dec_fn, key_inc_fn, out_ptr)`.
    /// `key_inc_fn` increments RC children of each copied key to prevent
    /// double-free when both the map and the output list are dropped.
    pub(crate) fn emit_map_keys(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_keys_to_list");

        let data = self
            .builder
            .extract_value(receiver, FIELD_DATA, "map.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let cap = self
            .builder
            .extract_value(receiver, FIELD_CAP, "map.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let len = self
            .builder
            .extract_value(receiver, FIELD_LEN, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Use narrowed key size if available.
        let collection_idx = self.pool.resolve_fully(map_ty);
        let key_size = self.collection_elem_size(collection_idx, key_ty);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_ty);
        let key_inc_fn = self.get_or_generate_elem_inc_fn(key_ty);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "keys.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                key_dec_fn,
                key_inc_fn,
                out_alloca,
            ],
            "keys",
        );

        Some(self.builder.load(list_ty, out_alloca, "keys.val"))
    }

    /// Emit `map.values()` — extract values as a new list.
    ///
    /// Calls `ori_map_values_to_list(data, cap, len, key_size, val_size,
    /// val_dec_fn, val_inc_fn, out_ptr)`. `val_inc_fn` prevents double-free
    /// on shared RC-tracked value data.
    pub(crate) fn emit_map_values(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_values_to_list");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty));
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_ty);
        let val_inc_fn = self.get_or_generate_elem_inc_fn(val_ty);

        let list_ty = self.list_struct_type();
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "values.out", list_ty);

        self.emit_rt_call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                val_size_val,
                val_dec_fn,
                val_inc_fn,
                out_alloca,
            ],
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
        map_ty: Option<Idx>,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_get");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, map_ty);

        let needle_ptr = self.elem_to_ptr(key, key_ty, "get.needle");
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;

        // Option<V> layout: {i64 tag, V value} — runtime (ori_rt) writes i64 tags
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

        self.emit_rt_call(
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
    ///
    /// When `map_ty` is `Some`, uses `collection_elem_size` for
    /// narrowed element sizes. Otherwise falls back to canonical sizes.
    pub(in crate::codegen::arc_emitter) fn extract_map_components(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        map_ty: Option<Idx>,
    ) -> (ValueId, ValueId, ValueId, ValueId, ValueId) {
        let data = self
            .builder
            .extract_value(receiver, FIELD_DATA, "map.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let cap = self
            .builder
            .extract_value(receiver, FIELD_CAP, "map.cap")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let len = self
            .builder
            .extract_value(receiver, FIELD_LEN, "map.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        // Use narrowed element sizes for map buffers.
        let (key_size, val_size) = if let Some(mt) = map_ty {
            let collection_idx = self.pool.resolve_fully(mt);
            (
                self.collection_elem_size(collection_idx, key_ty),
                self.collection_elem_size(collection_idx, val_ty),
            )
        } else {
            (
                self.element_store_size(key_ty),
                self.element_store_size(val_ty),
            )
        };
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);
        (data, cap, len, key_size_val, val_size_val)
    }

    /// Build the LLVM struct type `{i64, i64, ptr}` for map sret returns.
    pub(in crate::codegen::arc_emitter) fn map_struct_type(&mut self) -> LLVMTypeId {
        // Same as list/set — {i64 len, i64 cap, ptr data}
        self.list_struct_type()
    }

    /// Emit `map.insert(key, value)` — COW insert returning the (possibly mutated) map.
    ///
    /// Calls `ori_map_insert_cow(data, len, cap, key, value, key_size, val_size,
    ///         key_eq, key_hash, key_inc, val_inc, val_dec, cow_mode, out_ptr)`.
    pub(crate) fn emit_map_insert(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        value: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        cow_mode: ValueId,
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_insert_cow");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty));

        let key_ptr = self.elem_to_ptr(key, key_ty, "insert.key");
        let val_ptr = self.elem_to_ptr(value, val_ty, "insert.val");
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;
        let key_inc = self.get_or_generate_elem_inc_fn(key_ty);
        let val_inc = self.get_or_generate_elem_inc_fn(val_ty);
        let val_dec = self.get_or_generate_elem_dec_fn(val_ty);

        let map_ty = self.map_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "insert.out", map_ty);

        self.emit_rt_call(
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
                val_dec,
                cow_mode,
                out,
            ],
            "insert",
        );

        Some(self.builder.load(map_ty, out, "insert.val"))
    }

    /// Emit `map.remove(key)` — COW remove returning the (possibly mutated) map.
    ///
    /// Decs RC children of removed key/value on unique paths.
    ///
    /// Calls `ori_map_remove_cow(data, len, cap, key, key_size, val_size,
    ///         key_eq, key_hash, key_inc, val_inc, key_dec, val_dec, cow_mode, out_ptr)`.
    pub(crate) fn emit_map_remove(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        cow_mode: ValueId,
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_remove_cow");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty));

        let key_ptr = self.elem_to_ptr(key, key_ty, "remove.key");
        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;
        let key_inc = self.get_or_generate_elem_inc_fn(key_ty);
        let val_inc = self.get_or_generate_elem_inc_fn(val_ty);
        let key_dec = self.get_or_generate_elem_dec_fn(key_ty);
        let val_dec = self.get_or_generate_elem_dec_fn(val_ty);

        let map_ty = self.map_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "remove.out", map_ty);

        self.emit_rt_call(
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
                key_dec,
                val_dec,
                cow_mode,
                out,
            ],
            "remove",
        );

        Some(self.builder.load(map_ty, out, "remove.val"))
    }

    /// Emit `map.iter()` — call `ori_iter_from_map(data, cap, len, ks, vs, owns, k_dec, v_dec)`.
    ///
    /// The iterator takes ownership of one RC reference to the data buffer,
    /// releasing it via `ori_map_buffer_rc_dec` when dropped.
    ///
    /// Element cleanup contract: passes real key/val dec functions so that
    /// `ori_map_buffer_rc_dec` properly cleans up RC children when the buffer
    /// is freed. The AIMS pipeline marks destructured `(k, v)` variables as
    /// borrowed (via `collect_iter_element_defs()` transitive Project chain
    /// propagation), so AIMS skips their `RcDec` — the buffer's Drop handles
    /// all element cleanup.
    pub(crate) fn emit_map_iter(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_map");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty));

        let owns_data = self.builder.const_bool(true);
        // Real dec functions: `collect_iter_element_defs()` propagates
        // borrowed status through transitive Project chains (tuple
        // destructuring), so AIMS skips RcDec on destructured k/v.
        // The buffer's drop (via ori_map_buffer_rc_dec) handles all
        // element cleanup using these real dec functions.
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_ty);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_ty);

        self.emit_rt_call(
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
}
