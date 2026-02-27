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
//! - `HeapPointer`: collection layout switch (List field 2, Map fields 2+3)
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
    /// Extracts the data pointer(s) from known collection layouts and calls
    /// `ori_rc_inc` on each. For unknown types, treats the value itself as
    /// the RC pointer.
    fn emit_rc_inc_heap(&mut self, var: ArcVarId, count: u32, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let ptrs = self.extract_rc_data_ptrs(val, ty);
        self.call_rc_inc_all(&ptrs, count);
    }

    /// Dec a heap-allocated collection.
    ///
    /// For List/Set/Map: extracts len, cap, and data pointer(s), then calls
    /// `ori_buffer_rc_dec` which correctly handles element iteration and
    /// buffer freeing. For other heap types: falls back to `ori_rc_dec`.
    fn emit_rc_dec_heap(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);

        match tag {
            Tag::List | Tag::Set => self.emit_buffer_rc_dec_list_or_set(val, resolved, tag),
            Tag::Map => self.emit_buffer_rc_dec_map(val, resolved),
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
    fn emit_buffer_rc_dec_list_or_set(
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

        let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
        self.builder
            .call(func_id, &[data, len, cap, elem_size_val, elem_dec_fn], "");
    }

    /// Emit `ori_buffer_rc_dec` for a map value.
    ///
    /// Maps have two separate data buffers (keys and values). Each is
    /// independently RC-managed with its own element-dec function.
    fn emit_buffer_rc_dec_map(&mut self, val: super::ValueId, resolved: ori_types::Idx) {
        let Some(len) = self.builder.extract_value(val, 0, "rc.len") else {
            return;
        };
        // Maps use len as cap (no separate capacity tracking)
        let cap = len;

        let key_type = self.pool.map_key(resolved);
        let val_type = self.pool.map_value(resolved);

        // Dec keys buffer
        if let Some(keys) = self.builder.extract_value(val, 2, "rc.keys_ptr") {
            let key_size = self.element_store_size(key_type);
            let key_size_val = self.builder.const_i64(key_size as i64);
            let key_dec_fn = self.get_or_generate_elem_dec_fn(key_type);
            let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
            self.builder
                .call(func_id, &[keys, len, cap, key_size_val, key_dec_fn], "");
        }

        // Dec values buffer
        if let Some(vals) = self.builder.extract_value(val, 3, "rc.vals_ptr") {
            let val_size = self.element_store_size(val_type);
            let val_size_val = self.builder.const_i64(val_size as i64);
            let val_dec_fn = self.get_or_generate_elem_dec_fn(val_type);
            let func_id = self.builder.runtime_fn("ori_buffer_rc_dec");
            self.builder
                .call(func_id, &[vals, len, cap, val_size_val, val_dec_fn], "");
        }
    }

    // -----------------------------------------------------------------------
    // FatPointer handlers
    // -----------------------------------------------------------------------

    /// Inc a fat value (str = `{i64 len, ptr data}`).
    ///
    /// Data pointer is always at field 1. No Pool query needed.
    fn emit_rc_inc_fat(&mut self, var: ArcVarId, count: u32) {
        let val = self.var(var);
        let Some(data_ptr) = self.builder.extract_value(val, 1, "rc_inc.fat_data") else {
            return;
        };
        self.call_rc_inc_all(&[data_ptr], count);
    }

    /// Dec a fat value.
    ///
    /// Data pointer at field 1, drop function from the variable's type.
    fn emit_rc_dec_fat(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let Some(data_ptr) = self.builder.extract_value(val, 1, "rc_dec.fat_data") else {
            return;
        };
        let drop_fn = self.get_or_generate_drop_fn(ty);
        self.call_rc_dec_all(&[data_ptr], drop_fn);
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
    fn emit_rc_inc_closure(&mut self, val: super::ValueId, count: u32) {
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
    fn emit_rc_dec_closure(&mut self, val: super::ValueId) {
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
    // Value-level RC helpers (recursive field traversal)
    // -----------------------------------------------------------------------

    /// Increment RC for a value of known type.
    ///
    /// Dispatches by Pool tag to extract the correct data pointer(s) and call
    /// `ori_rc_inc` on each. Handles nested aggregates recursively.
    ///
    /// Used by [`emit_rc_inc_aggregate`] for struct/tuple field traversal.
    /// This replaces the `extract_rc_data_ptrs` → `ori_rc_inc` loop pattern
    /// for RC operations. Pool queries are used for type tags and field
    /// enumeration; these will be eliminated in Section 01.4.
    pub(super) fn inc_value_rc(&mut self, val: super::ValueId, ty: ori_types::Idx, count: u32) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            // Scalars and runtime-tagged types: no static RC action
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering
            | Tag::Result
            | Tag::Enum => {}

            // FatValue: data_ptr at field 1
            Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, 1, "rc_inc.data") {
                    self.call_rc_inc_all(&[dp], count);
                }
            }

            // Closure: null-check env_ptr
            Tag::Function => self.emit_rc_inc_closure(val, count),

            // Collections
            Tag::List | Tag::Set => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    self.call_rc_inc_all(&[dp], count);
                } else {
                    self.call_rc_inc_all(&[val], count);
                }
            }
            Tag::Map => {
                if let Some(k) = self.builder.extract_value(val, 2, "rc_inc.keys") {
                    self.call_rc_inc_all(&[k], count);
                }
                if let Some(v) = self.builder.extract_value(val, 3, "rc_inc.vals") {
                    self.call_rc_inc_all(&[v], count);
                }
            }

            // Struct: traverse RC fields
            Tag::Struct => {
                let fields = self.pool.struct_fields(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field count bounded by struct definition"
                )]
                for (i, (_, field_ty)) in fields.into_iter().enumerate() {
                    if self.classifier.needs_rc(field_ty) {
                        if let Some(fv) =
                            self.builder
                                .extract_value(val, i as u32, &format!("rc_inc.f.{i}"))
                        {
                            self.inc_value_rc(fv, field_ty, count);
                        }
                    }
                }
            }

            // Tuple: traverse RC elements
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "element count bounded by tuple arity"
                )]
                for (i, elem_ty) in elems.into_iter().enumerate() {
                    if self.classifier.needs_rc(elem_ty) {
                        if let Some(ev) =
                            self.builder
                                .extract_value(val, i as u32, &format!("rc_inc.e.{i}"))
                        {
                            self.inc_value_rc(ev, elem_ty, count);
                        }
                    }
                }
            }

            // Option: recurse into inner type at field 1
            // NOTE: latent bug — doesn't check runtime tag. If value is None,
            // field 1 is uninitialized. This matches the existing behavior and
            // is tracked in the plan (Section 01.5 test items).
            Tag::Option => {
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    if let Some(field) = self.builder.extract_value(val, 1, "rc_inc.opt_inner") {
                        self.inc_value_rc(field, inner, count);
                    }
                }
            }

            // Default: value is the RC pointer directly
            _ => self.call_rc_inc_all(&[val], count),
        }
    }

    /// Decrement RC for a value of known type.
    ///
    /// Like [`inc_value_rc`] but generates per-field drop functions and calls
    /// `ori_rc_dec`. Used by [`emit_rc_dec_aggregate`] and
    /// [`emit_inline_enum_dec`](super::ArcIrEmitter::emit_inline_enum_dec).
    pub(super) fn dec_value_rc(&mut self, val: super::ValueId, ty: ori_types::Idx) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            // Scalars and runtime-tagged types: no static RC action
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering
            | Tag::Result
            | Tag::Enum => {}

            // FatValue: data_ptr at field 1
            Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, 1, "rc_dec.data") {
                    let drop_fn = self.get_or_generate_drop_fn(ty);
                    self.call_rc_dec_all(&[dp], drop_fn);
                }
            }

            // Closure: null-check env_ptr, load drop_fn from env header
            Tag::Function => self.emit_rc_dec_closure(val),

            // Collections: use buffer-aware RC dec
            Tag::List | Tag::Set => {
                self.emit_buffer_rc_dec_list_or_set(val, resolved, tag);
            }
            Tag::Map => {
                self.emit_buffer_rc_dec_map(val, resolved);
            }

            // Struct: traverse RC fields, per-field drop functions
            Tag::Struct => {
                let fields = self.pool.struct_fields(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field count bounded by struct definition"
                )]
                for (i, (_, field_ty)) in fields.into_iter().enumerate() {
                    if self.classifier.needs_rc(field_ty) {
                        if let Some(fv) =
                            self.builder
                                .extract_value(val, i as u32, &format!("rc_dec.f.{i}"))
                        {
                            self.dec_value_rc(fv, field_ty);
                        }
                    }
                }
            }

            // Tuple: traverse RC elements
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "element count bounded by tuple arity"
                )]
                for (i, elem_ty) in elems.into_iter().enumerate() {
                    if self.classifier.needs_rc(elem_ty) {
                        if let Some(ev) =
                            self.builder
                                .extract_value(val, i as u32, &format!("rc_dec.e.{i}"))
                        {
                            self.dec_value_rc(ev, elem_ty);
                        }
                    }
                }
            }

            // Option: recurse into inner (same latent bug as inc)
            Tag::Option => {
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    if let Some(field) = self.builder.extract_value(val, 1, "rc_dec.opt_inner") {
                        self.dec_value_rc(field, inner);
                    }
                }
            }

            // Default: value is the RC pointer
            _ => {
                let drop_fn = self.get_or_generate_drop_fn(ty);
                self.call_rc_dec_all(&[val], drop_fn);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Call helpers
    // -----------------------------------------------------------------------

    /// Call `ori_rc_inc(ptr)` for each pointer, `count` times.
    fn call_rc_inc_all(&mut self, ptrs: &[super::ValueId], count: u32) {
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
