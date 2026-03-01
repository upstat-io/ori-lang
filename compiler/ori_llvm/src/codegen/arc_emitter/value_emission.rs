//! Value and literal emission for ARC IR → LLVM IR.
//!
//! Handles `ArcValue` emission (variables, literals, primitive operations),
//! hash combine, and catch cleanup. These are the leaf operations that
//! `emit_instr` delegates to for `Let` instructions.

use ori_arc::ir::{ArcFunction, ArcVarId, LitValue, PrimOp};
use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an `ArcValue` as an LLVM value.
    pub(super) fn emit_value(
        &mut self,
        value: &ori_arc::ir::ArcValue,
        ty: Idx,
        func: &ArcFunction,
    ) -> ValueId {
        match value {
            ori_arc::ir::ArcValue::Var(v) => self.var(*v),

            ori_arc::ir::ArcValue::Literal(lit) => self.emit_literal(lit),

            ori_arc::ir::ArcValue::PrimOp { op, args } => {
                let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
                self.emit_primop(*op, &arg_vals, ty, func, args)
            }
        }
    }

    /// Emit a literal value.
    fn emit_literal(&mut self, lit: &LitValue) -> ValueId {
        match lit {
            LitValue::Int(n) => self.builder.const_i64(*n),
            LitValue::Float(bits) => self.builder.const_f64(f64::from_bits(*bits)),
            LitValue::Bool(b) => self.builder.const_bool(*b),
            LitValue::Char(c) => self.builder.const_i32(*c as i32),
            LitValue::Unit => self.builder.const_i64(0),
            LitValue::String(name) => {
                let s = self.interner.lookup(*name);
                // Use ori_str_from_raw to create an SSO or RC-managed heap
                // copy of the string literal.
                let global = self.builder.build_global_string_ptr(s, "str");
                let len = self.builder.const_i64(s.len() as i64);
                let func_id = self.builder.runtime_fn("ori_str_from_raw");
                let str_ty = self.resolve_type(ori_types::Idx::STR);
                self.builder
                    .call_with_sret(func_id, &[global, len], str_ty, "str.val")
                    .unwrap_or_else(|| {
                        // Fallback: build inline struct (no RC safety)
                        let cap = self.builder.const_i64(s.len() as i64);
                        self.builder
                            .build_struct(str_ty, &[len, cap, global], "str.val")
                    })
            }
            LitValue::Duration { value, unit } => {
                let nanos = unit.to_nanos(*value);
                self.builder.const_i64(nanos)
            }
            LitValue::Size { value, unit } => {
                let bytes = unit.to_bytes(*value);
                self.builder.const_i64(bytes as i64)
            }
        }
    }

    /// Emit a primitive operation.
    fn emit_primop(
        &mut self,
        op: PrimOp,
        arg_vals: &[ValueId],
        _ty: Idx,
        func: &ArcFunction,
        arc_args: &[ArcVarId],
    ) -> ValueId {
        match op {
            PrimOp::Binary(bin_op) => {
                let lhs = arg_vals[0];
                let rhs = arg_vals[1];
                let lhs_ty = func.var_type(arc_args[0]);
                self.emit_binary_op(bin_op, lhs, rhs, lhs_ty)
            }
            PrimOp::Unary(un_op) => {
                let operand = arg_vals[0];
                let operand_ty = func.var_type(arc_args[0]);
                self.emit_unary_op(un_op, operand, operand_ty)
            }
        }
    }

    /// Emit `hash_combine(a, b)` inline using the boost `hash_combine` pattern.
    ///
    /// `a ^ (b + 0x9e3779b9 + (a << 6) + (a >> 2))`
    pub(crate) fn emit_hash_combine(&mut self, a: ValueId, b: ValueId) -> ValueId {
        let magic = self.builder.const_i64(0x9e37_79b9_i64);
        let six = self.builder.const_i64(6);
        let two = self.builder.const_i64(2);

        let a_shl6 = self.builder.shl(a, six, "hc.shl");
        let a_shr2 = self.builder.ashr(a, two, "hc.shr");
        let sum1 = self.builder.add(b, magic, "hc.sum1");
        let sum2 = self.builder.add(sum1, a_shl6, "hc.sum2");
        let sum3 = self.builder.add(sum2, a_shr2, "hc.sum3");
        self.builder.xor(a, sum3, "hc.result")
    }

    /// Emit `ori_catch_cleanup(exc_ptr)` to free a caught Rust exception.
    ///
    /// Calls `_Unwind_DeleteException` via the runtime wrapper, which invokes
    /// the cleanup callback in the Itanium ABI `_Unwind_Exception` header.
    /// This properly frees the Rust-allocated panic payload without requiring
    /// C++ EH ABI functions (`__cxa_begin_catch`/`__cxa_end_catch`), which
    /// are incompatible with Rust's panic infrastructure.
    ///
    /// Called in catch-style unwind blocks right after the landing pad,
    /// before any RC cleanup or catch handler logic.
    pub(super) fn emit_catch_cleanup(&mut self, exc_ptr: ValueId) {
        let func_id = self.builder.runtime_fn("ori_catch_cleanup");
        self.builder.call(func_id, &[exc_ptr], "");
    }
}
