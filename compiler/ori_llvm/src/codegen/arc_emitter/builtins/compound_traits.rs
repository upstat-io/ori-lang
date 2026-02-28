//! Trait method codegen for compound types (Option, Result, Tuple, List).
//!
//! Implements `equals`, `compare`, and `hash` for compound types by
//! structural recursion into element types. Each method dispatches to
//! the element's trait implementation via `emit_element_*` helpers.
//!
//! ## ARC enum convention
//!
//! - **Option**: `{i64 tag, T payload}` — Some=0, None=1
//! - **Result**: `{i64 tag, payload}`   — Ok=0, Err=1
//! - **Tuple**:  `{A, B, ...}`         — flat struct of resolved element types
//! - **List**:   `{i64 len, i64 cap, ptr data}` — element-wise iteration

declare_builtins! { emitter, ctx;
    // list structural trait methods
    ("list", "equals", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "is_equal", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "compare", borrow: true) => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_compare(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "hash", borrow: true) => emitter.emit_element_hash(ctx.arg_vals[0], ctx.receiver_ty),
    // Option structural trait methods
    ("Option", "equals", borrow: true) => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_equals(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "is_equal", borrow: true) => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_equals(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "compare", borrow: true) => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_compare(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "hash", borrow: true) => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            emitter.emit_option_hash(ctx.arg_vals[0], *inner)
        } else {
            None
        }
    },
    // Result structural trait methods
    ("Result", "equals", borrow: true) => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_equals(ctx.arg_vals[0], ctx.arg_vals[1], *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "is_equal", borrow: true) => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_equals(ctx.arg_vals[0], ctx.arg_vals[1], *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "compare", borrow: true) => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_compare(ctx.arg_vals[0], ctx.arg_vals[1], *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "hash", borrow: true) => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            emitter.emit_result_hash(ctx.arg_vals[0], *ok, *err)
        } else {
            None
        }
    },
    // tuple structural trait methods
    ("tuple", "clone", borrow: true) => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("tuple", "equals", borrow: true) => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_tuple_equals(ctx.arg_vals[0], ctx.arg_vals[1], elements)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("tuple", "compare", borrow: true) => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_tuple_compare(ctx.arg_vals[0], ctx.arg_vals[1], elements)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("tuple", "hash", borrow: true) => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            emitter.emit_tuple_hash(ctx.arg_vals[0], elements)
        } else {
            None
        }
    },
}

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Recursive element dispatch
    // -----------------------------------------------------------------------

    /// Emit `lhs.equals(rhs)` for any type, dispatching recursively.
    pub(crate) fn emit_element_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        match &type_info {
            TypeInfo::Int
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Ordering => Some(self.builder.icmp_eq(lhs, rhs, "elem_eq")),
            TypeInfo::Float => Some(self.builder.fcmp_oeq(lhs, rhs, "elem_eq")),
            TypeInfo::Str => Some(self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false)),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_equals(lhs, rhs, inner)
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_equals(lhs, rhs, ok, err)
            }
            TypeInfo::Tuple { elements } => self.emit_tuple_equals(lhs, rhs, elements),
            TypeInfo::List { element } => {
                let elem = *element;
                self.emit_list_equals(lhs, rhs, elem)
            }
            _ => None,
        }
    }

    /// Emit `lhs.compare(rhs)` for any type, dispatching recursively.
    ///
    /// Returns Ordering as i8 (Less=0, Equal=1, Greater=2).
    pub(crate) fn emit_element_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        match &type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                Some(self.builder.emit_icmp_ordering(lhs, rhs, "elem_cmp", true))
            }
            TypeInfo::Float => Some(self.builder.emit_fcmp_ordering(lhs, rhs, "elem_cmp")),
            TypeInfo::Bool | TypeInfo::Char | TypeInfo::Byte => {
                Some(self.builder.emit_icmp_ordering(lhs, rhs, "elem_cmp", false))
            }
            TypeInfo::Str => self.emit_str_compare_call(lhs, rhs),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_compare(lhs, rhs, inner)
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_compare(lhs, rhs, ok, err)
            }
            TypeInfo::Tuple { elements } => self.emit_tuple_compare(lhs, rhs, elements),
            TypeInfo::List { element } => {
                let elem = *element;
                self.emit_list_compare(lhs, rhs, elem)
            }
            _ => None,
        }
    }

    /// Emit `val.hash()` for any type, dispatching recursively.
    pub(crate) fn emit_element_hash(&mut self, val: ValueId, elem_ty: Idx) -> Option<ValueId> {
        let type_info = self.type_info.get(elem_ty);
        let i64_ty = self.builder.i64_type();
        match &type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => Some(val),
            TypeInfo::Float => {
                let arg_vals = [val];
                self.emit_trait_method("hash", &arg_vals, &type_info)
            }
            TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Ordering => {
                Some(self.builder.zext(val, i64_ty, "elem_hash"))
            }
            TypeInfo::Char => Some(self.builder.sext(val, i64_ty, "elem_hash")),
            TypeInfo::Str => self.emit_str_hash_call(val),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_hash(val, inner)
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_hash(val, ok, err)
            }
            TypeInfo::Tuple { elements } => self.emit_tuple_hash(val, elements),
            TypeInfo::List { element } => {
                let elem = *element;
                self.emit_list_hash(val, elem)
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Option trait methods
    // -----------------------------------------------------------------------

    /// `Option<T>.equals(other) -> bool`
    ///
    /// Tags differ → false. Both None → true. Both Some → payload equals.
    pub(super) fn emit_option_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "opt.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "opt.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        let lhs_val = self.builder.extract_value(lhs, 1, "opt.lhs.val")?;
        let rhs_val = self.builder.extract_value(rhs, 1, "opt.rhs.val")?;
        let payload_eq = self.emit_element_equals(lhs_val, rhs_val, inner_ty)?;

        // Both None (tag=1): equal. Both Some (tag=0): check payload.
        let one = self.builder.const_i64(1);
        let is_none = self.builder.icmp_eq(lhs_tag, one, "is_none");
        let true_val = self.builder.const_bool(true);
        let same_tag_result = self
            .builder
            .select(is_none, true_val, payload_eq, "same_eq");

        let false_val = self.builder.const_bool(false);
        Some(
            self.builder
                .select(tags_eq, same_tag_result, false_val, "opt_eq"),
        )
    }

    /// `Option<T>.compare(other) -> Ordering`
    ///
    /// Semantics: None < Some. If both Some, compare payloads.
    /// ARC tags: Some=0, None=1. Tag order is reversed from semantic order,
    /// so we compare `rhs_tag vs lhs_tag` (swapped) for the tag comparison.
    pub(super) fn emit_option_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "opt.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "opt.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        // Tags differ: reversed order (None(1) < Some(0) semantically).
        let tag_cmp = self
            .builder
            .emit_icmp_ordering(rhs_tag, lhs_tag, "tag_cmp", false);

        // Tags equal: None+None → Equal, Some+Some → compare payloads.
        let lhs_val = self.builder.extract_value(lhs, 1, "opt.lhs.val")?;
        let rhs_val = self.builder.extract_value(rhs, 1, "opt.rhs.val")?;
        let payload_cmp = self.emit_element_compare(lhs_val, rhs_val, inner_ty)?;

        let one = self.builder.const_i64(1);
        let is_none = self.builder.icmp_eq(lhs_tag, one, "is_none");
        let equal_ord = self.builder.const_i8(1);
        let same_tag_cmp = self
            .builder
            .select(is_none, equal_ord, payload_cmp, "same_cmp");

        Some(
            self.builder
                .select(tags_eq, same_tag_cmp, tag_cmp, "opt_cmp"),
        )
    }

    /// `Option<T>.hash() -> int`
    ///
    /// None → 0. `Some(x)` → `hash_combine(1, x.hash())`.
    pub(super) fn emit_option_hash(&mut self, val: ValueId, inner_ty: Idx) -> Option<ValueId> {
        let tag = self.builder.extract_value(val, 0, "opt.tag")?;
        let payload = self.builder.extract_value(val, 1, "opt.payload")?;

        let payload_hash = self.emit_element_hash(payload, inner_ty)?;
        let seed = self.builder.const_i64(1);
        let some_hash = self.emit_hash_combine(seed, payload_hash);

        let one = self.builder.const_i64(1);
        let is_none = self.builder.icmp_eq(tag, one, "is_none");
        let zero = self.builder.const_i64(0);
        Some(self.builder.select(is_none, zero, some_hash, "opt_hash"))
    }

    // -----------------------------------------------------------------------
    // Result trait methods
    // -----------------------------------------------------------------------

    /// `Result<Ok, Err>.equals(other) -> bool`
    ///
    /// Tags differ → false. Same tag → compare payloads.
    pub(super) fn emit_result_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        ok_ty: Idx,
        _err_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "res.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "res.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        let lhs_val = self.builder.extract_value(lhs, 1, "res.lhs.val")?;
        let rhs_val = self.builder.extract_value(rhs, 1, "res.rhs.val")?;
        let payload_eq = self.emit_element_equals(lhs_val, rhs_val, ok_ty)?;

        let false_val = self.builder.const_bool(false);
        Some(
            self.builder
                .select(tags_eq, payload_eq, false_val, "res_eq"),
        )
    }

    /// `Result<Ok, Err>.compare(other) -> Ordering`
    ///
    /// Tags differ → compare tags (Ok=0 < Err=1, matches numeric order).
    /// Same tag → compare payloads.
    pub(super) fn emit_result_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        ok_ty: Idx,
        _err_ty: Idx,
    ) -> Option<ValueId> {
        let lhs_tag = self.builder.extract_value(lhs, 0, "res.lhs.tag")?;
        let rhs_tag = self.builder.extract_value(rhs, 0, "res.rhs.tag")?;
        let tags_eq = self.builder.icmp_eq(lhs_tag, rhs_tag, "tags_eq");

        let tag_cmp = self
            .builder
            .emit_icmp_ordering(lhs_tag, rhs_tag, "tag_cmp", false);

        let lhs_val = self.builder.extract_value(lhs, 1, "res.lhs.val")?;
        let rhs_val = self.builder.extract_value(rhs, 1, "res.rhs.val")?;
        let payload_cmp = self.emit_element_compare(lhs_val, rhs_val, ok_ty)?;

        Some(
            self.builder
                .select(tags_eq, payload_cmp, tag_cmp, "res_cmp"),
        )
    }

    /// `Result<Ok, Err>.hash() -> int`
    ///
    /// `hash_combine(tag, payload.hash())`.
    pub(super) fn emit_result_hash(
        &mut self,
        val: ValueId,
        ok_ty: Idx,
        _err_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(val, 0, "res.tag")?;
        let payload = self.builder.extract_value(val, 1, "res.payload")?;
        let payload_hash = self.emit_element_hash(payload, ok_ty)?;
        Some(self.emit_hash_combine(tag, payload_hash))
    }

    // -----------------------------------------------------------------------
    // Tuple trait methods
    // -----------------------------------------------------------------------

    /// `Tuple.equals(other) -> bool`
    ///
    /// All fields must be equal (conjunction).
    pub(in crate::codegen::arc_emitter) fn emit_tuple_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        let mut result = self.builder.const_bool(true);

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate() {
            let lhs_field = self.builder.extract_value(lhs, i as u32, "tup.lhs")?;
            let rhs_field = self.builder.extract_value(rhs, i as u32, "tup.rhs")?;
            let field_eq = self.emit_element_equals(lhs_field, rhs_field, elem_ty)?;
            result = self.builder.and(result, field_eq, "tup_eq");
        }

        Some(result)
    }

    /// `Tuple.compare(other) -> Ordering`
    ///
    /// Lexicographic: compare field 0, if Equal compare field 1, etc.
    pub(super) fn emit_tuple_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i8(1)); // Equal
        }

        let equal_ord = self.builder.const_i8(1);
        let mut result = equal_ord;

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate() {
            let lhs_field = self.builder.extract_value(lhs, i as u32, "tup.lhs")?;
            let rhs_field = self.builder.extract_value(rhs, i as u32, "tup.rhs")?;
            let field_cmp = self.emit_element_compare(lhs_field, rhs_field, elem_ty)?;

            // If previous result was Equal, use this field's comparison.
            let prev_is_eq = self.builder.icmp_eq(result, equal_ord, "prev_eq");
            result = self
                .builder
                .select(prev_is_eq, field_cmp, result, "tup_cmp");
        }

        Some(result)
    }

    /// `Tuple.hash() -> int`
    ///
    /// Fold `hash_combine` over field hashes.
    pub(super) fn emit_tuple_hash(&mut self, val: ValueId, elements: &[Idx]) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i64(0));
        }

        let first_field = self.builder.extract_value(val, 0, "tup.f0")?;
        let mut result = self.emit_element_hash(first_field, elements[0])?;

        #[expect(
            clippy::cast_possible_truncation,
            reason = "tuple field count fits u32"
        )]
        for (i, &elem_ty) in elements.iter().enumerate().skip(1) {
            let field = self.builder.extract_value(val, i as u32, "tup.f")?;
            let field_hash = self.emit_element_hash(field, elem_ty)?;
            result = self.emit_hash_combine(result, field_hash);
        }

        Some(result)
    }

    // -----------------------------------------------------------------------
    // String helpers (exposed for compound trait use)
    // -----------------------------------------------------------------------

    /// Call `ori_str_compare(ptr, ptr) -> i8` for string comparison.
    fn emit_str_compare_call(&mut self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_compare");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        self.builder.call(func_id, &[lhs_ptr, rhs_ptr], "str_cmp")
    }

    /// Call `ori_str_hash(ptr) -> i64` for string hashing.
    fn emit_str_hash_call(&mut self, val: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_hash");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_hash.arg", str_ty);
        self.builder.store(val, ptr);

        self.builder.call(func_id, &[ptr], "str_hash")
    }
}
