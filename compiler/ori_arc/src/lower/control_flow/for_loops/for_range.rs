//! Range-based for-loop lowering.

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::expr::{ArcLowerer, LoopContext};

impl ArcLowerer<'_> {
    /// Lower `for i in <range> do body` using direct start/end projection.
    ///
    /// Range layout: `{i64 start, i64 end, i64 step, i64 inclusive}`.
    /// The loop condition uses sign-aware comparison to avoid overflow:
    /// - Ascending (step > 0): `i < end` (exclusive) or `i <= end` (inclusive)
    /// - Descending (step < 0): `i > end` (exclusive) or `i >= end` (inclusive)
    ///
    /// The old approach (`i < end + inclusive`) overflows for `0..=INT_MAX`
    /// because `INT_MAX + 1` wraps to `INT_MIN`.
    #[expect(
        clippy::too_many_lines,
        reason = "range loop lowering with guard/latch/mutable-var SSA merge is inherently sequential"
    )]
    pub(in crate::lower) fn lower_for_range(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        _iter_ty: Idx,
        guard: CanId,
        body: CanId,
    ) -> ArcVarId {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let latch_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        // Normal exit prep block: Branch can't carry args, so the normal
        // exit path (range exhausted) goes header → exit_prep → exit.
        let exit_prep_block = self.builder.new_block();

        // Collect mutable bindings for SSA merge through the loop.
        let pre_scope = self.scope.clone();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();

        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mut_info.push((name, var, var_ty));
        }

        tracing::debug!(
            pattern = ?pattern,
            header_bb = header_block.index(),
            body_bb = body_block.index(),
            latch_bb = latch_block.index(),
            exit_bb = exit_block.index(),
            mutable_vars = mut_info.len(),
            "for_range: enter"
        );

        // Header params: i_var first, then mutable vars.
        let i_var = self.builder.add_block_param(header_block, Idx::INT);
        let mut header_mut_params = Vec::new();
        for &(name, pre_var, var_ty) in &mut_info {
            let param = self.builder.add_block_param(header_block, var_ty);
            header_mut_params.push((name, pre_var, param));
        }

        // Latch params: mutable vars only (i_var from header dominates).
        let mut latch_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(latch_block, var_ty);
            latch_mut_params.push((name, param));
        }

        // Exit block params: result value (from break) + mutable vars.
        // Matches what lower_break() sends: [break_val, mut0, mut1, ...]
        let result_param = self.builder.add_block_param(exit_block, Idx::UNIT);
        let mut exit_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(exit_block, var_ty);
            exit_mut_params.push((name, param));
        }

        let start = self.builder.emit_project(Idx::INT, iter_val, 0, None);
        let end = self.builder.emit_project(Idx::INT, iter_val, 1, None);
        let step = self.builder.emit_project(Idx::INT, iter_val, 2, None);

        // Specialization: detect compile-time-constant step and inclusive
        // to emit a single bounds-check instruction instead of the general
        // 8-instruction condition. At -O1+ LLVM constant-folds anyway, but
        // at -O0 this reduces header bloat from 8 instructions to 1.
        //
        // Query inclusive without emitting a Project — only extract it in
        // the general path where it's actually needed.
        let step_lit = self.builder.get_literal_int(step);
        let incl_lit = self.builder.get_field_literal_int(iter_val, 3);

        // Zero-step guard: only needed when step is unknown at compile time.
        // Known non-zero steps (1, -1, etc.) skip the guard entirely.
        if step_lit.is_none() || step_lit == Some(0) {
            self.emit_zero_step_guard(step);
        }

        // Entry jump args match header param order: [start, mut0, mut1, ...]
        let mut entry_args = vec![start];
        entry_args.extend(header_mut_params.iter().map(|(_, pre_var, _)| *pre_var));
        self.builder.terminate_jump(header_block, entry_args);

        // Position in header block and rebind mutable vars to header params.
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for &(name, _, param_var) in &header_mut_params {
            self.scope.bind_mutable(name, param_var);
        }

        // Emit bounds check: specialized single instruction or general 8-instruction path.
        let in_bounds = match (step_lit, incl_lit) {
            // step=1, exclusive: i < end
            (Some(1), Some(0)) => self.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::Lt),
                    args: vec![i_var, end],
                },
                None,
            ),
            // step=1, inclusive: i <= end
            (Some(1), Some(1)) => self.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::LtEq),
                    args: vec![i_var, end],
                },
                None,
            ),
            // step=-1, exclusive: i > end
            (Some(-1), Some(0)) => self.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::Gt),
                    args: vec![i_var, end],
                },
                None,
            ),
            // step=-1, inclusive: i >= end
            (Some(-1), Some(1)) => self.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op: PrimOp::Binary(ori_ir::BinaryOp::GtEq),
                    args: vec![i_var, end],
                },
                None,
            ),
            // General path: extract inclusive field only here where it's needed.
            _ => {
                let inclusive = self.builder.emit_project(Idx::INT, iter_val, 3, None);
                self.emit_general_range_condition(i_var, end, step, inclusive)
            }
        };

        if guard.is_valid() {
            let guarded_block = self.builder.new_block();
            self.builder
                .terminate_branch(in_bounds, guarded_block, exit_prep_block);

            self.builder.position_at(guarded_block);
            self.bind_for_pattern(pattern, i_var, Idx::INT);
            let guard_val = self.lower_expr(guard);

            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            self.builder.position_at(guard_skip);
            let skip_args: Vec<_> = header_mut_params
                .iter()
                .map(|&(_, _, param_var)| param_var)
                .collect();
            self.builder.terminate_jump(latch_block, skip_args);
        } else {
            self.builder
                .terminate_branch(in_bounds, body_block, exit_prep_block);
        }

        self.builder.position_at(body_block);
        self.bind_for_pattern(pattern, i_var, Idx::INT);

        let prev_loop = self.loop_ctx.take();
        let mutable_var_entries: Vec<_> = header_mut_params
            .iter()
            .map(|&(name, _, param)| (name, param))
            .collect();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: latch_block,
            mutable_vars: mutable_var_entries,
            yield_ctx: None,
        });

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            let body_args: Vec<_> = header_mut_params
                .iter()
                .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
                .collect();
            self.builder.terminate_jump(latch_block, body_args);
        }

        self.loop_ctx = prev_loop;

        self.builder.position_at(latch_block);
        let next = self.builder.emit_let(
            Idx::INT,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                args: vec![i_var, step],
            },
            None,
        );
        let mut header_args = vec![next];
        header_args.extend(latch_mut_params.iter().map(|(_, param)| *param));
        self.builder.terminate_jump(header_block, header_args);

        // Exit prep: normal range exhaustion path. Passes unit (no break
        // value) + current mutable var values to the exit block.
        self.builder.position_at(exit_prep_block);
        let unit_val = self.emit_unit();
        let mut prep_args = vec![unit_val];
        prep_args.extend(header_mut_params.iter().map(|&(_, _, param_var)| param_var));
        self.builder.terminate_jump(exit_block, prep_args);

        // Exit: restore scope with mutable vars from exit block params.
        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        result_param
    }

    /// Emit a zero-step guard: panic at runtime if `step == 0`.
    ///
    /// Creates a branch: if step is zero, jump to a panic block;
    /// otherwise continue to a new loop-entry block. Positions the
    /// builder at the loop-entry block on return.
    fn emit_zero_step_guard(&mut self, step: ArcVarId) {
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

        // Panic block: emit "range step cannot be zero" and halt.
        self.builder.position_at(panic_block);
        let panic_msg = self.interner.intern("range step cannot be zero");
        let msg_var = self.builder.emit_let(
            Idx::STR,
            ArcValue::Literal(LitValue::String(panic_msg)),
            None,
        );
        let panic_fn = self.interner.intern("ori_panic");
        self.builder
            .emit_apply(Idx::UNIT, panic_fn, vec![msg_var], None);
        self.builder.terminate_unreachable();

        // Continue in loop entry block.
        self.builder.position_at(loop_entry_block);
    }

    /// Emit the general 8-instruction sign-aware range condition.
    ///
    /// ```text
    /// asc_part  = (step > 0) && (i < end)
    /// desc_part = (step < 0) && (i > end)
    /// base      = asc_part || desc_part
    /// incl_part = (inclusive > 0) && (i == end)
    /// in_bounds = base || incl_part
    /// ```
    fn emit_general_range_condition(
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
