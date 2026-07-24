//! Sign-aware range-loop condition construction.

use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::expr::ArcLowerer;

impl ArcLowerer<'_> {
    /// Branch to the runtime panic path when `step` is zero.
    pub(super) fn emit_zero_step_guard(&mut self, step: ArcVarId) {
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let step_is_zero = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![step, zero],
            },
            None,
        );
        let panic_block = self.builder.new_block();
        let loop_entry_block = self.builder.new_block();
        self.builder
            .terminate_branch(step_is_zero, panic_block, loop_entry_block);

        self.builder.position_at(panic_block);
        let panic_msg = self.interner.intern("range step cannot be zero");
        let msg_var = self.builder.emit_let(
            Idx::STR,
            ArcValue::Literal(LitValue::String(panic_msg)),
            None,
        );
        let panic_fn = self.interner.intern("ori_panic");
        self.builder
            .emit_apply(Idx::UNIT, panic_fn, vec![msg_var], None, None);
        self.builder.terminate_unreachable();
        self.builder.position_at(loop_entry_block);
    }

    /// Emit the sign-aware condition without adjusting either endpoint.
    pub(super) fn emit_general_range_condition(
        &mut self,
        i_var: ArcVarId,
        end: ArcVarId,
        step: ArcVarId,
        inclusive: ArcVarId,
    ) -> ArcVarId {
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let step_pos = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Gt),
                args: vec![step, zero],
            },
            None,
        );
        let step_neg = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Lt),
                args: vec![step, zero],
            },
            None,
        );
        let is_incl = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Gt),
                args: vec![inclusive, zero],
            },
            None,
        );
        let lt_val = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Lt),
                args: vec![i_var, end],
            },
            None,
        );
        let gt_val = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Gt),
                args: vec![i_var, end],
            },
            None,
        );
        let eq_val = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![i_var, end],
            },
            None,
        );
        let asc_part = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::And),
                args: vec![step_pos, lt_val],
            },
            None,
        );
        let desc_part = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::And),
                args: vec![step_neg, gt_val],
            },
            None,
        );
        let base = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Or),
                args: vec![asc_part, desc_part],
            },
            None,
        );
        let incl_part = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::And),
                args: vec![is_incl, eq_val],
            },
            None,
        );
        self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Or),
                args: vec![base, incl_part],
            },
            None,
        )
    }
}
