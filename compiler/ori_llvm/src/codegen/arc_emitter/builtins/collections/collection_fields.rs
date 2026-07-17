//! Shared collection field, pointer, and layout helpers.

use ori_types::Idx;

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Alloca+store a string value and return the pointer.
    ///
    /// Runtime string methods take `*const OriStr`, but LLVM values are
    /// `{ i64, i64, ptr }` aggregates. This helper allocates stack space, stores
    /// the aggregate, and returns the pointer for the runtime call.
    pub(crate) fn str_to_ptr(&mut self, val: ValueId, name: &str) -> ValueId {
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, str_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Like [`str_to_ptr`] but with borrowed parameter forwarding.
    ///
    /// If the variable has a known source pointer (from a `Reference`/`Indirect`
    /// parameter), returns it directly instead of creating an alloca+store.
    pub(crate) fn str_to_ptr_forwarded(
        &mut self,
        val: ValueId,
        var: ori_arc::ir::ArcVarId,
        name: &str,
    ) -> ValueId {
        if let Some(&src_ptr) = self.borrowed_param_ptrs.get(&var) {
            return src_ptr;
        }
        self.str_to_ptr(val, name)
    }

    /// Alloca+store an element value and return the pointer.
    ///
    /// List runtime methods take `*const u8` for elements. This helper
    /// allocates stack space for the element, stores the value, and
    /// returns the pointer.
    pub(crate) fn elem_to_ptr(&mut self, val: ValueId, elem_ty: Idx, name: &str) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, llvm_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Emit a collection `len`/`length` field-read with borrowed-parameter
    /// forwarding, mirroring [`Self::emit_str_length_forwarded`]. When the
    /// receiver is a borrowed pointer-only param its LLVM value is a zero
    /// `{i64, i64, ptr}` placeholder (the entry-block struct-value load was
    /// elided — 24-byte collections pass indirectly per the ABI), so read
    /// `FIELD_LEN` directly from the source pointer via GEP + load. This keeps
    /// the param pointer-only (no struct-value materialization, no RC-flow
    /// change). Otherwise the receiver is a loaded struct value and
    /// `extract_value` reads it. Shared by list/map/set — identical
    /// `{i64, i64, ptr}` fat-pointer layout.
    pub(crate) fn emit_collection_length_forwarded(
        &mut self,
        receiver: ValueId,
        var: ori_arc::ir::ArcVarId,
        name: &str,
    ) -> Option<ValueId> {
        if let Some(&src_ptr) = self.borrowed_param_ptrs.get(&var) {
            let struct_ty = self.list_struct_type();
            let len_ptr = self
                .builder
                .struct_gep(struct_ty, src_ptr, ori_ir::FIELD_LEN, name);
            let i64_ty = self
                .builder
                .register_type(self.builder.scx().type_i64().into());
            return Some(self.builder.load(i64_ty, len_ptr, name));
        }
        self.builder
            .extract_value(receiver, ori_ir::FIELD_LEN, name)
    }

    /// Extract the data and length fields shared by list/map/set values.
    pub(crate) fn extract_collection_data_and_len(
        &mut self,
        receiver: ValueId,
        data_name: &str,
        len_name: &str,
    ) -> Option<(ValueId, ValueId)> {
        let data = self
            .builder
            .extract_value(receiver, ori_ir::FIELD_DATA, data_name)?;
        let len = self
            .builder
            .extract_value(receiver, ori_ir::FIELD_LEN, len_name)?;
        Some((data, len))
    }

    /// Extract the canonical data/length/capacity fields shared by
    /// list/map/set values.
    pub(crate) fn extract_collection_fields(
        &mut self,
        receiver: ValueId,
        data_name: &str,
        len_name: &str,
        cap_name: &str,
    ) -> Option<(ValueId, ValueId, ValueId)> {
        let (data, len) = self.extract_collection_data_and_len(receiver, data_name, len_name)?;
        let cap = self
            .builder
            .extract_value(receiver, ori_ir::FIELD_CAP, cap_name)?;
        Some((data, len, cap))
    }

    /// Emit a collection `is_empty` (`len == 0`) with the same borrowed-parameter
    /// forwarding as [`Self::emit_collection_length_forwarded`]: read `FIELD_LEN`
    /// via the source pointer when the receiver is a borrowed pointer-only param
    /// (its struct value is a zero placeholder), else `extract_value`. Shared by
    /// list/map/set — identical `{i64, i64, ptr}` fat-pointer layout.
    pub(crate) fn emit_collection_is_empty_forwarded(
        &mut self,
        receiver: ValueId,
        var: ori_arc::ir::ArcVarId,
        name: &str,
    ) -> Option<ValueId> {
        let len = self.emit_collection_length_forwarded(receiver, var, name)?;
        let zero = self.builder.const_i64(0);
        Some(self.builder.icmp_eq(len, zero, name))
    }

    /// Build the LLVM struct type `{i64, i64, ptr}` — the shared list/map/set
    /// fat-pointer layout used for list sret returns AND the borrowed-parameter
    /// `FIELD_LEN` forwarding in [`Self::emit_collection_length_forwarded`].
    pub(crate) fn list_struct_type(&mut self) -> LLVMTypeId {
        self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        )
    }
}
