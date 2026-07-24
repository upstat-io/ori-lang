//! For-yield Option lowering — `for x in option yield body`.
//!
//! Produces a 0-or-1 element list.

use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp, YieldExtent};
use crate::lower::scope::ArcScope;

use super::super::expr::{ArcLowerer, ForYieldContext, ForYieldShape, LoopContext};
use super::for_yield::YieldListAllocation;

type MutableBinding = (Name, ArcVarId, Idx);

struct YieldOptionSetup {
    some_block: ArcBlockId,
    none_block: ArcBlockId,
    exit_block: ArcBlockId,
    pre_scope: ArcScope,
    mutable_bindings: Vec<MutableBinding>,
    exit_mut_params: Vec<(Name, ArcVarId)>,
    allocation: YieldListAllocation,
}

impl ArcLowerer<'_> {
    /// Lowers an option-backed comprehension into a zero-or-one-element list.
    ///
    /// Both discriminant paths converge with mutable bindings in exit-block
    /// parameter order; only the `Some` path evaluates the guard and body.
    pub(super) fn lower_for_yield_option(
        &mut self,
        shape: ForYieldShape,
        option_val: ArcVarId,
        elem_ty: Idx,
        label: ori_ir::Name,
    ) -> ArcVarId {
        let setup = self.prepare_yield_option(shape.result_ty, elem_ty);

        tracing::debug!(
            pattern = ?shape.pattern,
            exit_bb = setup.exit_block.index(),
            mutable_vars = setup.mutable_bindings.len(),
            has_guard = shape.guard.is_valid(),
            "for_yield_option: enter"
        );

        self.branch_yield_option(option_val, elem_ty, shape.pattern, &setup);
        let list_push = self.interner.intern("ori_list_push");
        self.push_yield_option_context(label, list_push, &setup);
        self.lower_yield_option_guard(shape.guard, &setup);
        self.lower_yield_option_body(shape.body, list_push, &setup);
        self.loop_ctx_stack.pop();
        self.finish_yield_option(shape.result_ty, setup)
    }

    fn prepare_yield_option(&mut self, result_ty: Idx, fallback_elem_ty: Idx) -> YieldOptionSetup {
        let some_block = self.builder.new_block();
        let none_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        let pre_scope = self.scope.clone();
        let mutable_bindings = pre_scope
            .mutable_bindings()
            .map(|(name, var)| (name, var, self.builder.var_type(var)))
            .collect::<Vec<_>>();

        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            fallback_elem_ty
        };
        let allocation = self.allocate_yield_list(body_elem_ty, YieldExtent::StaticExact(1));
        let exit_mut_params = mutable_bindings
            .iter()
            .map(|&(name, _, ty)| (name, self.builder.add_block_param(exit_block, ty)))
            .collect();

        YieldOptionSetup {
            some_block,
            none_block,
            exit_block,
            pre_scope,
            mutable_bindings,
            exit_mut_params,
            allocation,
        }
    }

    fn branch_yield_option(
        &mut self,
        option_val: ArcVarId,
        elem_ty: Idx,
        pattern: ori_ir::canon::CanBindingPatternId,
        setup: &YieldOptionSetup,
    ) {
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
            .terminate_branch(is_some, setup.some_block, setup.none_block);
        self.builder.position_at(setup.none_block);
        let none_args = setup
            .mutable_bindings
            .iter()
            .map(|&(_, pre_var, _)| pre_var)
            .collect();
        self.builder.terminate_jump(setup.exit_block, none_args);
        self.builder.position_at(setup.some_block);
        let elem = self.builder.emit_project(elem_ty, option_val, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);
    }

    fn push_yield_option_context(
        &mut self,
        label: Name,
        list_push: Name,
        setup: &YieldOptionSetup,
    ) {
        let mutable_vars = setup
            .mutable_bindings
            .iter()
            .map(|&(name, pre_var, _)| (name, pre_var))
            .collect();

        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block: setup.exit_block,
            continue_block: setup.exit_block,
            mutable_vars,
            abandon_iter: None,
            yield_ctx: Some(ForYieldContext {
                list_ptr: setup.allocation.list_ptr,
                elem_size: setup.allocation.elem_size_var,
                list_push_name: list_push,
            }),
        });
    }

    fn lower_yield_option_guard(&mut self, guard: ori_ir::canon::CanId, setup: &YieldOptionSetup) {
        if !guard.is_valid() {
            return;
        }
        let guard_val = self.lower_expr(guard);
        if self.builder.is_terminated() {
            return;
        }
        let body_block = self.builder.new_block();
        let guard_skip = self.builder.new_block();
        self.builder
            .terminate_branch(guard_val, body_block, guard_skip);
        self.builder.position_at(guard_skip);
        let skip_args = setup
            .mutable_bindings
            .iter()
            .map(|&(_, pre_var, _)| pre_var)
            .collect();
        self.builder.terminate_jump(setup.exit_block, skip_args);
        self.builder.position_at(body_block);
    }

    fn lower_yield_option_body(
        &mut self,
        body: ori_ir::canon::CanId,
        list_push: Name,
        setup: &YieldOptionSetup,
    ) {
        if self.builder.is_terminated() {
            return;
        }
        let body_val = self.lower_expr(body);
        if self.builder.is_terminated() {
            return;
        }
        self.builder.emit_apply(
            Idx::UNIT,
            list_push,
            vec![
                setup.allocation.list_ptr,
                body_val,
                setup.allocation.elem_size_var,
            ],
            None,
            None,
        );

        let exit_args = setup
            .mutable_bindings
            .iter()
            .map(|&(name, pre_var, _)| self.scope.lookup(name).unwrap_or(pre_var))
            .collect();
        self.builder.terminate_jump(setup.exit_block, exit_args);
    }

    fn finish_yield_option(&mut self, result_ty: Idx, setup: YieldOptionSetup) -> ArcVarId {
        self.builder.position_at(setup.exit_block);
        self.scope = setup.pre_scope;
        for &(name, param) in &setup.exit_mut_params {
            self.scope.bind_mutable(name, param);
        }

        let list_take = self.interner.intern("ori_list_take");
        let result = self.builder.emit_apply(
            result_ty,
            list_take,
            vec![setup.allocation.list_ptr],
            None,
            None,
        );

        self.builder.note_yield_allocation(
            setup.allocation.list_ptr,
            result,
            setup.allocation.elem_ty,
            setup.allocation.elem_size_var,
            setup.allocation.elem_size,
            setup.allocation.extent,
        );
        result
    }
}
