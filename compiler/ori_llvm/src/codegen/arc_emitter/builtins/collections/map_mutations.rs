//! Copy-on-write map mutation emission.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

#[derive(Clone, Copy, Debug)]
struct MapInsertArgs {
    receiver: ValueId,
    key: ValueId,
    value: ValueId,
    key_ty: Idx,
    val_ty: Idx,
    map_ty: Idx,
    cow_mode: ValueId,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `a.merge(b)` — COW map merge desugared from `{...a, ...b}`.
    ///
    /// Calls `ori_map_merge_cow(a_data, a_len, a_cap, b_data, b_len, b_cap,
    ///         key_size, val_size, key_eq, key_hash, key_inc, val_inc, key_dec,
    ///         val_dec, cow_mode, out_ptr)`. Both operands are consumed (`merge` is in
    /// `consuming_receiver_builtins` + `consuming_second_arg_builtins`): `a`
    /// is the accumulator; each occupied entry of `b` is inc'd into the result,
    /// then `b`'s buffer is released by `ori_map_merge_cow` itself. The caller
    /// emits NO scope-exit dec for either operand (both marked `Owned`).
    pub(crate) fn emit_map_merge(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        cow_mode: ValueId,
        map_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_map_merge_cow");

        let (a_data, a_cap, a_len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty))?;
        let (b_data, b_cap, b_len, _bks, _bvs) =
            self.extract_map_components(other, key_ty, val_ty, Some(map_ty))?;

        let key_eq = self.get_or_create_eq_thunk(key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(key_ty)?;
        let key_inc = self.get_or_generate_elem_inc_fn(key_ty);
        let val_inc = self.get_or_generate_elem_inc_fn(val_ty);
        let key_dec = self.get_or_generate_elem_dec_fn(key_ty);
        let val_dec = self.get_or_generate_elem_dec_fn(val_ty);

        let map_struct = self.map_struct_type();
        let out = self
            .builder
            .create_entry_alloca(self.current_function, "merge.out", map_struct);

        self.emit_rt_call(
            func_id,
            &[
                a_data,
                a_len,
                a_cap,
                b_data,
                b_len,
                b_cap,
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
            "merge",
        );

        Some(self.builder.load(map_struct, out, "merge.val"))
    }

    /// Emit `map.insert(key, value)` — COW insert returning the (possibly mutated) map.
    ///
    /// Calls `ori_map_insert_cow(data, len, cap, key, value, key_size, val_size,
    ///         key_eq, key_hash, key_inc, val_inc, key_dec, val_dec, cow_mode, out_ptr)`.
    /// Key and value are borrowed (the buffer copy gets its own RC increment).
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
        self.emit_map_insert_like(
            "ori_map_insert_cow",
            MapInsertArgs {
                receiver,
                key,
                value,
                key_ty,
                val_ty,
                map_ty,
                cow_mode,
            },
        )
    }

    /// Emit `map.updated(key, value)` — COW insert-or-replace (`IndexSet.updated`).
    ///
    /// Same runtime substrate as `insert`; the value is MOVED into the map
    /// (`ori_map_updated_cow` releases the caller's reference after the buffer
    /// increment — `arg_ownership` marks the value `Owned`, so no caller-side
    /// `RcDec` follows).
    pub(crate) fn emit_map_updated(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        value: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        cow_mode: ValueId,
        map_ty: Idx,
    ) -> Option<ValueId> {
        self.emit_map_insert_like(
            "ori_map_updated_cow",
            MapInsertArgs {
                receiver,
                key,
                value,
                key_ty,
                val_ty,
                map_ty,
                cow_mode,
            },
        )
    }

    /// Shared emission for `insert` / `updated` — both call a runtime function
    /// with the `ori_map_insert_cow` parameter shape.
    fn emit_map_insert_like(
        &mut self,
        runtime_fn: &'static str,
        args: MapInsertArgs,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn(runtime_fn);

        let (data, cap, len, key_size_val, val_size_val) = self.extract_map_components(
            args.receiver,
            args.key_ty,
            args.val_ty,
            Some(args.map_ty),
        )?;

        let key_ptr = self.elem_to_ptr(args.key, args.key_ty, "insert.key");
        let val_ptr = self.elem_to_ptr(args.value, args.val_ty, "insert.val");
        let key_eq = self.get_or_create_eq_thunk(args.key_ty)?;
        let key_hash = self.get_or_create_hash_thunk(args.key_ty)?;
        let key_inc = self.get_or_generate_elem_inc_fn(args.key_ty);
        let val_inc = self.get_or_generate_elem_inc_fn(args.val_ty);
        let key_dec = self.get_or_generate_elem_dec_fn(args.key_ty);
        let val_dec = self.get_or_generate_elem_dec_fn(args.val_ty);

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
                key_dec,
                val_dec,
                args.cow_mode,
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
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty))?;

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
    /// is freed. AIMS marks destructured `(k, v)` variables as borrowed (via
    /// `collect_iter_element_defs()` transitive Project chain propagation), so
    /// their logical plan contains no independent release. The current compiled
    /// adapter therefore emits no `RcDec`; the buffer's Drop handles all element
    /// cleanup.
    pub(crate) fn emit_map_iter(
        &mut self,
        receiver: ValueId,
        key_ty: Idx,
        val_ty: Idx,
        map_ty: Idx,
        owns_data: bool,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_from_map");

        let (data, cap, len, key_size_val, val_size_val) =
            self.extract_map_components(receiver, key_ty, val_ty, Some(map_ty))?;

        // owns_data threads the ARC arg-ownership of the .iter() receiver into the
        // runtime ctor, mirroring the list path: a borrowed-rooted receiver (the
        // flatten inner `m.iter()` on a trampoline-closure param, co-owned by the
        // outer `[{K:V}]`) → owns_data = false so the outer container's elem_dec_fn
        // frees the map buffer exactly once.
        let owns_data = self.builder.const_bool(owns_data);
        // Real dec functions: `collect_iter_element_defs()` propagates
        // borrowed status through transitive Project chains (tuple
        // destructuring), so their logical plan has no independent release.
        // The current adapter emits no RcDec on destructured k/v; the buffer's
        // drop (via ori_map_buffer_rc_dec) handles all element cleanup using
        // these real dec functions.
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
