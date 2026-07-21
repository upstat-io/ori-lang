//! Per-strategy RC increment/decrement functions.
//!
//! Each [`RcStrategy`] variant has a dedicated `emit_rc_inc_*` and
//! `emit_rc_dec_*` function. These replace the monolithic Pool-querying
//! handlers emitted from `emit_instr`.
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
//! | `Iterator`        | `emit_rc_inc_iterator`   | `emit_rc_dec_iterator`    |
//! | `UserDrop`        | *(no-op)*                | `emit_rc_dec_user_drop`   |
//!
//! `UserDrop` Inc is a no-op (a scalar value carries no RC header); Dec routes
//! to `emit_rc_dec_user_drop`, which emits ONLY the user `@drop` call — no field
//! walk, no `ori_rc_dec`.
//!
//! # `InlineEnum`
//!
//! `InlineEnum` Inc and Dec both perform a tag-switch with per-variant
//! field traversal. The container itself is stack-allocated (no container
//! refcount), but inner RC-typed fields need inc/dec for correct sharing.
//!
//! # Design: strategy handlers extract directly; the heap-fallback delegates
//!
//! Each named strategy below knows its own layout and extracts pointers
//! directly. Only the `emit_rc_inc_heap` `_` catch-all (for heap types without a
//! named strategy) delegates to `extract_rc_data_ptrs`; an `Option`/`Result`/`Enum`
//! value never reaches that catch-all — it routes through `InlineEnum` (the
//! tag-aware `emit_inline_enum_inc`/`_dec`):
//!
//! - `HeapPointer`: slice-aware for List/Set (data+cap → `ori_list_rc_inc`); Map field 2
//! - `FatPointer`: always field 1 (the `data_ptr` half)
//! - `Closure`: field 1 (`env_ptr`) with null-check
//! - `AggregateFields`: struct/tuple field traversal via [`inc_value_rc`] / [`dec_value_rc`]
//! - `InlineEnum`: Inc via `emit_inline_enum_inc`; Dec via `emit_inline_enum_dec` (both tag-switch)
//!
//! `extract_rc_data_ptrs` remains in `mod.rs` for non-RC uses (closure env
//! drop, drop function generation, builtin clone).
//!
//! Pool queries for physical tags and field enumeration are a migration gap.
//! `ValueRepr` remains a logical ownership carrier; `CompiledLayoutPlan` must
//! make extraction width, offsets, and encoding explicit for this projection.

