//! Option trait method codegen.
//!
//! Implements `equals`, `compare`, and `hash` for `Option<T>`.
//!
//! ## ARC enum convention
//!
//! `Option<T>` is represented as `{i64 tag, T payload}` — Some=0, None=1.

use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// `Option<T>.equals(other) -> bool`
    ///
    /// Tags differ → false. Both None → true. Both Some → payload equals.
    pub(in super::super) fn emit_option_equals(
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

        // Both None → equal. Both Some → check payload.
        let none_tag = self
            .builder
            .const_int_matching(lhs_tag, ori_ir::OPTION_TAG_NONE as u64);
        let is_none = self.builder.icmp_eq(lhs_tag, none_tag, "is_none");
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
    pub(in super::super) fn emit_option_compare(
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

        let none_tag = self
            .builder
            .const_int_matching(lhs_tag, ori_ir::OPTION_TAG_NONE as u64);
        let is_none = self.builder.icmp_eq(lhs_tag, none_tag, "is_none");
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
    pub(in super::super) fn emit_option_hash(
        &mut self,
        val: ValueId,
        inner_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(val, 0, "opt.tag")?;
        let payload = self.builder.extract_value(val, 1, "opt.payload")?;

        let payload_hash = self.emit_element_hash(payload, inner_ty)?;
        let seed = self.builder.const_i64(1);
        let some_hash = self.emit_hash_combine(seed, payload_hash);

        let none_tag = self
            .builder
            .const_int_matching(tag, ori_ir::OPTION_TAG_NONE as u64);
        let is_none = self.builder.icmp_eq(tag, none_tag, "is_none");
        let none_hash = self.builder.const_i64(0);
        Some(
            self.builder
                .select(is_none, none_hash, some_hash, "opt_hash"),
        )
    }

    /// `Option<T>.iter()` → iterator over 0 or 1 elements.
    ///
    /// Calls `ori_iter_from_option(is_some, payload_ptr, elem_size, elem_dec_fn)`
    /// which allocates a 1-element buffer for Some, or creates an empty iterator
    /// for None.
    pub(in super::super) fn emit_option_iter(
        &mut self,
        receiver: ValueId,
        inner_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
        let some_const = self
            .builder
            .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
        let is_some_i1 = self.builder.icmp_eq(tag, some_const, "is_some");

        // Alloca the Option struct so we can GEP to the payload field.
        let opt_llvm = self.resolve_type_for_option(inner_ty);
        let alloca = self.builder.alloca(opt_llvm, "opt.iter.a");
        self.builder.store(receiver, alloca);
        let payload_ptr = self
            .builder
            .struct_gep(opt_llvm, alloca, 1, "opt.iter.payload");

        let elem_size = crate::codegen::abi::abi_size(inner_ty, self.type_info) as i64;
        let elem_size_val = self.builder.const_i64(elem_size);

        // Get elem_dec_fn for proper cleanup of RC'd elements.
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(inner_ty);

        let func_id = self.builder.runtime_fn("ori_iter_from_option");
        self.emit_rt_call(
            func_id,
            &[is_some_i1, payload_ptr, elem_size_val, elem_dec_fn],
            "opt.iter",
        )
    }
}
