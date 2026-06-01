//! Recursive field-by-field RC increment and decrement.
//!
//! [`inc_value_rc`] and [`dec_value_rc`] walk a value's type structure
//! (via Pool tag) to find every RC-managed sub-field and emit the
//! appropriate `ori_rc_inc` / `ori_rc_dec` calls. Used by the
//! `AggregateFields` strategy in [`rc_ops`](super::rc_ops) for
//! struct/tuple field traversal, and by
//! [`emit_inline_enum_dec`](super::ArcIrEmitter::emit_inline_enum_dec)
//! for per-variant cleanup.

use ori_ir::{FIELD_CAP, FIELD_DATA};
use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Increment RC for a value of known type.
    ///
    /// Dispatches by Pool tag to extract the correct data pointer(s) and call
    /// `ori_rc_inc` on each. Handles nested aggregates recursively.
    ///
    /// Used by [`emit_rc_inc_aggregate`](Self::emit_rc_inc_aggregate) for
    /// struct/tuple field traversal. This replaces the
    /// `extract_rc_data_ptrs` → `ori_rc_inc` loop pattern for RC
    /// operations. Pool queries are used for type tags and field
    /// enumeration; these will be eliminated in Section 01.4.
    pub(super) fn inc_value_rc(&mut self, val: super::ValueId, ty: ori_types::Idx, count: u32) {
        ori_stack::ensure_sufficient_stack(|| self.inc_value_rc_inner(val, ty, count));
    }

    fn inc_value_rc_inner(&mut self, val: super::ValueId, ty: ori_types::Idx, count: u32) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            // Scalars: no RC action
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
            | Tag::Ordering => {}

            // Iterators (Box-allocated, no RC header): Inc is a no-op.
            // Iterators are unique-owned — they are moved through
            // `iter_next`, never copied — so there is nothing to
            // refcount. See and `emit_rc_inc_iterator` in
            // `rc_ops.rs`.
            Tag::Iterator | Tag::DoubleEndedIterator => {
                let _ = val;
                let _ = count;
                tracing::trace!(?tag, "inc_value_rc on iterator — no-op (unique ownership)");
            }

            // Result/Enum: tag-switch per variant, inc RC children
            Tag::Result | Tag::Enum => {
                self.emit_inline_enum_inc(val, resolved, tag, count);
            }

            // Str: slice-aware RC inc via ori_str_rc_inc(data, cap)
            // Handles SSO, heap, and seamless slices from str.split().
            Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, FIELD_DATA, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, FIELD_CAP, "rc_inc.str_cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    self.call_str_rc_inc(dp, cap, count);
                }
            }

            // Closure: null-check env_ptr
            Tag::Function => self.emit_rc_inc_closure(val, count),

            // Collections: slice-aware RC inc via ori_list_rc_inc(data, cap)
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
            Tag::Map => {
                // Single data buffer at field 2 — same as List/Set
                if let Some(dp) = self.builder.extract_value(val, FIELD_DATA, "rc_inc.data") {
                    self.call_rc_inc_all(&[dp], count);
                }
            }

            // Struct/Tuple: traverse RC fields/elements (remap to memory order)
            Tag::Struct => {
                let fields: Vec<Idx> = self
                    .pool
                    .struct_fields(resolved)
                    .into_iter()
                    .map(|(_, t)| t)
                    .collect();
                self.inc_aggregate_fields(val, resolved, &fields, count);
            }
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(resolved);
                self.inc_aggregate_fields(val, resolved, &elems, count);
            }

            // Option: recurse into inner type at field 1
            // NOTE: latent bug — doesn't check runtime tag. If value is None,
            // field 1 is uninitialized. This matches the existing behavior and
            // is tracked in the plan (Section 01.5 test items).
            Tag::Option => {
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    if is_boxed_enum_field(self.pool, resolved, inner) {
                        // Boxed recursive inner: the None payload slot holds the
                        // niche/tag, not a pointer. Route through the tag-aware
                        // inline path so only Some incs the box pointer.
                        self.emit_inline_enum_inc(val, resolved, tag, count);
                    } else if let Some(field) =
                        self.builder.extract_value(val, 1, "rc_inc.opt_inner")
                    {
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
    /// Like [`inc_value_rc`](Self::inc_value_rc) but generates per-field
    /// drop functions and calls `ori_rc_dec`. Used by
    /// [`emit_rc_dec_aggregate`](Self::emit_rc_dec_aggregate) and
    /// [`emit_inline_enum_dec`](super::ArcIrEmitter::emit_inline_enum_dec).
    pub(super) fn dec_value_rc(&mut self, val: super::ValueId, ty: ori_types::Idx) {
        ori_stack::ensure_sufficient_stack(|| self.dec_value_rc_inner(val, ty));
    }

    fn dec_value_rc_inner(&mut self, val: super::ValueId, ty: ori_types::Idx) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            // Scalars: no RC action
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
            | Tag::Ordering => {}

            // Iterators: call `ori_iter_drop(ptr)` to free the
            // Box-allocated state. There is no RC header to decrement,
            // so `ori_rc_dec` would corrupt memory by reading a
            // non-existent refcount. This arm is reached for iterator
            // *fields* inside compound types (struct, tuple, enum
            // variants); the direct `RcDec` dispatch for top-level
            // iterator variables goes through `RcStrategy::Iterator`
            // in `rc_ops.rs`. See
            Tag::Iterator | Tag::DoubleEndedIterator => {
                self.call_iter_drop(val);
            }

            // Result/Enum: tag-switch per variant, dec RC children
            Tag::Result | Tag::Enum => {
                self.emit_inline_enum_dec(val, resolved, tag);
            }

            // Str: slice-aware RC dec via ori_str_rc_dec(data, cap, drop_fn)
            // Handles SSO, heap, and seamless slices from str.split().
            Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, FIELD_DATA, "rc_dec.data") {
                    let cap = self
                        .builder
                        .extract_value(val, FIELD_CAP, "rc_dec.str_cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    let drop_fn = self.get_or_generate_drop_fn(ty);
                    self.call_str_rc_dec(dp, cap, drop_fn);
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

            // Struct/Tuple: traverse RC fields/elements (remap to memory order)
            Tag::Struct => {
                let fields: Vec<Idx> = self
                    .pool
                    .struct_fields(resolved)
                    .into_iter()
                    .map(|(_, t)| t)
                    .collect();
                self.dec_aggregate_fields(val, resolved, &fields);
            }
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(resolved);
                self.dec_aggregate_fields(val, resolved, &elems);
            }

            // Option: recurse into inner (same latent bug as inc)
            Tag::Option => {
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    if is_boxed_enum_field(self.pool, resolved, inner) {
                        // Boxed recursive inner: the None payload slot holds the
                        // niche/tag, not a pointer. Route through the tag-aware
                        // inline path so only Some decs the box pointer.
                        self.emit_inline_enum_dec(val, resolved, tag);
                    } else if let Some(field) =
                        self.builder.extract_value(val, 1, "rc_dec.opt_inner")
                    {
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

    /// Inc RC for each RC-managed field/element of an aggregate (struct/tuple).
    ///
    /// `owner` is the fully-resolved aggregate `Idx`; `field_types` is the
    /// declaration-order list. A boxed recursive field's slot holds the RC box
    /// pointer directly (inc it); a non-boxed field recurses.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field/element count bounded by aggregate definition"
    )]
    fn inc_aggregate_fields(
        &mut self,
        val: super::ValueId,
        owner: Idx,
        field_types: &[Idx],
        count: u32,
    ) {
        for (i, &field_ty) in field_types.iter().enumerate() {
            if !self.classifier.needs_rc(field_ty) {
                continue;
            }
            let mem_i = self.remap_struct_field(owner, i as u32);
            let Some(fv) = self
                .builder
                .extract_value(val, mem_i, &format!("rc_inc.f.{i}"))
            else {
                continue;
            };
            if is_boxed_enum_field(self.pool, owner, field_ty) {
                self.call_rc_inc_all(&[fv], count);
            } else {
                self.inc_value_rc(fv, field_ty, count);
            }
        }
    }

    /// Dec RC for each RC-managed field/element of an aggregate (struct/tuple).
    ///
    /// A boxed recursive field's slot holds the RC box pointer directly (dec it
    /// through the child drop fn); a non-boxed field recurses.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field/element count bounded by aggregate definition"
    )]
    fn dec_aggregate_fields(&mut self, val: super::ValueId, owner: Idx, field_types: &[Idx]) {
        // Build the REVERSE declaration-order (LIFO) RC-field walk per
        // `drop-trait-proposal.md §Drop and panic` — matches the heap drop-fn
        // walk (`emit_drop_fields`) so user `@drop` side effects observe the
        // same field-teardown order. `(memory_index, field_type)` per field.
        let mut walk: Vec<(u32, Idx)> = Vec::new();
        for (i, &field_ty) in field_types.iter().enumerate().rev() {
            if !self.classifier.needs_rc(field_ty) {
                continue;
            }
            walk.push((self.remap_struct_field(owner, i as u32), field_ty));
        }
        self.dec_field_walk(val, owner, &walk, 0);
    }

    /// Drop the aggregate's RC fields `walk[start..]` with PER-FIELD
    /// sibling-continue: when a field's own user `@drop` may unwind (Itanium),
    /// it is `invoke`d so a panic routes to a cleanup pad that still drops the
    /// panicking field's children + every remaining sibling field, then
    /// `resume`s the foreign exception. A field whose `@drop` cannot unwind
    /// (scalar/no-`@drop`/non-Itanium) takes the plain path.
    fn dec_field_walk(
        &mut self,
        val: super::ValueId,
        owner: Idx,
        walk: &[(u32, Idx)],
        start: usize,
    ) {
        let mut j = start;
        while j < walk.len() {
            let (mem_i, field_ty) = walk[j];
            let Some(fv) = self
                .builder
                .extract_value(val, mem_i, &format!("rc_dec.f.{mem_i}"))
            else {
                j += 1;
                continue;
            };
            if is_boxed_enum_field(self.pool, owner, field_ty) {
                // Boxed field: its `_ori_drop$<field_ty>` carries the @drop.
                let drop_fn = self.get_or_generate_drop_fn(field_ty);
                self.call_rc_dec_all(&[fv], drop_fn);
                j += 1;
                continue;
            }
            let unwinds = self.user_drop_method(field_ty).is_some()
                && self.drop_may_unwind(field_ty)
                && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium;
            if unwinds {
                let cont = self.builder.append_block(self.current_function, "fld.cont");
                let cleanup = self
                    .builder
                    .append_block(self.current_function, "fld.cleanup");
                if self.invoke_user_drop_for_inline_value(field_ty, fv, cont, cleanup) {
                    // Cleanup pad: drop the panicking field's own RC children +
                    // every remaining sibling field (plain — a nested panic
                    // aborts via the drop-cleanup-depth guard), then resume.
                    self.builder.position_at_end(cleanup);
                    let personality = self.builder.runtime_fn("ori_eh_personality");
                    let lp = self.builder.landingpad(personality, true, "fld.lp");
                    let enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
                    self.builder.call(enter, &[], "");
                    self.dec_value_rc(fv, field_ty);
                    self.dec_field_walk_plain(val, owner, walk, j + 1);
                    let exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
                    self.builder.call(exit, &[], "");
                    self.builder.resume(lp);
                    // Normal continuation: this field's children, then advance.
                    self.builder.position_at_end(cont);
                    self.dec_value_rc(fv, field_ty);
                    j += 1;
                    continue;
                }
            }
            // Plain field: run its user `@drop` (if any, non-unwinding) before
            // recursing into its own field walk.
            self.emit_user_drop_for_inline_value(field_ty, fv);
            self.dec_value_rc(fv, field_ty);
            j += 1;
        }
    }

    /// Plain (non-`invoke`) drop of `walk[start..]` — used inside a cleanup pad
    /// where a further (nested) panic must abort via the depth guard, not
    /// re-enter sibling-continue.
    fn dec_field_walk_plain(
        &mut self,
        val: super::ValueId,
        owner: Idx,
        walk: &[(u32, Idx)],
        start: usize,
    ) {
        for &(mem_i, field_ty) in &walk[start..] {
            let Some(fv) = self
                .builder
                .extract_value(val, mem_i, &format!("rc_dec.cl.f.{mem_i}"))
            else {
                continue;
            };
            if is_boxed_enum_field(self.pool, owner, field_ty) {
                let drop_fn = self.get_or_generate_drop_fn(field_ty);
                self.call_rc_dec_all(&[fv], drop_fn);
            } else {
                self.emit_user_drop_for_inline_value(field_ty, fv);
                self.dec_value_rc(fv, field_ty);
            }
        }
    }
}
