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

            BinaryOp::Lt => self.compile_comparison(
                inkwell::FloatPredicate::OLT, inkwell::IntPredicate::SLT,
                "flt", "ilt", is_struct, is_float, lhs, rhs,
            ),

            BinaryOp::LtEq => self.compile_comparison(
                inkwell::FloatPredicate::OLE, inkwell::IntPredicate::SLE,
                "fle", "ile", is_struct, is_float, lhs, rhs,
            ),

            BinaryOp::Gt => self.compile_comparison(
                inkwell::FloatPredicate::OGT, inkwell::IntPredicate::SGT,
                "fgt", "igt", is_struct, is_float, lhs, rhs,
            ),

            BinaryOp::GtEq => self.compile_comparison(
                inkwell::FloatPredicate::OGE, inkwell::IntPredicate::SGE,
                "fge", "ige", is_struct, is_float, lhs, rhs,
            ),

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

    /// Call a binary string runtime function by name.
    ///
    /// Shared helper for `compile_str_concat`, `compile_str_eq`, `compile_str_ne`.
    /// Allocates temporaries for the two string struct values, stores them,
    /// and calls the named runtime function.
    fn call_binary_string_op(
        &self,
        fn_name: &str,
        label: &str,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let func = self.module.get_function(fn_name)?;

        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        let result = self.builder.build_call(
            func,
            &[lhs_ptr.into(), rhs_ptr.into()],
            label,
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile string concatenation by calling runtime function.
    fn compile_str_concat(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.call_binary_string_op("ori_str_concat", "str_concat_result", lhs, rhs)
    }

    /// Compile string equality by calling runtime function.
    fn compile_str_eq(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.call_binary_string_op("ori_str_eq", "str_eq_result", lhs, rhs)
    }

    /// Compile string inequality by calling runtime function.
    fn compile_str_ne(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.call_binary_string_op("ori_str_ne", "str_ne_result", lhs, rhs)
    }

    /// Compile a numeric comparison (float or integer).
    ///
    /// Shared helper for Lt, LtEq, Gt, GtEq operators. Returns `None` for
    /// struct types (not supported).
    fn compile_comparison(
        &self,
        float_pred: inkwell::FloatPredicate,
        int_pred: inkwell::IntPredicate,
        float_label: &str,
        int_label: &str,
        is_struct: bool,
        is_float: bool,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        if is_struct {
            None
        } else if is_float {
            let l = lhs.into_float_value();
            let r = rhs.into_float_value();
            Some(self.builder.build_float_compare(float_pred, l, r, float_label).ok()?.into())
        } else {
            let l = lhs.into_int_value();
            let r = rhs.into_int_value();
            Some(self.builder.build_int_compare(int_pred, l, r, int_label).ok()?.into())
        }
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
