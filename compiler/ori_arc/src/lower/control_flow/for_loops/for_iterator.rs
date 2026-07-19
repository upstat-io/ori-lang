//! Iterator-based for-loop lowering.

use ori_ir::canon::CanBindingPatternId;
use ori_ir::canon::CanId;
use ori_types::Idx;

use crate::ir::ArcVarId;
use crate::lower::control_flow::iterator_flow::IteratorFlowSetup;
use crate::lower::expr::ArcLowerer;

impl ArcLowerer<'_> {
    /// Lowers iterator traversal while threading mutable bindings through loop blocks.
    pub(in crate::lower) fn lower_for_iterator(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
        label: ori_ir::Name,
    ) -> ArcVarId {
        let setup = self.prepare_iterator_flow(Some(Idx::UNIT));

        tracing::debug!(
            pattern = ?pattern,
            header_bb = setup.header_block.index(),
            body_bb = setup.body_block.index(),
            exit_bb = setup.exit_block.index(),
            mutable_vars = setup.header_mut_params.len(),
            has_guard = guard.is_valid(),
            "for_iterator: enter"
        );

        let (next_result, has_more) = self.emit_iterator_next(iter_val, elem_ty);
        self.push_iterator_loop_context(label, iter_val, &setup, None);
        self.lower_iterator_guard(pattern, elem_ty, guard, next_result, has_more, &setup);
        self.lower_iterator_body(pattern, elem_ty, body, next_result, &setup);
        self.loop_ctx_stack.pop();
        self.finish_iterator_loop(iter_val, setup)
    }

    fn lower_iterator_body(
        &mut self,
        pattern: CanBindingPatternId,
        elem_ty: Idx,
        body: CanId,
        next_result: ArcVarId,
        setup: &IteratorFlowSetup,
    ) {
        self.builder.position_at(setup.body_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);
        self.lower_expr(body);
        if self.builder.is_terminated() {
            return;
        }
        let body_args = setup
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
            .collect();
        self.builder.terminate_jump(setup.header_block, body_args);
    }

    fn finish_iterator_loop(&mut self, iter_val: ArcVarId, setup: IteratorFlowSetup) -> ArcVarId {
        self.builder.position_at(setup.exit_prep_block);
        let unit_val = self.emit_unit();
        let mut prep_args = vec![unit_val];
        prep_args.extend(setup.header_mut_params.iter().map(|&(_, _, param)| param));
        self.builder.terminate_jump(setup.exit_block, prep_args);

        self.builder.position_at(setup.exit_block);
        let iter_drop_name = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop_name, vec![iter_val], None, None);
        self.scope = setup.pre_scope;
        for &(name, param) in &setup.exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        setup
            .result_param
            .unwrap_or_else(|| unreachable!("unit iterator loop must carry an exit result"))
    }
}
