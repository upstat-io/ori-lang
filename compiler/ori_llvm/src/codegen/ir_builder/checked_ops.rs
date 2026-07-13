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

use inkwell::basic_block::BasicBlock;
use inkwell::intrinsics::Intrinsic;
use inkwell::types::IntType;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

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

    /// Build checked integer division: panics on division by zero or overflow.
    ///
    /// Checks: (1) `rhs == 0` → panic "division by zero",
    /// (2) `lhs == MIN && rhs == -1` (MIN at the operand's own width) →
    /// panic "integer overflow in division". Both cases are UB in LLVM's `sdiv`.
    pub fn checked_div(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_div_rem(lhs, rhs, name, true)
    }

    /// Build checked integer remainder: panics on division by zero.
    ///
    /// Checks: `rhs == 0` → panic "remainder by zero".
    /// No overflow check needed: `i64::MIN % -1 == 0` is well-defined.
    pub fn checked_rem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_div_rem(lhs, rhs, name, false)
    }

    /// Emit a panic block: panic via `ori_panic_cstr(msg)`, then terminate.
    ///
    /// THE single `ori_panic_cstr` carrier for every checked-op panic site.
    /// Positions the builder at `block`, emits the panic, and adds a
    /// terminator. Does NOT reposition the builder after — caller positions at
    /// the next block.
    ///
    /// When a same-frame catch landing pad is in scope
    /// (`self.builder.catch_unwind_target` is `Some`), the panic is emitted as
    /// `invoke @ori_panic_cstr → [normal: a fresh unreachable block, unwind:
    /// catch landing pad]` so the in-frame `catch(expr:)` recovers the panic
    /// instead of aborting. Otherwise it stays `call` + `unreachable` — the
    /// uncaught path that bubbles to the runtime top-level handler.
    fn emit_panic_block(&mut self, block: BasicBlock<'ctx>, msg: &str, label: &str) {
        self.builder.position_at_end(block);
        let msg_id = self.build_global_string_ptr(msg, label);
        let panic_fn_id = self.runtime_fn("ori_panic_cstr");

        if let Some(catch_target) = self.catch_unwind_target {
            // Caught path: invoke so the unwind reaches the catch landing pad.
            // The normal dest is a fresh unreachable block — ori_panic_cstr
            // never returns, but `invoke` requires a normal successor.
            let func_id = self.current_function.expect("no current function");
            let func_llvm = self.arena.get_function(func_id);
            let normal_bb = self
                .scx
                .llcx
                .append_basic_block(func_llvm, &format!("{label}.unreachable"));
            let normal_id = self.arena.push_block(normal_bb);
            // `invoke` builds at the current insertion point (already positioned
            // at `block`). Returns None for void `ori_panic_cstr` — ignored.
            self.invoke(panic_fn_id, &[msg_id], normal_id, catch_target, "");
            self.builder.position_at_end(normal_bb);
            self.builder.build_unreachable().expect("unreachable");
        } else {
            // Uncaught path: plain call + unreachable (bubbles to the runtime
            // top-level handler / abort).
            self.call(panic_fn_id, &[msg_id], "");
            self.builder.build_unreachable().expect("unreachable");
        }
    }

    /// Emit the `rhs == 0` check: branch to `panic_zero_bb` on zero, else
    /// `after_zero_bb`; `panic_zero_bb` panics with the div/rem-specific
    /// zero message.
    fn emit_div_rem_zero_check(
        &mut self,
        rhs_int: IntValue<'ctx>,
        op_ty: IntType<'ctx>,
        name: &str,
        panic_zero_bb: BasicBlock<'ctx>,
        after_zero_bb: BasicBlock<'ctx>,
        is_div: bool,
    ) {
        let zero = op_ty.const_zero();
        let is_zero = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                rhs_int,
                zero,
                &format!("{name}.rhs_z"),
            )
            .expect("icmp eq zero");
        self.builder
            .build_conditional_branch(is_zero, panic_zero_bb, after_zero_bb)
            .expect("zero check branch");

        let zero_msg = if is_div {
            "division by zero"
        } else {
            "remainder by zero"
        };
        self.emit_panic_block(panic_zero_bb, zero_msg, &format!("{name}.dz_msg"));
    }

    /// Position at `after_zero_bb` and emit the MIN/-1 overflow check for
    /// division: `lhs == MIN && rhs == -1` (MIN at `op_ty`'s own bit width)
    /// branches to a fresh panic block, else falls through to `continue_bb`.
    fn emit_div_overflow_guard(
        &mut self,
        lhs_int: IntValue<'ctx>,
        rhs_int: IntValue<'ctx>,
        op_ty: IntType<'ctx>,
        name: &str,
        after_zero_bb: BasicBlock<'ctx>,
        continue_bb: BasicBlock<'ctx>,
    ) {
        let func_id = self.current_function.expect("no current function");
        let func_llvm = self.arena.get_function(func_id);
        let panic_ovf_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.ovf_panic"));

        self.builder.position_at_end(after_zero_bb);
        // Signed MIN for this width's bit pattern: the sign bit alone
        // (e.g. 1<<63 for i64, 1<<7 for i8) — the two's-complement MIN
        // value of the operand's own width, not always i64::MIN.
        let bit_width = op_ty.get_bit_width();
        let min_val = op_ty.const_int(1_u64 << (bit_width - 1), true);
        let neg_one = op_ty.const_all_ones();
        let is_min = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                lhs_int,
                min_val,
                &format!("{name}.is_min"),
            )
            .expect("icmp eq MIN");
        let is_neg1 = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                rhs_int,
                neg_one,
                &format!("{name}.is_n1"),
            )
            .expect("icmp eq -1");
        let is_ovf = self
            .builder
            .build_and(is_min, is_neg1, &format!("{name}.ovf"))
            .expect("and");
        self.builder
            .build_conditional_branch(is_ovf, panic_ovf_bb, continue_bb)
            .expect("overflow branch");

        self.emit_panic_block(
            panic_ovf_bb,
            "integer overflow in division",
            &format!("{name}.ovf_msg"),
        );
    }

    /// Position at `continue_bb` and emit the `sdiv`/`srem` instruction.
    fn emit_div_rem_result(
        &mut self,
        continue_bb: BasicBlock<'ctx>,
        lhs_int: IntValue<'ctx>,
        rhs_int: IntValue<'ctx>,
        name: &str,
        is_div: bool,
    ) -> ValueId {
        self.builder.position_at_end(continue_bb);
        let result = if is_div {
            self.builder
                .build_int_signed_div(lhs_int, rhs_int, name)
                .expect("sdiv")
        } else {
            self.builder
                .build_int_signed_rem(lhs_int, rhs_int, name)
                .expect("srem")
        };

        let continue_block_id = self.arena.push_block(continue_bb);
        self.current_block = Some(continue_block_id);
        self.arena.push_value(result.into())
    }

    /// Shared implementation for checked div/rem.
    ///
    /// Emits a zero-check branch (panic on `rhs == 0`), and for division
    /// an additional MIN/-1 overflow check. The final block contains the
    /// `sdiv` or `srem` instruction.
    fn emit_checked_div_rem(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        is_div: bool,
    ) -> ValueId {
        let Some((lhs_int, rhs_int, op_ty)) =
            self.validate_checked_int_operands(lhs, rhs, name, "checked div/rem")
        else {
            return self.const_i64(0);
        };

        let func_id = self.current_function.expect("no current function");
        let func_llvm = self.arena.get_function(func_id);

        // Create blocks. For div: also create an overflow check block
        // between zero-check and continue.
        let panic_zero_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.div0_panic"));
        let continue_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.ok"));
        let after_zero_bb = if is_div {
            self.scx
                .llcx
                .append_basic_block(func_llvm, &format!("{name}.check_ovf"))
        } else {
            continue_bb
        };

        self.emit_div_rem_zero_check(rhs_int, op_ty, name, panic_zero_bb, after_zero_bb, is_div);

        if is_div {
            self.emit_div_overflow_guard(lhs_int, rhs_int, op_ty, name, after_zero_bb, continue_bb);
        }

        self.emit_div_rem_result(continue_bb, lhs_int, rhs_int, name, is_div)
    }

    /// Build checked shift left: panics if count < 0 or count >= the
    /// operand's own bit width.
    ///
    /// Spec: Clause 14.3 — shift by negative count or by >= bit width panics.
    /// LLVM's `shl` produces poison for count >= bit width, which is UB.
    pub fn checked_shl(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_shift(lhs, rhs, name, true)
    }

    /// Build checked shift right (arithmetic): panics if count < 0 or count
    /// >= the operand's own bit width.
    pub fn checked_shr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_shift(lhs, rhs, name, false)
    }

    /// Emit the negative-count check: `rhs < 0` branches to `panic_neg_bb`,
    /// else `check_width_bb`.
    fn emit_shift_negative_check(
        &mut self,
        rhs_int: IntValue<'ctx>,
        op_ty: IntType<'ctx>,
        name: &str,
        panic_neg_bb: BasicBlock<'ctx>,
        check_width_bb: BasicBlock<'ctx>,
    ) {
        let zero = op_ty.const_zero();
        let is_neg = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::SLT,
                rhs_int,
                zero,
                &format!("{name}.rhs_neg"),
            )
            .expect("icmp slt zero");
        self.builder
            .build_conditional_branch(is_neg, panic_neg_bb, check_width_bb)
            .expect("neg check branch");

        self.emit_panic_block(
            panic_neg_bb,
            "shift by negative count",
            &format!("{name}.neg_msg"),
        );
    }

    /// Position at `check_width_bb` and emit the width-limit check: `rhs >=
    /// op_ty`'s bit width branches to `panic_width_bb`, else `continue_bb`.
    fn emit_shift_width_check(
        &mut self,
        rhs_int: IntValue<'ctx>,
        op_ty: IntType<'ctx>,
        name: &str,
        check_width_bb: BasicBlock<'ctx>,
        panic_width_bb: BasicBlock<'ctx>,
        continue_bb: BasicBlock<'ctx>,
    ) {
        self.builder.position_at_end(check_width_bb);
        let bit_width = op_ty.const_int(u64::from(op_ty.get_bit_width()), false);
        let is_too_wide = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::SGE,
                rhs_int,
                bit_width,
                &format!("{name}.rhs_wide"),
            )
            .expect("icmp sge bit width");
        self.builder
            .build_conditional_branch(is_too_wide, panic_width_bb, continue_bb)
            .expect("width check branch");

        self.emit_panic_block(
            panic_width_bb,
            "shift count overflow",
            &format!("{name}.width_msg"),
        );
    }

    /// Position at `continue_bb` and emit a checked left shift: `shl`
    /// followed by an arithmetic-right-shift roundtrip check for lost bits
    /// (`(result >> count) != lhs` means bits were lost).
    fn emit_shift_left_result(
        &mut self,
        func_llvm: FunctionValue<'ctx>,
        lhs_int: IntValue<'ctx>,
        rhs_int: IntValue<'ctx>,
        name: &str,
        continue_bb: BasicBlock<'ctx>,
    ) -> ValueId {
        self.builder.position_at_end(continue_bb);
        let result = self
            .builder
            .build_left_shift(lhs_int, rhs_int, name)
            .expect("shl");
        let roundtrip = self
            .builder
            .build_right_shift(result, rhs_int, true, &format!("{name}.rt"))
            .expect("ashr roundtrip");
        let lost_bits = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::NE,
                roundtrip,
                lhs_int,
                &format!("{name}.ovf"),
            )
            .expect("icmp ne roundtrip");

        let panic_ovf_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.shl_ovf_panic"));
        let done_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.shl_done"));
        self.builder
            .build_conditional_branch(lost_bits, panic_ovf_bb, done_bb)
            .expect("ovf branch");

        self.emit_panic_block(
            panic_ovf_bb,
            "integer overflow on left shift",
            &format!("{name}.shl_ovf_msg"),
        );

        self.builder.position_at_end(done_bb);
        let done_block_id = self.arena.push_block(done_bb);
        self.current_block = Some(done_block_id);
        self.arena.push_value(result.into())
    }

    /// Position at `continue_bb` and emit an arithmetic right shift — no
    /// overflow check needed, since arithmetic shift preserves sign.
    fn emit_shift_right_result(
        &mut self,
        continue_bb: BasicBlock<'ctx>,
        lhs_int: IntValue<'ctx>,
        rhs_int: IntValue<'ctx>,
        name: &str,
    ) -> ValueId {
        self.builder.position_at_end(continue_bb);
        let result = self
            .builder
            .build_right_shift(lhs_int, rhs_int, true, name)
            .expect("ashr");
        let continue_block_id = self.arena.push_block(continue_bb);
        self.current_block = Some(continue_block_id);
        self.arena.push_value(result.into())
    }

    /// Shared implementation for checked shl/shr.
    ///
    /// Checks: (1) `rhs < 0` → panic "shift by negative count",
    /// (2) `rhs >= <operand bit width>` → panic "shift count overflow",
    /// (3) for shl: `(result >> count) != lhs` → panic "integer overflow".
    /// All three cases produce poison or UB in LLVM.
    fn emit_checked_shift(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        is_left: bool,
    ) -> ValueId {
        let Some((lhs_int, rhs_int, op_ty)) =
            self.validate_checked_int_operands(lhs, rhs, name, "checked shift")
        else {
            return self.const_i64(0);
        };

        let func_id = self.current_function.expect("no current function");
        let func_llvm = self.arena.get_function(func_id);
        let dir = if is_left { "shl" } else { "shr" };

        // Create blocks.
        let panic_neg_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.{dir}_neg_panic"));
        let check_width_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.{dir}_check_width"));
        let panic_width_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.{dir}_width_panic"));
        let continue_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.{dir}_ok"));

        self.emit_shift_negative_check(rhs_int, op_ty, name, panic_neg_bb, check_width_bb);
        self.emit_shift_width_check(
            rhs_int,
            op_ty,
            name,
            check_width_bb,
            panic_width_bb,
            continue_bb,
        );

        if is_left {
            self.emit_shift_left_result(func_llvm, lhs_int, rhs_int, name, continue_bb)
        } else {
            self.emit_shift_right_result(continue_bb, lhs_int, rhs_int, name)
        }
    }
}
