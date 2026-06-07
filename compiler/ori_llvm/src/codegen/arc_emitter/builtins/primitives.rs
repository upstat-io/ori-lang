//! Primitive type builtin methods.
//!
//! Handles `clone` (identity), `to_int`, `byte`, `f`, `to_float`, `to_str`, `abs` for scalar types.

declare_builtins! { emitter, ctx;
    // int
    ("int", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "to_int") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "byte") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "f") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "to_float") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "into") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    ("int", "abs") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // float
    ("float", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "to_int") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    ("float", "abs") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // bool
    ("bool", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "to_int") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    // char
    ("char", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "to_int") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    // byte
    ("byte", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "to_int") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    // Duration
    ("Duration", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    // Size
    ("Size", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    // Ordering
    ("Ordering", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Ordering", "to_int") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Ordering", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
}

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a primitive method (`clone`, `to_int`, `byte`, `f`).
    ///
    /// Scalar types are trivially copyable — `clone` is identity.
    /// Type conversions use direct LLVM cast instructions.
    pub(crate) fn emit_primitive_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        type_info: &TypeInfo,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];

        match method {
            // clone: identity for all scalar types
            "clone" => Some(receiver),

            // to_int: convert to i64
            "to_int" => match type_info {
                TypeInfo::Int => Some(receiver),
                TypeInfo::Float => self.emit_checked_float_to_int(receiver, "to_int"),
                TypeInfo::Bool => {
                    let i64_ty = self.builder.i64_type();
                    Some(self.builder.zext(receiver, i64_ty, "to_int"))
                }
                TypeInfo::Char => {
                    // Char is i32 (Unicode scalar), widen to i64
                    let i64_ty = self.builder.i64_type();
                    Some(self.builder.sext(receiver, i64_ty, "to_int"))
                }
                TypeInfo::Byte => {
                    // Byte is i8, zero-extend to i64
                    let i64_ty = self.builder.i64_type();
                    Some(self.builder.zext(receiver, i64_ty, "to_int"))
                }
                TypeInfo::Ordering => {
                    // Ordering is i8 (0=Less, 1=Equal, 2=Greater)
                    let i64_ty = self.builder.i64_type();
                    Some(self.builder.sext(receiver, i64_ty, "to_int"))
                }
                _ => None,
            },

            // byte: int -> i8
            "byte" => match type_info {
                TypeInfo::Int => {
                    let i8_ty = self.builder.i8_type();
                    Some(self.builder.trunc(receiver, i8_ty, "byte"))
                }
                _ => None,
            },

            // f: int -> f64
            "f" => match type_info {
                TypeInfo::Int => {
                    let f64_ty = self.builder.f64_type();
                    Some(self.builder.si_to_fp(receiver, f64_ty, "f"))
                }
                _ => None,
            },

            // to_float: int -> f64
            "to_float" => match type_info {
                TypeInfo::Int => {
                    let f64_ty = self.builder.f64_type();
                    Some(self.builder.si_to_fp(receiver, f64_ty, "to_float"))
                }
                _ => None,
            },

            // into: type conversion (int -> float)
            "into" => match type_info {
                TypeInfo::Int => {
                    let f64_ty = self.builder.f64_type();
                    Some(self.builder.si_to_fp(receiver, f64_ty, "into"))
                }
                _ => None,
            },

            // to_str: delegate to runtime str_from_* functions
            "to_str" => self.emit_to_str(receiver, type_info),

            // abs: int/float absolute value
            "abs" => match type_info {
                TypeInfo::Int => {
                    // abs(x) = x < 0 ? -x : x
                    let zero = self.builder.const_i64(0);
                    let is_neg = self.builder.icmp_slt(receiver, zero, "is_neg");
                    let negated = self.builder.neg(receiver, "neg");
                    Some(self.builder.select(is_neg, negated, receiver, "abs"))
                }
                TypeInfo::Float => {
                    // abs(x) via fneg + select
                    let zero = self.builder.const_f64(0.0);
                    let is_neg = self.builder.fcmp_olt(receiver, zero, "is_neg");
                    let negated = self.builder.fneg(receiver, "neg");
                    Some(self.builder.select(is_neg, negated, receiver, "abs"))
                }
                _ => None,
            },

            _ => None,
        }
    }

    /// Emit a checked float -> int conversion (panics on NaN / infinity /
    /// out-of-i64-range, matching eval's `to_int` / `int()` semantics for
    /// dual-execution parity — raw `fptosi` is poison on those inputs).
    ///
    /// Guard order matches eval: NaN first, then infinity, then the exact
    /// f64 range bounds (`-2^63 <= value < 2^63`; both bounds exactly
    /// representable in f64). The surviving path emits plain `fptosi`,
    /// which is defined for every value the guards admit.
    pub(crate) fn emit_checked_float_to_int(
        &mut self,
        receiver: ValueId,
        label: &str,
    ) -> Option<ValueId> {
        // NaN: `fcmp ord x, x` is false iff x is NaN.
        let not_nan = self
            .builder
            .fcmp_ord(receiver, receiver, &format!("{label}.ord"));
        self.emit_unwrap_branch(
            not_nan,
            "cannot convert NaN to int",
            &format!("{label}.nan"),
        )?;

        // Infinity: ordered != against both infinities (NaN already excluded).
        let pos_inf = self.builder.const_f64(f64::INFINITY);
        let neg_inf = self.builder.const_f64(f64::NEG_INFINITY);
        let ne_pos = self
            .builder
            .fcmp_one(receiver, pos_inf, &format!("{label}.ne_pinf"));
        let ne_neg = self
            .builder
            .fcmp_one(receiver, neg_inf, &format!("{label}.ne_ninf"));
        let not_inf = self.builder.and(ne_pos, ne_neg, &format!("{label}.finite"));
        self.emit_unwrap_branch(
            not_inf,
            "cannot convert infinity to int",
            &format!("{label}.inf"),
        )?;

        // Range: -2^63 <= value < 2^63 (2^63 itself is NOT representable
        // in i64 — it must panic, not saturate to i64::MAX).
        let lo = self.builder.const_f64(-9_223_372_036_854_775_808.0);
        let hi = self.builder.const_f64(9_223_372_036_854_775_808.0);
        let ge_lo = self
            .builder
            .fcmp_oge(receiver, lo, &format!("{label}.ge_lo"));
        let lt_hi = self
            .builder
            .fcmp_olt(receiver, hi, &format!("{label}.lt_hi"));
        let in_range = self.builder.and(ge_lo, lt_hi, &format!("{label}.in_range"));
        self.emit_unwrap_branch(
            in_range,
            "float out of range for int conversion",
            &format!("{label}.range"),
        )?;

        let i64_ty = self.builder.i64_type();
        Some(self.builder.fp_to_si(receiver, i64_ty, label))
    }

    /// Emit `to_str` for a primitive type via runtime conversion function.
    pub(crate) fn emit_to_str(
        &mut self,
        receiver: ValueId,
        type_info: &TypeInfo,
    ) -> Option<ValueId> {
        let func_name = match type_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "ori_str_from_int",
            TypeInfo::Float => "ori_str_from_float",
            TypeInfo::Bool => "ori_str_from_bool",
            TypeInfo::Char => "ori_str_from_char",
            _ => return None,
        };

        let func_id = self.builder.runtime_fn(func_name);
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[receiver], str_ty, "to_str")
    }
}
