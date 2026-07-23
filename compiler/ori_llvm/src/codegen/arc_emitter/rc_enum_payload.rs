//! Field-walk adapter for inline tagged-enum payload destruction.

use ori_types::Idx;

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::context::is_boxed_enum_field;
use super::drop_enum::variant_field_offset;
use super::field_walk::FieldWalkOps;
use super::ArcIrEmitter;

/// [`FieldWalkOps`] for an inline tagged-enum variant payload.
///
/// Option and Result use typed struct fields. General enums use byte offsets
/// into their payload array.
pub(super) struct TaggedEnumPayloadOps {
    pub(super) alloca: ValueId,
    pub(super) enum_llvm_ty: LLVMTypeId,
    pub(super) owner_ty: Idx,
    /// Option/Result: typed payload field at struct index `1 + field_index`.
    /// `false` for general enum: byte-offset GEP into the payload array.
    pub(super) is_option_result: bool,
    /// Byte offsets (general-enum payload only); empty for Option/Result.
    pub(super) offsets: Vec<u64>,
}

impl FieldWalkOps for TaggedEnumPayloadOps {
    fn load<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        walk: &[(u32, Idx)],
        idx: usize,
    ) -> Option<(ValueId, bool)> {
        let (field_index, field_type) = walk[idx];
        let boxed = is_boxed_enum_field(emitter.pool, self.owner_ty, field_type);
        let field_ptr = if self.is_option_result {
            emitter.builder.struct_gep(
                self.enum_llvm_ty,
                self.alloca,
                1 + field_index,
                "dec.payload.ptr",
            )
        } else {
            let payload_ptr =
                emitter
                    .builder
                    .struct_gep(self.enum_llvm_ty, self.alloca, 1, "dec.payload");
            let i8_ty = emitter.builder.i8_type();
            let byte_off = variant_field_offset(&self.offsets, field_index as usize);
            let off = emitter.builder.const_i64(byte_off as i64);
            emitter
                .builder
                .gep(i8_ty, payload_ptr, &[off], "dec.field.ptr")
        };
        if boxed {
            let ptr_ty = emitter.builder.ptr_type();
            let rc_ptr = emitter.builder.load(ptr_ty, field_ptr, "dec.payload.rc");
            Some((rc_ptr, true))
        } else {
            let field_llvm_ty = emitter.resolve_type(field_type);
            let fv = emitter
                .builder
                .load(field_llvm_ty, field_ptr, "dec.payload");
            Some((fv, false))
        }
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
