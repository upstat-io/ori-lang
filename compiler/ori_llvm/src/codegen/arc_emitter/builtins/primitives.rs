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
    ("Duration", "nanoseconds") => emitter.emit_unit_accessor(ctx.arg_vals[0], 1),
    ("Duration", "microseconds") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::duration::NS_PER_US),
    ("Duration", "milliseconds") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::duration::NS_PER_MS),
    ("Duration", "seconds") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::duration::NS_PER_S),
    ("Duration", "minutes") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::duration::NS_PER_M),
    ("Duration", "hours") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::duration::NS_PER_H),
    // Size
    ("Size", "clone") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "to_str") => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    ("Size", "bytes") => emitter.emit_unit_accessor(ctx.arg_vals[0], 1),
    ("Size", "kilobytes") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::size::BYTES_PER_KB as i64),
    ("Size", "megabytes") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::size::BYTES_PER_MB as i64),
    ("Size", "gigabytes") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::size::BYTES_PER_GB as i64),
    ("Size", "terabytes") => emitter.emit_unit_accessor(ctx.arg_vals[0], ori_ir::builtin_constants::size::BYTES_PER_TB as i64),
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

            // byte: int -> i8 (range-checked; eval errors outside 0..=255)
            "byte" => match type_info {
                TypeInfo::Int => self.emit_checked_int_to_byte(receiver, "byte"),
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
                    // abs(i64::MIN) overflows — eval's checked_abs panics
                    // ("integer overflow"); a bare neg+select wraps to MIN.
                    let min = self.builder.const_i64(i64::MIN);
                    let not_min = self.builder.icmp_ne(receiver, min, "abs.not_min");
                    self.emit_unwrap_branch(
                        not_min,
                        "integer overflow computing absolute value",
                        "abs.min",
                    )?;
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

    /// Emit a Duration/Size unit accessor (`nanoseconds`, `seconds`, `bytes`,
    /// `kilobytes`, ...).
    ///
    /// Duration is i64 nanoseconds, Size is i64 bytes; an accessor returns the
    /// value divided by the unit's base-unit count (`ns_per_unit` /
    /// `bytes_per_unit`). The base-unit accessor (`nanoseconds` / `bytes`) has
    /// divisor 1 and is the identity. Matches eval's signed integer division
    /// in `dispatch_duration_method` / `dispatch_size_method`.
    pub(crate) fn emit_unit_accessor(
        &mut self,
        receiver: ValueId,
        divisor: i64,
    ) -> Option<ValueId> {
        if divisor == 1 {
            return Some(receiver);
        }
        let d = self.builder.const_i64(divisor);
        Some(self.builder.sdiv(receiver, d, "unit_accessor"))
    }

    /// Emit a checked int -> byte conversion (panics outside 0..=255,
    /// matching eval's `u8::try_from` semantics for `.byte()` / `byte()` —
    /// silent truncation diverges on out-of-range inputs).
    pub(crate) fn emit_checked_int_to_byte(
        &mut self,
        receiver: ValueId,
        label: &str,
    ) -> Option<ValueId> {
        let lo = self.builder.const_i64(0);
        let hi = self.builder.const_i64(255);
        let ge = self.builder.icmp_sge(receiver, lo, &format!("{label}.ge"));
        let le = self.builder.icmp_sle(receiver, hi, &format!("{label}.le"));
        let valid = self.builder.and(ge, le, &format!("{label}.valid"));
        self.emit_unwrap_branch(
            valid,
            "byte value out of range (0-255)",
            &format!("{label}.range"),
        )?;
        let i8_ty = self.builder.i8_type();
        Some(self.builder.trunc(receiver, i8_ty, label))
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
