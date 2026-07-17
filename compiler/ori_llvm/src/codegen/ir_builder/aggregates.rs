//! Aggregate operations (extract, insert, struct construction) for `IrBuilder`.

use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

impl IrBuilder<'_, '_> {
    /// Extract a value from an aggregate (struct/array) by index.
    pub fn extract_value(&mut self, agg: ValueId, index: u32, name: &str) -> Option<ValueId> {
        let raw = self.arena.get_value(agg);
        let result = match raw {
            BasicValueEnum::StructValue(value) => {
                self.builder.build_extract_value(value, index, name)
            }
            BasicValueEnum::ArrayValue(value) => {
                self.builder.build_extract_value(value, index, name)
            }
            _ => {
                let msg = format!(
                    "extract_value on non-aggregate value (index {index}) — type resolution produced wrong layout"
                );
                tracing::error!(?raw, index, "{msg}");
                self.record_codegen_error_with_msg(msg);
                return None;
            }
        };
        match result {
            Ok(value) => Some(self.arena.push_value(value)),
            Err(error) => {
                let msg = format!("extract_value failed at index {index}: {error}");
                tracing::error!(?raw, index, "{msg}");
                self.record_codegen_error_with_msg(msg);
                None
            }
        }
    }

    /// Insert a value into an aggregate at the given index.
    pub fn insert_value(&mut self, agg: ValueId, val: ValueId, index: u32, name: &str) -> ValueId {
        let raw_agg = self.arena.get_value(agg);
        let v = self.arena.get_value(val);
        let result = match raw_agg {
            BasicValueEnum::StructValue(aggregate) => {
                self.builder.build_insert_value(aggregate, v, index, name)
            }
            BasicValueEnum::ArrayValue(aggregate) => {
                self.builder.build_insert_value(aggregate, v, index, name)
            }
            _ => {
                let msg = format!(
                    "insert_value on non-aggregate value (index {index}) — type resolution produced wrong layout"
                );
                tracing::error!(?raw_agg, index, "{msg}");
                self.record_codegen_error_with_msg(&msg);
                panic!("{msg}");
            }
        };
        let Ok(result) = result else {
            panic!("insert_value index and value type must match the aggregate");
        };
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
        assert!(
            !indices.is_empty(),
            "insert_value_nested needs at least one index"
        );
        match indices.len() {
            0 => unreachable!("empty index path rejected above"),
            1 => self.insert_value(agg, val, indices[0], name),
            _ => {
                // Multi-index: extract the inner aggregate, insert into it, re-insert.
                // For path [1, i]: extract field 1 (the array), insert val at index i,
                // then insert the modified array back at field 1.
                let outer_idx = indices[0];
                let inner_indices = &indices[1..];

                // Extract the inner aggregate (e.g., the [M x i64] payload array).
                let inner = self.extract_value_any(agg, outer_idx, name);

                // Recursively insert into the inner aggregate.
                let modified_inner = self.insert_value_nested_raw(inner, val, inner_indices, name);

                // Re-insert the modified inner aggregate at the outer index.
                self.insert_value(agg, modified_inner, outer_idx, name)
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
                panic!("extract_value_any requires an aggregate value");
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
        assert!(
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
                    panic!("insert_value_nested_raw requires an aggregate value");
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
            let inner = self.extract_value_any(agg, indices[0], name);
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
            panic!("build_struct requires a struct LLVM type");
        };

        let Ok(field_count) = usize::try_from(struct_ty.count_fields()) else {
            panic!("LLVM struct field count must fit host usize");
        };
        assert_eq!(
            values.len(),
            field_count,
            "build_struct value count must match the LLVM struct field count"
        );

        let mut result = struct_ty.get_undef();
        for (i, &val_id) in values.iter().enumerate() {
            let v = self.arena.get_value(val_id);
            let Ok(index) = u32::try_from(i) else {
                panic!("struct field index must fit u32");
            };
            let Ok(agg) = self.builder.build_insert_value(result, v, index, name) else {
                panic!("struct field index and value type must match the LLVM layout");
            };
            match agg {
                inkwell::values::AggregateValueEnum::StructValue(sv) => result = sv,
                inkwell::values::AggregateValueEnum::ArrayValue(_) => {
                    unreachable!("inserting into a struct must return a struct")
                }
            }
        }
        self.arena.push_value(result.into())
    }
}
