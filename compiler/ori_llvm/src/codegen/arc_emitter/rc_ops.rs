//! Per-strategy RC increment/decrement functions.
//!
//! Each [`RcStrategy`] selects a handler whose layout protocol matches the
//! carrier. Inline enums traverse only the active payload; `UserDrop` values call
//! the user destructor without touching a container counter.

use ori_arc::ir::{ArcFunction, ArcVarId, RcStrategy};
use ori_ir::{FIELD_CAP, FIELD_DATA};
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
            // INVARIANT: Ignoring an undefined retain undercounts a live allocation.
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
            // INVARIANT: Ignoring an undefined release leaks a live allocation.
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

        // INVARIANT: Header-only projections and scalar stack yields own no storage.
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
    /// Scalar values carry no RC header or managed fields, so this operation is
    /// counter-neutral and runs only the user destructor.
    fn emit_rc_dec_user_drop(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        // INVARIANT: A UserDrop strategy always resolves a user destructor.
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
        self.emit_inline_user_drop(resolved, val, "user_drop", None);
    }

    /// Emit an inline-value user `@drop` with its recoverable-panic cleanup pad.
    ///
    /// Recoverable Itanium panics enter a cleanup pad. `unwind_rc_walk` supplies
    /// the managed fields still owed before re-raising; `None` denotes a scalar
    /// value with no field cleanup.
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
}
