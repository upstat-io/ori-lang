//! Raw iterator-protocol step emission.
//!
//! The decomposed `(tag, scratch, elem)` step shared by the for-loop
//! `__iter_next` path and the user-facing `iter.next()` / `iter.next_back()`
//! protocol methods, plus the `(Option<T>, Self)` tuple builder both protocol
//! directions share.

use ori_types::Idx;

use crate::codegen::{LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `<rt_fn>(iter, scratch, elem_size)` and decompose the result into a
    /// `(tag, scratch_ptr, elem_llvm_ty)` triple (tag is i64: 0=done, 1=has
    /// element; the scratch alloca holds the element data). `rt_fn` selects the
    /// forward (`ori_iter_next`) or backward (`ori_iter_next_back`) runtime step.
    ///
    /// Element size/type are canonical (NR-3 — narrowing is confined to the
    /// list storage boundary `emit_list_iter`, never the iterator pipeline).
    fn emit_iter_step_raw(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
        rt_fn: &'static str,
    ) -> Option<(ValueId, ValueId, LLVMTypeId)> {
        let func_id = self.builder.runtime_fn(rt_fn);
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let scratch = self.builder.create_entry_alloca(
            self.current_function,
            "iter.step.scratch",
            elem_llvm_ty,
        );
        let has_next_i8 = self.emit_rt_call(
            func_id,
            &[iter_ptr, scratch, elem_size_val],
            "iter.step.has",
        )?;
        // Zero-extend i8 → i64 for ARC IR tag (projected as Idx::INT).
        let i64_ty = self.builder.i64_type();
        let tag = self.builder.zext(has_next_i8, i64_ty, "iter.step.tag");
        Some((tag, scratch, elem_llvm_ty))
    }

    /// Emit the forward decomposed step (`ori_iter_next`).
    ///
    /// Returns the `(tag, scratch_ptr, elem_llvm_ty)` triple. The caller
    /// registers the decomposed result so `emit_project` can extract the tag
    /// (field 0) and element (field 1) without an intermediate `{i64, T}`
    /// wrapper struct. Backs the for-loop `__iter_next` protocol.
    pub(in crate::codegen) fn emit_iter_next(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<(ValueId, ValueId, LLVMTypeId)> {
        self.emit_iter_step_raw(iter_ptr, elem_ty, "ori_iter_next")
    }

    /// Emit the backward analogue of [`emit_iter_next`].
    ///
    /// Calls `ori_iter_next_back(iter, scratch, elem_size)` and returns the
    /// decomposed `(tag, scratch_ptr, elem_llvm_ty)` triple. Backs the
    /// `DoubleEndedIterator.next_back()` protocol.
    fn emit_iter_next_back_raw(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<(ValueId, ValueId, LLVMTypeId)> {
        self.emit_iter_step_raw(iter_ptr, elem_ty, "ori_iter_next_back")
    }

    /// Emit the user-facing `iter.next()` protocol method.
    ///
    /// Produces the `(Option<T>, Self)` tuple: advances the iterator forward by
    /// one step (mutating the handle in place) and packages the yielded element
    /// (or `None`) with the advanced iterator handle.
    pub(crate) fn emit_iter_next_protocol(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        let (tag, scratch, elem_llvm) = self.emit_iter_next(iter_ptr, elem_ty)?;
        self.build_protocol_next_tuple(tag, scratch, elem_llvm, elem_ty, iter_ptr, dst_ty)
    }

    /// Emit the user-facing `iter.next_back()` protocol method (backward step).
    pub(crate) fn emit_iter_next_back_protocol(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        let (tag, scratch, elem_llvm) = self.emit_iter_next_back_raw(iter_ptr, elem_ty)?;
        self.build_protocol_next_tuple(tag, scratch, elem_llvm, elem_ty, iter_ptr, dst_ty)
    }

    /// Build the `(Option<T>, Self)` result tuple for the raw `next`/`next_back`
    /// protocol from a decomposed `(tag, scratch, elem_llvm)` step result.
    ///
    /// `tag` is the i64 has-element flag (1=Some, 0=None); the element payload is
    /// loaded from `scratch`. `iter_ptr` is the advanced handle that
    /// becomes the tuple's `Self` field. No RC op is emitted — scalar elements
    /// need none, and heap-element ownership of the extracted payload is decided
    /// by the surrounding realized ARC IR.
    fn build_protocol_next_tuple(
        &mut self,
        tag: ValueId,
        scratch: ValueId,
        elem_llvm: LLVMTypeId,
        elem_ty: Idx,
        iter_ptr: ValueId,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        // Option<T> = { i64 tag, T payload }. SOME=0, NONE=1.
        let opt_llvm = self.resolve_type_for_option(elem_ty);
        let zero = self.builder.const_i64(0);
        let has_next = self.builder.icmp_ne(tag, zero, "next.has");
        let some_c = self.builder.const_i64(ori_ir::OPTION_TAG_SOME);
        let none_c = self.builder.const_i64(ori_ir::OPTION_TAG_NONE);
        let opt_tag = self
            .builder
            .select(has_next, some_c, none_c, "next.opt_tag");
        let payload = self.builder.load(elem_llvm, scratch, "next.payload");
        let opt = self.builder.const_zero_ty(opt_llvm);
        let opt = self
            .builder
            .insert_value(opt, opt_tag, 0, "next.opt_tag_set");
        let opt = self
            .builder
            .insert_value(opt, payload, 1, "next.opt_payload_set");

        // Result tuple (Option<T>, Self). Field 0 = Option, field 1 = iterator
        // handle (the same mutated `iter_ptr`).
        let tuple_llvm = self.resolve_type(dst_ty);
        let tuple = self.builder.const_zero_ty(tuple_llvm);
        let tuple = self.builder.insert_value(tuple, opt, 0, "next.tuple_opt");
        let tuple = self
            .builder
            .insert_value(tuple, iter_ptr, 1, "next.tuple_iter");
        Some(tuple)
    }
}
