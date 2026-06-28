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
use super::field_walk::FieldWalkOps;
use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

/// [`FieldWalkOps`] for an inline aggregate VALUE (struct / tuple field, or
/// enum-payload inline field reached via the value-traversal dec path).
///
/// `base` is the loaded aggregate value; `owner` parameterizes boxed-field
/// detection. Fields are accessed via `extract_value` at the memory-order
/// index carried in `walk`, and dec'd via `dec_value_rc` (value traversal).
pub(super) struct InlineAggregateOps {
    pub(super) base: ValueId,
    pub(super) owner: Idx,
}

impl FieldWalkOps for InlineAggregateOps {
    fn load<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        walk: &[(u32, Idx)],
        idx: usize,
    ) -> Option<(ValueId, bool)> {
        let (mem_i, field_ty) = walk[idx];
        let fv = emitter
            .builder
            .extract_value(self.base, mem_i, &format!("rc_dec.f.{mem_i}"))?;
        let boxed = is_boxed_enum_field(emitter.pool, self.owner, field_ty);
        Some((fv, boxed))
    }

    fn dec_boxed<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        rc_ptr: ValueId,
        field_type: Idx,
    ) {
        let drop_fn = emitter.get_or_generate_drop_fn(field_type);
        emitter.call_rc_dec_all(&[rc_ptr], drop_fn);
    }

    fn dec_children<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        field_value: ValueId,
        field_type: Idx,
    ) {
        emitter.dec_value_rc(field_value, field_type);
    }
}

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
    /// enumeration.
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

            // Option: route through the tag-aware inline-enum path, which loads
            // the discriminant and walks only the live variant's payload — a
            // None incs nothing; a Some incs its inner. Covers boxed AND
            // non-boxed inner; never an un-guarded field-1 payload read.
            Tag::Option => {
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    self.emit_inline_enum_inc(val, resolved, tag, count);
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

            // Option: tag-aware via the shared inline-enum path (lockstep with
            // the inc arm) — a None decs nothing; a Some decs its inner. Never
            // an un-guarded field-1 payload read.
            Tag::Option => {
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    self.emit_inline_enum_dec(val, resolved, tag);
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
        let mut decl_walk: Vec<(u32, Idx)> = Vec::new();
        for (i, &field_ty) in field_types.iter().enumerate() {
            if !self.classifier.needs_rc(field_ty) {
                continue;
            }
            decl_walk.push((self.remap_struct_field(owner, i as u32), field_ty));
        }
        let walk = super::emitter_utils::field_rc_walk_order(&decl_walk, true);
        let ops = InlineAggregateOps { base: val, owner };
        self.dec_fields_may_unwind(&ops, &walk, 0);
    }
}
