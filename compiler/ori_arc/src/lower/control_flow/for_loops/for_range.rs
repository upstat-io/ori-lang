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
    /// Inclusive ranges (`0..=5`) use `i < end + inclusive` so both exclusive
    /// and inclusive work with a single `Lt` comparison.
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
        let mut mutable_var_names = Vec::new();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();

        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mutable_var_names.push(name);
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
        // Field 3 = inclusive flag (0 or 1). Adding it to end makes `i < end + inclusive`
        // work for both exclusive (i < end) and inclusive (i < end + 1 ≡ i <= end).
        let inclusive = self.builder.emit_project(Idx::INT, iter_val, 3, None);
        let adjusted_end = self.builder.emit_let(
            Idx::INT,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                args: vec![end, inclusive],
            },
            None,
        );

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

        let in_bounds = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Lt),
                args: vec![i_var, adjusted_end],
            },
            None,
        );

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
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: latch_block,
            mutable_vars: mutable_var_names,
        });

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            let body_args: Vec<_> = header_mut_params
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            self.builder.terminate_jump(latch_block, body_args);
        }

        self.loop_ctx = prev_loop;

        self.builder.position_at(latch_block);
        let one = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
        let next = self.builder.emit_let(
            Idx::INT,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                args: vec![i_var, one],
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
}
