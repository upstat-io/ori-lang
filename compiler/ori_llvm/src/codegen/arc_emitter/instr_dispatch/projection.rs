//! ARC `Project` instruction emission.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::context::{is_boxed_enum_field, EmittedValue};
use super::super::ArcIrEmitter;

mod enum_payload;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `Project` on a decomposed `__iter_next` result.
    ///
    /// Field 0 returns the `i64` tag. Field 1 loads the element from the
    /// scratch buffer and records its pointer for borrowed-parameter forwarding.
    fn emit_project_iter_next(
        &mut self,
        dst: ArcVarId,
        field: u32,
        tag: ValueId,
        scratch_ptr: ValueId,
        elem_llvm_ty: LLVMTypeId,
        func: &ArcFunction,
    ) {
        if field == 0 {
            self.def_var_repr(dst, tag, func);
        } else {
            // Why: Map iterators preserve runtime-written `(key, value)` declaration order.
            let elem = self
                .builder
                .load(elem_llvm_ty, scratch_ptr, &format!("proj.{field}"));
            self.def_var_repr(dst, elem, func);
            // Why: Forwarding the scratch pointer avoids spilling borrowed iterator elements.
            self.borrowed_param_ptrs.insert(dst, scratch_ptr);
        }
    }

    /// Returns `true` after projecting an enum/Result payload, or `false` when
    /// the normal `extractvalue` path must handle the field.
    #[expect(
        clippy::too_many_arguments,
        reason = "payload projection carries source and destination representations"
    )]
    fn try_emit_project_enum_payload(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        val: ValueId,
        result_ty: LLVMTypeId,
        func: &ArcFunction,
    ) -> bool {
        let val_ty = func.var_type(value);
        let val_type_info = self.type_info.get(val_ty);
        if !matches!(
            val_type_info,
            crate::codegen::type_info::TypeInfo::Result { .. }
                | crate::codegen::type_info::TypeInfo::Enum { .. }
        ) {
            return false;
        }

        let resolved_field_ty = self.pool.resolve_fully(ty);
        if matches!(self.pool.tag(resolved_field_ty), Tag::Unit | Tag::Never) {
            let zero = self.builder.const_zero_ty(result_ty);
            self.def_var_repr(dst, zero, func);
            return true;
        }

        // Why: Tagless enums store fields directly without a tag or payload wrapper.
        if self.is_tagless_enum(val_ty) {
            self.emit_project_tagless_field(dst, ty, val_ty, value, field, result_ty, func);
            return true;
        }

        let is_general_enum = matches!(
            val_type_info,
            crate::codegen::type_info::TypeInfo::Enum { .. }
        );

        let byte_offset = if is_general_enum {
            let Some(byte_offset) =
                self.compute_general_enum_payload_byte_offset(val_ty, field, ty)
            else {
                return true;
            };
            byte_offset
        } else {
            0
        };

        let slot_index = byte_offset / 8;
        if is_general_enum
            && !is_boxed_enum_field(self.pool, val_ty, ty)
            && self.builder.is_struct_value(val)
            && self.builder.is_single_slot_type(result_ty)
            && byte_offset % 8 == 0
        {
            let Some(payload) = self.builder.extract_value(val, 1, "proj.payload") else {
                self.builder.record_codegen_error_with_msg(
                    "general-enum scalar projection requires a struct payload",
                );
                return true;
            };

            let Ok(slot_index) = u32::try_from(slot_index) else {
                self.builder
                    .record_codegen_error_with_msg("general-enum payload has too many slots");
                return true;
            };
            let raw =
                self.builder
                    .extract_value_any(payload, slot_index, &format!("proj.{field}.raw"));

            let converted =
                self.builder
                    .reinterpret_from_i64(raw, result_ty, &format!("proj.{field}"));
            self.def_var_repr(dst, converted, func);
            return true;
        }

        let llvm_val_ty = self.resolve_type(val_ty);
        let alloca = self.builder.alloca(llvm_val_ty, "proj.alloca");
        self.builder.store(val, alloca);
        if is_general_enum {
            self.emit_project_general_enum_field(
                dst,
                ty,
                val_ty,
                llvm_val_ty,
                alloca,
                byte_offset,
                field,
                result_ty,
                func,
            );
        } else {
            self.emit_project_tagged_union_field(
                dst,
                ty,
                val_ty,
                llvm_val_ty,
                alloca,
                field,
                result_ty,
                func,
            );
        }
        true
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "projection carries destination, field, enum, and LLVM storage identities"
    )]
    fn emit_project_general_enum_field(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        val_ty: Idx,
        llvm_val_ty: LLVMTypeId,
        alloca: ValueId,
        byte_offset: u64,
        field: u32,
        result_ty: LLVMTypeId,
        func: &ArcFunction,
    ) {
        let payload_ptr = self
            .builder
            .struct_gep(llvm_val_ty, alloca, 1, "proj.payload");
        let i8_ty = self.builder.i8_type();
        let Ok(byte_offset) = i64::try_from(byte_offset) else {
            self.builder
                .record_codegen_error_with_msg("general-enum payload offset exceeds LLVM limits");
            return;
        };
        let offset_val = self.builder.const_i64(byte_offset);
        let slot_ptr = self.builder.gep(
            i8_ty,
            payload_ptr,
            &[offset_val],
            &format!("proj.{field}.gep"),
        );

        let loaded = if is_boxed_enum_field(self.pool, val_ty, ty) {
            let ptr_ty = self.builder.ptr_type();
            let rc_ptr = self
                .builder
                .load(ptr_ty, slot_ptr, &format!("proj.{field}.ptr"));
            self.builder
                .load(result_ty, rc_ptr, &format!("proj.{field}"))
        } else {
            self.builder
                .load(result_ty, slot_ptr, &format!("proj.{field}"))
        };
        self.def_var_repr(dst, loaded, func);
    }

    /// Project a `Result`/`Option` payload through its physical struct slot.
    ///
    /// Niche layouts omit the tag slot, while boxed recursive fields require
    /// dereferencing the stored RC pointer.
    #[expect(
        clippy::too_many_arguments,
        reason = "projection threads dst/types/alloca/field"
    )]
    fn emit_project_tagged_union_field(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        val_ty: Idx,
        llvm_val_ty: LLVMTypeId,
        alloca: ValueId,
        field: u32,
        result_ty: LLVMTypeId,
        func: &ArcFunction,
    ) {
        let omitted_tag_slots = u32::from(self.get_niche_encoding(val_ty).is_some());
        let struct_idx = field - omitted_tag_slots;

        let gep = self.builder.struct_gep(
            llvm_val_ty,
            alloca,
            struct_idx,
            &format!("proj.{field}.gep"),
        );

        if is_boxed_enum_field(self.pool, val_ty, ty) {
            let ptr_ty = self.builder.ptr_type();
            let box_ptr = self.builder.load(ptr_ty, gep, &format!("proj.{field}.box"));
            let loaded = self
                .builder
                .load(result_ty, box_ptr, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        } else {
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

    /// Returns `true` after recording a niche field as the switch scrutinee, or
    /// `false` when `val_ty` is not niche-encoded.
    fn try_emit_project_niche_tag(&mut self, dst: ArcVarId, value: ArcVarId, val_ty: Idx) -> bool {
        let Some(encoding) = self.get_niche_encoding(val_ty) else {
            return false;
        };

        let Some((niche_idx, _, _)) = encoding.niche_fields() else {
            self.builder
                .record_codegen_error_with_msg("niche projection requires a niche tag encoding");
            return true;
        };
        let v = self.var(value);
        let llvm_ty = self.resolve_type(val_ty);
        let niche_val =
            if let Some(extracted) = self.builder.extract_value(v, niche_idx, "niche.field") {
                extracted
            } else {
                let Some(field_ty) = self.builder.struct_field_type(llvm_ty, niche_idx) else {
                    self.builder.record_codegen_error_with_msg(
                        "niche projection layout is missing its sentinel field",
                    );
                    return true;
                };

                let gep = self
                    .builder
                    .struct_gep(llvm_ty, v, niche_idx, "niche.field.gep");
                self.builder.load(field_ty, gep, "niche.field")
            };
        self.niche_scrutinees.insert(dst, val_ty);
        self.def_var(dst, EmittedValue::Immediate(niche_val));
        true
    }

    /// Emit a bare integer sum discriminant for `Project { field: 0 }`.
    ///
    /// The value is already the discriminant. It is zero-extended to the
    /// destination width so switch/comparison operands have matching integer
    /// types. Returns `false` for aggregate and non-integer representations.
    fn try_emit_project_scalar_tag(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        val_ty: Idx,
    ) -> bool {
        let val_llvm_ty = self.resolve_type(val_ty);
        if !self.builder.is_scalar_int_type(val_llvm_ty) {
            return false;
        }
        let tag = self.var(value);
        let dst_ty = self.resolve_type(ty);
        let widened = if dst_ty == val_llvm_ty {
            tag
        } else {
            self.builder.zext(tag, dst_ty, "scalar.tag.zext")
        };
        self.def_var(dst, EmittedValue::Immediate(widened));
        true
    }

    /// Emit `Project` field extraction across iterator, enum, boxed, narrowed,
    /// and aggregate representations.
    pub(super) fn emit_project(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        func: &ArcFunction,
    ) {
        // Why: Decomposed iterator results have no aggregate wrapper to extract.
        if let Some(&(tag, scratch_ptr, elem_llvm_ty)) = self.iter_next_decomposed.get(&value) {
            self.emit_project_iter_next(dst, field, tag, scratch_ptr, elem_llvm_ty, func);
            return;
        }

        // INVARIANT: Tagged-pointer enums store field 0 in low bits and their sole pointer above it.
        let val_ty = func.var_type(value);
        if self.get_tagged_ptr_encoding(val_ty).is_some() {
            let v = self.var(value);
            if field == 0 {
                let tag = self.tagged_ptr_decode_tag(v, "tagged.tag");
                self.def_var(dst, EmittedValue::Immediate(tag));
            } else {
                let ptr = self.tagged_ptr_decode_ptr(v, "tagged.ptr");
                self.def_var_repr(dst, ptr, func);
            }
            return;
        }

        // Why: A tagless single-variant enum's only discriminant is zero.
        if field == 0 && self.is_tagless_enum(val_ty) {
            let zero = self.builder.const_i64(0);
            self.def_var_repr(dst, zero, func);
            return;
        }

        if field == 0 && self.try_emit_project_scalar_tag(dst, ty, value, val_ty) {
            return;
        }

        if field == 0 && self.try_emit_project_niche_tag(dst, value, val_ty) {
            return;
        }

        let val = self.var(value);
        let result_ty = self.resolve_type(ty);

        if field > 0
            && self.try_emit_project_enum_payload(dst, ty, value, field, val, result_ty, func)
        {
            return;
        }

        let val_ty = func.var_type(value);
        let mem_field = self.remap_struct_field(val_ty, field);

        // Why: Boxed recursive fields store an RC pointer instead of the child value inline.
        if is_boxed_enum_field(self.pool, val_ty, ty) {
            let ptr_ty = self.builder.ptr_type();
            let rc_ptr = if let Some(extracted) =
                self.builder
                    .extract_value(val, mem_field, &format!("proj.{field}.ptr"))
            {
                extracted
            } else {
                let llvm_val_ty = self.resolve_type(val_ty);
                let gep = self.builder.struct_gep(
                    llvm_val_ty,
                    val,
                    mem_field,
                    &format!("proj.{field}.gep"),
                );
                self.builder.load(ptr_ty, gep, &format!("proj.{field}.ptr"))
            };

            let loaded = self
                .builder
                .load(result_ty, rc_ptr, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
            return;
        }

        if let Some(extracted) =
            self.builder
                .extract_value(val, mem_field, &format!("proj.{field}"))
        {
            let dst_ty = func.var_type(dst);
            let widened = self.sext_narrowed_field(extracted, field, dst_ty);
            self.def_var_repr(dst, widened, func);
        } else {
            let llvm_val_ty = self.resolve_type(val_ty);
            let gep =
                self.builder
                    .struct_gep(llvm_val_ty, val, mem_field, &format!("proj.{field}.gep"));
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }
}
