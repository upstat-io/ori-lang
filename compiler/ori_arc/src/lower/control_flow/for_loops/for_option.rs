//! Option-based for-loop lowering (0-or-1 element iteration).

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::expr::{ArcLowerer, LoopContext};

/// SSA-merge params for an option for-loop: `(mut_info, exit_mut_params, continue_mut_params)`.
type OptionMergeParams = (
    Vec<(Name, ArcVarId, Idx)>,
    Vec<(Name, ArcVarId)>,
    Vec<ArcVarId>,
);

impl ArcLowerer<'_> {
    /// Lower `for x in <option> do body` — 0-or-1 element iteration.
    ///
    /// The logical Option carrier exposes a discriminant and payload path.
    /// Physical tags, niches, and payload offsets belong to compiled layout.
    /// Structure:
    /// ```text
    /// entry: branch on the logical Some discriminant
    /// none_block: jump(exit, [pre_muts...])
    /// some_block: bind elem; [guard check]; body; jump(exit, [post_muts...])
    /// exit(muts_merged...): ...
    /// ```
    pub(in crate::lower) fn lower_for_option(
        &mut self,
        pattern: CanBindingPatternId,
        option_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
        label: ori_ir::Name,
    ) -> ArcVarId {
        let some_block = self.builder.new_block();
        let none_block = self.builder.new_block();
        // continue_block funnels a guard/body `continue` (which carries no value)
        // into exit_block, supplying the unit result the exit param expects.
        let continue_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        tracing::debug!(
            pattern = ?pattern,
            some_bb = some_block.index(),
            none_bb = none_block.index(),
            exit_bb = exit_block.index(),
            has_guard = guard.is_valid(),
            "for_option: enter"
        );

        // Collect mutable bindings and add the SSA-merge block params for the
        // exit and continue blocks.
        let pre_scope = self.scope.clone();
        let (mut_info, exit_mut_params, continue_mut_params) =
            self.build_option_merge_params(exit_block, continue_block);

        // Check tag: project field 0. Compare to OPTION_TAG_SOME.
        let tag = self.builder.emit_project(Idx::INT, option_val, 0, None);
        let some_tag = self.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(ori_ir::OPTION_TAG_SOME)),
            None,
        );
        let is_some = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag, some_tag],
            },
            None,
        );
        self.builder
            .terminate_branch(is_some, some_block, none_block);

        // None path: jump to exit with [unit, ...unmodified mutable vars].
        self.builder.position_at(none_block);
        let none_unit = self.emit_unit();
        let mut none_args = vec![none_unit];
        none_args.extend(mut_info.iter().map(|(_, var, _)| *var));
        self.builder.terminate_jump(exit_block, none_args);

        // Continue block: a `continue` jumps here with [...muts]; forward to exit
        // supplying a unit result (0-or-1 iteration has no next element).
        self.builder.position_at(continue_block);
        let cont_unit = self.emit_unit();
        let mut cont_args = vec![cont_unit];
        cont_args.extend(continue_mut_params.iter().copied());
        self.builder.terminate_jump(exit_block, cont_args);

        // Some path: extract element, bind, optionally check guard, run body.
        self.builder.position_at(some_block);
        self.scope = pre_scope.clone();
        let elem = self.builder.emit_project(elem_ty, option_val, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        // Install this for's loop context BEFORE the guard so a guard-position
        // break/continue targets this for (exit_block / continue_block), not an
        // enclosing loop. Option is not a loop — continue funnels to exit.
        let mutable_var_entries: Vec<_> =
            mut_info.iter().map(|&(name, var, _)| (name, var)).collect();
        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block,
            continue_block,
            mutable_vars: mutable_var_entries,
            abandon_iter: None,
            yield_ctx: None,
        });

        if guard.is_valid() {
            let guard_val = self.lower_expr(guard);

            // Only branch on the guard value when a guard-position break/continue
            // did NOT already terminate the some-block.
            if !self.builder.is_terminated() {
                let body_block = self.builder.new_block();
                let guard_skip = self.builder.new_block();
                self.builder
                    .terminate_branch(guard_val, body_block, guard_skip);

                // Guard skip: jump to exit with [unit, ...unmodified mutable vars].
                self.builder.position_at(guard_skip);
                let gskip_unit = self.emit_unit();
                let mut gskip_args = vec![gskip_unit];
                gskip_args.extend(mut_info.iter().map(|(_, var, _)| *var));
                self.builder.terminate_jump(exit_block, gskip_args);

                self.builder.position_at(body_block);
            }
        }

        // Body: skipped entirely when a guard-position break/continue already
        // diverted control (some-block terminated).
        if !self.builder.is_terminated() {
            self.lower_expr(body);

            if !self.builder.is_terminated() {
                let body_unit = self.emit_unit();
                let mut body_args = vec![body_unit];
                body_args.extend(
                    mut_info
                        .iter()
                        .map(|(name, var, _)| self.scope.lookup(*name).unwrap_or(*var)),
                );
                self.builder.terminate_jump(exit_block, body_args);
            }
        }

        self.loop_ctx_stack.pop();

        // Exit: restore scope with merged mutable vars (result param discarded).
        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        self.emit_unit()
    }

    /// Build the SSA-merge block params for an option for-loop.
    ///
    /// Collects the live mutable bindings, then adds the merge params to the
    /// exit and continue blocks:
    /// - exit block: result value (break value; unit for for-do — break value
    ///   is E0860) followed by the mutable vars. The result param makes a
    ///   guard/body `break` (which sends `[break_val, ...muts]`) arg-count-match
    ///   this exit block.
    /// - continue block: mutable vars only (continue carries no value).
    ///
    /// Returns `(mut_info, exit_mut_params, continue_mut_params)`.
    fn build_option_merge_params(
        &mut self,
        exit_block: ArcBlockId,
        continue_block: ArcBlockId,
    ) -> OptionMergeParams {
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();
        for (name, var) in self.scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mut_info.push((name, var, var_ty));
        }

        let _result_param = self.builder.add_block_param(exit_block, Idx::UNIT);
        let mut exit_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(exit_block, var_ty);
            exit_mut_params.push((name, param));
        }

        let mut continue_mut_params = Vec::new();
        for &(_, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(continue_block, var_ty);
            continue_mut_params.push(param);
        }

        (mut_info, exit_mut_params, continue_mut_params)
    }
}
