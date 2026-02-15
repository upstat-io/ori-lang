//! Signed, unsigned, float, and bitwise arithmetic for `IrBuilder`.

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

impl IrBuilder<'_, '_> {
    // -- Signed arithmetic --

    /// Build integer addition.
    pub fn add(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "add on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_add(l.into_int_value(), r.into_int_value(), name)
            .expect("add");
        self.arena.push_value(v.into())
    }

    /// Build integer subtraction.
    pub fn sub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "sub on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_sub(l.into_int_value(), r.into_int_value(), name)
            .expect("sub");
        self.arena.push_value(v.into())
    }

    /// Build integer multiplication.
    pub fn mul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "mul on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_mul(l.into_int_value(), r.into_int_value(), name)
            .expect("mul");
        self.arena.push_value(v.into())
    }

    /// Build signed integer division.
    pub fn sdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "sdiv on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_signed_div(l.into_int_value(), r.into_int_value(), name)
            .expect("sdiv");
        self.arena.push_value(v.into())
    }

    /// Build signed integer remainder.
    pub fn srem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "srem on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_signed_rem(l.into_int_value(), r.into_int_value(), name)
            .expect("srem");
        self.arena.push_value(v.into())
    }

    /// Build integer negation.
    pub fn neg(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "neg on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_neg(v.into_int_value(), name)
            .expect("neg");
        self.arena.push_value(result.into())
    }

    // -- Unsigned arithmetic --

    /// Build unsigned integer division.
    pub fn udiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "udiv on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_unsigned_div(l.into_int_value(), r.into_int_value(), name)
            .expect("udiv");
        self.arena.push_value(v.into())
    }

    /// Build unsigned integer remainder.
    pub fn urem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "urem on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_unsigned_rem(l.into_int_value(), r.into_int_value(), name)
            .expect("urem");
        self.arena.push_value(v.into())
    }

    /// Build logical right shift (zero-extending).
    pub fn lshr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "lshr on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_right_shift(l.into_int_value(), r.into_int_value(), false, name)
            .expect("lshr");
        self.arena.push_value(v.into())
    }

    // -- Float arithmetic --

    /// Build floating-point addition.
    pub fn fadd(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fadd on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_add(l.into_float_value(), r.into_float_value(), name)
            .expect("fadd");
        self.arena.push_value(v.into())
    }

    /// Build floating-point subtraction.
    pub fn fsub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fsub on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_sub(l.into_float_value(), r.into_float_value(), name)
            .expect("fsub");
        self.arena.push_value(v.into())
    }

    /// Build floating-point multiplication.
    pub fn fmul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fmul on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_mul(l.into_float_value(), r.into_float_value(), name)
            .expect("fmul");
        self.arena.push_value(v.into())
    }

    /// Build floating-point division.
    pub fn fdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fdiv on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_div(l.into_float_value(), r.into_float_value(), name)
            .expect("fdiv");
        self.arena.push_value(v.into())
    }

    /// Build floating-point remainder.
    pub fn frem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "frem on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_rem(l.into_float_value(), r.into_float_value(), name)
            .expect("frem");
        self.arena.push_value(v.into())
    }

    /// Build floating-point negation.
    pub fn fneg(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fneg on non-float operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_float_neg(v.into_float_value(), name)
            .expect("fneg");
        self.arena.push_value(result.into())
    }

    // -- Bitwise operations --

    /// Build bitwise AND.
    pub fn and(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "and on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_and(l.into_int_value(), r.into_int_value(), name)
            .expect("and");
        self.arena.push_value(v.into())
    }

    /// Build bitwise OR.
    pub fn or(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "or on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_or(l.into_int_value(), r.into_int_value(), name)
            .expect("or");
        self.arena.push_value(v.into())
    }

    /// Build bitwise XOR.
    pub fn xor(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "xor on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_xor(l.into_int_value(), r.into_int_value(), name)
            .expect("xor");
        self.arena.push_value(v.into())
    }

    /// Build bitwise NOT (complement).
    pub fn not(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "not on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_not(v.into_int_value(), name)
            .expect("not");
        self.arena.push_value(result.into())
    }

    /// Build left shift.
    pub fn shl(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "shl on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_left_shift(l.into_int_value(), r.into_int_value(), name)
            .expect("shl");
        self.arena.push_value(v.into())
    }

    /// Build arithmetic right shift (sign-extending).
    pub fn ashr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "ashr on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_right_shift(l.into_int_value(), r.into_int_value(), true, name)
            .expect("ashr");
        self.arena.push_value(v.into())
    }
}
