//! Trait method codegen for primitive and string types.
//!
//! Implements inline LLVM IR for Eq, Comparable, and Hashable trait methods
//! on scalar types (int, float, bool, char, byte, Ordering) and strings.
//!
//! | Method              | Returns   | Scalar IR                       | String IR            |
//! |---------------------|-----------|---------------------------------|----------------------|
//! | `equals(other)`     | `bool`    | `icmp eq` / `fcmp oeq`          | `ori_str_eq`         |
//! | `compare(other)`    | `Ordering`| `emit_icmp_ordering` (signed)   | `ori_str_compare`    |
//! | `hash()`            | `int`     | identity / bitcast / zext       | `ori_str_hash`       |
//! | `is_equal(other)`   | `bool`    | same as equals                  | same as equals       |
//! | `is_less(other)`    | `bool`    | `icmp slt` / `fcmp olt`         | compare-then-check   |
//! | `is_greater(other)` | `bool`    | `icmp sgt` / `fcmp ogt`         | compare-then-check   |
//! | `is_less_or_equal`  | `bool`    | `icmp sle` / `fcmp ole`         | compare-then-check   |
//! | `is_greater_or_equal`| `bool`   | `icmp sge` / `fcmp oge`         | compare-then-check   |
//!
//! Ordering predicates (`is_less`, `is_equal`, `is_greater`, `is_less_or_equal`,
//! `is_greater_or_equal`, `reverse`) are also implemented on the `Ordering` type
//! itself (i8 with Less=0, Equal=1, Greater=2).

