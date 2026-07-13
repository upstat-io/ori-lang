//! Checked integer shift left / right (arithmetic).

use inkwell::basic_block::BasicBlock;
use inkwell::types::IntType;
use inkwell::values::{FunctionValue, IntValue};

use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::value_id::ValueId;

impl<'ctx> IrBuilder<'_, 'ctx> {
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
