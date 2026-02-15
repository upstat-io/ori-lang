//! Aggregate operations (extract, insert, struct construction) for `IrBuilder`.

use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

impl IrBuilder<'_, '_> {
    /// Extract a value from an aggregate (struct/array) by index.
    pub fn extract_value(&mut self, agg: ValueId, index: u32, name: &str) -> Option<ValueId> {
        let raw = self.arena.get_value(agg);
        let BasicValueEnum::StructValue(v) = raw else {
            tracing::error!(?raw, index, "extract_value on non-struct value");
            self.record_codegen_error();
            return None;
        };
        self.builder
            .build_extract_value(v, index, name)
            .ok()
            .map(|result| self.arena.push_value(result))
    }

    /// Insert a value into an aggregate at the given index.
    pub fn insert_value(&mut self, agg: ValueId, val: ValueId, index: u32, name: &str) -> ValueId {
        let raw_agg = self.arena.get_value(agg);
        let BasicValueEnum::StructValue(a) = raw_agg else {
            tracing::error!(?raw_agg, index, "insert_value on non-struct value");
            self.record_codegen_error();
            return agg; // Return unchanged aggregate
        };
        let v = self.arena.get_value(val);
        let result = self
            .builder
            .build_insert_value(a, v, index, name)
            .expect("insert_value");
        match result {
            inkwell::values::AggregateValueEnum::StructValue(sv) => {
                self.arena.push_value(sv.into())
            }
            inkwell::values::AggregateValueEnum::ArrayValue(av) => self.arena.push_value(av.into()),
        }
    }

    /// Build a struct from values by successive `insert_value`.
    pub fn build_struct(&mut self, ty: LLVMTypeId, values: &[ValueId], name: &str) -> ValueId {
        let raw_ty = self.arena.get_type(ty);

        // Defensive: verify this is actually a struct type
        let BasicTypeEnum::StructType(struct_ty) = raw_ty else {
            tracing::error!(
                ?raw_ty,
                "build_struct called with non-struct type — falling back"
            );
            self.record_codegen_error();
            return values.first().copied().unwrap_or_else(|| self.const_i64(0));
        };

        let mut result = struct_ty.get_undef();
        for (i, &val_id) in values.iter().enumerate() {
            let v = self.arena.get_value(val_id);
            let Some(agg) = self
                .builder
                .build_insert_value(result, v, i as u32, &format!("{name}.{i}"))
                .ok()
            else {
                tracing::error!(
                    index = i,
                    num_fields = struct_ty.count_fields(),
                    "build_struct: insert_value failed (index out of bounds?)"
                );
                self.record_codegen_error();
                return self.arena.push_value(struct_ty.get_undef().into());
            };
            match agg {
                inkwell::values::AggregateValueEnum::StructValue(sv) => result = sv,
                inkwell::values::AggregateValueEnum::ArrayValue(_) => {
                    tracing::error!(index = i, "build_struct insert_value returned array");
                    self.record_codegen_error();
                    return self.arena.push_value(struct_ty.get_undef().into());
                }
            }
        }
        self.arena.push_value(result.into())
    }
}
