//! Checked arithmetic operations for `IrBuilder`.
//!
//! Integer add/sub/mul/neg use checked overflow intrinsics
//! (`llvm.sadd.with.overflow`, etc.) that panic on overflow. This matches
//! Ori's spec (overflow = panic) and avoids LLVM UB. For compile-time
//! constant operands, LLVM constant-folds the overflow branch away entirely.
//!
//! A per-ARC-block CSE cache eliminates redundant checked operations within
//! a single block. For example, `total += i + 1; i += 1` computes `i + 1`
//! once and reuses the result. The cache is cleared at ARC block boundaries
//! via [`IrBuilder::clear_cse_cache`].
//!
//! Checked division/remainder live in the sibling `div_rem` module; checked
//! shift in the sibling `shift` module. Both share this module's
//! `validate_checked_int_operands` and the sibling `panic` module's
//! `emit_panic_block` carrier.

use inkwell::intrinsics::Intrinsic;
use inkwell::types::IntType;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

mod div_rem;
mod panic;
mod shift;

/// Normalized operand for CSE cache keys.
///
/// Two different `ValueId`s may represent the same LLVM constant (e.g.,
/// two separate `const_i64(1)` calls). This enum normalizes constants
/// so they match in the cache, while SSA values use their `ValueId` identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum CseOperand {
    /// A compile-time integer constant, normalized by value.
    ConstInt(u64),
    /// An SSA value (phi, instruction result), identified by `ValueId`.
    Ssa(ValueId),
}

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Normalize a `ValueId` to a `CseOperand` for cache keying.
    ///
    /// If the value is a compile-time integer constant, returns
    /// `ConstInt(bits)` so that two different `ValueId`s for the same
    /// constant will match. Otherwise returns `Ssa(id)`.
    fn cse_operand(&self, id: ValueId) -> CseOperand {
        let val = self.arena.get_value(id);
        if let BasicValueEnum::IntValue(iv) = val {
            if let Some(c) = iv.get_zero_extended_constant() {
                return CseOperand::ConstInt(c);
            }
        }
        CseOperand::Ssa(id)
    }

    /// Build checked integer addition: panics on overflow.
    ///
    /// Emits `llvm.sadd.with.overflow` at the operands' own width (i64 for
    /// int/Duration/Size, i8 for byte), extracts the result and overflow
    /// flag, and branches to a panic block on overflow. LLVM constant-folds
    /// the branch away when both operands are compile-time constants.
    pub fn checked_add(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_binop(
            "llvm.sadd.with.overflow",
            lhs,
            rhs,
            name,
            "integer overflow on addition",
        )
    }

    /// Build checked integer subtraction: panics on overflow.
    pub fn checked_sub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_binop(
            "llvm.ssub.with.overflow",
            lhs,
            rhs,
            name,
            "integer overflow on subtraction",
        )
    }

    /// Build checked integer multiplication: panics on overflow.
    pub fn checked_mul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.checked_mul_msg(lhs, rhs, name, "integer overflow on multiplication")
    }

    /// Build checked integer multiplication with a caller-supplied overflow
    /// panic message. `checked_mul` is the canonical
    /// `"integer overflow on multiplication"` form; unit factories supply the
    /// interpreter's `"integer overflow in <duration|size> factory conversion"`
    /// for dual-execution parity.
    pub fn checked_mul_msg(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        panic_msg: &str,
    ) -> ValueId {
        self.emit_checked_binop("llvm.smul.with.overflow", lhs, rhs, name, panic_msg)
    }

    /// Build checked integer negation: panics on overflow.
    ///
    /// Negation is `0 - x`, so we reuse `@llvm.ssub.with.overflow(0, x)` at
    /// `x`'s own width (int/Duration/Size are i64; byte is i8). The only
    /// overflowing case is `-MIN` (result doesn't fit in that width).
    pub fn checked_neg(&mut self, val: ValueId, name: &str) -> ValueId {
        let zero = self.const_int_matching(val, 0);
        self.emit_checked_binop(
            "llvm.ssub.with.overflow",
            zero,
            val,
            name,
            "integer overflow on negation",
        )
    }

    /// Validate both operands are ints of matching width, returning the
    /// widened `IntValue`s and their shared `IntType`. Records a codegen
    /// error and returns `None` on a non-int operand or a width mismatch.
    ///
    /// Shared by `emit_checked_binop`, `emit_checked_div_rem`, and
    /// `emit_checked_shift` — all three overload their LLVM instruction on
    /// operand width (int/Duration/Size are i64, byte is i8; Spec: Clause
    /// 14.3; codegen-rules.md TR-1), and a fixed-width constant against a
    /// narrower operand is an LLVM IR verification failure.
    fn validate_checked_int_operands(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        operation: &'static str,
    ) -> Option<(IntValue<'ctx>, IntValue<'ctx>, IntType<'ctx>)> {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(
                lhs_type = ?l.get_type(), rhs_type = ?r.get_type(),
                name, "{operation} on non-int operands"
            );
            self.record_codegen_error();
            return None;
        }
        let lhs_int = l.into_int_value();
        let rhs_int = r.into_int_value();

        let op_ty = lhs_int.get_type();
        if op_ty != rhs_int.get_type() {
            tracing::error!(
                lhs_type = ?op_ty, rhs_type = ?rhs_int.get_type(),
                name, "{operation} operand width mismatch"
            );
            self.record_codegen_error();
            return None;
        }
        Some((lhs_int, rhs_int, op_ty))
    }

    /// Look up and declare the named overflow intrinsic at `op_ty`'s width.
    /// Records a codegen error and returns `None` if the intrinsic is
    /// unknown or cannot be declared at that width.
    fn declare_overflow_intrinsic(
        &mut self,
        intrinsic_name: &'static str,
        op_ty: IntType<'ctx>,
    ) -> Option<FunctionValue<'ctx>> {
        let Some(intrinsic) = Intrinsic::find(intrinsic_name) else {
            tracing::error!(intrinsic_name, "LLVM intrinsic not found");
            self.record_codegen_error();
            return None;
        };
        let Some(func_val) = intrinsic.get_declaration(&self.scx.llmod, &[op_ty.into()]) else {
            tracing::error!(intrinsic_name, "failed to declare intrinsic");
            self.record_codegen_error();
            return None;
        };
        Some(func_val)
    }

    /// Call the overflow intrinsic and extract its `{ result, overflow_flag }`
    /// pair. Records a codegen error and returns `None` if the call or
    /// extraction fails.
    fn call_overflow_intrinsic(
        &mut self,
        func_val: FunctionValue<'ctx>,
        lhs_int: IntValue<'ctx>,
        rhs_int: IntValue<'ctx>,
        name: &str,
        intrinsic_name: &'static str,
    ) -> Option<(BasicValueEnum<'ctx>, BasicValueEnum<'ctx>)> {
        let call_val = self
            .builder
            .build_call(func_val, &[lhs_int.into(), rhs_int.into()], name)
            .expect("overflow intrinsic call");
        let result_struct = call_val
            .try_as_basic_value()
            .basic()
            .expect("overflow intrinsic returns a value");

        let BasicValueEnum::StructValue(sv) = result_struct else {
            tracing::error!(intrinsic_name, "overflow intrinsic did not return struct");
            self.record_codegen_error();
            return None;
        };
        let result = self
            .builder
            .build_extract_value(sv, 0, &format!("{name}.val"))
            .expect("extract result");
        let overflow = self
            .builder
            .build_extract_value(sv, 1, &format!("{name}.ovf"))
            .expect("extract overflow flag");
        Some((result, overflow))
    }

    /// Shared implementation for checked arithmetic intrinsics.
    ///
    /// Calls the named overflow intrinsic, extracts `{ result, overflow_flag }`,
    /// branches to a panic block on overflow, and returns the result in the
    /// continue block.
    ///
    /// Uses the CSE cache to avoid emitting duplicate checked operations
    /// within the same ARC block. The cache key normalizes constant operands
    /// so that two different `ValueId`s for the same constant (e.g., two
    /// separate `const_i64(1)` calls) will hit the same cache entry.
    fn emit_checked_binop(
        &mut self,
        intrinsic_name: &'static str,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        panic_msg: &str,
    ) -> ValueId {
        // CSE cache lookup: normalize operands so identical constants
        // match regardless of which ValueId they were assigned.
        let cache_key = (intrinsic_name, self.cse_operand(lhs), self.cse_operand(rhs));
        if let Some(&cached) = self.cse_cache.get(&cache_key) {
            return cached;
        }

        let Some((lhs_int, rhs_int, op_ty)) =
            self.validate_checked_int_operands(lhs, rhs, name, "checked binop")
        else {
            return self.const_i64(0);
        };

        let Some(func_val) = self.declare_overflow_intrinsic(intrinsic_name, op_ty) else {
            return self.const_i64(0);
        };

        let Some((result, overflow)) =
            self.call_overflow_intrinsic(func_val, lhs_int, rhs_int, name, intrinsic_name)
        else {
            return self.const_i64(0);
        };

        // Create continue and panic blocks; branch on the overflow flag.
        let func_id = self.current_function.expect("no current function");
        let func_llvm = self.arena.get_function(func_id);
        let continue_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.ok"));
        let panic_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.ovf_panic"));

        let BasicValueEnum::IntValue(ovf_flag) = overflow else {
            tracing::error!(intrinsic_name, "overflow flag is not i1");
            self.record_codegen_error();
            return self.const_i64(0);
        };
        self.builder
            .build_conditional_branch(ovf_flag, panic_bb, continue_bb)
            .expect("overflow branch");

        // Panic block: route through the single emit_panic_block carrier so
        // add/sub/mul/neg overflow panics flow through the same invoke-when-
        // caught path as div/mod/shift (one carrier, no inline
        // ori_panic_cstr duplication).
        self.emit_panic_block(panic_bb, panic_msg, "ovf.msg");

        // Position at continue block, track it for save/restore, and store
        // the result in the CSE cache for reuse within this ARC block.
        self.builder.position_at_end(continue_bb);
        let continue_block_id = self.arena.push_block(continue_bb);
        self.current_block = Some(continue_block_id);
        let result_id = self.arena.push_value(result);
        self.cse_cache.insert(cache_key, result_id);

        result_id
    }
}
