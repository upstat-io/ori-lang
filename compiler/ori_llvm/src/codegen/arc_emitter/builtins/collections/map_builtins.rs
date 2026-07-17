//! Map builtin method codegen for LLVM.
//!
//! Handles `length`, `len`, `is_empty`, `contains_key`, `keys`, `values`,
//! `get`, `insert`, `remove`, and `iter` for maps.
//!
//! Map mutations use COW semantics: when the collection is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each mutating method returns a `{i64 len, i64 cap, ptr data}` struct.

use ori_types::Idx;

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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

        let (data, len, cap) =
            self.extract_collection_fields(receiver, "map.data", "map.len", "map.cap")?;

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

        let (data, len, cap) =
            self.extract_collection_fields(receiver, "map.data", "map.len", "map.cap")?;

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
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty))?;
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
            self.extract_map_components(receiver, key_ty, val_ty, map_ty)?;

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

        // ori_map_get does a shallow byte-copy of the value from the map's
        // internal storage. For RC-managed value types (lists, maps, strings,
        // closures), the caller needs its own reference — emit RcInc on the
        // value when the tag is Some. Without this, the returned value shares
        // the map's internal pointer: when the map is freed (elem cleanup frees
        // the inner value), the caller's copy becomes a dangling pointer.
        let val_tag = self.pool.tag(self.pool.resolve_fully(val_ty));
        let needs_rc = !matches!(
            val_tag,
            ori_types::Tag::Int
                | ori_types::Tag::Float
                | ori_types::Tag::Bool
                | ori_types::Tag::Char
                | ori_types::Tag::Byte
                | ori_types::Tag::Unit
                | ori_types::Tag::Never
                | ori_types::Tag::Duration
                | ori_types::Tag::Size
        );

        if needs_rc {
            // Conditionally inc: only when tag == SOME (0)
            let tag_ptr = self
                .builder
                .struct_gep(option_ty, out_alloca, 0, "get.tag.ptr");
            let i64_ty = self.builder.i64_type();
            let tag_val = self.builder.load(i64_ty, tag_ptr, "get.tag");
            let zero = self.builder.const_i64(0);
            let is_some = self.builder.icmp_eq(tag_val, zero, "get.is_some");

            let inc_bb = self.builder.append_block(self.current_function, "get.inc");
            let cont_bb = self.builder.append_block(self.current_function, "get.cont");
            self.builder.cond_br(is_some, inc_bb, cont_bb);

            self.builder.position_at_end(inc_bb);
            let val_ptr = self
                .builder
                .struct_gep(option_ty, out_alloca, 1, "get.val.ptr");
            let val_loaded = self.builder.load(val_llvm_ty, val_ptr, "get.val.rc");
            self.inc_value_rc(val_loaded, val_ty, 1);
            self.builder.br(cont_bb);

            self.builder.position_at_end(cont_bb);
        }

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
    ) -> Option<(ValueId, ValueId, ValueId, ValueId, ValueId)> {
        let (data, len, cap) =
            self.extract_collection_fields(receiver, "map.data", "map.len", "map.cap")?;
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
        Some((data, cap, len, key_size_val, val_size_val))
    }

    /// Build the LLVM struct type `{i64, i64, ptr}` for map sret returns.
    pub(in crate::codegen::arc_emitter) fn map_struct_type(&mut self) -> LLVMTypeId {
        // Same as list/set — {i64 len, i64 cap, ptr data}
        self.list_struct_type()
    }
}
