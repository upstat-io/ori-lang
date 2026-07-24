use inkwell::types::BasicTypeEnum;
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Truncate canonical values to their narrowed physical field widths.
    pub(in crate::codegen::arc_emitter) fn trunc_for_narrowed_struct(
        &mut self,
        struct_ty_id: LLVMTypeId,
        args: &[ValueId],
        ctor_type: Idx,
    ) -> Vec<ValueId> {
        let raw_ty = self.builder.arena.get_type(struct_ty_id);
        let BasicTypeEnum::StructType(st) = raw_ty else {
            return args.to_vec();
        };

        let resolved = self.pool.resolve_fully(ctor_type);
        let pool_tag = self.pool.tag(resolved);
        let decl_pool_types: Vec<Idx> = if pool_tag == Tag::Struct {
            self.pool
                .struct_fields(resolved)
                .into_iter()
                .map(|(_, idx)| idx)
                .collect()
        } else if pool_tag == Tag::Tuple {
            self.pool.tuple_elems(resolved)
        } else {
            return args.to_vec();
        };

        let field_pool_types: Vec<Idx> =
            if let Some(repr) = self.repr_plan.and_then(|p| p.get_repr(resolved)) {
                let fields = match repr {
                    ori_repr::MachineRepr::Struct(s) => &s.fields[..],
                    ori_repr::MachineRepr::Tuple(t) => &t.elements[..],
                    _ => return args.to_vec(),
                };
                fields
                    .iter()
                    .map(|f| decl_pool_types[f.original_index as usize])
                    .collect()
            } else {
                decl_pool_types
            };

        args.iter()
            .enumerate()
            .map(|(i, &val)| {
                let field_pool_tag = field_pool_types
                    .get(i)
                    .map(|&idx| self.pool.tag(self.pool.resolve_fully(idx)));

                if field_pool_tag == Some(Tag::Int) {
                    let Some(BasicTypeEnum::IntType(field_int)) =
                        st.get_field_type_at_index(i as u32)
                    else {
                        return val;
                    };
                    let field_bits = field_int.get_bit_width();
                    if field_bits >= 64 {
                        return val;
                    }
                    let value = self.builder.arena.get_value(val);
                    if !value.is_int_value() {
                        return val;
                    }
                    let value_bits = value.into_int_value().get_type().get_bit_width();
                    if value_bits > field_bits {
                        let field_ty_id = self.builder.register_type(field_int.into());
                        return self
                            .builder
                            .trunc(val, field_ty_id, &format!("narrow.trunc.{i}"));
                    }
                    return val;
                }

                if field_pool_tag == Some(Tag::Float) {
                    let Some(BasicTypeEnum::FloatType(field_float)) =
                        st.get_field_type_at_index(i as u32)
                    else {
                        return val;
                    };
                    let value = self.builder.arena.get_value(val);
                    if !value.is_float_value() {
                        return val;
                    }
                    let value_float_ty = value.into_float_value().get_type();
                    if value_float_ty != field_float {
                        let field_ty_id = self.builder.register_type(field_float.into());
                        return self.builder.float_trunc(
                            val,
                            field_ty_id,
                            &format!("narrow.fptrunc.{i}"),
                        );
                    }
                }

                val
            })
            .collect()
    }

    /// Widen a narrowed field value back to its canonical ARC width.
    pub(in crate::codegen::arc_emitter) fn sext_narrowed_field(
        &mut self,
        extracted: ValueId,
        field_index: u32,
        dst_type: Idx,
    ) -> ValueId {
        let resolved = self.pool.resolve_fully(dst_type);
        let tag = self.pool.tag(resolved);

        if tag == Tag::Int {
            let value = self.builder.arena.get_value(extracted);
            if !value.is_int_value() {
                return extracted;
            }
            let bits = value.into_int_value().get_type().get_bit_width();
            if bits >= 64 {
                return extracted;
            }
            let i64_ty = self
                .builder
                .register_type(self.builder.scx.type_i64().into());
            return self
                .builder
                .sext(extracted, i64_ty, &format!("narrow.sext.{field_index}"));
        }

        if tag == Tag::Float {
            let value = self.builder.arena.get_value(extracted);
            if !value.is_float_value() {
                return extracted;
            }
            let value_float_ty = value.into_float_value().get_type();
            let canonical_f64 = self.builder.scx.type_f64();
            if value_float_ty != canonical_f64 {
                let f64_ty_id = self.builder.register_type(canonical_f64.into());
                return self.builder.float_ext(
                    extracted,
                    f64_ty_id,
                    &format!("narrow.fpext.{field_index}"),
                );
            }
        }

        extracted
    }
}
