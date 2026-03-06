//! Constant value construction for `IrBuilder`.

use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, UnnamedAddress};

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Create an i8 constant.
    #[inline]
    pub fn const_i8(&mut self, val: i8) -> ValueId {
        let v = self.scx.type_i8().const_int(val as u64, val < 0);
        self.arena.push_value(v.into())
    }

    /// Create an i32 constant.
    #[inline]
    pub fn const_i32(&mut self, val: i32) -> ValueId {
        let v = self.scx.type_i32().const_int(val as u64, val < 0);
        self.arena.push_value(v.into())
    }

    /// Create an i64 constant.
    #[inline]
    pub fn const_i64(&mut self, val: i64) -> ValueId {
        let v = self.scx.type_i64().const_int(val as u64, val < 0);
        self.arena.push_value(v.into())
    }

    /// Create an f64 constant.
    #[inline]
    pub fn const_f64(&mut self, val: f64) -> ValueId {
        let v = self.scx.type_f64().const_float(val);
        self.arena.push_value(v.into())
    }

    /// Create an i1 (boolean) constant.
    #[inline]
    pub fn const_bool(&mut self, val: bool) -> ValueId {
        let v = self.scx.type_i1().const_int(u64::from(val), false);
        self.arena.push_value(v.into())
    }

    /// Create an integer constant matching the type of an existing value.
    ///
    /// Used by switch emission to ensure case constants have the same LLVM
    /// integer type as the scrutinee (e.g. `i1` for bools, `i32` for chars).
    pub fn const_int_matching(&mut self, reference: ValueId, val: u64) -> ValueId {
        let ref_val = self.arena.get_value(reference);
        let int_type = ref_val.into_int_value().get_type();
        let v = int_type.const_int(val, false);
        self.arena.push_value(v.into())
    }

    /// Create a null pointer constant.
    #[inline]
    pub fn const_null_ptr(&mut self) -> ValueId {
        let v = self.scx.type_ptr().const_null();
        self.arena.push_value(v.into())
    }

    /// Create a zero/null constant from an `LLVMTypeId`.
    ///
    /// Convenience wrapper around [`Self::const_zero`] that resolves the
    /// type ID first. Used for zero-initializing enum allocas to prevent
    /// LLVM poison values in unwritten payload fields.
    pub fn const_zero_ty(&mut self, ty_id: super::super::value_id::LLVMTypeId) -> ValueId {
        let ty = self.arena.get_type(ty_id);
        self.const_zero(ty)
    }

    /// Create a zero/null constant of any LLVM basic type.
    ///
    /// Used for zero-initializing Option/Result payloads when the inner
    /// type is not i64 (e.g., `option[bool]` needs an `i1 0` payload,
    /// `option[str]` needs a `{i64 0, ptr null}` payload).
    pub fn const_zero(&mut self, ty: BasicTypeEnum<'ctx>) -> ValueId {
        let v: BasicValueEnum<'ctx> = match ty {
            BasicTypeEnum::IntType(t) => t.const_int(0, false).into(),
            BasicTypeEnum::FloatType(t) => t.const_float(0.0).into(),
            BasicTypeEnum::StructType(t) => t.const_zero().into(),
            BasicTypeEnum::PointerType(t) => t.const_null().into(),
            BasicTypeEnum::ArrayType(t) => t.const_zero().into(),
            BasicTypeEnum::VectorType(t) => t.const_zero().into(),
            BasicTypeEnum::ScalableVectorType(_) => {
                // Scalable vectors don't support const_zero; fall back to i64.
                self.scx.type_i64().const_int(0, false).into()
            }
        };
        self.arena.push_value(v)
    }

    /// Create a constant string value (non-null-terminated byte array).
    pub fn const_string(&mut self, s: &[u8]) -> ValueId {
        let v = self.scx.llcx.const_string(s, false);
        self.arena.push_value(v.into())
    }

    /// Create a global null-terminated string and return a pointer to it.
    ///
    /// Deduplicates by content: if an identical string has already been
    /// emitted, returns the existing global pointer. Deduplicated globals
    /// are marked `unnamed_addr` to enable linker-level COMDAT folding.
    pub fn build_global_string_ptr(&mut self, value: &str, name: &str) -> ValueId {
        if let Some(&cached) = self.global_strings.get(value) {
            return cached;
        }
        let global = self
            .builder
            .build_global_string_ptr(value, name)
            .expect("build_global_string_ptr");
        // Address is irrelevant — only content matters. Enables linker merging.
        global.set_unnamed_address(UnnamedAddress::Global);
        let v = global.as_pointer_value();
        let id = self.arena.push_value(v.into());
        self.global_strings.insert(value.to_string(), id);
        id
    }
}
