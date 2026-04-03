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

use inkwell::intrinsics::Intrinsic;
use inkwell::values::BasicValueEnum;

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

impl IrBuilder<'_, '_> {
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
    /// Emits `llvm.sadd.with.overflow.i64`, extracts the result and overflow
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
        self.emit_checked_binop(
            "llvm.smul.with.overflow",
            lhs,
            rhs,
            name,
            "integer overflow on multiplication",
        )
    }

    /// Build checked integer negation: panics on overflow.
    ///
    /// Negation is `0 - x`, so we reuse `@llvm.ssub.with.overflow.i64(0, x)`.
    /// The only overflowing case is `-INT_MIN` (result doesn't fit in i64).
    pub fn checked_neg(&mut self, val: ValueId, name: &str) -> ValueId {
        let zero = self.const_i64(0);
        self.emit_checked_binop(
            "llvm.ssub.with.overflow",
            zero,
            val,
            name,
            "integer overflow on negation",
        )
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

        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(
                lhs_type = ?l.get_type(), rhs_type = ?r.get_type(),
                intrinsic_name, "checked binop on non-int operands"
            );
            self.record_codegen_error();
            return self.const_i64(0);
        }

        // 1. Get or declare the overflow intrinsic.
        let Some(intrinsic) = Intrinsic::find(intrinsic_name) else {
            tracing::error!(intrinsic_name, "LLVM intrinsic not found");
            self.record_codegen_error();
            return self.const_i64(0);
        };
        let i64_ty = self.scx.llcx.i64_type();
        let Some(func_val) = intrinsic.get_declaration(&self.scx.llmod, &[i64_ty.into()]) else {
            tracing::error!(intrinsic_name, "failed to declare intrinsic");
            self.record_codegen_error();
            return self.const_i64(0);
        };

        // 2. Call the intrinsic: returns { i64, i1 }.
        let lhs_int = l.into_int_value();
        let rhs_int = r.into_int_value();
        let call_val = self
            .builder
            .build_call(func_val, &[lhs_int.into(), rhs_int.into()], name)
            .expect("overflow intrinsic call");
        let result_struct = call_val
            .try_as_basic_value()
            .basic()
            .expect("overflow intrinsic returns a value");

        // 3. Extract result (index 0) and overflow flag (index 1).
        let BasicValueEnum::StructValue(sv) = result_struct else {
            tracing::error!(intrinsic_name, "overflow intrinsic did not return struct");
            self.record_codegen_error();
            return self.const_i64(0);
        };
        let result = self
            .builder
            .build_extract_value(sv, 0, &format!("{name}.val"))
            .expect("extract result");
        let overflow = self
            .builder
            .build_extract_value(sv, 1, &format!("{name}.ovf"))
            .expect("extract overflow flag");

        // 4. Create continue and panic blocks.
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

        // 5. Branch: overflow → panic, else → continue.
        let BasicValueEnum::IntValue(ovf_flag) = overflow else {
            tracing::error!(intrinsic_name, "overflow flag is not i1");
            self.record_codegen_error();
            return self.const_i64(0);
        };
        self.builder
            .build_conditional_branch(ovf_flag, panic_bb, continue_bb)
            .expect("overflow branch");

        // 6. Panic block: call ori_panic_cstr with message, then unreachable.
        //    Uses the IrBuilder wrapper for string deduplication.
        self.builder.position_at_end(panic_bb);
        let msg_id = self.build_global_string_ptr(panic_msg, "ovf.msg");
        let panic_fn_id = self.runtime_fn("ori_panic_cstr");
        self.call(panic_fn_id, &[msg_id], "");
        self.builder.build_unreachable().expect("unreachable");

        // 7. Position at continue block and return the result.
        self.builder.position_at_end(continue_bb);
        // Track current block for save/restore.
        let continue_block_id = self.arena.push_block(continue_bb);
        self.current_block = Some(continue_block_id);
        let result_id = self.arena.push_value(result);

        // 8. Store in CSE cache for reuse within this ARC block.
        self.cse_cache.insert(cache_key, result_id);

        result_id
    }

    /// Build checked integer division: panics on division by zero or overflow.
    ///
    /// Checks: (1) `rhs == 0` → panic "division by zero",
    /// (2) `lhs == i64::MIN && rhs == -1` → panic "integer overflow in division".
    /// Both cases are UB in LLVM's `sdiv`.
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

    /// Emit a panic block: call `ori_panic_cstr(msg)` + `unreachable`.
    ///
    /// Positions the builder at the given block, emits the panic call, and
    /// adds an `unreachable` terminator. Does NOT reposition the builder
    /// after — caller must position at the next block.
    fn emit_panic_block(
        &mut self,
        block: inkwell::basic_block::BasicBlock<'_>,
        msg: &str,
        label: &str,
    ) {
        self.builder.position_at_end(block);
        let msg_id = self.build_global_string_ptr(msg, label);
        let panic_fn_id = self.runtime_fn("ori_panic_cstr");
        self.call(panic_fn_id, &[msg_id], "");
        self.builder.build_unreachable().expect("unreachable");
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
        let (l, r) = (self.arena.get_value(lhs), self.arena.get_value(rhs));
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(),
                name, "checked div/rem on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let (lhs_int, rhs_int) = (l.into_int_value(), r.into_int_value());
        let i64_ty = self.scx.llcx.i64_type();

        let func_id = self.current_function.expect("no current function");
        let func_llvm = self.arena.get_function(func_id);

        // Create blocks.
        let panic_zero_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.div0_panic"));
        let continue_bb = self
            .scx
            .llcx
            .append_basic_block(func_llvm, &format!("{name}.ok"));

        // For div: also create an overflow check block between zero-check and continue.
        let after_zero_bb = if is_div {
            self.scx
                .llcx
                .append_basic_block(func_llvm, &format!("{name}.check_ovf"))
        } else {
            continue_bb
        };

        // 1. Zero check: rhs == 0 → panic.
        let zero = i64_ty.const_zero();
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

        // 2. Panic-zero block.
        let zero_msg = if is_div {
            "division by zero"
        } else {
            "remainder by zero"
        };
        self.emit_panic_block(panic_zero_bb, zero_msg, &format!("{name}.dz_msg"));

        // 3. For div: overflow check (MIN / -1).
        if is_div {
            let panic_ovf_bb = self
                .scx
                .llcx
                .append_basic_block(func_llvm, &format!("{name}.ovf_panic"));

            self.builder.position_at_end(after_zero_bb);
            let min_val = i64_ty.const_int(i64::MIN as u64, false);
            let neg_one = i64_ty.const_all_ones();
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

        // 4. Continue block: emit the actual sdiv/srem.
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

    /// Build checked shift left: panics if count < 0 or count >= 64.
    ///
    /// Spec: Clause 9 — shift by negative count or by >= bit width panics.
    /// LLVM's `shl` produces poison for count >= bit width, which is UB.
    pub fn checked_shl(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_shift(lhs, rhs, name, true)
    }

    /// Build checked shift right (arithmetic): panics if count < 0 or count >= 64.
    pub fn checked_shr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.emit_checked_shift(lhs, rhs, name, false)
    }

    /// Shared implementation for checked shl/shr.
    ///
    /// Checks: (1) `rhs < 0` → panic "shift by negative count",
    /// (2) `rhs >= 64` → panic "shift count overflow",
    /// (3) for shl: `(result >> count) != lhs` → panic "integer overflow".
    /// All three cases produce poison or UB in LLVM.
    #[expect(
        clippy::too_many_lines,
        reason = "count validation + shift + overflow check is one indivisible operation"
    )]
    fn emit_checked_shift(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        is_left: bool,
    ) -> ValueId {
        let (l, r) = (self.arena.get_value(lhs), self.arena.get_value(rhs));
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(),
                name, "checked shift on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let (lhs_int, rhs_int) = (l.into_int_value(), r.into_int_value());
        let i64_ty = self.scx.llcx.i64_type();

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

        // 1. Negative count check: rhs < 0 → panic.
        let zero = i64_ty.const_zero();
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

        // 2. Width check: rhs >= 64 → panic.
        self.builder.position_at_end(check_width_bb);
        let bit_width = i64_ty.const_int(64, false);
        let is_too_wide = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::SGE,
                rhs_int,
                bit_width,
                &format!("{name}.rhs_wide"),
            )
            .expect("icmp sge 64");
        self.builder
            .build_conditional_branch(is_too_wide, panic_width_bb, continue_bb)
            .expect("width check branch");

        self.emit_panic_block(
            panic_width_bb,
            "shift count overflow",
            &format!("{name}.width_msg"),
        );

        // 3. Continue block: emit the actual shift.
        self.builder.position_at_end(continue_bb);
        if is_left {
            // Left shift: check for signed overflow.
            // `(result >> count) != lhs` means bits were lost (overflow).
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
        } else {
            // Right shift: no value overflow possible (arithmetic shift preserves sign).
            let result = self
                .builder
                .build_right_shift(lhs_int, rhs_int, true, name)
                .expect("ashr");
            let continue_block_id = self.arena.push_block(continue_bb);
            self.current_block = Some(continue_block_id);
            self.arena.push_value(result.into())
        }
    }
}
