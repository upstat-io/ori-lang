//! Shared control-flow scaffolding for iterator-backed loops.

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::scope::ArcScope;

use super::super::expr::{ArcLowerer, ForYieldContext, LoopContext};

pub(super) type HeaderMutableParam = (Name, ArcVarId, ArcVarId);

pub(super) struct IteratorFlowSetup {
    pub(super) header_block: ArcBlockId,
    pub(super) body_block: ArcBlockId,
    pub(super) exit_block: ArcBlockId,
    pub(super) exit_prep_block: ArcBlockId,
    pub(super) pre_scope: ArcScope,
    pub(super) header_mut_params: Vec<HeaderMutableParam>,
    pub(super) exit_mut_params: Vec<(Name, ArcVarId)>,
    pub(super) result_param: Option<ArcVarId>,
}

impl ArcLowerer<'_> {
    pub(super) fn prepare_iterator_flow(
        &mut self,
        exit_result_ty: Option<Idx>,
    ) -> IteratorFlowSetup {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        let exit_prep_block = self.builder.new_block();
        let pre_scope = self.scope.clone();
        let mutable_bindings: Vec<_> = pre_scope
            .mutable_bindings()
            .map(|(name, var)| (name, var, self.builder.var_type(var)))
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
        let result_param = exit_result_ty.map(|ty| self.builder.add_block_param(exit_block, ty));
        let exit_mut_params = mutable_bindings
            .iter()
            .map(|&(name, _, ty)| (name, self.builder.add_block_param(exit_block, ty)))
            .collect();

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

        IteratorFlowSetup {
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

    pub(super) fn emit_iterator_next(
        &mut self,
        iter_val: ArcVarId,
        elem_ty: Idx,
    ) -> (ArcVarId, ArcVarId) {
        let iter_next = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // INVARIANT: The scalar wrapper carries a typed marker for physical scratch sizing.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result = self.builder.emit_apply(
            Idx::INT,
            iter_next,
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

    pub(super) fn push_iterator_loop_context(
        &mut self,
        label: Name,
        iter_val: ArcVarId,
        setup: &IteratorFlowSetup,
        yield_ctx: Option<ForYieldContext>,
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
            yield_ctx,
        });
    }

    pub(super) fn lower_iterator_guard(
        &mut self,
        pattern: CanBindingPatternId,
        elem_ty: Idx,
        guard: CanId,
        next_result: ArcVarId,
        has_more: ArcVarId,
        setup: &IteratorFlowSetup,
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
}
