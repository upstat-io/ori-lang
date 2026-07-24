//! Storage/boundary aggregate conversion at ABI edges.
//!
//! Spec: Annex E §Representation Optimization — narrowing is a storage
//! optimization; parameters and return values carry the canonical form on both
//! sides of a call.

use inkwell::types::BasicTypeEnum;
use ori_types::Idx;

use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Widen,
    Narrow,
}

impl<'ctx> ArcIrEmitter<'_, '_, 'ctx, '_> {
    /// Convert a narrowed storage aggregate to its canonical boundary form.
    pub(in crate::codegen::arc_emitter) fn widen_to_boundary(
        &mut self,
        value: ValueId,
        idx: Idx,
    ) -> ValueId {
        self.convert_across_boundary(value, idx, Direction::Widen)
    }

    /// Convert a canonical boundary aggregate to its narrowed storage form.
    pub(in crate::codegen::arc_emitter) fn narrow_to_storage(
        &mut self,
        value: ValueId,
        idx: Idx,
    ) -> ValueId {
        self.convert_across_boundary(value, idx, Direction::Narrow)
    }

    fn convert_across_boundary(
        &mut self,
        value: ValueId,
        idx: Idx,
        direction: Direction,
    ) -> ValueId {
        if !self.type_resolver.is_narrowed_aggregate(idx) {
            return value;
        }
        let storage = self.type_resolver.resolve(idx);
        let boundary = self.type_resolver.resolve_boundary(idx);
        let (source, target) = match direction {
            Direction::Widen => (storage, boundary),
            Direction::Narrow => (boundary, storage),
        };
        let (BasicTypeEnum::StructType(source_st), BasicTypeEnum::StructType(target_st)) =
            (source, target)
        else {
            return value;
        };
        let field_count = source_st.count_fields();
        if field_count != target_st.count_fields() {
            self.builder.record_codegen_error_with_msg(format!(
                "narrowed aggregate field-count mismatch: {field_count} vs {}",
                target_st.count_fields()
            ));
            return value;
        }

        let target_id = self.builder.register_type(target);
        let mut aggregate = self.builder.const_zero_ty(target_id);
        for index in 0..field_count {
            let field = self
                .builder
                .extract_value_any(value, index, "boundary.field");
            let (Some(from_ty), Some(to_ty)) = (
                source_st.get_field_type_at_index(index),
                target_st.get_field_type_at_index(index),
            ) else {
                return value;
            };
            let converted = self.convert_field(field, from_ty, to_ty, direction);
            aggregate = self
                .builder
                .insert_value(aggregate, converted, index, "boundary.agg");
        }
        aggregate
    }

    fn convert_field(
        &mut self,
        field: ValueId,
        from_ty: BasicTypeEnum<'ctx>,
        to_ty: BasicTypeEnum<'ctx>,
        direction: Direction,
    ) -> ValueId {
        if from_ty == to_ty {
            return field;
        }
        let to_id = self.builder.register_type(to_ty);
        match (from_ty, to_ty) {
            (BasicTypeEnum::IntType(_), BasicTypeEnum::IntType(_)) => {
                if direction == Direction::Widen {
                    self.builder.sext(field, to_id, "boundary.sext")
                } else {
                    self.builder.trunc(field, to_id, "boundary.trunc")
                }
            }
            (BasicTypeEnum::FloatType(_), BasicTypeEnum::FloatType(_)) => {
                if direction == Direction::Widen {
                    self.builder.float_ext(field, to_id, "boundary.fpext")
                } else {
                    self.builder.float_trunc(field, to_id, "boundary.fptrunc")
                }
            }
            _ => field,
        }
    }
}
