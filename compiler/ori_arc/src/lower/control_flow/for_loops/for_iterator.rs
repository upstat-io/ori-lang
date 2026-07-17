//! Iterator-based for-loop lowering.

use ori_ir::canon::CanBindingPatternId;
use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::expr::{ArcLowerer, LoopContext};
use crate::lower::scope::ArcScope;

type MutableBinding = (Name, ArcVarId, Idx);
type HeaderMutableParam = (Name, ArcVarId, ArcVarId);

struct IteratorLoopSetup {
    header_block: ArcBlockId,
    body_block: ArcBlockId,
    exit_block: ArcBlockId,
    exit_prep_block: ArcBlockId,
    pre_scope: ArcScope,
    header_mut_params: Vec<HeaderMutableParam>,
    exit_mut_params: Vec<(Name, ArcVarId)>,
    result_param: ArcVarId,
}

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
        let setup = self.prepare_iterator_loop();

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
        self.push_iterator_loop_context(label, iter_val, &setup);
        self.lower_iterator_guard(pattern, elem_ty, guard, next_result, has_more, &setup);
        self.lower_iterator_body(pattern, elem_ty, body, next_result, &setup);
        self.loop_ctx_stack.pop();
        self.finish_iterator_loop(iter_val, setup)
    }

    fn prepare_iterator_loop(&mut self) -> IteratorLoopSetup {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        let exit_prep_block = self.builder.new_block();
        let pre_scope = self.scope.clone();
        let mutable_bindings: Vec<MutableBinding> = pre_scope
            .mutable_bindings()
            .map(|(name, var)| (name, var, self.builder.var_type_or_unit(var)))
            .collect();
        let header_mut_params = mutable_bindings
            .iter()
            .map(|&(name, pre_var, ty)| {
                (
                    name,
                    pre_var,
                    self.builder.add_block_param(header_block, ty),
                )
            })
            .collect::<Vec<_>>();
        let result_param = self.builder.add_block_param(exit_block, Idx::UNIT);
        let exit_mut_params = mutable_bindings
            .iter()
            .map(|&(name, _, ty)| (name, self.builder.add_block_param(exit_block, ty)))
            .collect::<Vec<_>>();

        let entry_args = header_mut_params
            .iter()
            .map(|&(_, pre_var, _)| pre_var)
            .collect();
        self.builder.terminate_jump(header_block, entry_args);
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for &(name, _, param) in &header_mut_params {
            self.scope.bind_mutable(name, param);
        }

        IteratorLoopSetup {
            header_block,
            body_block,
            exit_block,
            exit_prep_block,
            pre_scope,
            header_mut_params,
            exit_mut_params,
            result_param,
        }
    }

    fn emit_iterator_next(&mut self, iter_val: ArcVarId, elem_ty: Idx) -> (ArcVarId, ArcVarId) {
        let iter_next_name = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // INVARIANT: The scalar wrapper carries a typed marker for physical scratch sizing.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result = self.builder.emit_apply(
            Idx::INT,
            iter_next_name,
            vec![iter_val, elem_ty_marker],
            None,
            None,
        );
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
        (next_result, has_more)
    }

    fn push_iterator_loop_context(
        &mut self,
        label: Name,
        iter_val: ArcVarId,
        setup: &IteratorLoopSetup,
    ) {
        let mutable_vars = setup
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| (name, param))
            .collect();
        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block: setup.exit_block,
            continue_block: setup.header_block,
            mutable_vars,
            abandon_iter: Some(iter_val),
            yield_ctx: None,
        });
    }

    fn lower_iterator_guard(
        &mut self,
        pattern: CanBindingPatternId,
        elem_ty: Idx,
        guard: CanId,
        next_result: ArcVarId,
        has_more: ArcVarId,
        setup: &IteratorLoopSetup,
    ) {
        if !guard.is_valid() {
            self.builder
                .terminate_branch(has_more, setup.body_block, setup.exit_prep_block);
            return;
        }
        let guarded_block = self.builder.new_block();
        self.builder
            .terminate_branch(has_more, guarded_block, setup.exit_prep_block);
        self.builder.position_at(guarded_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);
        let guard_val = self.lower_expr(guard);
        if self.builder.is_terminated() {
            return;
        }
        let guard_skip = self.builder.new_block();
        self.builder
            .terminate_branch(guard_val, setup.body_block, guard_skip);
        self.builder.position_at(guard_skip);
        let skip_args = setup
            .header_mut_params
            .iter()
            .map(|&(_, _, param)| param)
            .collect();
        self.builder.terminate_jump(setup.header_block, skip_args);
    }

    fn lower_iterator_body(
        &mut self,
        pattern: CanBindingPatternId,
        elem_ty: Idx,
        body: CanId,
        next_result: ArcVarId,
        setup: &IteratorLoopSetup,
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

    fn finish_iterator_loop(&mut self, iter_val: ArcVarId, setup: IteratorLoopSetup) -> ArcVarId {
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
        setup.result_param
    }
}
