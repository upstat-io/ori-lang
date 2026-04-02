//! Option-based for-loop lowering (0-or-1 element iteration).

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::expr::ArcLowerer;

impl ArcLowerer<'_> {
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
    pub(in crate::lower) fn lower_for_option(
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
}
