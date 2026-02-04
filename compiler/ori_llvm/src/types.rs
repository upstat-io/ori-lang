//! LLVM type mapping and construction helpers.

use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::BasicValueEnum;
use ori_ir::TypeId;

use crate::builder::Builder;
use crate::context::CodegenCx;

impl<'ll> CodegenCx<'ll, '_> {
    /// Map a Ori `TypeId` to an LLVM metadata type (for function params).
    pub fn llvm_metadata_type(&self, type_id: TypeId) -> BasicMetadataTypeEnum<'ll> {
        self.llvm_type(type_id).into()
    }
}

impl<'ll> Builder<'_, 'll, '_> {
    /// Coerce a value to i64 for storage in tagged unions.
    ///
    /// This is used for Option/Result payloads which use a standardized
    /// i64 payload slot for consistent ABI.
    pub(crate) fn coerce_to_i64(
        &self,
        val: BasicValueEnum<'ll>,
    ) -> Option<inkwell::values::IntValue<'ll>> {
        match val {
            BasicValueEnum::IntValue(i) => {
                let bit_width = i.get_type().get_bit_width();
                match bit_width.cmp(&64) {
                    std::cmp::Ordering::Equal => Some(i),
                    std::cmp::Ordering::Less => {
                        // Zero-extend smaller integers
                        Some(
                            self.raw_builder()
                                .build_int_z_extend(i, self.cx().scx.type_i64(), "coerce_zext")
                                .ok()?,
                        )
                    }
                    std::cmp::Ordering::Greater => {
                        // Truncate larger integers (shouldn't happen with our types)
                        Some(
                            self.raw_builder()
                                .build_int_truncate(i, self.cx().scx.type_i64(), "coerce_trunc")
                                .ok()?,
                        )
                    }
                }
            }
            BasicValueEnum::FloatValue(f) => {
                // Convert float to bits for storage
                Some(
                    self.raw_builder()
                        .build_bit_cast(f, self.cx().scx.type_i64(), "coerce_float")
                        .ok()?
                        .into_int_value(),
                )
            }
            BasicValueEnum::PointerValue(p) => {
                // Convert pointer to int
                Some(
                    self.raw_builder()
                        .build_ptr_to_int(p, self.cx().scx.type_i64(), "coerce_ptr")
                        .ok()?,
                )
            }
            _ => {
                // For other types, return a placeholder
                Some(self.cx().scx.type_i64().const_int(0, false))
            }
        }
    }

    /// Coerce an i64 value back to the target type.
    ///
    /// This is the inverse of `coerce_to_i64`, used when extracting
    /// Option/Result payloads.
    pub(crate) fn coerce_from_i64(
        &self,
        val: inkwell::values::IntValue<'ll>,
        target_type: TypeId,
    ) -> Option<BasicValueEnum<'ll>> {
        use inkwell::types::BasicTypeEnum;

        let llvm_type = self.cx().llvm_type(target_type);

        match llvm_type {
            BasicTypeEnum::IntType(int_ty) => {
                let bit_width = int_ty.get_bit_width();
                match bit_width.cmp(&64) {
                    std::cmp::Ordering::Equal => Some(val.into()),
                    std::cmp::Ordering::Less => {
                        // Truncate to smaller integer
                        Some(
                            self.raw_builder()
                                .build_int_truncate(val, int_ty, "coerce_trunc")
                                .ok()?
                                .into(),
                        )
                    }
                    std::cmp::Ordering::Greater => {
                        // Zero-extend to larger integer (shouldn't happen)
                        Some(
                            self.raw_builder()
                                .build_int_z_extend(val, int_ty, "coerce_zext")
                                .ok()?
                                .into(),
                        )
                    }
                }
            }
            BasicTypeEnum::FloatType(float_ty) => {
                // Convert bits back to float
                Some(
                    self.raw_builder()
                        .build_bit_cast(val, float_ty, "coerce_float")
                        .ok()?,
                )
            }
            BasicTypeEnum::PointerType(ptr_ty) => {
                // Convert int back to pointer
                Some(
                    self.raw_builder()
                        .build_int_to_ptr(val, ptr_ty, "coerce_ptr")
                        .ok()?
                        .into(),
                )
            }
            _ => {
                // For other types, just return the i64 (might not be correct but avoids crash)
                Some(val.into())
            }
        }
    }
}
