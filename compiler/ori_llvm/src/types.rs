//! LLVM type mapping and construction helpers.

use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::BasicValueEnum;
use ori_ir::TypeId;

use crate::LLVMCodegen;

impl<'ctx> LLVMCodegen<'ctx> {
    /// Map a Ori TypeId to an LLVM type.
    pub(crate) fn llvm_type(&self, type_id: TypeId) -> BasicTypeEnum<'ctx> {
        match type_id {
            TypeId::INT => self.context.i64_type().into(),
            TypeId::FLOAT => self.context.f64_type().into(),
            TypeId::BOOL => self.context.bool_type().into(),
            TypeId::CHAR => self.context.i32_type().into(), // Unicode codepoint
            TypeId::BYTE => self.context.i8_type().into(),
            // For now, other types become opaque pointers
            _ => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
        }
    }

    /// Map a Ori TypeId to an LLVM metadata type (for function params).
    pub(crate) fn llvm_metadata_type(&self, type_id: TypeId) -> BasicMetadataTypeEnum<'ctx> {
        self.llvm_type(type_id).into()
    }

    /// Get a default value for a type.
    pub(crate) fn default_value(&self, type_id: TypeId) -> BasicValueEnum<'ctx> {
        match type_id {
            TypeId::INT => self.context.i64_type().const_int(0, false).into(),
            TypeId::FLOAT => self.context.f64_type().const_float(0.0).into(),
            TypeId::BOOL => self.context.bool_type().const_int(0, false).into(),
            TypeId::CHAR => self.context.i32_type().const_int(0, false).into(),
            TypeId::BYTE => self.context.i8_type().const_int(0, false).into(),
            _ => self.context.ptr_type(inkwell::AddressSpace::default()).const_null().into(),
        }
    }

    /// Get a default value for an LLVM type.
    pub(crate) fn default_value_for_type(&self, ty: BasicTypeEnum<'ctx>) -> BasicValueEnum<'ctx> {
        match ty {
            BasicTypeEnum::IntType(t) => t.const_int(0, false).into(),
            BasicTypeEnum::FloatType(t) => t.const_float(0.0).into(),
            BasicTypeEnum::PointerType(t) => t.const_null().into(),
            BasicTypeEnum::StructType(t) => t.get_undef().into(),
            BasicTypeEnum::ArrayType(t) => t.get_undef().into(),
            BasicTypeEnum::VectorType(t) => t.get_undef().into(),
            BasicTypeEnum::ScalableVectorType(t) => t.get_undef().into(),
        }
    }

    /// Get the string type: { i64 len, i8* data }
    pub(crate) fn string_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Create an Option type (tag i8 + payload).
    ///
    /// Layout: { i8 tag, T value }
    /// - tag = 0: None
    /// - tag = 1: Some(value)
    pub(crate) fn option_type(&self, payload_type: BasicTypeEnum<'ctx>) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[self.context.i8_type().into(), payload_type],
            false,
        )
    }

    /// Create a Result type (tag i8 + payload).
    ///
    /// Layout: { i8 tag, max(T, E) value }
    /// - tag = 0: Ok(value)
    /// - tag = 1: Err(value)
    ///
    /// For simplicity, we use the same payload type for both Ok and Err.
    /// A more sophisticated implementation would use a union.
    pub(crate) fn result_type(&self, payload_type: BasicTypeEnum<'ctx>) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[self.context.i8_type().into(), payload_type],
            false,
        )
    }

    /// Get the list type: { i64 len, i64 cap, ptr data }
    pub(crate) fn list_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Get the map type: { i64 len, i64 cap, ptr keys, ptr vals }
    pub(crate) fn map_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Get the range type: { i64 start, i64 end, i1 inclusive }
    pub(crate) fn range_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
                self.context.bool_type().into(),
            ],
            false,
        )
    }
}