use ori_arc::ir::{ArcFunction, ArcVarId, RcStrategy};
use ori_ir::{CLOSURE_FIELD_ENV, FIELD_CAP, FIELD_DATA};
use ori_types::{Idx, Tag};

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // Dispatch

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
            // An RcInc on an undefined variable is an upstream invariant
            // violation (use-before-def in realized ARC IR). Skipping it
            // silently under-counts the reference and frees live memory;
            // record a codegen error so the compile fails loudly instead.
            tracing::error!(
                var = var.raw(),
                ?strategy,
                "RcInc on undefined variable — realized ARC IR use-before-def"
            );
            self.builder.record_codegen_error_with_msg(format!(
                "RcInc on undefined variable v{} ({strategy:?}) — realized ARC IR use-before-def",
                var.raw()
            ));
            return;
        }

        // Why: debug-only cross-check that the instruction's strategy matches the
        // Pool-derived expectation; `UserDrop` is excluded because `from_repr`
        // rejects Scalar repr and never produces it.
        #[cfg(debug_assertions)]
        if strategy != RcStrategy::UserDrop {
            if let Some(repr) = func.var_repr(var) {
                let expected = RcStrategy::from_repr(repr, self.pool, func.var_type(var));
                debug_assert_eq!(
                    strategy, expected,
                    "RcStrategy mismatch for var {var:?}: instruction has {strategy:?}, Pool says {expected:?}",
                );
            }
        }

        match strategy {
            RcStrategy::HeapPointer => self.emit_rc_inc_heap(var, count, func),
            RcStrategy::FatPointer => self.emit_rc_inc_fat(var, count),
            RcStrategy::Closure => self.emit_rc_inc_closure(self.var(var), count),
            RcStrategy::AggregateFields => self.emit_rc_inc_aggregate(var, count, func),
            RcStrategy::InlineEnum => self.emit_rc_inc_inline_enum(var, count, func),
            RcStrategy::Iterator => self.emit_rc_inc_iterator(var, count),
            // A scalar-repr value carrying a user `@drop` has no RC header —
            // inc is a no-op (Spec: Annex E §AIMS RL-DROP, balance-neutral).
            RcStrategy::UserDrop => {}
        }
    }

    /// Dispatch an RC decrement to the appropriate per-strategy handler.
    pub(super) fn emit_rc_dec(&mut self, var: ArcVarId, strategy: RcStrategy, func: &ArcFunction) {
        let val = self.var(var);
        if val.is_none() {
            // An RcDec on an undefined variable is an upstream invariant
            // violation (use-before-def in realized ARC IR). Skipping it
            // silently leaks the allocation; record a codegen error so the
            // compile fails loudly instead.
            tracing::error!(
                var = var.raw(),
                ?strategy,
                "RcDec on undefined variable — realized ARC IR use-before-def"
            );
            self.builder.record_codegen_error_with_msg(format!(
                "RcDec on undefined variable v{} ({strategy:?}) — realized ARC IR use-before-def",
                var.raw()
            ));
            return;
        }

        // Qualified length projections return a header-only physical value
        // with null data, and scalar stack yields own neither a heap buffer nor
        // element destructors. Their logical release therefore has no physical
        // operation; non-scalar stack yields retain the ordinary drop walk.
        if self.is_length_projection_result(func, var)
            || self.is_scalar_stack_slot_yield_receiver(func, var)
        {
            return;
        }

        // Why: debug-only cross-check that the instruction's strategy matches the
        // Pool-derived expectation; `UserDrop` is excluded because `from_repr`
        // rejects Scalar repr and never produces it.
        #[cfg(debug_assertions)]
        if strategy != RcStrategy::UserDrop {
            if let Some(repr) = func.var_repr(var) {
                let expected = RcStrategy::from_repr(repr, self.pool, func.var_type(var));
                debug_assert_eq!(
                    strategy, expected,
                    "RcStrategy mismatch for var {var:?}: instruction has {strategy:?}, Pool says {expected:?}",
                );
            }
        }

        match strategy {
            RcStrategy::HeapPointer => self.emit_rc_dec_heap(var, func),
            RcStrategy::FatPointer => self.emit_rc_dec_fat(var, func),
            RcStrategy::Closure => self.emit_rc_dec_closure(self.var(var)),
            RcStrategy::AggregateFields => self.emit_rc_dec_aggregate(var, func),
            RcStrategy::InlineEnum => self.emit_rc_dec_inline_enum(var, func),
            RcStrategy::Iterator => self.emit_rc_dec_iterator(var),
            RcStrategy::UserDrop => self.emit_rc_dec_user_drop(var, func),
        }
    }

    // UserDrop handlers

    /// Dec a scalar-repr value whose type carries a user `@drop`.
    ///
    /// The value has `ValueRepr::Scalar` — no RC header, no RC fields — so this
    /// emits ONLY the user `@drop` CALL: an `invoke` to a re-raise-only cleanup
    /// pad on the recoverable-panic path (Itanium), a plain call otherwise.
    /// There is NO `dec_value_rc` field walk and NO `ori_rc_dec`, so the op is
    /// reference-count-neutral. Spec: Annex E §AIMS RL-DROP
    /// (`RLDROP_scalar_lifecycle_sound`).
    fn emit_rc_dec_user_drop(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        // The UserDrop strategy is assigned (in burden lowering) ONLY for a
        // type carrying a user `@drop`; a missing method here is an upstream
        // invariant violation (a `UserDrop` strategy on a non-Drop type).
        // Record a codegen error so the compile fails loudly rather than
        // silently dropping the op (no invisible gaps) — mirrors the
        // use-before-def handler in `emit_rc_dec`.
        if self.user_drop_method(resolved).is_none() {
            let mut registered: Vec<_> = self
                .ctx
                .user_drop_functions
                .keys()
                .map(|candidate| candidate.raw())
                .collect();
            registered.sort_unstable();
            tracing::error!(
                var = var.raw(),
                ty = ty.raw(),
                resolved = resolved.raw(),
                registered = ?registered,
                "RcDec UserDrop on a type with no user @drop — realized ARC IR invariant violation"
            );
            self.builder.record_codegen_error_with_msg(format!(
                "RcDec UserDrop on v{} whose type v{} resolves to v{}, but the executable user-drop table contains {:?} — realized ARC IR invariant violation",
                var.raw(),
                ty.raw(),
                resolved.raw(),
                registered,
            ));
            return;
        }
        // Scalar repr — no RC fields to free on the unwind path, so the cleanup
        // pad re-raises directly (`unwind_rc_walk = None`).
        self.emit_inline_user_drop(resolved, val, "user_drop", None);
    }

    /// Emit an inline-value user `@drop` with its recoverable-panic cleanup pad.
    ///
    /// `block_prefix` names the `cont`/`cleanup`/landingpad blocks. A may-unwind
    /// `@drop` on the Itanium model is `invoke`d with a cleanup pad: `unwind_rc_walk
    /// = Some(ty)` walks the value's RC fields (a heap-field aggregate whose
    /// `@drop` panicked still owes its field decs, fenced by
    /// `ori_drop_cleanup_enter`/`exit`); `None` leaves a bare re-raise (scalar
    /// repr has no RC fields). The `@drop` is reference-count-neutral.
    /// Spec: Annex E §AIMS RL-DROP (`RLDROP_scalar_lifecycle_sound`).
    fn emit_inline_user_drop(
        &mut self,
        resolved: Idx,
        val: super::ValueId,
        block_prefix: &str,
        unwind_rc_walk: Option<Idx>,
    ) {
        let itanium = self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium;
        let unwinds = self.drop_may_unwind(resolved) && itanium;
        if unwinds {
            let cont = self
                .builder
                .append_block(self.current_function, &format!("{block_prefix}.cont"));
            let cleanup = self
                .builder
                .append_block(self.current_function, &format!("{block_prefix}.cleanup"));
            if self.invoke_user_drop_for_inline_value(resolved, val, cont, cleanup) {
                self.builder.position_at_end(cleanup);
                let personality = self.builder.runtime_fn("ori_eh_personality");
                let lp = self
                    .builder
                    .landingpad(personality, true, &format!("{block_prefix}.lp"));
                if let Some(walk_ty) = unwind_rc_walk {
                    let enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
                    self.builder.call(enter, &[], "");
                    self.dec_value_rc(val, walk_ty);
                    let exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
                    self.builder.call(exit, &[], "");
                }
                self.builder.resume(lp);

                self.builder.position_at_end(cont);
            }
        } else {
            self.emit_user_drop_for_inline_value(resolved, val);
        }
    }

    // HeapPointer handlers

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
                if let Some(dp) = self.builder.extract_value(val, FIELD_DATA, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, FIELD_CAP, "rc_inc.cap")
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

    // AggregateFields handlers

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
        let resolved = self.pool.resolve_fully(ty);
        if self.user_drop_method(resolved).is_some() {
            // Heap-field-bearing aggregate: run the `@drop` first, walking the
            // RC fields on the unwind path (`unwind_rc_walk = Some(ty)`).
            self.emit_inline_user_drop(resolved, val, "agg_drop", Some(ty));
        }
        self.dec_value_rc(val, ty);
    }

    // Closure handlers

    /// Inc a closure (`{fn_ptr, env_ptr}`).
    ///
    /// Extract `env_ptr` (field 1), null-check (zero-capture closures have
    /// null env), then call `ori_rc_inc` on the non-null env.
    pub(super) fn emit_rc_inc_closure(&mut self, val: super::ValueId, count: u32) {
        let func_id = self.builder.runtime_fn("ori_rc_inc");

        let Some(env_ptr) = self
            .builder
            .extract_value(val, CLOSURE_FIELD_ENV, "rc_inc.env")
        else {
            return;
        };

        // Non-capturing closures have a constant null env pointer — skip.
        if self.builder.is_const_null_ptr(env_ptr) {
            return;
        }

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
            self.emit_rt_call(func_id, &[env_ptr], "");
        }
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    /// Dec a closure (`{fn_ptr, env_ptr}`).
    ///
    /// Extract `env_ptr`, null-check, then load the drop function pointer
    /// from the env header and call `ori_rc_dec(env_ptr, drop_fn)`.
    pub(super) fn emit_rc_dec_closure(&mut self, val: super::ValueId) {
        let Some(env_ptr) = self
            .builder
            .extract_value(val, CLOSURE_FIELD_ENV, "rc_dec.env")
        else {
            return;
        };

        // Non-capturing closures have a constant null env pointer —
        // skip the entire RcDec (no blocks, no branches, no dead code).
        if self.builder.is_const_null_ptr(env_ptr) {
            return;
        }

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
        self.emit_rt_call(func_id, &[env_ptr, drop_fn], "");
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    // InlineEnum handlers

    /// Inc an inline enum — tag-switch with per-variant RC field inc.
    ///
    /// Inline enums (Result, Enum, Option) are stack-allocated, so there
    /// is no container refcount. But their RC-typed fields (strings, lists,
    /// recursive pointers, etc.) must be incremented when the value is
    /// shared. Mirrors `emit_rc_dec_inline_enum` structurally.
    fn emit_rc_inc_inline_enum(&mut self, var: ArcVarId, count: u32, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let pool_tag = self.pool.tag(resolved);
        self.emit_inline_enum_inc(val, resolved, pool_tag, count);
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

    // Iterator handlers

    /// `Inc` for an iterator handle is a **no-op**.
    ///
    /// Iterators are Box-allocated with no RC header, and in idiomatic
    /// Ori they are moved (not copied) — each `iter_next` call consumes
    /// the old handle and returns a new one. If `RcInc` is ever emitted
    /// for an iterator, something upstream tried to duplicate a value
    /// that has unique ownership semantics; there is no refcount header
    /// to bump. We don't emit a runtime call so we don't corrupt
    /// memory, but we leave a trace event so the situation is
    /// discoverable during debugging.
    #[expect(
        clippy::unused_self,
        reason = "part of the strategy-dispatch API on ArcIrEmitter — called by emit_rc_inc() alongside every other emit_rc_inc_<strategy> method"
    )]
    fn emit_rc_inc_iterator(&mut self, var: ArcVarId, count: u32) {
        tracing::trace!(
            var = var.raw(),
            count,
            "RcInc on iterator handle — no-op (iterators are move-only)"
        );
    }

    /// `Dec` for an iterator handle: emit `ori_iter_drop(ptr)`.
    ///
    /// The runtime function frees the Box-allocated iterator state.
    /// There is no refcount header, so `ori_rc_dec` would corrupt
    /// memory by reading a non-existent header — we bypass that path
    /// entirely.
    fn emit_rc_dec_iterator(&mut self, var: ArcVarId) {
        let val = self.var(var);
        self.call_iter_drop(val);
    }

    /// Call `ori_iter_drop(ptr)` — frees Box-allocated iterator state.
    pub(super) fn call_iter_drop(&mut self, ptr: super::ValueId) {
        let func_id = self.builder.runtime_fn("ori_iter_drop");
        self.emit_rt_call(func_id, &[ptr], "");
    }

    // Call helpers

    /// Call `ori_rc_inc(ptr)` for each pointer, `count` times.
    pub(super) fn call_rc_inc_all(&mut self, ptrs: &[super::ValueId], count: u32) {
        if ptrs.is_empty() {
            return;
        }
        let func_id = self.builder.runtime_fn("ori_rc_inc");
        for &ptr in ptrs {
            for _ in 0..count {
                self.emit_rt_call(func_id, &[ptr], "");
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
            self.emit_rt_call(func_id, &[data, cap], "");
        }
    }

    /// Call `ori_rc_dec(ptr, drop_fn)` for each pointer.
    pub(super) fn call_rc_dec_all(&mut self, ptrs: &[super::ValueId], drop_fn: super::ValueId) {
        if ptrs.is_empty() {
            return;
        }
        let func_id = self.builder.runtime_fn("ori_rc_dec");
        for &ptr in ptrs {
            self.emit_rt_call(func_id, &[ptr, drop_fn], "");
        }
    }

    /// Emit a may-unwind RC dec of a boxed-recursive child as an `invoke` of
    /// `ori_rc_dec_unwind` → `normal_bb` / `cleanup_bb` (Itanium recoverable
    /// path). `ori_rc_dec_unwind` calls `drop_fn` directly (no `catch_unwind`),
    /// so a panicking user `@drop` inside the child's drop tree unwinds to
    /// `cleanup_bb` instead of aborting at the plain `ori_rc_dec` boundary.
    /// Caller owns both blocks + the post-invoke continuation.
    pub(super) fn invoke_rc_dec_unwind(
        &mut self,
        ptr: super::ValueId,
        drop_fn: super::ValueId,
        normal_bb: crate::codegen::value_id::BlockId,
        cleanup_bb: crate::codegen::value_id::BlockId,
    ) {
        let func_id = self.builder.runtime_fn("ori_rc_dec_unwind");
        self.builder
            .invoke(func_id, &[ptr, drop_fn], normal_bb, cleanup_bb, "");
    }

    /// Call `ori_str_rc_inc(data_ptr, cap)` — handles SSO, heap, and slices.
    pub(super) fn call_str_rc_inc(
        &mut self,
        data_ptr: super::ValueId,
        cap: super::ValueId,
        count: u32,
    ) {
        let func_id = self.builder.runtime_fn("ori_str_rc_inc");
        for _ in 0..count {
            self.emit_rt_call(func_id, &[data_ptr, cap], "");
        }
    }

    /// Call `ori_str_rc_dec(data_ptr, cap, drop_fn)` — handles SSO, heap, and slices.
    pub(super) fn call_str_rc_dec(
        &mut self,
        data_ptr: super::ValueId,
        cap: super::ValueId,
        drop_fn: super::ValueId,
    ) {
        let func_id = self.builder.runtime_fn("ori_str_rc_dec");
        self.emit_rt_call(func_id, &[data_ptr, cap, drop_fn], "");
    }
}
