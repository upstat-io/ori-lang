//! Composite and Ori-specific debug type creation for [`DebugInfoBuilder`].
//!
//! Provides methods for creating DWARF type descriptions for:
//! - **Generic composites**: struct, enum, pointer, array, typedef
//! - **Ori built-in types**: str, Option, Result, list

use inkwell::debug_info::{
    AsDIScope, DICompositeType, DIFlags, DIFlagsConstants, DISubroutineType, DIType,
};

use super::builder::{DebugInfoBuilder, FieldInfo};
use super::config::DebugInfoError;

impl<'ctx> DebugInfoBuilder<'ctx> {
    /// Create a subroutine (function) type.
    pub fn create_subroutine_type(
        &self,
        return_type: Option<DIType<'ctx>>,
        param_types: &[DIType<'ctx>],
    ) -> DISubroutineType<'ctx> {
        self.inner
            .create_subroutine_type(self.file(), return_type, param_types, DIFlags::ZERO)
    }

    /// Create a struct type with fields.
    pub fn create_struct_type(
        &self,
        name: &str,
        line: u32,
        size_bits: u64,
        align_bits: u32,
        fields: &[FieldInfo<'_, 'ctx>],
    ) -> DICompositeType<'ctx> {
        let member_types: Vec<DIType<'ctx>> = fields
            .iter()
            .map(|field| {
                self.inner
                    .create_member_type(
                        self.compile_unit.as_debug_info_scope(),
                        field.name,
                        self.file(),
                        field.line,
                        field.size_bits,
                        align_bits,
                        field.offset_bits,
                        DIFlags::ZERO,
                        field.ty,
                    )
                    .as_type()
            })
            .collect();

