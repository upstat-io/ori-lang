//! Type conversion operations (casts, extensions, truncations) for `IrBuilder`.

use inkwell::builder::Builder as InkwellBuilder;
use inkwell::types::BasicTypeEnum;
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

#[derive(Clone, Copy)]
enum ConversionValueKind {
    Int,
    Float,
}

impl ConversionValueKind {
    fn accepts(self, value: BasicValueEnum<'_>) -> bool {
        match self {
            Self::Int => value.is_int_value(),
            Self::Float => value.is_float_value(),
        }
    }

    fn zero(self, builder: &mut IrBuilder<'_, '_>) -> ValueId {
        match self {
            Self::Int => builder.const_i64(0),
            Self::Float => builder.const_f64(0.0),
        }
    }
}

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Emit a typed conversion after validating the operand kind.
    fn typed_conversion(
        &mut self,
        val: ValueId,
        ty: LLVMTypeId,
        name: &str,
        op: &str,
        source: ConversionValueKind,
        target: ConversionValueKind,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            BasicValueEnum<'ctx>,
            BasicTypeEnum<'ctx>,
            &str,
        ) -> BasicValueEnum<'ctx>,
    ) -> ValueId {
        let value = self.arena.get_value(val);
        let target_ty = self.arena.get_type(ty);
        if !source.accepts(value) {
            match source {
                ConversionValueKind::Int => {
                    tracing::error!(val_type = ?value.get_type(), "{op} on non-int operand");
                }
                ConversionValueKind::Float => {
                    tracing::error!(val_type = ?value.get_type(), "{op} on non-float operand");
                }
            }
            self.record_codegen_error();
            return target.zero(self);
        }
        let result = build(&self.builder, value, target_ty, name);
        self.arena.push_value(result)
    }

    /// Emit an int→int conversion after checking the operand is an int.
    /// On a non-int operand records a codegen error and returns `const_i64(0)`.
    fn int_to_int_conv(
        &mut self,
        val: ValueId,
        ty: LLVMTypeId,
        name: &str,
        op: &str,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            inkwell::values::IntValue<'ctx>,
            inkwell::types::IntType<'ctx>,
            &str,
        ) -> inkwell::values::IntValue<'ctx>,
    ) -> ValueId {
        self.typed_conversion(
            val,
            ty,
            name,
            op,
            ConversionValueKind::Int,
            ConversionValueKind::Int,
            |builder, value, target, label| {
                build(
                    builder,
                    value.into_int_value(),
                    target.into_int_type(),
                    label,
                )
                .into()
            },
        )
    }

    /// Emit an int→float conversion after checking the operand is an int.
    /// On a non-int operand records a codegen error and returns `const_f64(0.0)`.
    fn int_to_float_conv(
        &mut self,
        val: ValueId,
        ty: LLVMTypeId,
        name: &str,
        op: &str,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            inkwell::values::IntValue<'ctx>,
            inkwell::types::FloatType<'ctx>,
            &str,
        ) -> inkwell::values::FloatValue<'ctx>,
    ) -> ValueId {
        self.typed_conversion(
            val,
            ty,
            name,
            op,
            ConversionValueKind::Int,
            ConversionValueKind::Float,
            |builder, value, target, label| {
                build(
                    builder,
                    value.into_int_value(),
                    target.into_float_type(),
                    label,
                )
                .into()
            },
        )
    }

    /// Emit a float→int conversion after checking the operand is a float.
    /// On a non-float operand records a codegen error and returns `const_i64(0)`.
    fn float_to_int_conv(
        &mut self,
        val: ValueId,
        ty: LLVMTypeId,
        name: &str,
        op: &str,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            inkwell::values::FloatValue<'ctx>,
            inkwell::types::IntType<'ctx>,
            &str,
        ) -> inkwell::values::IntValue<'ctx>,
    ) -> ValueId {
        self.typed_conversion(
            val,
            ty,
            name,
            op,
            ConversionValueKind::Float,
            ConversionValueKind::Int,
            |builder, value, target, label| {
                build(
                    builder,
                    value.into_float_value(),
                    target.into_int_type(),
                    label,
                )
                .into()
            },
        )
    }

    /// Emit a float→float conversion after checking the operand is a float.
    /// On a non-float operand records a codegen error and returns `const_f64(0.0)`.
    fn float_to_float_conv(
        &mut self,
        val: ValueId,
        ty: LLVMTypeId,
        name: &str,
        op: &str,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            inkwell::values::FloatValue<'ctx>,
            inkwell::types::FloatType<'ctx>,
            &str,
        ) -> inkwell::values::FloatValue<'ctx>,
    ) -> ValueId {
        self.typed_conversion(
            val,
            ty,
            name,
            op,
            ConversionValueKind::Float,
            ConversionValueKind::Float,
            |builder, value, target, label| {
                build(
                    builder,
                    value.into_float_value(),
                    target.into_float_type(),
                    label,
                )
                .into()
            },
        )
    }

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
        self.int_to_int_conv(val, ty, name, "trunc", |b, v, t, n| {
            b.build_int_truncate(v, t, n).expect("trunc")
        })
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
        self.int_to_int_conv(val, ty, name, "sext", |b, v, t, n| {
            b.build_int_s_extend(v, t, n).expect("sext")
        })
    }

    /// Build zero extension (to a larger integer type).
    pub fn zext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        self.int_to_int_conv(val, ty, name, "zext", |b, v, t, n| {
            b.build_int_z_extend(v, t, n).expect("zext")
        })
    }

    /// Build signed integer to floating-point conversion.
    pub fn si_to_fp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        self.int_to_float_conv(val, ty, name, "si_to_fp", |b, v, t, n| {
            b.build_signed_int_to_float(v, t, n).expect("si_to_fp")
        })
    }

    /// Build a SATURATING floating-point to signed integer conversion via
    /// `llvm.fptosi.sat` — NaN maps to 0, out-of-range values clamp to the
    /// target's MIN/MAX. Matches Rust `as` semantics (and therefore the
    /// interpreter's `f as int` cast, which uses Rust `as`); raw `fptosi`
    /// is poison on those inputs.
    pub fn fp_to_si_sat(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        use inkwell::intrinsics::Intrinsic;
        use inkwell::values::BasicMetadataValueEnum;

        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fp_to_si_sat on non-float operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let Some(intrinsic) = Intrinsic::find("llvm.fptosi.sat") else {
            tracing::error!("llvm.fptosi.sat intrinsic not found");
            self.record_codegen_error();
            return self.const_i64(0);
        };
        let float_ty = v.get_type();
        let Some(func_val) = intrinsic.get_declaration(&self.scx.llmod, &[target.into(), float_ty])
        else {
            tracing::error!("failed to declare llvm.fptosi.sat");
            self.record_codegen_error();
            return self.const_i64(0);
        };
        let arg: BasicMetadataValueEnum<'_> = v.into_float_value().into();
        let call = self
            .builder
            .build_call(func_val, &[arg], name)
            .expect("fptosi.sat call");
        let result = call
            .try_as_basic_value()
            .basic()
            .expect("fptosi.sat returns a value");
        self.arena.push_value(result)
    }

    /// Build floating-point to signed integer conversion.
    pub fn fp_to_si(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        self.float_to_int_conv(val, ty, name, "fp_to_si", |b, v, t, n| {
            b.build_float_to_signed_int(v, t, n).expect("fp_to_si")
        })
    }

    /// Build unsigned integer to floating-point conversion.
    pub fn uitofp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        self.int_to_float_conv(val, ty, name, "uitofp", |b, v, t, n| {
            b.build_unsigned_int_to_float(v, t, n).expect("uitofp")
        })
    }

    /// Build floating-point to unsigned integer conversion.
    pub fn fptoui(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        self.float_to_int_conv(val, ty, name, "fptoui", |b, v, t, n| {
            b.build_float_to_unsigned_int(v, t, n).expect("fptoui")
        })
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
        self.float_to_float_conv(val, ty, name, "float_trunc", |b, v, t, n| {
            b.build_float_trunc(v, t, n).expect("float_trunc")
        })
    }

    /// Build float extension (e.g., f32 → f64).
    pub fn float_ext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        self.float_to_float_conv(val, ty, name, "float_ext", |b, v, t, n| {
            b.build_float_ext(v, t, n).expect("float_ext")
        })
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
