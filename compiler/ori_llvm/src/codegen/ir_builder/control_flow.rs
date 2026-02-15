//! Control flow operations (branch, switch, select, return) for `IrBuilder`.

use inkwell::values::IntValue;

use super::IrBuilder;
use crate::codegen::value_id::{BlockId, ValueId};

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Build an unconditional branch.
    pub fn br(&mut self, dest: BlockId) {
        let bb = self.arena.get_block(dest);
        self.builder
            .build_unconditional_branch(bb)
            .expect("build_br");
    }

    /// Build a conditional branch.
    ///
    /// Defensive: if `cond` is not an i1/int value, falls back to an
    /// unconditional branch to the else block instead of panicking.
    pub fn cond_br(&mut self, cond: ValueId, then_bb: BlockId, else_bb: BlockId) {
        let raw = self.arena.get_value(cond);
        if !raw.is_int_value() {
            tracing::error!(val_type = ?raw.get_type(), "cond_br on non-int — branching to else");
            self.record_codegen_error();
            self.br(else_bb);
            return;
        }
        let then_block = self.arena.get_block(then_bb);
        let else_block = self.arena.get_block(else_bb);
        self.builder
            .build_conditional_branch(raw.into_int_value(), then_block, else_block)
            .expect("build_cond_br");
    }

    /// Build a switch instruction.
    ///
    /// Defensive: if the scrutinee or any case value is not an int, falls
    /// back to a branch to the default block instead of panicking.
    pub fn switch(&mut self, val: ValueId, default: BlockId, cases: &[(ValueId, BlockId)]) {
        let raw = self.arena.get_value(val);
        if !raw.is_int_value() {
            tracing::error!(val_type = ?raw.get_type(), "switch on non-int — branching to default");
            self.record_codegen_error();
            self.br(default);
            return;
        }
        let default_bb = self.arena.get_block(default);
        let mut resolved: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::with_capacity(cases.len());
        for &(case_val, case_bb) in cases {
            let case_raw = self.arena.get_value(case_val);
            if !case_raw.is_int_value() {
                tracing::error!(val_type = ?case_raw.get_type(), "switch case is non-int — branching to default");
                self.record_codegen_error();
                self.br(default);
                return;
            }
            resolved.push((case_raw.into_int_value(), self.arena.get_block(case_bb)));
        }
        let switch = self
            .builder
            .build_switch(raw.into_int_value(), default_bb, &resolved)
            .expect("build_switch");
        let _ = switch;
    }

    /// Build a select (ternary) instruction.
    ///
    /// Defensive: if `cond` is not an i1/int, returns the else value
    /// instead of panicking.
    pub fn select(
        &mut self,
        cond: ValueId,
        then_val: ValueId,
        else_val: ValueId,
        name: &str,
    ) -> ValueId {
        let raw = self.arena.get_value(cond);
        if !raw.is_int_value() {
            tracing::error!(val_type = ?raw.get_type(), "select on non-int cond — returning else");
            self.record_codegen_error();
            return else_val;
        }
        let t = self.arena.get_value(then_val);
        let e = self.arena.get_value(else_val);
        let v = self
            .builder
            .build_select(raw.into_int_value(), t, e, name)
            .expect("select");
        self.arena.push_value(v)
    }

    /// Build a return with a value.
    pub fn ret(&mut self, val: ValueId) {
        let v = self.arena.get_value(val);
        self.builder.build_return(Some(&v)).expect("build_return");
    }

    /// Build a void return.
    pub fn ret_void(&mut self) {
        self.builder.build_return(None).expect("build_return");
    }

    /// Build an unreachable terminator.
    pub fn unreachable(&mut self) {
        self.builder.build_unreachable().expect("build_unreachable");
    }
}