        self.inner.create_struct_type(
            self.compile_unit.as_debug_info_scope(),
            name,
            self.file(),
            line,
            size_bits,
            align_bits,
            DIFlags::ZERO,
            None, // No base type
            &member_types,
            0,    // Runtime language
            None, // No vtable holder
            name, // Unique identifier
        )
    }

    /// Create an enum/sum type with discriminant variants.
    pub fn create_enum_type(
        &self,
        name: &str,
        line: u32,
        size_bits: u64,
        align_bits: u32,
        variants: &[(&str, i64)],
        underlying_type: DIType<'ctx>,
    ) -> DICompositeType<'ctx> {
        let enumerators: Vec<_> = variants
            .iter()
            .map(|(variant_name, value)| self.inner.create_enumerator(variant_name, *value, false))
            .collect();

        self.inner.create_enumeration_type(
            self.compile_unit.as_debug_info_scope(),
            name,
            self.file(),
            line,
            size_bits,
            align_bits,
            &enumerators,
            underlying_type,
        )
    }

    /// Create a pointer type.
    pub fn create_pointer_type(
        &self,
        name: &str,
        pointee: DIType<'ctx>,
        size_bits: u64,
    ) -> DIType<'ctx> {
        self.inner
            .create_pointer_type(
                name,
                pointee,
                size_bits,
                size_bits as u32, // alignment = size for pointers
                inkwell::AddressSpace::default(),
            )
            .as_type()
    }

    /// Create an array type with a 1D subscript range.
    // Single-element vec with range is intentional here for LLVM's debug info API
    // which requires a slice of subscript ranges even for 1D arrays.
    #[allow(
        clippy::single_range_in_vec_init,
        reason = "LLVM debug API requires a slice of subscript ranges even for 1D arrays"
    )]
    pub fn create_array_type(
        &self,
        element_type: DIType<'ctx>,
        count: u64,
        size_bits: u64,
        align_bits: u32,
    ) -> DICompositeType<'ctx> {
        let subscripts = if count > 0 {
            vec![0..(count as i64)]
        } else {
            vec![]
        };

        self.inner
            .create_array_type(element_type, size_bits, align_bits, &subscripts)
    }

    /// Create a typedef (type alias).
    pub fn create_typedef(
        &self,
        name: &str,
        underlying: DIType<'ctx>,
        line: u32,
        size_bits: u64,
    ) -> DIType<'ctx> {
        self.inner
            .create_typedef(
                underlying,
                name,
                self.file(),
                line,
                self.compile_unit.as_debug_info_scope(),
                size_bits as u32,
            )
            .as_type()
    }

    // -- Ori-specific type helpers --

    /// Create debug info for Ori's string type: `{ len: int, cap: int, data: *byte }`.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying int or byte types.
    pub fn string_type(&self) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let int_ty = self.int_type()?.as_type();
        let ptr_ty = self.create_pointer_type("*byte", self.byte_type()?.as_type(), 64);

        let fields = [
            FieldInfo {
                name: "len",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "cap",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 0,
            },
            FieldInfo {
                name: "data",
                ty: ptr_ty,
                size_bits: 64,
                offset_bits: 128,
                line: 0,
            },
        ];

        Ok(self.create_struct_type("str", 0, 192, 64, &fields))
    }

    /// Create debug info for `Option<T>`: `{ tag: byte, payload: T }`.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying byte type for the tag.
    pub fn option_type(
        &self,
        payload_ty: DIType<'ctx>,
        payload_size_bits: u64,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let byte_ty = self.byte_type()?.as_type();

        // Alignment: max of tag (8) and payload alignment
        let align_bits = 64u32; // Assume 8-byte alignment for simplicity

        // Option enum: None=0, Some=1
        let tag_ty =
            self.create_enum_type("OptionTag", 0, 8, 8, &[("None", 0), ("Some", 1)], byte_ty);

        let fields = [
            FieldInfo {
                name: "tag",
                ty: tag_ty.as_type(),
                size_bits: 8,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "payload",
                ty: payload_ty,
                size_bits: payload_size_bits,
                offset_bits: 64, // Aligned to 8 bytes
                line: 0,
            },
        ];

        let total_size = 64 + payload_size_bits; // tag + padding + payload
        Ok(self.create_struct_type("Option", 0, total_size, align_bits, &fields))
    }

    /// Create debug info for `Result<T, E>`: `{ tag: byte, payload: union }`.
    ///
    /// The payload size is the maximum of ok and error sizes, representing
    /// the union semantics of a sum type where either variant can occupy the space.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying byte type for the tag.
    pub fn result_type(
        &self,
        ok_ty: DIType<'ctx>,
        ok_size_bits: u64,
        err_ty: DIType<'ctx>,
        err_size_bits: u64,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let byte_ty = self.byte_type()?.as_type();

        // Result enum: Ok=0, Err=1
        let tag_ty = self.create_enum_type("ResultTag", 0, 8, 8, &[("Ok", 0), ("Err", 1)], byte_ty);

        // Use the larger of ok and error sizes for proper union semantics
        let payload_size = ok_size_bits.max(err_size_bits);
        // Use the type with the larger size for the payload field in debug info
        let (payload_ty, payload_name) = if ok_size_bits >= err_size_bits {
            (ok_ty, "ok_payload")
        } else {
            (err_ty, "err_payload")
        };

        let fields = [
            FieldInfo {
                name: "tag",
                ty: tag_ty.as_type(),
                size_bits: 8,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: payload_name,
                ty: payload_ty,
                size_bits: payload_size,
                offset_bits: 64,
                line: 0,
            },
        ];

        let total_size = 64 + payload_size;
        Ok(self.create_struct_type("Result", 0, total_size, 64, &fields))
    }

    /// Create debug info for a list type: `{ len, cap, data }`.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying int type for length and capacity.
    pub fn list_type(
        &self,
        element_ty: DIType<'ctx>,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let int_ty = self.int_type()?.as_type();
        let ptr_ty = self.create_pointer_type("*elem", element_ty, 64);

        let fields = [
            FieldInfo {
                name: "len",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "cap",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 0,
            },
            FieldInfo {
                name: "data",
                ty: ptr_ty,
                size_bits: 64,
                offset_bits: 128,
                line: 0,
            },
        ];

        Ok(self.create_struct_type("[T]", 0, 192, 64, &fields))
    }
}
