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
use ori_types::Tag;

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
            // refcount. See TPR-07-008 and `emit_rc_inc_iterator` in
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

            // Struct: traverse RC fields (remap to memory order)
            Tag::Struct => {
                let fields = self.pool.struct_fields(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field count bounded by struct definition"
                )]
                for (i, (_, field_ty)) in fields.into_iter().enumerate() {
                    if self.classifier.needs_rc(field_ty) {
                        let mem_i = self.remap_struct_field(resolved, i as u32);
                        if let Some(fv) =
                            self.builder
                                .extract_value(val, mem_i, &format!("rc_inc.f.{i}"))
                        {
                            self.inc_value_rc(fv, field_ty, count);
                        }
                    }
                }
            }

            // Tuple: traverse RC elements (remap to memory order)
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "element count bounded by tuple arity"
                )]
                for (i, elem_ty) in elems.into_iter().enumerate() {
                    if self.classifier.needs_rc(elem_ty) {
                        let mem_i = self.remap_struct_field(resolved, i as u32);
                        if let Some(ev) =
                            self.builder
                                .extract_value(val, mem_i, &format!("rc_inc.e.{i}"))
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
            // in `rc_ops.rs`. See TPR-07-008.
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

            // Struct: traverse RC fields, per-field drop functions (remap to memory order)
            Tag::Struct => {
                let fields = self.pool.struct_fields(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field count bounded by struct definition"
                )]
                for (i, (_, field_ty)) in fields.into_iter().enumerate() {
                    if self.classifier.needs_rc(field_ty) {
                        let mem_i = self.remap_struct_field(resolved, i as u32);
                        if let Some(fv) =
                            self.builder
                                .extract_value(val, mem_i, &format!("rc_dec.f.{i}"))
                        {
                            self.dec_value_rc(fv, field_ty);
                        }
                    }
                }
            }

            // Tuple: traverse RC elements (remap to memory order)
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(resolved);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "element count bounded by tuple arity"
                )]
                for (i, elem_ty) in elems.into_iter().enumerate() {
                    if self.classifier.needs_rc(elem_ty) {
                        let mem_i = self.remap_struct_field(resolved, i as u32);
                        if let Some(ev) =
                            self.builder
                                .extract_value(val, mem_i, &format!("rc_dec.e.{i}"))
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
}
