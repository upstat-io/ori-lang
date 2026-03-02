//! Per-strategy RC increment/decrement functions.
//!
//! Each [`RcStrategy`] variant has a dedicated `emit_rc_inc_*` and
//! `emit_rc_dec_*` function. These replace the monolithic Pool-querying
//! handlers that previously lived inline in `emit_instr`.
//!
//! # Strategy → handler mapping
//!
//! | Strategy          | Inc handler              | Dec handler               |
//! |-------------------|--------------------------|---------------------------|
//! | `HeapPointer`     | `emit_rc_inc_heap`       | `emit_rc_dec_heap`        |
//! | `FatPointer`      | `emit_rc_inc_fat`        | `emit_rc_dec_fat`         |
//! | `Closure`         | `emit_rc_inc_closure`    | `emit_rc_dec_closure`     |
//! | `AggregateFields` | `emit_rc_inc_aggregate`  | `emit_rc_dec_aggregate`   |
//! | `InlineEnum`      | `emit_rc_inc_inline_enum`| `emit_rc_dec_inline_enum` |
//!
//! # Asymmetry: `InlineEnum`
//!
//! `InlineEnum` Inc is a **no-op** — the container is stack-allocated, and
//! inner fields are managed at extraction or Dec time. Dec performs a
//! tag-switch with per-variant field cleanup.
//!
//! # Design: no `extract_rc_data_ptrs` calls
//!
//! None of the handlers in this module call `extract_rc_data_ptrs`. Each
//! strategy knows its own layout and extracts pointers directly:
//!
//! - `HeapPointer`: slice-aware for List/Set (data+cap → `ori_list_rc_inc`); Map field 2
//! - `FatPointer`: always field 1 (the `data_ptr` half)
//! - `Closure`: field 1 (`env_ptr`) with null-check
//! - `AggregateFields`: struct/tuple field traversal via [`inc_value_rc`] / [`dec_value_rc`]
//! - `InlineEnum`: Inc no-op; Dec via `emit_inline_enum_dec` (tag-switch)
//!
//! `extract_rc_data_ptrs` remains in `mod.rs` for non-RC uses (closure env
//! drop, drop function generation, builtin clone).
//!
//! Pool queries are still used for type tags and field enumeration. These will
//! be eliminated in Section 01.4 when `ValueRepr` propagation makes layouts
//! fully explicit.

