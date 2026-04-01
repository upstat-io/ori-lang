//! Type introspection helpers for LLVM values and types.

use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

impl IrBuilder<'_, '_> {
    /// Check if a value is a single-word type (fits in one i64 slot).
    ///
    /// Returns `true` for `i64`, `i32`, `i16`, `i8`, `i1`, `f64`, `ptr` —
    /// all types that occupy at most 8 bytes. Returns `false` for structs,
    /// arrays, and other multi-word aggregates.
    ///
    /// Used by variant construction to determine if the `insertvalue` chain
    /// optimization is safe: only single-word fields can be inserted into
    /// the `[M x i64]` payload array by value.
    pub fn is_single_word(&self, id: ValueId) -> bool {
        let val = self.arena.get_value(id);
        matches!(
            val,
            BasicValueEnum::IntValue(_)
                | BasicValueEnum::FloatValue(_)
                | BasicValueEnum::PointerValue(_)
        )
    }

    /// Check if an LLVM type is a single-slot scalar (fits in one i64 payload slot).
    ///
    /// Returns `true` for integer, float, and pointer types. Returns `false`
    /// for struct, array, and vector types (multi-word aggregates).
    ///
    /// Used by enum payload extraction to decide between the `extractvalue`
    /// path (scalar fields) and the alloca+GEP+load path (multi-word fields).
    pub fn is_single_slot_type(&self, ty: LLVMTypeId) -> bool {
        let raw = self.arena.get_type(ty);
        matches!(
            raw,
            BasicTypeEnum::IntType(_) | BasicTypeEnum::FloatType(_) | BasicTypeEnum::PointerType(_)
        )
    }

    /// Check if a value is a struct (SSA aggregate), not a pointer or scalar.
    pub fn is_struct_value(&self, val: ValueId) -> bool {
        matches!(self.arena.get_value(val), BasicValueEnum::StructValue(_))
    }

    /// Check if a value is a pointer (used by §07.2 niche switch for ptr niches).
    pub fn is_pointer_value(&self, val: ValueId) -> bool {
        matches!(self.arena.get_value(val), BasicValueEnum::PointerValue(_))
    }

    /// Reinterpret raw `i64` bits as the target type.
    ///
    /// Enum payloads store all fields as `i64` in a `[N x i64]` array.
    /// After `extractvalue`, we get the raw `i64` and need to reinterpret
    /// the bits as the actual field type:
    /// - `i64 → i64`: identity (no-op)
    /// - `i64 → double`: `bitcast`
    /// - `i64 → i1/i8/i32`: `trunc`
    /// - `i64 → ptr`: `inttoptr`
    pub fn reinterpret_from_i64(
        &mut self,
        val: ValueId,
        target_ty: LLVMTypeId,
        name: &str,
    ) -> ValueId {
        let raw = self.arena.get_type(target_ty);
        match raw {
            BasicTypeEnum::IntType(it) => {
                if it.get_bit_width() == 64 {
                    val
                } else {
                    self.trunc(val, target_ty, name)
                }
            }
            BasicTypeEnum::FloatType(_) => self.bitcast(val, target_ty, name),
            BasicTypeEnum::PointerType(_) => self.int_to_ptr(val, name),
            _ => {
                tracing::error!(
                    ?raw,
                    "reinterpret_from_i64: unexpected non-scalar target type"
                );
                self.record_codegen_error();
                val
            }
        }
    }

    /// Check if a struct type's field at the given index is an array type.
    ///
    /// Used by variant construction to detect enum payload layout:
    /// - `{ i64, [M x i64] }` (user-defined enums) has an array at field 1
    /// - `{ i64, T }` (Option/Result) has a direct type at field 1
    ///
    /// Returns `false` if the type is not a struct or the field index is
    /// out of bounds.
    pub fn is_struct_field_array(&self, ty: LLVMTypeId, field_index: u32) -> bool {
        let llvm_ty = self.arena.get_type(ty);
        let BasicTypeEnum::StructType(st) = llvm_ty else {
            return false;
        };
        if field_index >= st.count_fields() {
            return false;
        }
        matches!(
            st.get_field_type_at_index(field_index),
            Some(BasicTypeEnum::ArrayType(_))
        )
    }

    /// Get the LLVM type of a struct's field at the given index.
    ///
    /// Returns `None` if the type is not a struct or the field index is
    /// out of bounds. Used by enum tag access to determine the tag's LLVM
    /// type (i8/i16/i32/i64) after discriminant narrowing (§07.1).
    pub fn struct_field_type(&mut self, ty: LLVMTypeId, field_index: u32) -> Option<LLVMTypeId> {
        let llvm_ty = self.arena.get_type(ty);
        let BasicTypeEnum::StructType(st) = llvm_ty else {
            return None;
        };
        let field_ty = st.get_field_type_at_index(field_index)?;
        Some(self.arena.push_type(field_ty))
    }

    /// Create an integer constant whose type matches a struct's field at
    /// `field_index`. Used to build tag constants that match the narrowed
    /// tag type (i8/i16/i32/i64) of enum structs (§07.1).
    ///
    /// Falls back to `const_i64` if the struct type or field is unavailable.
    pub fn const_int_for_struct_field(
        &mut self,
        struct_ty: LLVMTypeId,
        field_index: u32,
        val: u64,
    ) -> ValueId {
        let llvm_ty = self.arena.get_type(struct_ty);
        let BasicTypeEnum::StructType(st) = llvm_ty else {
            return self.const_i64(val as i64);
        };
        let Some(field_ty) = st.get_field_type_at_index(field_index) else {
            return self.const_i64(val as i64);
        };
        let BasicTypeEnum::IntType(int_ty) = field_ty else {
            return self.const_i64(val as i64);
        };
        let v = int_ty.const_int(val, false);
        self.arena.push_value(v.into())
    }

    /// Check if a value's LLVM type matches a struct field's type at the given index.
    ///
    /// Used by variant construction to determine if `insertvalue` is type-safe
    /// for Option/Result layouts where the payload slot type may differ from
    /// the value being stored (e.g., `Ok(42)` stores i64 into a `{ i64, i64, ptr }` slot).
    pub fn value_type_matches_struct_field(
        &self,
        struct_ty: LLVMTypeId,
        field_index: u32,
        val: ValueId,
    ) -> bool {
        let llvm_ty = self.arena.get_type(struct_ty);
        let BasicTypeEnum::StructType(st) = llvm_ty else {
            return false;
        };
        let Some(field_ty) = st.get_field_type_at_index(field_index) else {
            return false;
        };
        let val_ty = self.arena.get_value(val).get_type();
        field_ty == val_ty
    }
}
