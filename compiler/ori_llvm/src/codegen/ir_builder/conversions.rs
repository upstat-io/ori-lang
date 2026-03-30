//! Type conversion operations (casts, extensions, truncations) for `IrBuilder`.

use super::IrBuilder;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

impl IrBuilder<'_, '_> {
    /// Build a bitcast.
    pub fn bitcast(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty);
        let result = self
            .builder
            .build_bit_cast(v, target, name)
            .expect("bitcast");
        self.arena.push_value(result)
    }

    /// Build integer truncation (to a smaller integer type).
    pub fn trunc(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "trunc on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_truncate(v.into_int_value(), target, name)
            .expect("trunc");
        self.arena.push_value(result.into())
    }

    /// Sign-extend an integer value to i64 if it is narrower. Returns the
    /// value unchanged if it is already i64 or wider, or if it is not an
    /// integer. Used by derive codegen to normalize narrowed struct fields
    /// (i8/i16/i32 from integer narrowing) back to canonical width.
    pub fn sext_to_i64_if_narrower(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            return val;
        }
        let iv = v.into_int_value();
        if iv.get_type().get_bit_width() >= 64 {
            return val;
        }
        let i64_ty = self.scx.type_i64();
        let result = self
            .builder
            .build_int_s_extend(iv, i64_ty, name)
            .expect("sext_to_i64");
        self.arena.push_value(result.into())
    }

    /// Build sign extension (to a larger integer type).
    pub fn sext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "sext on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_s_extend(v.into_int_value(), target, name)
            .expect("sext");
        self.arena.push_value(result.into())
    }

    /// Build zero extension (to a larger integer type).
    pub fn zext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "zext on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_z_extend(v.into_int_value(), target, name)
            .expect("zext");
        self.arena.push_value(result.into())
    }

    /// Build signed integer to floating-point conversion.
    pub fn si_to_fp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_float_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "si_to_fp on non-int operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_signed_int_to_float(v.into_int_value(), target, name)
            .expect("si_to_fp");
        self.arena.push_value(result.into())
    }

    /// Build floating-point to signed integer conversion.
    pub fn fp_to_si(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fp_to_si on non-float operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_float_to_signed_int(v.into_float_value(), target, name)
            .expect("fp_to_si");
        self.arena.push_value(result.into())
    }

    /// Build unsigned integer to floating-point conversion.
    pub fn uitofp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_float_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "uitofp on non-int operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_unsigned_int_to_float(v.into_int_value(), target, name)
            .expect("uitofp");
        self.arena.push_value(result.into())
    }

    /// Build floating-point to unsigned integer conversion.
    pub fn fptoui(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fptoui on non-float operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_float_to_unsigned_int(v.into_float_value(), target, name)
            .expect("fptoui");
        self.arena.push_value(result.into())
    }

    /// Extend a float value to f64 if it is narrower (f32). Returns the
    /// value unchanged if it is already f64 or if it is not a float.
    /// Used by derive hash codegen to normalize narrowed struct fields
    /// (f32 from float narrowing) back to canonical width.
    pub fn fpext_to_f64_if_narrower(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_float_value() {
            return val;
        }
        let fv = v.into_float_value();
        let f64_ty = self.scx.type_f64();
        if fv.get_type() == f64_ty {
            return val;
        }
        let result = self
            .builder
            .build_float_ext(fv, f64_ty, &format!("{name}.fpext"))
            .expect("fpext_to_f64");
        self.arena.push_value(result.into())
    }

    /// Build float truncation (e.g., f64 → f32).
    pub fn float_trunc(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_float_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "float_trunc on non-float operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_float_trunc(v.into_float_value(), target, name)
            .expect("float_trunc");
        self.arena.push_value(result.into())
    }

    /// Build float extension (e.g., f32 → f64).
    pub fn float_ext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_float_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "float_ext on non-float operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_float_ext(v.into_float_value(), target, name)
            .expect("float_ext");
        self.arena.push_value(result.into())
    }

    /// Build pointer-to-integer conversion.
    pub fn ptr_to_int(&mut self, ptr: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let p = self.arena.get_value(ptr);
        let target = self.arena.get_type(ty).into_int_type();
        if !p.is_pointer_value() {
            tracing::error!(val_type = ?p.get_type(), "ptr_to_int on non-pointer operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_ptr_to_int(p.into_pointer_value(), target, name)
            .expect("ptr_to_int");
        self.arena.push_value(result.into())
    }

    /// Build integer-to-pointer conversion.
    pub fn int_to_ptr(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "int_to_ptr on non-int operand");
            self.record_codegen_error();
            return self.const_null_ptr();
        }
        let result = self
            .builder
            .build_int_to_ptr(v.into_int_value(), self.scx.type_ptr(), name)
            .expect("int_to_ptr");
        self.arena.push_value(result.into())
    }
}
