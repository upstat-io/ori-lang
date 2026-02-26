//! Primitive type builtin methods.
//!
//! Handles `clone` (identity), `to_int`, `byte`, `f`, `to_float`, `to_str`, `abs` for scalar types.

declare_builtins! { emitter, ctx;
    // int
    ("int", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "to_int", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "byte", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "f", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "to_float", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "into", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "to_str", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("int", "abs", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // float
    ("float", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "to_int", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "to_str", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("float", "abs", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // bool
    ("bool", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "to_int", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("bool", "to_str", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // char
    ("char", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("char", "to_int", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // byte
    ("byte", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("byte", "to_int", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // Duration
    ("Duration", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Duration", "to_str", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // Size
    ("Size", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Size", "to_str", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    // Ordering
    ("Ordering", "clone", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
    ("Ordering", "to_int", borrow: true) => emitter.emit_primitive_method(ctx.method, ctx.arg_vals, ctx.type_info),
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
                TypeInfo::Float => {
                    let i64_ty = self.builder.i64_type();
                    Some(self.builder.fp_to_si(receiver, i64_ty, "to_int"))
                }
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
            _ => return None,
        };

        let func_id = self.builder.runtime_fn(func_name);
        self.builder.call(func_id, &[receiver], "to_str")
    }
}
