//! Constant value construction for `IrBuilder`.

use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

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

    /// Create a null pointer constant.
    #[inline]
    pub fn const_null_ptr(&mut self) -> ValueId {
        let v = self.scx.type_ptr().const_null();
        self.arena.push_value(v.into())
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
    pub fn build_global_string_ptr(&mut self, value: &str, name: &str) -> ValueId {
        let v = self
            .builder
            .build_global_string_ptr(value, name)
            .expect("build_global_string_ptr")
            .as_pointer_value();
        self.arena.push_value(v.into())
    }
}
