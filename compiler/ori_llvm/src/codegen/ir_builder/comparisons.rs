//! Integer and floating-point comparison operations for `IrBuilder`.

use inkwell::{FloatPredicate, IntPredicate};

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

impl IrBuilder<'_, '_> {
    // -- Integer comparisons --

    /// Generic integer comparison.
    ///
    /// Defensive: if either operand is not an integer, returns `false` (i1 0)
    /// instead of panicking. This prevents process-killing crashes when type
    /// mismatches reach codegen (e.g., comparing str values with `icmp`).
    fn icmp_impl(&mut self, pred: IntPredicate, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(
                lhs_type = ?l.get_type(),
                rhs_type = ?r.get_type(),
                "icmp on non-int operands — returning false"
            );
            self.record_codegen_error();
            return self.const_bool(false);
        }
        let v = self
            .builder
            .build_int_compare(pred, l.into_int_value(), r.into_int_value(), name)
            .expect("icmp");
        self.arena.push_value(v.into())
    }

    /// Integer equal.
    pub fn icmp_eq(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::EQ, lhs, rhs, name)
    }

    /// Integer not equal.
    pub fn icmp_ne(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::NE, lhs, rhs, name)
    }

    /// Signed less than.
    pub fn icmp_slt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SLT, lhs, rhs, name)
    }

    /// Signed greater than.
    pub fn icmp_sgt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SGT, lhs, rhs, name)
    }

    /// Signed less than or equal.
    pub fn icmp_sle(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SLE, lhs, rhs, name)
    }

    /// Signed greater than or equal.
    pub fn icmp_sge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SGE, lhs, rhs, name)
    }

    /// Unsigned less than.
    pub fn icmp_ult(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::ULT, lhs, rhs, name)
    }

    /// Unsigned greater than.
    pub fn icmp_ugt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::UGT, lhs, rhs, name)
    }

    /// Unsigned less than or equal.
    pub fn icmp_ule(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::ULE, lhs, rhs, name)
    }

    /// Unsigned greater than or equal.
    pub fn icmp_uge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::UGE, lhs, rhs, name)
    }

    // -- Float comparisons --

    /// Generic float comparison.
    ///
    /// Defensive: if either operand is not a float, returns `false` (i1 0)
    /// instead of panicking. Prevents crashes from type mismatches.
    fn fcmp_impl(
        &mut self,
        pred: FloatPredicate,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
    ) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(
                lhs_type = ?l.get_type(),
                rhs_type = ?r.get_type(),
                "fcmp on non-float operands — returning false"
            );
            self.record_codegen_error();
            return self.const_bool(false);
        }
        let v = self
            .builder
            .build_float_compare(pred, l.into_float_value(), r.into_float_value(), name)
            .expect("fcmp");
        self.arena.push_value(v.into())
    }

    /// Ordered equal.
    pub fn fcmp_oeq(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OEQ, lhs, rhs, name)
    }

    /// Ordered less than.
    pub fn fcmp_olt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OLT, lhs, rhs, name)
    }

    /// Ordered greater than.
    pub fn fcmp_ogt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OGT, lhs, rhs, name)
    }

    /// Ordered less than or equal.
    pub fn fcmp_ole(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OLE, lhs, rhs, name)
    }

    /// Ordered greater than or equal.
    pub fn fcmp_oge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OGE, lhs, rhs, name)
    }

    /// Ordered not equal (false if either is NaN).
    pub fn fcmp_one(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::ONE, lhs, rhs, name)
    }

    /// Unordered not equal (true if either is NaN or values differ).
    /// This is the correct IEEE 754 `!=` — NaN != NaN returns true.
    pub fn fcmp_une(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::UNE, lhs, rhs, name)
    }

    /// Ordered (both non-NaN).
    pub fn fcmp_ord(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::ORD, lhs, rhs, name)
    }

    /// Unordered (either NaN).
    pub fn fcmp_uno(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::UNO, lhs, rhs, name)
    }

    // -- Ordering emission helpers --

    /// Emit `icmp lt/gt → select` chain returning Ori `Ordering` (i8).
    ///
    /// Returns: 0 (Less), 1 (Equal), 2 (Greater).
    /// Uses signed comparison when `signed` is true, unsigned otherwise.
    pub fn emit_icmp_ordering(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        signed: bool,
    ) -> ValueId {
        let lt = if signed {
            self.icmp_slt(lhs, rhs, &format!("{name}.lt"))
        } else {
            self.icmp_ult(lhs, rhs, &format!("{name}.lt"))
        };
        let gt = if signed {
            self.icmp_sgt(lhs, rhs, &format!("{name}.gt"))
        } else {
            self.icmp_ugt(lhs, rhs, &format!("{name}.gt"))
        };
        let less = self.const_i8(0);
        let equal = self.const_i8(1);
        let greater = self.const_i8(2);
        let gt_or_eq = self.select(gt, greater, equal, &format!("{name}.gt_or_eq"));
        self.select(lt, less, gt_or_eq, &format!("{name}.ord"))
    }

    /// Emit `fcmp olt/ogt → select` chain returning Ori `Ordering` (i8).
    ///
    /// Returns: 0 (Less), 1 (Equal), 2 (Greater).
    /// NaN comparisons produce Equal (both fcmp return false).
    pub fn emit_fcmp_ordering(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let lt = self.fcmp_olt(lhs, rhs, &format!("{name}.lt"));
        let gt = self.fcmp_ogt(lhs, rhs, &format!("{name}.gt"));
        let less = self.const_i8(0);
        let equal = self.const_i8(1);
        let greater = self.const_i8(2);
        let gt_or_eq = self.select(gt, greater, equal, &format!("{name}.gt_or_eq"));
        self.select(lt, less, gt_or_eq, &format!("{name}.ord"))
    }
}
