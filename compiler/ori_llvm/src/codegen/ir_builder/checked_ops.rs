//! Checked arithmetic operations for `IrBuilder`.
//!
//! Integer add/sub/mul/neg use checked overflow intrinsics
//! (`llvm.sadd.with.overflow`, etc.) that panic on overflow. This matches
//! Ori's spec (overflow = panic) and avoids LLVM UB. For compile-time
//! constant operands, LLVM constant-folds the overflow branch away entirely.

use inkwell::intrinsics::Intrinsic;
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::ValueId;

impl IrBuilder<'_, '_> {
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
    fn emit_checked_binop(
        &mut self,
        intrinsic_name: &str,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        panic_msg: &str,
    ) -> ValueId {
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
        let intrinsic = Intrinsic::find(intrinsic_name).unwrap_or_else(|| {
            panic!("LLVM intrinsic `{intrinsic_name}` not found");
        });
        let i64_ty = self.scx.llcx.i64_type();
        let func_val = intrinsic
            .get_declaration(&self.scx.llmod, &[i64_ty.into()])
            .unwrap_or_else(|| {
                panic!("failed to declare `{intrinsic_name}.i64`");
            });

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
            panic!("overflow intrinsic did not return struct");
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
            panic!("overflow flag is not i1");
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
        self.arena.push_value(result)
    }
}
