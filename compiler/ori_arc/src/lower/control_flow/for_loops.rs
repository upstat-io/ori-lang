//! For-loop variant lowering — iterator, option, and range.
//!
//! Each variant produces a distinct block structure optimized for its
//! iteration pattern. Mutable variables from the enclosing scope flow
//! through the loop as SSA block parameters.
//!
//! - **Iterator**: `__iter_next` loop with tag/element projection.
//! - **Option**: 0-or-1 element branch (Some/None).
//! - **Range**: Direct counter-based loop with `start..end` projection.

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::super::expr::{ArcLowerer, LoopContext};

impl ArcLowerer<'_> {
    /// Lower `for x in <iterator> do body` using `__iter_next`.
    ///
    /// Loop structure:
    /// ```text
    /// entry → header
    /// header: next = __iter_next(iter); tag = project(next, 0);
    ///         has_more = (tag != 0); branch(has_more, body, exit)
    /// body: elem = project(next, 1); bind(elem); ... → header
    /// exit: ...
    /// ```
    #[expect(
        clippy::too_many_lines,
        reason = "iterator loop lowering with guard/mutable-var SSA merge is inherently sequential"
    )]
    pub(super) fn lower_for_iterator(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
    ) -> ArcVarId {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        // Normal exit prep block: Branch can't carry args, so the normal
        // exit path (iterator exhausted) goes header → exit_prep → exit.
        let exit_prep_block = self.builder.new_block();

        // Collect mutable bindings for SSA merge.
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
            exit_bb = exit_block.index(),
            mutable_vars = mut_info.len(),
            has_guard = guard.is_valid(),
            "for_iterator: enter"
        );

        // Header params: mutable vars only (no counter variable).
        let mut header_mut_params = Vec::new();
        for &(name, pre_var, var_ty) in &mut_info {
            let param = self.builder.add_block_param(header_block, var_ty);
            header_mut_params.push((name, pre_var, param));
        }

        // Exit block params: result value (from break) + mutable vars.
        // Matches what lower_break() sends: [break_val, mut0, mut1, ...]
        let result_param = self.builder.add_block_param(exit_block, Idx::UNIT);
        let mut exit_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(exit_block, var_ty);
            exit_mut_params.push((name, param));
        }

        // Entry jump: pass current mutable var values to header.
        let entry_args: Vec<_> = header_mut_params
            .iter()
            .map(|(_, pre_var, _)| *pre_var)
            .collect();
        self.builder.terminate_jump(header_block, entry_args);

        // Header: call __iter_next(iter) → {i8 has_more, T element}
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for &(name, _, param_var) in &header_mut_params {
            self.scope.bind_mutable(name, param_var);
        }

        let iter_next_name = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // Use INT for the result type to suppress ARC RC management on the
        // `{tag, elem}` wrapper struct.  The actual element (accessed via
        // Project at index 1) carries elem_ty and gets correct RC.
        // Pass a zero marker of elem_ty as args[1] so the LLVM emitter can
        // recover the element type for scratch buffer sizing.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result = self.builder.emit_apply(
            Idx::INT,
            iter_next_name,
            vec![iter_val, elem_ty_marker],
            None,
        );

        // Tag is field 0: 0 = done, 1 = has element
        let tag = self.builder.emit_project(Idx::INT, next_result, 0, None);
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let has_more = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::NotEq),
                args: vec![tag, zero],
            },
            None,
        );

        if guard.is_valid() {
            let guarded_block = self.builder.new_block();
            self.builder
                .terminate_branch(has_more, guarded_block, exit_prep_block);

            self.builder.position_at(guarded_block);
            let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
            self.bind_for_pattern(pattern, elem, elem_ty);
            let guard_val = self.lower_expr(guard);

            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            // Guard skip: jump back to header with unmodified mutable vars.
            self.builder.position_at(guard_skip);
            let skip_args: Vec<_> = header_mut_params
                .iter()
                .map(|&(_, _, param_var)| param_var)
                .collect();
            self.builder.terminate_jump(header_block, skip_args);
        } else {
            self.builder
                .terminate_branch(has_more, body_block, exit_prep_block);
        }

        // Body: extract element and bind.
        self.builder.position_at(body_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: header_block,
            mutable_vars: mutable_var_names,
        });

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            // Jump back to header with updated mutable var values.
            let body_args: Vec<_> = header_mut_params
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            self.builder.terminate_jump(header_block, body_args);
        }

        self.loop_ctx = prev_loop;

        // Exit prep: normal loop exhaustion path. Passes unit (no break
        // value) + current mutable var values to the exit block.
        self.builder.position_at(exit_prep_block);
        let unit_val = self.emit_unit();
        let mut prep_args = vec![unit_val];
        prep_args.extend(header_mut_params.iter().map(|&(_, _, param_var)| param_var));
        self.builder.terminate_jump(exit_block, prep_args);

        // Exit: drop the iterator handle, then restore scope.
        // Both normal exhaustion and break paths converge here.
        self.builder.position_at(exit_block);
        let iter_drop_name = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop_name, vec![iter_val], None);
        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        result_param
    }

    /// Lower `for x in <option> do body` — 0-or-1 element iteration.
    ///
    /// Option layout: `{i64 tag, T payload}` (tag 0=None, 1=Some).
    /// Structure:
    /// ```text
    /// entry: is_some = (tag != 0); branch(is_some, some_block, none_block)
    /// none_block: jump(exit, [pre_muts...])
    /// some_block: bind elem; [guard check]; body; jump(exit, [post_muts...])
    /// exit(muts_merged...): ...
    /// ```
    pub(super) fn lower_for_option(
        &mut self,
        pattern: CanBindingPatternId,
        option_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
    ) -> ArcVarId {
        let some_block = self.builder.new_block();
        let none_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        tracing::debug!(
            pattern = ?pattern,
            some_bb = some_block.index(),
            none_bb = none_block.index(),
            exit_bb = exit_block.index(),
            has_guard = guard.is_valid(),
            "for_option: enter"
        );

        // Collect mutable bindings for SSA merge.
        let pre_scope = self.scope.clone();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mut_info.push((name, var, var_ty));
        }

        // Exit params: mutable vars (to merge some vs none paths).
        let mut exit_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(exit_block, var_ty);
            exit_mut_params.push((name, param));
        }

        // Check tag: project field 0. ARC convention: Some=0, None=1.
        let tag = self.builder.emit_project(Idx::INT, option_val, 0, None);
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let is_some = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag, zero],
            },
            None,
        );
        self.builder
            .terminate_branch(is_some, some_block, none_block);

        // None path: jump to exit with unmodified mutable vars.
        self.builder.position_at(none_block);
        let none_args: Vec<_> = mut_info.iter().map(|(_, var, _)| *var).collect();
        self.builder.terminate_jump(exit_block, none_args);

        // Some path: extract element, bind, optionally check guard, run body.
        self.builder.position_at(some_block);
        self.scope = pre_scope.clone();
        let elem = self.builder.emit_project(elem_ty, option_val, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        if guard.is_valid() {
            let body_block = self.builder.new_block();
            let guard_val = self.lower_expr(guard);

            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            // Guard skip: jump to exit with unmodified mutable vars.
            self.builder.position_at(guard_skip);
            let gskip_args: Vec<_> = mut_info.iter().map(|(_, var, _)| *var).collect();
            self.builder.terminate_jump(exit_block, gskip_args);

            self.builder.position_at(body_block);
        }

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            let body_args: Vec<_> = mut_info
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            self.builder.terminate_jump(exit_block, body_args);
        }

        // Exit: restore scope with merged mutable vars.
        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        self.emit_unit()
    }

    /// Lower `for i in <range> do body` using direct start/end projection.
    ///
    /// Range layout: `{i64 start, i64 end, i64 step, i64 inclusive}`.
    /// Inclusive ranges (`0..=5`) use `i < end + inclusive` so both exclusive
    /// and inclusive work with a single `Lt` comparison.
    #[expect(
        clippy::too_many_lines,
        reason = "range loop lowering with guard/latch/mutable-var SSA merge is inherently sequential"
    )]
    pub(super) fn lower_for_range(
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
