//! Checked integer division and remainder.

use inkwell::basic_block::BasicBlock;
use inkwell::types::IntType;
use inkwell::values::IntValue;

use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::value_id::ValueId;

impl<'ctx> IrBuilder<'_, 'ctx> {
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
}