use ori_arc::ir::{ArcFunction, ArcVarId, RcStrategy};
use ori_types::Tag;

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    /// Dispatch an RC increment to the appropriate per-strategy handler.
    pub(super) fn emit_rc_inc(
        &mut self,
        var: ArcVarId,
        count: u32,
        strategy: RcStrategy,
        func: &ArcFunction,
    ) {
        let val = self.var(var);
        if val.is_none() {
            tracing::warn!(
                var = var.raw(),
                ?strategy,
                "skipping RcInc on undefined variable"
            );
            return;
        }

        // Temporary validation: verify strategy matches Pool-derived expectation.
        // Removed once all Pool queries are eliminated from the emitter (Section 01.4).
        #[cfg(debug_assertions)]
        if let Some(repr) = func.var_repr(var) {
            let expected = RcStrategy::from_var(repr, self.pool, func.var_type(var));
            debug_assert_eq!(
                strategy, expected,
                "RcStrategy mismatch for var {var:?}: instruction has {strategy:?}, Pool says {expected:?}",
            );
        }

        match strategy {
            RcStrategy::HeapPointer => self.emit_rc_inc_heap(var, count, func),
            RcStrategy::FatPointer => self.emit_rc_inc_fat(var, count),
            RcStrategy::Closure => self.emit_rc_inc_closure(self.var(var), count),
            RcStrategy::AggregateFields => self.emit_rc_inc_aggregate(var, count, func),
            RcStrategy::InlineEnum => Self::emit_rc_inc_inline_enum(),
        }
    }

    /// Dispatch an RC decrement to the appropriate per-strategy handler.
    pub(super) fn emit_rc_dec(&mut self, var: ArcVarId, strategy: RcStrategy, func: &ArcFunction) {
        let val = self.var(var);
        if val.is_none() {
            tracing::warn!(
                var = var.raw(),
                ?strategy,
                "skipping RcDec on undefined variable"
            );
            return;
        }

        // Temporary validation: verify strategy matches Pool-derived expectation.
        #[cfg(debug_assertions)]
        if let Some(repr) = func.var_repr(var) {
            let expected = RcStrategy::from_var(repr, self.pool, func.var_type(var));
            debug_assert_eq!(
                strategy, expected,
                "RcStrategy mismatch for var {var:?}: instruction has {strategy:?}, Pool says {expected:?}",
            );
        }

        match strategy {
            RcStrategy::HeapPointer => self.emit_rc_dec_heap(var, func),
            RcStrategy::FatPointer => self.emit_rc_dec_fat(var, func),
            RcStrategy::Closure => self.emit_rc_dec_closure(self.var(var)),
            RcStrategy::AggregateFields => self.emit_rc_dec_aggregate(var, func),
            RcStrategy::InlineEnum => self.emit_rc_dec_inline_enum(var, func),
        }
    }

    // -----------------------------------------------------------------------
    // HeapPointer handlers
    // -----------------------------------------------------------------------

    /// Inc a heap-allocated collection (List, Map, Set, etc.).
    ///
    /// For List/Set: uses slice-aware `ori_list_rc_inc(data, cap)` which
    /// handles seamless slices (where `data` is interior to another buffer).
    /// For other types: extracts data pointer(s) and calls `ori_rc_inc`.
    fn emit_rc_inc_heap(&mut self, var: ArcVarId, count: u32, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);

        match tag {
            Tag::List | Tag::Set => {
                // Slice-aware: extract data + cap, call ori_list_rc_inc
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, 1, "rc_inc.cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    self.call_list_rc_inc(dp, cap, count);
                } else {
                    self.call_rc_inc_all(&[val], count);
                }
            }
            _ => {
                let ptrs = self.extract_rc_data_ptrs(val, ty);
                self.call_rc_inc_all(&ptrs, count);
            }
        }
    }

    /// Dec a heap-allocated collection.
    ///
    /// For List/Set/Map: extracts len, cap, and data pointer(s), then calls
    /// `ori_buffer_rc_dec` which correctly handles element iteration and
    /// buffer freeing. For other heap types: falls back to `ori_rc_dec`.
    ///
    /// When drop hints indicate the collection is provably unique (RC == 1),
    /// emits a call to `ori_buffer_drop_unique` / `ori_map_buffer_drop_unique`
    /// instead, skipping the atomic RC decrement entirely.
    fn emit_rc_dec_heap(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);

        // Check drop hints: if this RcDec is on a provably unique collection,
        // use the fast unique-drop path (no atomic RC decrement).
        let is_unique = func
            .drop_hints
            .is_unique_drop(self.current_block_idx, self.current_instr_idx);

        match tag {
            Tag::List | Tag::Set => {
                if is_unique {
                    self.emit_buffer_drop_unique_list_or_set(val, resolved, tag);
                } else {
                    self.emit_buffer_rc_dec_list_or_set(val, resolved, tag);
                }
            }
            Tag::Map => {
                if is_unique {
                    self.emit_buffer_drop_unique_map(val, resolved);
                } else {
                    self.emit_buffer_rc_dec_map(val, resolved);
                }
            }
            _ => {
                let drop_fn = self.get_or_generate_drop_fn(ty);
                self.call_rc_dec_all(&[val], drop_fn);
            }
        }
    }

    /// Emit `ori_buffer_rc_dec` for a list or set value.
    ///
    /// Extracts `{len, cap, data}` from the collection value, computes
    /// the element size and element-dec function, and calls the runtime.
    ///
    /// Lists use `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)`.
    /// Sets use `ori_set_buffer_rc_dec(data, cap, len, elem_size, elem_dec_fn)`
    /// (cap and len swapped for hash table layout).
    pub(super) fn emit_buffer_rc_dec_list_or_set(
        &mut self,
        val: super::ValueId,
        resolved: ori_types::Idx,
        tag: Tag,
    ) {
        let Some(data) = self.builder.extract_value(val, 2, "rc.data_ptr") else {
            return;
        };
        let Some(len) = self.builder.extract_value(val, 0, "rc.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "rc.cap") else {
            return;
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
        let elem_size = self.element_store_size(elem_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_type);

        if tag == Tag::Set {
            // Sets use hash table layout: (data, cap, len, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_set_buffer_rc_dec");
            self.builder
                .call(func_id, &[data, cap, len, elem_size_val, elem_dec_fn], "");
        } else {
            // Lists use packed layout: (data, len, cap, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
            self.builder
                .call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
        }
    }

    /// Emit `ori_map_buffer_rc_dec` for a map value.
    ///
    /// Maps use a single combined data buffer `[keys...|vals...]` with one RC
    /// header. The runtime function handles cleaning up both key and value
    /// children at their respective offsets.
    pub(super) fn emit_buffer_rc_dec_map(&mut self, val: super::ValueId, resolved: ori_types::Idx) {
        let Some(len) = self.builder.extract_value(val, 0, "rc.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "rc.cap") else {
            return;
        };
        let Some(data) = self.builder.extract_value(val, 2, "rc.data_ptr") else {
            return;
        };

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        let key_size = self.element_store_size(key_type);
        let val_size = self.element_store_size(val_type);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_type);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_type);

        let func_id = self.builder.runtime_fn("ori_map_buffer_rc_dec");
        self.builder.call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                val_size_val,
                key_dec_fn,
                val_dec_fn,
            ],
            "",
        );
    }

    // -----------------------------------------------------------------------
    // Unique-drop handlers (skip atomic RC dec for provably unique collections)
    // -----------------------------------------------------------------------

    /// Emit `ori_buffer_drop_unique` / `ori_set_buffer_drop_unique` for a
    /// provably unique list or set.
    ///
    /// Same argument extraction as `emit_buffer_rc_dec_list_or_set`, but calls
    /// the unique-drop function which skips the atomic RC decrement.
    ///
    /// Lists use `ori_buffer_drop_unique(data, len, cap, elem_size, elem_dec_fn)`.
    /// Sets use `ori_set_buffer_drop_unique(data, cap, len, elem_size, elem_dec_fn)`
    /// (cap and len swapped for hash table layout).
    fn emit_buffer_drop_unique_list_or_set(
        &mut self,
        val: super::ValueId,
        resolved: ori_types::Idx,
        tag: Tag,
    ) {
        let Some(data) = self.builder.extract_value(val, 2, "udrop.data_ptr") else {
            return;
        };
        let Some(len) = self.builder.extract_value(val, 0, "udrop.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "udrop.cap") else {
            return;
        };

        let elem_type = if tag == Tag::List {
            self.pool.list_elem(resolved)
        } else {
            self.pool.set_elem(resolved)
        };
        let elem_size = self.element_store_size(elem_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_type);

        if tag == Tag::Set {
            // Sets use hash table layout: (data, cap, len, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_set_buffer_drop_unique");
            self.builder
                .call(func_id, &[data, cap, len, elem_size_val, elem_dec_fn], "");
        } else {
            // Lists use packed layout: (data, len, cap, elem_size, elem_dec_fn)
            let func_id = self.builder.runtime_fn("ori_buffer_drop_unique");
            self.builder
                .call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
        }
    }

    /// Emit `ori_map_buffer_drop_unique` for a provably unique map.
    ///
    /// Same argument extraction as `emit_buffer_rc_dec_map`, but calls
    /// the unique-drop function which skips the atomic RC decrement.
    fn emit_buffer_drop_unique_map(&mut self, val: super::ValueId, resolved: ori_types::Idx) {
        let Some(len) = self.builder.extract_value(val, 0, "udrop.len") else {
            return;
        };
        let Some(cap) = self.builder.extract_value(val, 1, "udrop.cap") else {
            return;
        };
        let Some(data) = self.builder.extract_value(val, 2, "udrop.data_ptr") else {
            return;
        };

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        let key_size = self.element_store_size(key_type);
        let val_size = self.element_store_size(val_type);
        let key_size_val = self.builder.const_i64(key_size as i64);
        let val_size_val = self.builder.const_i64(val_size as i64);
        let key_dec_fn = self.get_or_generate_elem_dec_fn(key_type);
        let val_dec_fn = self.get_or_generate_elem_dec_fn(val_type);

        let func_id = self.builder.runtime_fn("ori_map_buffer_drop_unique");
        self.builder.call(
            func_id,
            &[
                data,
                cap,
                len,
                key_size_val,
                val_size_val,
                key_dec_fn,
                val_dec_fn,
            ],
            "",
        );
    }

    // -----------------------------------------------------------------------
    // FatPointer handlers
    // -----------------------------------------------------------------------

    /// Inc a fat value (str = `{i64 len, i64 cap, ptr data}`).
    ///
    /// Data pointer is always at field 2. SSO strings (inline, no heap)
    /// are detected by the MSB of the data pointer field and skipped.
    fn emit_rc_inc_fat(&mut self, var: ArcVarId, count: u32) {
        let val = self.var(var);
        let Some(data_ptr) = self.builder.extract_value(val, 2, "rc_inc.fat_data") else {
            return;
        };
        let do_inc = self
            .builder
            .append_block(self.current_function, "rc_inc.heap");
        let skip = self
            .builder
            .append_block(self.current_function, "rc_inc.sso_skip");
        let is_sso = self.emit_sso_check(data_ptr, "rc_inc");
        self.builder.cond_br(is_sso, skip, do_inc);

        self.builder.position_at_end(do_inc);
        self.call_rc_inc_all(&[data_ptr], count);
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    /// Dec a fat value.
    ///
    /// Data pointer at field 2, drop function from the variable's type.
    /// SSO strings are detected and skipped (no heap allocation to free).
    fn emit_rc_dec_fat(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let Some(data_ptr) = self.builder.extract_value(val, 2, "rc_dec.fat_data") else {
            return;
        };
        let do_dec = self
            .builder
            .append_block(self.current_function, "rc_dec.heap");
        let skip = self
            .builder
            .append_block(self.current_function, "rc_dec.sso_skip");
        let is_sso = self.emit_sso_check(data_ptr, "rc_dec");
        self.builder.cond_br(is_sso, skip, do_dec);

        self.builder.position_at_end(do_dec);
        let drop_fn = self.get_or_generate_drop_fn(ty);
        self.call_rc_dec_all(&[data_ptr], drop_fn);
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    /// Check if a string's data pointer field indicates SSO (inline storage).
    ///
    /// SSO strings store inline bytes in the union. The SSO flag is the MSB
    /// of byte 23 (the `flags` field). On little-endian x86-64, this maps to
    /// the MSB of the pointer field. We also treat null pointers as "skip RC"
    /// (empty heap strings have null data).
    pub(super) fn emit_sso_check(
        &mut self,
        data_ptr: super::ValueId,
        prefix: &str,
    ) -> super::ValueId {
        let i64_ty = self.builder.i64_type();
        let ptr_int = self
            .builder
            .ptr_to_int(data_ptr, i64_ty, &format!("{prefix}.p2i"));
        let sso_mask = self.builder.const_i64(i64::MIN); // 0x8000_0000_0000_0000
        let masked = self
            .builder
            .and(ptr_int, sso_mask, &format!("{prefix}.sso_flag"));
        let zero = self.builder.const_i64(0);
        let is_sso = self
            .builder
            .icmp_ne(masked, zero, &format!("{prefix}.is_sso"));
        // Also skip if null (empty heap strings)
        let is_null = self
            .builder
            .is_null_ptr(data_ptr, &format!("{prefix}.null"));
        self.builder
            .or(is_sso, is_null, &format!("{prefix}.skip_rc"))
    }

    // -----------------------------------------------------------------------
    // AggregateFields handlers
    // -----------------------------------------------------------------------

    /// Inc a struct/tuple aggregate by traversing RC-typed fields.
    fn emit_rc_inc_aggregate(&mut self, var: ArcVarId, count: u32, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        self.inc_value_rc(val, ty, count);
    }

    /// Dec a struct/tuple aggregate by traversing RC-typed fields.
    fn emit_rc_dec_aggregate(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        self.dec_value_rc(val, ty);
    }

    // -----------------------------------------------------------------------
    // Closure handlers
    // -----------------------------------------------------------------------

    /// Inc a closure (`{fn_ptr, env_ptr}`).
    ///
    /// Extract `env_ptr` (field 1), null-check (zero-capture closures have
    /// null env), then call `ori_rc_inc` on the non-null env.
    pub(super) fn emit_rc_inc_closure(&mut self, val: super::ValueId, count: u32) {
        let func_id = self.builder.runtime_fn("ori_rc_inc");

        let Some(env_ptr) = self.builder.extract_value(val, 1, "rc_inc.env") else {
            return;
        };

        let is_null = self.builder.is_null_ptr(env_ptr, "rc_inc.null");
        let do_inc = self
            .builder
            .append_block(self.current_function, "rc_inc.do");
        let skip = self
            .builder
            .append_block(self.current_function, "rc_inc.skip");
        self.builder.cond_br(is_null, skip, do_inc);

        self.builder.position_at_end(do_inc);
        for _ in 0..count {
            self.builder.call(func_id, &[env_ptr], "");
        }
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    /// Dec a closure (`{fn_ptr, env_ptr}`).
    ///
    /// Extract `env_ptr`, null-check, then load the drop function pointer
    /// from the env header and call `ori_rc_dec(env_ptr, drop_fn)`.
    pub(super) fn emit_rc_dec_closure(&mut self, val: super::ValueId) {
        let Some(env_ptr) = self.builder.extract_value(val, 1, "rc_dec.env") else {
            return;
        };

        let is_null = self.builder.is_null_ptr(env_ptr, "rc_dec.null");
        let do_dec = self
            .builder
            .append_block(self.current_function, "rc_dec.do");
        let skip = self
            .builder
            .append_block(self.current_function, "rc_dec.skip");
        self.builder.cond_br(is_null, skip, do_dec);

        self.builder.position_at_end(do_dec);
        let ptr_ty = self.builder.ptr_type();
        let drop_fn = self.builder.load(ptr_ty, env_ptr, "rc_dec.drop_fn");
        let func_id = self.builder.runtime_fn("ori_rc_dec");
        self.builder.call(func_id, &[env_ptr, drop_fn], "");
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    // -----------------------------------------------------------------------
    // InlineEnum handlers
    // -----------------------------------------------------------------------

    /// Inc an inline enum — intentional no-op.
    ///
    /// Inline enums (Result, Enum, Option) are stack-allocated. Inner RC
    /// fields are managed at extraction time or during Dec. Incrementing
    /// the container itself is meaningless.
    fn emit_rc_inc_inline_enum() {
        tracing::trace!("RcInc on InlineEnum — no-op (stack-allocated container)");
    }

    /// Dec an inline enum (Result, Enum) — tag-switch with per-variant cleanup.
    ///
    /// Delegates to `emit_inline_enum_dec` which performs:
    /// 1. Store to alloca
    /// 2. Load tag
    /// 3. Switch on tag
    /// 4. Per-variant: extract RC fields, call `ori_rc_dec` for each
    fn emit_rc_dec_inline_enum(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let pool_tag = self.pool.tag(resolved);
        self.emit_inline_enum_dec(val, resolved, pool_tag);
    }

    // -----------------------------------------------------------------------
    // Call helpers
    // -----------------------------------------------------------------------

    /// Call `ori_rc_inc(ptr)` for each pointer, `count` times.
    pub(super) fn call_rc_inc_all(&mut self, ptrs: &[super::ValueId], count: u32) {
        if ptrs.is_empty() {
            return;
        }
        let func_id = self.builder.runtime_fn("ori_rc_inc");
        for &ptr in ptrs {
            for _ in 0..count {
                self.builder.call(func_id, &[ptr], "");
            }
        }
    }

    /// Call `ori_list_rc_inc(data, cap)` — slice-aware RC inc for list/set.
    ///
    /// Unlike `call_rc_inc_all` which calls `ori_rc_inc(data)` directly,
    /// this passes `cap` so the runtime can check `is_slice_cap(cap)` and
    /// find the original allocation's RC header for slices.
    pub(super) fn call_list_rc_inc(
        &mut self,
        data: super::ValueId,
        cap: super::ValueId,
        count: u32,
    ) {
        let func_id = self.builder.runtime_fn("ori_list_rc_inc");
        for _ in 0..count {
            self.builder.call(func_id, &[data, cap], "");
        }
    }

    /// Call `ori_rc_dec(ptr, drop_fn)` for each pointer.
    pub(super) fn call_rc_dec_all(&mut self, ptrs: &[super::ValueId], drop_fn: super::ValueId) {
        if ptrs.is_empty() {
            return;
        }
        let func_id = self.builder.runtime_fn("ori_rc_dec");
        for &ptr in ptrs {
            self.builder.call(func_id, &[ptr, drop_fn], "");
        }
    }
}
