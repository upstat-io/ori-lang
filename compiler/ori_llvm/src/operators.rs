//! Binary and unary operation compilation.

use inkwell::values::BasicValueEnum;
use ori_ir::ast::{BinaryOp, UnaryOp};
use ori_types::Idx;

use crate::builder::Builder;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a binary operation.
    ///
    /// # Parameters
    /// - `op`: The binary operator
    /// - `lhs`, `rhs`: The compiled operand values
    /// - `operand_type`: The `Idx` of the left operand (used to distinguish struct types)
    #[expect(
        clippy::too_many_lines,
        reason = "large match on BinaryOp - splitting would obscure the operation dispatch"
    )]
    pub(crate) fn compile_binary_op(
        &self,
        op: BinaryOp,
        lhs: BasicValueEnum<'ll>,
        rhs: BasicValueEnum<'ll>,
        operand_type: Idx,
    ) -> Option<BasicValueEnum<'ll>> {
        // Determine the operand type - both must be the same type for binary ops
        let lhs_is_struct = matches!(lhs, BasicValueEnum::StructValue(_));
        let rhs_is_struct = matches!(rhs, BasicValueEnum::StructValue(_));
        let both_struct = lhs_is_struct && rhs_is_struct;
        let lhs_is_float = matches!(lhs, BasicValueEnum::FloatValue(_));
        let rhs_is_float = matches!(rhs, BasicValueEnum::FloatValue(_));
        let is_ptr = matches!(lhs, BasicValueEnum::PointerValue(_))
            || matches!(rhs, BasicValueEnum::PointerValue(_));
        // If one is struct and the other isn't, we can't do the operation
        let is_struct = lhs_is_struct || rhs_is_struct;

        // Check if this is specifically a string type (for struct operations)
        let is_string_type = operand_type == Idx::STR;

        // Pointer arithmetic is not supported through normal binary ops
        if is_ptr {
            return None;
        }

        // Mixed float/int operations not supported - require explicit conversion
        if lhs_is_float != rhs_is_float {
            return None;
        }
        let is_float = lhs_is_float;

        match op {
            // Arithmetic
            BinaryOp::Add => {
                if both_struct && is_string_type {
                    // String concatenation - call runtime function
                    self.compile_str_concat(lhs, rhs)
                } else if is_struct {
                    // Struct types other than strings don't support +
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.fadd(l, r, "fadd").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.add(l, r, "iadd").into())
                }
            }

            BinaryOp::Sub => {
                if is_struct {
                    // Struct types don't support subtraction
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.fsub(l, r, "fsub").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.sub(l, r, "isub").into())
                }
            }

            BinaryOp::Mul => {
                if is_struct {
                    // Struct types don't support multiplication
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.fmul(l, r, "fmul").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.mul(l, r, "imul").into())
                }
            }

            BinaryOp::Div => {
                if is_struct {
                    // Struct types don't support division
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.fdiv(l, r, "fdiv").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.sdiv(l, r, "idiv").into())
                }
            }

            BinaryOp::Mod => {
                if is_struct {
                    // Struct types don't support modulo
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.frem(l, r, "frem").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.srem(l, r, "irem").into())
                }
            }

            // Comparisons
            BinaryOp::Eq => {
                if both_struct && is_string_type {
                    self.compile_str_eq(lhs, rhs)
                } else if is_struct {
                    // Struct equality for non-string types not yet implemented
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.fcmp(inkwell::FloatPredicate::OEQ, l, r, "feq").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.icmp(inkwell::IntPredicate::EQ, l, r, "ieq").into())
                }
            }

            BinaryOp::NotEq => {
                if both_struct && is_string_type {
                    self.compile_str_ne(lhs, rhs)
                } else if is_struct {
                    // Struct inequality for non-string types not yet implemented
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.fcmp(inkwell::FloatPredicate::ONE, l, r, "fne").into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.icmp(inkwell::IntPredicate::NE, l, r, "ine").into())
                }
            }

            BinaryOp::Lt => self.compile_comparison(
                inkwell::FloatPredicate::OLT,
                inkwell::IntPredicate::SLT,
                "flt",
                "ilt",
                is_struct,
                is_float,
                lhs,
                rhs,
            ),

            BinaryOp::LtEq => self.compile_comparison(
                inkwell::FloatPredicate::OLE,
                inkwell::IntPredicate::SLE,
                "fle",
                "ile",
                is_struct,
                is_float,
                lhs,
                rhs,
            ),

            BinaryOp::Gt => self.compile_comparison(
                inkwell::FloatPredicate::OGT,
                inkwell::IntPredicate::SGT,
                "fgt",
                "igt",
                is_struct,
                is_float,
                lhs,
                rhs,
            ),

            BinaryOp::GtEq => self.compile_comparison(
                inkwell::FloatPredicate::OGE,
                inkwell::IntPredicate::SGE,
                "fge",
                "ige",
                is_struct,
                is_float,
                lhs,
                rhs,
            ),

            // Logical
            BinaryOp::And => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.and(l, r, "and").into())
            }

            BinaryOp::Or => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.or(l, r, "or").into())
            }

            // Bitwise
            BinaryOp::BitAnd => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.and(l, r, "bitand").into())
            }

            BinaryOp::BitOr => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.or(l, r, "bitor").into())
            }

            BinaryOp::BitXor => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.xor(l, r, "bitxor").into())
            }

            BinaryOp::Shl => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.shl(l, r, "shl").into())
            }

            BinaryOp::Shr => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.ashr(l, r, "shr").into())
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
        lhs: BasicValueEnum<'ll>,
        rhs: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        let func = self.cx().llmod().get_function(fn_name)?;

        let str_type = self.cx().string_type();

        let lhs_ptr = self.alloca(str_type.into(), "lhs_str");
        let rhs_ptr = self.alloca(str_type.into(), "rhs_str");

        self.store(lhs, lhs_ptr);
        self.store(rhs, rhs_ptr);

        self.call(func, &[lhs_ptr.into(), rhs_ptr.into()], label)
    }

    /// Compile string concatenation by calling runtime function.
    fn compile_str_concat(
        &self,
        lhs: BasicValueEnum<'ll>,
        rhs: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        self.call_binary_string_op("ori_str_concat", "str_concat_result", lhs, rhs)
    }

    /// Compile string equality by calling runtime function.
    fn compile_str_eq(
        &self,
        lhs: BasicValueEnum<'ll>,
        rhs: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        self.call_binary_string_op("ori_str_eq", "str_eq_result", lhs, rhs)
    }

    /// Compile string inequality by calling runtime function.
    fn compile_str_ne(
        &self,
        lhs: BasicValueEnum<'ll>,
        rhs: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        self.call_binary_string_op("ori_str_ne", "str_ne_result", lhs, rhs)
    }

    /// Compile a numeric comparison (float or integer).
    ///
    /// Shared helper for Lt, `LtEq`, Gt, `GtEq` operators. Returns `None` for
    /// struct types (not supported).
    fn compile_comparison(
        &self,
        float_pred: inkwell::FloatPredicate,
        int_pred: inkwell::IntPredicate,
        float_label: &str,
        int_label: &str,
        is_struct: bool,
        is_float: bool,
        lhs: BasicValueEnum<'ll>,
        rhs: BasicValueEnum<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        if is_struct {
            None
        } else if is_float {
            let l = lhs.into_float_value();
            let r = rhs.into_float_value();
            Some(self.fcmp(float_pred, l, r, float_label).into())
        } else {
            let l = lhs.into_int_value();
            let r = rhs.into_int_value();
            Some(self.icmp(int_pred, l, r, int_label).into())
        }
    }

    /// Compile a unary operation.
    pub(crate) fn compile_unary_op(
        &self,
        op: UnaryOp,
        val: BasicValueEnum<'ll>,
        _result_type: Idx,
    ) -> Option<BasicValueEnum<'ll>> {
        match op {
            UnaryOp::Neg => match val {
                BasicValueEnum::IntValue(i) => Some(self.neg(i, "neg").into()),
                BasicValueEnum::FloatValue(f) => Some(self.fneg(f, "fneg").into()),
                _ => None,
            },

            UnaryOp::Not => {
                let i = val.into_int_value();
                Some(self.not(i, "not").into())
            }

            UnaryOp::BitNot => {
                let i = val.into_int_value();
                Some(self.not(i, "bitnot").into())
            }

            UnaryOp::Try => {
                // Try operator needs special handling (error propagation)
                // For now, just return the value
                Some(val)
            }
        }
    }
}
