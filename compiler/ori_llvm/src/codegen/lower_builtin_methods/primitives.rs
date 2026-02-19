//! Primitive type method lowering: int, float, bool, byte, char, ordering, str.

use ori_ir::canon::CanRange;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // Int methods

    pub(super) fn lower_int_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_icmp_ordering(recv, other, "cmp", true))
            }
            "abs" => {
                let zero = self.builder.const_i64(0);
                let is_neg = self.builder.icmp_slt(recv, zero, "abs.neg");
                let negated = self.builder.neg(recv, "abs.negated");
                Some(self.builder.select(is_neg, negated, recv, "abs"))
            }
            // Value types: clone/hash are identity operations
            "clone" | "hash" => Some(recv),
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "int.eq"))
            }
            // Into<float>: lossless widening via sitofp
            "into" | "to_float" => {
                let f64_ty = self.builder.f64_type();
                Some(self.builder.si_to_fp(recv, f64_ty, "int.into"))
            }
            _ => None,
        }
    }

    // Float methods

    pub(super) fn lower_float_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_fcmp_ordering(recv, other, "fcmp"))
            }
            "abs" => {
                let zero = self.builder.const_f64(0.0);
                let is_neg = self.builder.fcmp_olt(recv, zero, "fabs.neg");
                let negated = self.builder.fneg(recv, "fabs.negated");
                Some(self.builder.select(is_neg, negated, recv, "fabs"))
            }
            "hash" => {
                // Float hash: normalize ±0.0 → +0.0, NaN → canonical, then bitcast to i64
                let normalized = self.normalize_float_for_hash(recv);
                let i64_ty = self.builder.i64_type();
                Some(self.builder.bitcast(normalized, i64_ty, "float.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.fcmp_oeq(recv, other, "float.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Normalize a float for hashing: ±0.0 → +0.0.
    ///
    /// This ensures that `(-0.0).hash() == (0.0).hash()` as required by
    /// the contract `a.equals(b) → a.hash() == b.hash()`.
    pub(super) fn normalize_float_for_hash(&mut self, val: ValueId) -> ValueId {
        let pos_zero = self.builder.const_f64(0.0);
        let is_zero = self.builder.fcmp_oeq(val, pos_zero, "hash.is_zero");
        self.builder
            .select(is_zero, pos_zero, val, "hash.normalized")
    }

    // Bool methods

    pub(super) fn lower_bool_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                // false < true: zext to i8 then unsigned compare
                let i8_ty = self.builder.i8_type();
                let lhs = self.builder.zext(recv, i8_ty, "b2i8.self");
                let i8_ty2 = self.builder.i8_type();
                let rhs = self.builder.zext(other, i8_ty2, "b2i8.other");
                Some(self.emit_icmp_ordering(lhs, rhs, "bcmp", false))
            }
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(recv, i64_ty, "bool.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "bool.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // Byte methods

    pub(super) fn lower_byte_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_icmp_ordering(recv, other, "byte.cmp", false))
            }
            "hash" => {
                // Byte is unsigned (8-bit) — use zext to match evaluator semantics
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(recv, i64_ty, "byte.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "byte.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // Char methods

    pub(super) fn lower_char_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                // Char is i32 (Unicode scalar) — unsigned comparison
                Some(self.emit_icmp_ordering(recv, other, "char.cmp", false))
            }
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(recv, i64_ty, "char.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "char.eq"))
            }
            "to_int" => {
                let i64_ty = self.builder.i64_type();
                // Char is i32 (Unicode scalar 0..=0x10FFFF), always non-negative
                Some(self.builder.zext(recv, i64_ty, "char.to_int"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // Ordering methods

    pub(super) fn lower_ordering_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let less = self.builder.const_i8(0);
        let equal = self.builder.const_i8(1);
        let greater = self.builder.const_i8(2);

        match method {
            "is_less" => Some(self.builder.icmp_eq(recv, less, "ord.is_less")),
            "is_equal" => Some(self.builder.icmp_eq(recv, equal, "ord.is_equal")),
            "is_greater" => Some(self.builder.icmp_eq(recv, greater, "ord.is_greater")),
            "is_less_or_equal" => Some(self.builder.icmp_ne(recv, greater, "ord.is_le")),
            "is_greater_or_equal" => Some(self.builder.icmp_ne(recv, less, "ord.is_ge")),
            "reverse" => {
                // 2 - tag: Less(0)↔Greater(2), Equal(1) unchanged
                Some(self.builder.sub(greater, recv, "ord.reverse"))
            }
            "compare" => {
                // Ordering.compare(): unsigned i8 comparison
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_icmp_ordering(recv, other, "ord.cmp", false))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "ord.eq"))
            }
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(recv, i64_ty, "ord.hash"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // String methods

    pub(super) fn lower_str_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "str.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "str.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "str.is_empty"))
            }
            "concat" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_str_concat_call(recv, other, "str.concat"))
            }
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_str_runtime_call(recv, other, "ori_str_compare", "str.cmp"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_str_eq_call(recv, other, "str.eq"))
            }
            "hash" => Some(self.emit_str_hash_call(recv, "str.hash")),
            "clone" => Some(recv),
            _ => None,
        }
    }
}
