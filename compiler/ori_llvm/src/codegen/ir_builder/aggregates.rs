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
            let msg = format!(
                "extract_value on non-struct value (index {index}) — type resolution produced wrong layout"
            );
            tracing::error!(?raw, index, "{msg}");
            self.record_codegen_error_with_msg(msg);
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
            let msg = format!(
                "insert_value on non-struct value (index {index}) — type resolution produced wrong layout"
            );
            tracing::error!(?raw_agg, index, "{msg}");
            self.record_codegen_error_with_msg(msg);
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

    /// Insert a value into a nested aggregate at a multi-index path.
    ///
    /// LLVM's `insertvalue` supports multi-index paths (e.g., `insertvalue %agg, val, 1, 0`
    /// to insert into `{ i64, [M x i64] }` at field 1, element 0). inkwell only exposes
    /// single-index `build_insert_value`, so for multi-index paths we decompose into:
    /// extract inner → insert into inner → insert modified inner back.
    ///
    /// For single-index paths, delegates directly to [`Self::insert_value`].
    pub fn insert_value_nested(
        &mut self,
        agg: ValueId,
        val: ValueId,
        indices: &[u32],
        name: &str,
    ) -> ValueId {
        match indices.len() {
            0 => {
                tracing::error!("insert_value_nested with empty indices");
                self.record_codegen_error();
                agg
            }
            1 => self.insert_value(agg, val, indices[0], name),
            _ => {
                // Multi-index: extract the inner aggregate, insert into it, re-insert.
                // For path [1, i]: extract field 1 (the array), insert val at index i,
                // then insert the modified array back at field 1.
                let outer_idx = indices[0];
                let inner_indices = &indices[1..];

                // Extract the inner aggregate (e.g., the [M x i64] payload array).
                let inner = self.extract_value_any(agg, outer_idx, &format!("{name}.inner"));

                // Recursively insert into the inner aggregate.
                let modified_inner = self.insert_value_nested_raw(inner, val, inner_indices, name);

                // Re-insert the modified inner aggregate at the outer index.
                self.insert_value(agg, modified_inner, outer_idx, &format!("{name}.reinsert"))
            }
        }
    }

    /// Extract a value from an aggregate at the given index.
    ///
    /// Unlike [`Self::extract_value`], this works on any aggregate (struct or array),
    /// returning the raw value without requiring the outer to be a struct.
    pub fn extract_value_any(&mut self, agg: ValueId, index: u32, name: &str) -> ValueId {
        let raw = self.arena.get_value(agg);
        match raw {
            BasicValueEnum::StructValue(v) => {
                let result = self
                    .builder
                    .build_extract_value(v, index, name)
                    .expect("extract_value on struct");
                self.arena.push_value(result)
            }
            BasicValueEnum::ArrayValue(v) => {
                let result = self
                    .builder
                    .build_extract_value(v, index, name)
                    .expect("extract_value on array");
                self.arena.push_value(result)
            }
            _ => {
                let msg = format!(
                    "extract_value_any on non-aggregate value (index {index}) — got {raw:?}"
                );
                tracing::error!("{msg}");
                self.record_codegen_error_with_msg(msg);
                self.const_i64(0)
            }
        }
    }

    /// Raw recursive helper for multi-index `insertvalue` on inner aggregates.
    ///
    /// Handles the case where the inner value may be an array (not just a struct).
    fn insert_value_nested_raw(
        &mut self,
        agg: ValueId,
        val: ValueId,
        indices: &[u32],
        name: &str,
    ) -> ValueId {
        debug_assert!(
            !indices.is_empty(),
            "insert_value_nested_raw needs at least 1 index"
        );
        let raw_agg = self.arena.get_value(agg);
        let v = self.arena.get_value(val);

        if indices.len() == 1 {
            // Base case: single index insert into the inner aggregate.
            let result = match raw_agg {
                BasicValueEnum::StructValue(a) => self
                    .builder
                    .build_insert_value(a, v, indices[0], name)
                    .expect("insert_value into struct"),
                BasicValueEnum::ArrayValue(a) => self
                    .builder
                    .build_insert_value(a, v, indices[0], name)
                    .expect("insert_value into array"),
                _ => {
                    let msg = format!(
                        "insert_value_nested_raw on non-aggregate (index {}) — got {raw_agg:?}",
                        indices[0]
                    );
                    tracing::error!("{msg}");
                    self.record_codegen_error_with_msg(msg);
                    return agg;
                }
            };
            match result {
                inkwell::values::AggregateValueEnum::StructValue(sv) => {
                    self.arena.push_value(sv.into())
                }
                inkwell::values::AggregateValueEnum::ArrayValue(av) => {
                    self.arena.push_value(av.into())
                }
            }
        } else {
            // Recursive case: extract, recurse, re-insert.
            let inner = self.extract_value_any(agg, indices[0], &format!("{name}.inner"));
            let modified = self.insert_value_nested_raw(inner, val, &indices[1..], name);
            self.insert_value_nested_raw(agg, modified, &indices[..1], name)
        }
    }

    /// Build a struct from values by successive `insert_value`.
    pub fn build_struct(&mut self, ty: LLVMTypeId, values: &[ValueId], name: &str) -> ValueId {
        let raw_ty = self.arena.get_type(ty);

        // Defensive: verify this is actually a struct type
        let BasicTypeEnum::StructType(struct_ty) = raw_ty else {
            let msg = format!(
                "build_struct called with non-struct LLVM type ({raw_ty:?}) — type resolution produced wrong layout"
            );
            tracing::error!("{msg}");
            self.record_codegen_error_with_msg(msg);
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
