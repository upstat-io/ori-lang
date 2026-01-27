//! Binary and unary operation compilation.

use inkwell::values::BasicValueEnum;
use ori_ir::ast::{BinaryOp, UnaryOp};
use ori_ir::TypeId;

use crate::LLVMCodegen;

impl<'ctx> LLVMCodegen<'ctx> {
    /// Compile a binary operation.
    pub(crate) fn compile_binary_op(
        &self,
        op: BinaryOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
        _result_type: TypeId,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Determine the operand type - both must be the same type for binary ops
        let lhs_is_struct = matches!(lhs, BasicValueEnum::StructValue(_));
        let rhs_is_struct = matches!(rhs, BasicValueEnum::StructValue(_));
        let both_struct = lhs_is_struct && rhs_is_struct;
        let is_float = matches!(lhs, BasicValueEnum::FloatValue(_));
        // If one is struct and the other isn't, we can't do the operation
        let is_struct = lhs_is_struct || rhs_is_struct;

        match op {
            // Arithmetic
            BinaryOp::Add => {
                if both_struct {
                    // String concatenation - call runtime function
                    self.compile_str_concat(lhs, rhs)
                } else if is_struct {
                    // Mixed types - not supported
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_add(l, r, "fadd").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_add(l, r, "iadd").ok()?.into())
                }
            }

            BinaryOp::Sub => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_sub(l, r, "fsub").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_sub(l, r, "isub").ok()?.into())
                }
            }

            BinaryOp::Mul => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_mul(l, r, "fmul").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_mul(l, r, "imul").ok()?.into())
                }
            }

            BinaryOp::Div => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_div(l, r, "fdiv").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_signed_div(l, r, "idiv").ok()?.into())
                }
            }

            BinaryOp::Mod => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_rem(l, r, "frem").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_signed_rem(l, r, "irem").ok()?.into())
                }
            }

            // Comparisons
            BinaryOp::Eq => {
                if both_struct {
                    self.compile_str_eq(lhs, rhs)
                } else if is_struct {
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OEQ, l, r, "feq"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::EQ, l, r, "ieq"
                    ).ok()?.into())
                }
            }

            BinaryOp::NotEq => {
                if both_struct {
                    self.compile_str_ne(lhs, rhs)
                } else if is_struct {
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::ONE, l, r, "fne"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::NE, l, r, "ine"
                    ).ok()?.into())
                }
            }

            BinaryOp::Lt => {
                if is_struct {
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OLT, l, r, "flt"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SLT, l, r, "ilt"
                    ).ok()?.into())
                }
            }

            BinaryOp::LtEq => {
                if is_struct {
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OLE, l, r, "fle"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SLE, l, r, "ile"
                    ).ok()?.into())
                }
            }

            BinaryOp::Gt => {
                if is_struct {
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OGT, l, r, "fgt"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SGT, l, r, "igt"
                    ).ok()?.into())
                }
            }

            BinaryOp::GtEq => {
                if is_struct {
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OGE, l, r, "fge"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SGE, l, r, "ige"
                    ).ok()?.into())
                }
            }

            // Logical
            BinaryOp::And => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_and(l, r, "and").ok()?.into())
            }

            BinaryOp::Or => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_or(l, r, "or").ok()?.into())
            }

            // Bitwise
            BinaryOp::BitAnd => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_and(l, r, "bitand").ok()?.into())
            }

            BinaryOp::BitOr => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_or(l, r, "bitor").ok()?.into())
            }

            BinaryOp::BitXor => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_xor(l, r, "bitxor").ok()?.into())
            }

            BinaryOp::Shl => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_left_shift(l, r, "shl").ok()?.into())
            }

            BinaryOp::Shr => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_right_shift(l, r, true, "shr").ok()?.into())
            }

            // Not yet implemented
            _ => None,
        }
    }

    /// Compile string concatenation by calling runtime function.
    fn compile_str_concat(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let str_concat = self.module.get_function("ori_str_concat")?;

        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        let result = self.builder.build_call(
            str_concat,
            &[lhs_ptr.into(), rhs_ptr.into()],
            "str_concat_result"
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile string equality by calling runtime function.
    fn compile_str_eq(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let str_eq = self.module.get_function("ori_str_eq")?;

        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        let result = self.builder.build_call(
            str_eq,
            &[lhs_ptr.into(), rhs_ptr.into()],
            "str_eq_result"
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile string inequality by calling runtime function.
    fn compile_str_ne(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let str_ne = self.module.get_function("ori_str_ne")?;

        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        let result = self.builder.build_call(
            str_ne,
            &[lhs_ptr.into(), rhs_ptr.into()],
            "str_ne_result"
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile a unary operation.
    pub(crate) fn compile_unary_op(
        &self,
        op: UnaryOp,
        val: BasicValueEnum<'ctx>,
        _result_type: TypeId,
    ) -> Option<BasicValueEnum<'ctx>> {
        match op {
            UnaryOp::Neg => {
                match val {
                    BasicValueEnum::IntValue(i) => {
                        Some(self.builder.build_int_neg(i, "neg").ok()?.into())
                    }
                    BasicValueEnum::FloatValue(f) => {
                        Some(self.builder.build_float_neg(f, "fneg").ok()?.into())
                    }
                    _ => None,
                }
            }

            UnaryOp::Not => {
                let i = val.into_int_value();
                Some(self.builder.build_not(i, "not").ok()?.into())
            }

            UnaryOp::BitNot => {
                let i = val.into_int_value();
                Some(self.builder.build_not(i, "bitnot").ok()?.into())
            }

            UnaryOp::Try => {
                // Try operator needs special handling (error propagation)
                // For now, just return the value
                Some(val)
            }
        }
    }
}