declare_builtins! { emitter, ctx;
    // Scalar trait methods: int, float, bool, char, byte, Duration, Size
    // equals / is_equal
    ("int", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "equals") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "is_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // compare
    ("int", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "compare") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // hash
    ("int", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "hash") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // comparison predicates
    ("int", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "is_less") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "is_greater") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "is_less_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "is_greater_or_equal") => emitter.emit_trait_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // String trait methods (equals, compare, hash, comparison predicates)
    ("str", "equals") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "is_equal") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "compare") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "hash") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "is_less") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "is_greater") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "is_less_or_equal") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    ("str", "is_greater_or_equal") => emitter.emit_str_trait_method(ctx.method, ctx.arg_vals),
    // Ordering trait methods
    ("Ordering", "equals") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "compare") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "hash") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "is_less") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "is_equal") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "is_greater") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "is_less_or_equal") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "is_greater_or_equal") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
    ("Ordering", "reverse") => emitter.emit_ordering_method(ctx.method, ctx.arg_vals),
}

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a trait method (equals, compare, hash, `is_less`, etc.) for primitive types.
    ///
    /// Returns `Some(result)` if handled, `None` to fall through.
    pub(crate) fn emit_trait_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        type_info: &TypeInfo,
    ) -> Option<ValueId> {
        match method {
            "equals" | "is_equal" => self.emit_equals(arg_vals, type_info),
            "compare" => self.emit_compare(arg_vals, type_info),
            "hash" => self.emit_hash(arg_vals, type_info),
            "is_less" => self.emit_comparison_predicate(arg_vals, type_info, CmpPredicate::Less),
            "is_greater" => {
                self.emit_comparison_predicate(arg_vals, type_info, CmpPredicate::Greater)
            }
            "is_less_or_equal" => {
                self.emit_comparison_predicate(arg_vals, type_info, CmpPredicate::LessOrEqual)
            }
            "is_greater_or_equal" => {
                self.emit_comparison_predicate(arg_vals, type_info, CmpPredicate::GreaterOrEqual)
            }
            _ => None,
        }
    }

    /// Emit Ordering-specific methods (predicates + reverse).
    pub(crate) fn emit_ordering_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];
        let has_other = arg_vals.len() >= 2;

        match method {
            // Binary: Ordering.equals(other) / Ordering.compare(other)
            "equals" if has_other => Some(self.builder.icmp_eq(receiver, arg_vals[1], "ord_eq")),
            "compare" if has_other => {
                Some(
                    self.builder
                        .emit_icmp_ordering(receiver, arg_vals[1], "ord_cmp", false),
                )
            }

            // Unary predicates on the Ordering value itself
            "is_less" => {
                let zero = self.builder.const_i8(0);
                Some(self.builder.icmp_eq(receiver, zero, "is_less"))
            }
            "is_equal" => {
                let one = self.builder.const_i8(1);
                Some(self.builder.icmp_eq(receiver, one, "is_equal"))
            }
            "is_greater" => {
                let two = self.builder.const_i8(2);
                Some(self.builder.icmp_eq(receiver, two, "is_greater"))
            }
            "is_less_or_equal" => {
                let two = self.builder.const_i8(2);
                Some(self.builder.icmp_ne(receiver, two, "is_le"))
            }
            "is_greater_or_equal" => {
                let zero = self.builder.const_i8(0);
                Some(self.builder.icmp_ne(receiver, zero, "is_ge"))
            }

            // Ordering.reverse(): Less(0)→2, Equal(1)→1, Greater(2)→0
            "reverse" => {
                let two = self.builder.const_i8(2);
                Some(self.builder.sub(two, receiver, "reverse"))
            }

            // Ordering.hash() → zext to i64
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(receiver, i64_ty, "ord_hash"))
            }
            _ => None,
        }
    }

    // equals / is_equal

    /// Emit `equals(self, other) -> bool`.
    fn emit_equals(&mut self, arg_vals: &[ValueId], type_info: &TypeInfo) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let lhs = arg_vals[0];
        let rhs = arg_vals[1];

        match type_info {
            TypeInfo::Int
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Duration
            | TypeInfo::Size => Some(self.builder.icmp_eq(lhs, rhs, "equals")),
            TypeInfo::Float => Some(self.builder.fcmp_oeq(lhs, rhs, "equals")),
            _ => None,
        }
    }

    // compare → Ordering (i8: Less=0, Equal=1, Greater=2)

    /// Emit `compare(self, other) -> Ordering`.
    fn emit_compare(&mut self, arg_vals: &[ValueId], type_info: &TypeInfo) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let lhs = arg_vals[0];
        let rhs = arg_vals[1];

        match type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                Some(self.builder.emit_icmp_ordering(lhs, rhs, "cmp", true))
            }
            TypeInfo::Float => Some(self.builder.emit_fcmp_ordering(lhs, rhs, "cmp")),
            // Bool/Char/Byte use unsigned comparison (0 < 1 for bool, Unicode for char)
            TypeInfo::Bool | TypeInfo::Char | TypeInfo::Byte => {
                Some(self.builder.emit_icmp_ordering(lhs, rhs, "cmp", false))
            }
            _ => None,
        }
    }

    // hash → int (i64)

    /// Emit `hash(self) -> int`.
    fn emit_hash(&mut self, arg_vals: &[ValueId], type_info: &TypeInfo) -> Option<ValueId> {
        let receiver = arg_vals[0];
        let i64_ty = self.builder.i64_type();

        match type_info {
            // int.hash() = identity (already i64)
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => Some(receiver),
            // float.hash(): normalize ±0 to +0, then bitcast to i64
            TypeInfo::Float => Some(self.emit_float_hash(receiver)),
            // bool/byte.hash() = zext to i64 (unsigned extension)
            TypeInfo::Bool | TypeInfo::Byte => Some(self.builder.zext(receiver, i64_ty, "hash")),
            // char.hash() = sext i32 to i64 (Unicode scalar value)
            TypeInfo::Char => Some(self.builder.sext(receiver, i64_ty, "hash")),
            _ => None,
        }
    }

    /// Emit float hash with ±0 normalization.
    ///
    /// IEEE 754: -0.0 == 0.0 but have different bits. The hash contract
    /// requires equal values to produce equal hashes, so we normalize -0.0
    /// to +0.0 before bitcasting to i64.
    fn emit_float_hash(&mut self, receiver: ValueId) -> ValueId {
        let i64_ty = self.builder.i64_type();

        // Check if value == 0.0 (catches both +0.0 and -0.0)
        let zero = self.builder.const_f64(0.0);
        let is_zero = self.builder.fcmp_oeq(receiver, zero, "is_zero");

        // Normalize: if zero then +0.0 else original
        let normalized = self.builder.select(is_zero, zero, receiver, "normalized");

        // Bitcast f64 → i64
        self.builder.bitcast(normalized, i64_ty, "hash")
    }

    // Comparison predicates (is_less, is_greater, is_less_or_equal, etc.)

    /// Emit a comparison predicate method.
    fn emit_comparison_predicate(
        &mut self,
        arg_vals: &[ValueId],
        type_info: &TypeInfo,
        predicate: CmpPredicate,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let lhs = arg_vals[0];
        let rhs = arg_vals[1];

        match type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                Some(self.emit_int_predicate(lhs, rhs, predicate))
            }
            TypeInfo::Float => Some(self.emit_float_predicate(lhs, rhs, predicate)),
            TypeInfo::Bool | TypeInfo::Byte => {
                Some(self.emit_unsigned_predicate(lhs, rhs, predicate))
            }
            TypeInfo::Char => Some(self.emit_unsigned_predicate(lhs, rhs, predicate)),
            _ => None,
        }
    }

    /// Signed integer comparison predicate.
    fn emit_int_predicate(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        predicate: CmpPredicate,
    ) -> ValueId {
        match predicate {
            CmpPredicate::Less => self.builder.icmp_slt(lhs, rhs, "is_less"),
            CmpPredicate::Greater => self.builder.icmp_sgt(lhs, rhs, "is_greater"),
            CmpPredicate::LessOrEqual => self.builder.icmp_sle(lhs, rhs, "is_le"),
            CmpPredicate::GreaterOrEqual => self.builder.icmp_sge(lhs, rhs, "is_ge"),
        }
    }

    /// Float comparison predicate (ordered).
    fn emit_float_predicate(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        predicate: CmpPredicate,
    ) -> ValueId {
        match predicate {
            CmpPredicate::Less => self.builder.fcmp_olt(lhs, rhs, "is_less"),
            CmpPredicate::Greater => self.builder.fcmp_ogt(lhs, rhs, "is_greater"),
            CmpPredicate::LessOrEqual => self.builder.fcmp_ole(lhs, rhs, "is_le"),
            CmpPredicate::GreaterOrEqual => self.builder.fcmp_oge(lhs, rhs, "is_ge"),
        }
    }

    /// Unsigned comparison predicate (bool, byte, char).
    fn emit_unsigned_predicate(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        predicate: CmpPredicate,
    ) -> ValueId {
        match predicate {
            CmpPredicate::Less => self.builder.icmp_ult(lhs, rhs, "is_less"),
            CmpPredicate::Greater => self.builder.icmp_ugt(lhs, rhs, "is_greater"),
            CmpPredicate::LessOrEqual => self.builder.icmp_ule(lhs, rhs, "is_le"),
            CmpPredicate::GreaterOrEqual => self.builder.icmp_uge(lhs, rhs, "is_ge"),
        }
    }

    // String trait methods (delegate to runtime)

    /// Emit string trait methods: equals, compare, hash.
    pub(crate) fn emit_str_trait_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
    ) -> Option<ValueId> {
        match method {
            "equals" | "is_equal" if arg_vals.len() >= 2 => {
                Some(self.emit_str_runtime_call("ori_str_eq", arg_vals[0], arg_vals[1], false))
            }
            "compare" if arg_vals.len() >= 2 => self.emit_str_compare(arg_vals[0], arg_vals[1]),
            "hash" => self.emit_str_hash(arg_vals[0]),
            "is_less" if arg_vals.len() >= 2 => {
                self.emit_str_cmp_predicate(arg_vals[0], arg_vals[1], CmpPredicate::Less)
            }
            "is_greater" if arg_vals.len() >= 2 => {
                self.emit_str_cmp_predicate(arg_vals[0], arg_vals[1], CmpPredicate::Greater)
            }
            "is_less_or_equal" if arg_vals.len() >= 2 => {
                self.emit_str_cmp_predicate(arg_vals[0], arg_vals[1], CmpPredicate::LessOrEqual)
            }
            "is_greater_or_equal" if arg_vals.len() >= 2 => {
                self.emit_str_cmp_predicate(arg_vals[0], arg_vals[1], CmpPredicate::GreaterOrEqual)
            }
            _ => None,
        }
    }

    /// Emit `str.compare(other)` via `ori_str_compare`.
    ///
    /// `ori_str_compare(ptr, ptr) -> i8` returns the Ordering directly.
    fn emit_str_compare(&mut self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_compare");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "str_cmp.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], "str_cmp")
    }

    /// Emit `str.hash()` via `ori_str_hash`.
    fn emit_str_hash(&mut self, receiver: ValueId) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_str_hash");

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_hash.arg", str_ty);
        self.builder.store(receiver, ptr);

        self.emit_rt_call(func_id, &[ptr], "str_hash")
    }

    /// Emit string comparison predicate via compare-then-check.
    ///
    /// Calls `ori_str_compare` then checks the i8 result against the
    /// expected Ordering tag (Less=0, Equal=1, Greater=2).
    pub(in crate::codegen::arc_emitter) fn emit_str_cmp_predicate(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        predicate: CmpPredicate,
    ) -> Option<ValueId> {
        let cmp_result = self.emit_str_compare(lhs, rhs)?;
        Some(match predicate {
            CmpPredicate::Less => {
                let zero = self.builder.const_i8(0);
                self.builder.icmp_eq(cmp_result, zero, "str_is_less")
            }
            CmpPredicate::Greater => {
                let two = self.builder.const_i8(2);
                self.builder.icmp_eq(cmp_result, two, "str_is_greater")
            }
            CmpPredicate::LessOrEqual => {
                let two = self.builder.const_i8(2);
                self.builder.icmp_ne(cmp_result, two, "str_is_le")
            }
            CmpPredicate::GreaterOrEqual => {
                let zero = self.builder.const_i8(0);
                self.builder.icmp_ne(cmp_result, zero, "str_is_ge")
            }
        })
    }
}

/// Comparison predicate kind for code reuse.
#[derive(Clone, Copy)]
pub(in crate::codegen::arc_emitter) enum CmpPredicate {
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
}
