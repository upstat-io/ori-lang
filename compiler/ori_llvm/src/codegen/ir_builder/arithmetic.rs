//! Signed, unsigned, float, and bitwise arithmetic for `IrBuilder`.
//!
//! Checked overflow arithmetic (add/sub/mul/neg with panic on overflow)
//! lives in the sibling `checked_ops` module.

use inkwell::builder::Builder as InkwellBuilder;
use inkwell::values::{FloatValue, IntValue};

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Emit a binary integer instruction after checking both operands are ints.
    /// On a non-int operand records a codegen error and returns `const_i64(0)`.
    fn int_binop(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        op: &str,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            IntValue<'ctx>,
            IntValue<'ctx>,
            &str,
        ) -> IntValue<'ctx>,
    ) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "{op} on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = build(&self.builder, l.into_int_value(), r.into_int_value(), name);
        self.arena.push_value(v.into())
    }

    /// Emit a unary integer instruction after checking the operand is an int.
    fn int_unop(
        &mut self,
        val: ValueId,
        name: &str,
        op: &str,
        build: impl FnOnce(&InkwellBuilder<'ctx>, IntValue<'ctx>, &str) -> IntValue<'ctx>,
    ) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "{op} on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = build(&self.builder, v.into_int_value(), name);
        self.arena.push_value(result.into())
    }

    /// Emit a binary float instruction after checking both operands are floats.
    /// On a non-float operand records a codegen error and returns `const_f64(0.0)`.
    fn float_binop(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        op: &str,
        build: impl FnOnce(
            &InkwellBuilder<'ctx>,
            FloatValue<'ctx>,
            FloatValue<'ctx>,
            &str,
        ) -> FloatValue<'ctx>,
    ) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "{op} on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = build(
            &self.builder,
            l.into_float_value(),
            r.into_float_value(),
            name,
        );
        self.arena.push_value(v.into())
    }

    /// Emit a unary float instruction after checking the operand is a float.
    fn float_unop(
        &mut self,
        val: ValueId,
        name: &str,
        op: &str,
        build: impl FnOnce(&InkwellBuilder<'ctx>, FloatValue<'ctx>, &str) -> FloatValue<'ctx>,
    ) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "{op} on non-float operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = build(&self.builder, v.into_float_value(), name);
        self.arena.push_value(result.into())
    }

    // Signed arithmetic

    /// Build integer addition.
    pub fn add(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "add", |b, l, r, n| {
            b.build_int_add(l, r, n).expect("add")
        })
    }

    /// Build integer subtraction.
    pub fn sub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "sub", |b, l, r, n| {
            b.build_int_sub(l, r, n).expect("sub")
        })
    }

    /// Build integer multiplication.
    pub fn mul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "mul", |b, l, r, n| {
            b.build_int_mul(l, r, n).expect("mul")
        })
    }

    /// Build signed integer division.
    pub fn sdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "sdiv", |b, l, r, n| {
            b.build_int_signed_div(l, r, n).expect("sdiv")
        })
    }

    /// Build signed integer remainder.
    pub fn srem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "srem", |b, l, r, n| {
            b.build_int_signed_rem(l, r, n).expect("srem")
        })
    }

    /// Build integer negation (unchecked — no overflow detection).
    ///
    /// Prefer `checked_neg()` for user-facing integer negation.
    pub fn neg(&mut self, val: ValueId, name: &str) -> ValueId {
        self.int_unop(val, name, "neg", |b, v, n| {
            b.build_int_neg(v, n).expect("neg")
        })
    }

    // Unsigned arithmetic

    /// Build unsigned integer division.
    pub fn udiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "udiv", |b, l, r, n| {
            b.build_int_unsigned_div(l, r, n).expect("udiv")
        })
    }

    /// Build unsigned integer remainder.
    pub fn urem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "urem", |b, l, r, n| {
            b.build_int_unsigned_rem(l, r, n).expect("urem")
        })
    }

    /// Build logical right shift (zero-extending).
    pub fn lshr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "lshr", |b, l, r, n| {
            b.build_right_shift(l, r, false, n).expect("lshr")
        })
    }

    // Float arithmetic

    /// Build floating-point addition.
    pub fn fadd(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.float_binop(lhs, rhs, name, "fadd", |b, l, r, n| {
            b.build_float_add(l, r, n).expect("fadd")
        })
    }

    /// Build floating-point subtraction.
    pub fn fsub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.float_binop(lhs, rhs, name, "fsub", |b, l, r, n| {
            b.build_float_sub(l, r, n).expect("fsub")
        })
    }

    /// Build floating-point multiplication.
    pub fn fmul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.float_binop(lhs, rhs, name, "fmul", |b, l, r, n| {
            b.build_float_mul(l, r, n).expect("fmul")
        })
    }

    /// Build floating-point division.
    pub fn fdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.float_binop(lhs, rhs, name, "fdiv", |b, l, r, n| {
            b.build_float_div(l, r, n).expect("fdiv")
        })
    }

    /// Build floating-point remainder.
    pub fn frem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.float_binop(lhs, rhs, name, "frem", |b, l, r, n| {
            b.build_float_rem(l, r, n).expect("frem")
        })
    }

    /// Build floating-point negation.
    pub fn fneg(&mut self, val: ValueId, name: &str) -> ValueId {
        self.float_unop(val, name, "fneg", |b, v, n| {
            b.build_float_neg(v, n).expect("fneg")
        })
    }

    // Bitwise operations

    /// Build bitwise AND.
    pub fn and(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "and", |b, l, r, n| {
            b.build_and(l, r, n).expect("and")
        })
    }

    /// Build bitwise OR.
    pub fn or(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "or", |b, l, r, n| {
            b.build_or(l, r, n).expect("or")
        })
    }

    /// Build bitwise XOR.
    pub fn xor(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "xor", |b, l, r, n| {
            b.build_xor(l, r, n).expect("xor")
        })
    }

    /// Build bitwise NOT (complement).
    pub fn not(&mut self, val: ValueId, name: &str) -> ValueId {
        self.int_unop(val, name, "not", |b, v, n| b.build_not(v, n).expect("not"))
    }

    /// Build left shift.
    pub fn shl(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "shl", |b, l, r, n| {
            b.build_left_shift(l, r, n).expect("shl")
        })
    }

    /// Build arithmetic right shift (sign-extending).
    pub fn ashr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.int_binop(lhs, rhs, name, "ashr", |b, l, r, n| {
            b.build_right_shift(l, r, true, n).expect("ashr")
        })
    }
}
