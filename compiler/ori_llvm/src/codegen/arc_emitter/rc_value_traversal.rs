//! Recursive field-by-field RC increment and decrement.
//!
//! [`inc_value_rc`] and [`dec_value_rc`] walk a value's type structure
//! (via Pool tag) to find every RC-managed sub-field and emit the
//! appropriate `ori_rc_inc` / `ori_rc_dec` calls. Used by the
//! `AggregateFields` strategy in [`rc_ops`](super::rc_ops) for
//! struct/tuple field traversal, and by
//! [`emit_inline_enum_dec`](super::ArcIrEmitter::emit_inline_enum_dec)
//! for per-variant cleanup.

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

            // Str: SSO check + RC inc on heap data pointer
            Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    let do_inc = self
                        .builder
                        .append_block(self.current_function, "rc_inc.str_heap");
                    let skip = self
                        .builder
                        .append_block(self.current_function, "rc_inc.str_skip");
                    let is_sso = self.emit_sso_check(dp, "rc_inc.str");
                    self.builder.cond_br(is_sso, skip, do_inc);

                    self.builder.position_at_end(do_inc);
                    self.call_rc_inc_all(&[dp], count);
                    self.builder.br(skip);

                    self.builder.position_at_end(skip);
                }
            }

            // Closure: null-check env_ptr
            Tag::Function => self.emit_rc_inc_closure(val, count),

            // Collections: slice-aware RC inc via ori_list_rc_inc(data, cap)
            Tag::List | Tag::Set => {
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
            Tag::Map => {
                // Single data buffer at field 2 — same as List/Set
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    self.call_rc_inc_all(&[dp], count);
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

            // Str: SSO check + RC dec on heap data pointer
            Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_dec.data") {
                    let do_dec = self
                        .builder
                        .append_block(self.current_function, "rc_dec.str_heap");
                    let skip = self
                        .builder
                        .append_block(self.current_function, "rc_dec.str_skip");
                    let is_sso = self.emit_sso_check(dp, "rc_dec.str");
                    self.builder.cond_br(is_sso, skip, do_dec);

                    self.builder.position_at_end(do_dec);
                    let drop_fn = self.get_or_generate_drop_fn(ty);
                    self.call_rc_dec_all(&[dp], drop_fn);
                    self.builder.br(skip);

                    self.builder.position_at_end(skip);
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
}
