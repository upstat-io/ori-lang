//! Range-based for-loop lowering.

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, PrimOp, YieldExtent};
use crate::lower::expr::{ArcLowerer, ForYieldContext, ForYieldShape, LoopContext};
use crate::lower::scope::ArcScope;

type MutableBinding = (Name, ArcVarId, Idx);
type HeaderMutableParam = (Name, ArcVarId, ArcVarId);

struct RangeLoopSetup {
    header_block: ArcBlockId,
    body_block: ArcBlockId,
    latch_block: ArcBlockId,
    exit_block: ArcBlockId,
    exit_prep_block: ArcBlockId,
    pre_scope: ArcScope,
    i_var: ArcVarId,
    header_mut_params: Vec<HeaderMutableParam>,
    latch_mut_params: Vec<(Name, ArcVarId)>,
    exit_mut_params: Vec<(Name, ArcVarId)>,
    result_param: Option<ArcVarId>,
}

impl ArcLowerer<'_> {
    /// Lowers a range loop from its logical `start`, `end`, `step`, and inclusive flag.
    ///
    /// Sign-aware comparisons avoid overflow-prone endpoint adjustment. Mutable
    /// bindings flow through header, latch, and exit block parameters.
    pub(in crate::lower) fn lower_for_range(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        _iter_ty: Idx,
        guard: CanId,
        body: CanId,
        label: ori_ir::Name,
    ) -> ArcVarId {
        let setup = self.prepare_range_loop(false);

        tracing::debug!(
            pattern = ?pattern,
            header_bb = setup.header_block.index(),
            body_bb = setup.body_block.index(),
            latch_bb = setup.latch_block.index(),
            exit_bb = setup.exit_block.index(),
            mutable_vars = setup.header_mut_params.len(),
            "for_range: enter"
        );

        let (step, in_bounds) = self.enter_range_header(iter_val, &setup);
        self.push_range_loop_context(label, &setup);
        self.lower_range_guard(pattern, guard, in_bounds, &setup);
        self.lower_range_body(pattern, body, &setup);
        self.loop_ctx_stack.pop();
        self.emit_range_latch(step, &setup);
        self.finish_range_loop(setup)
    }

    /// Lower a range-backed comprehension without materializing an iterator.
    pub(in crate::lower) fn lower_for_yield_range(
        &mut self,
        shape: ForYieldShape,
        iter_val: ArcVarId,
        extent: YieldExtent,
        label: Name,
    ) -> ArcVarId {
        let elem_ty = if self.pool.tag(shape.result_ty) == Tag::List {
            self.pool.list_elem(shape.result_ty)
        } else {
            Idx::INT
        };
        let elem_size = self.compute_elem_size(elem_ty).cast_unsigned();
        let (list_ptr, elem_size_var) = self.allocate_yield_list(elem_ty, extent);
        let setup = self.prepare_range_loop(true);
        let (step, in_bounds) = self.enter_range_header(iter_val, &setup);
        let list_push = self.interner.intern("ori_list_push");
        self.push_range_yield_context(
            label,
            &setup,
            ForYieldContext {
                list_ptr,
                elem_size: elem_size_var,
                list_push_name: list_push,
            },
        );
        self.lower_range_guard(shape.pattern, shape.guard, in_bounds, &setup);
        self.lower_range_yield_body(
            shape.pattern,
            shape.body,
            list_push,
            list_ptr,
            elem_size_var,
            &setup,
        );
        self.loop_ctx_stack.pop();
        self.emit_range_latch(step, &setup);
        self.finish_range_yield_loop(shape.result_ty, list_ptr, elem_ty, elem_size, extent, setup)
    }

    fn prepare_range_loop(&mut self, for_yield: bool) -> RangeLoopSetup {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let latch_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        let exit_prep_block = self.builder.new_block();
        let pre_scope = self.scope.clone();
        let mutable_bindings: Vec<MutableBinding> = pre_scope
            .mutable_bindings()
            .map(|(name, var)| (name, var, self.builder.var_type(var)))
            .collect();

        let i_var = self.builder.add_block_param(header_block, Idx::INT);
        let header_mut_params = mutable_bindings
            .iter()
            .map(|&(name, pre_var, ty)| {
                (
                    name,
                    pre_var,
                    self.builder.add_block_param(header_block, ty),
                )
            })
            .collect();
        let latch_mut_params = mutable_bindings
            .iter()
            .map(|&(name, _, ty)| (name, self.builder.add_block_param(latch_block, ty)))
            .collect();
        let result_param =
            (!for_yield).then(|| self.builder.add_block_param(exit_block, Idx::UNIT));
        let exit_mut_params = mutable_bindings
            .iter()
            .map(|&(name, _, ty)| (name, self.builder.add_block_param(exit_block, ty)))
            .collect();

        RangeLoopSetup {
            header_block,
            body_block,
            latch_block,
            exit_block,
            exit_prep_block,
            pre_scope,
            i_var,
            header_mut_params,
            latch_mut_params,
            exit_mut_params,
            result_param,
        }
    }

    fn enter_range_header(
        &mut self,
        iter_val: ArcVarId,
        setup: &RangeLoopSetup,
    ) -> (ArcVarId, ArcVarId) {
        let start = self.builder.emit_project(Idx::INT, iter_val, 0, None);
        let end = self.builder.emit_project(Idx::INT, iter_val, 1, None);
        let step = self.builder.emit_project(Idx::INT, iter_val, 2, None);
        let step_lit = self.builder.get_literal_int(step);
        let inclusive_lit = self.builder.get_field_literal_int(iter_val, 3);
        if step_lit.is_none() || step_lit == Some(0) {
            self.emit_zero_step_guard(step);
        }

        let mut entry_args = vec![start];
        entry_args.extend(
            setup
                .header_mut_params
                .iter()
                .map(|&(_, pre_var, _)| pre_var),
        );
        self.builder.terminate_jump(setup.header_block, entry_args);
        self.builder.position_at(setup.header_block);
        self.scope = setup.pre_scope.clone();
        for &(name, _, param) in &setup.header_mut_params {
            self.scope.bind_mutable(name, param);
        }
        let in_bounds =
            self.emit_range_bounds(iter_val, setup.i_var, end, step, step_lit, inclusive_lit);
        (step, in_bounds)
    }

    fn emit_range_bounds(
        &mut self,
        iter_val: ArcVarId,
        i_var: ArcVarId,
        end: ArcVarId,
        step: ArcVarId,
        step_lit: Option<i64>,
        inclusive_lit: Option<i64>,
    ) -> ArcVarId {
        match (step_lit, inclusive_lit) {
            (Some(1), Some(0)) => self.emit_range_comparison(ori_ir::BinaryOp::Lt, i_var, end),
            (Some(1), Some(1)) => self.emit_range_comparison(ori_ir::BinaryOp::LtEq, i_var, end),
            (Some(-1), Some(0)) => self.emit_range_comparison(ori_ir::BinaryOp::Gt, i_var, end),
            (Some(-1), Some(1)) => self.emit_range_comparison(ori_ir::BinaryOp::GtEq, i_var, end),
            _ => {
                let inclusive = self.builder.emit_project(Idx::INT, iter_val, 3, None);
                self.emit_general_range_condition(i_var, end, step, inclusive)
            }
        }
    }

    fn emit_range_comparison(
        &mut self,
        op: ori_ir::BinaryOp,
        lhs: ArcVarId,
        rhs: ArcVarId,
    ) -> ArcVarId {
        self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(op),
                args: vec![lhs, rhs],
            },
            None,
        )
    }

    fn push_range_loop_context(&mut self, label: Name, setup: &RangeLoopSetup) {
        let mutable_vars = setup
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| (name, param))
            .collect();
        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block: setup.exit_block,
            continue_block: setup.latch_block,
            mutable_vars,
            abandon_iter: None,
            yield_ctx: None,
        });
    }

    fn push_range_yield_context(
        &mut self,
        label: Name,
        setup: &RangeLoopSetup,
        yield_ctx: ForYieldContext,
    ) {
        let mutable_vars = setup
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| (name, param))
            .collect();
        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block: setup.exit_block,
            continue_block: setup.latch_block,
            mutable_vars,
            abandon_iter: None,
            yield_ctx: Some(yield_ctx),
        });
    }

    fn lower_range_guard(
        &mut self,
        pattern: CanBindingPatternId,
        guard: CanId,
        in_bounds: ArcVarId,
        setup: &RangeLoopSetup,
    ) {
        if !guard.is_valid() {
            self.builder
                .terminate_branch(in_bounds, setup.body_block, setup.exit_prep_block);
            return;
        }
        let guarded_block = self.builder.new_block();
        self.builder
            .terminate_branch(in_bounds, guarded_block, setup.exit_prep_block);
        self.builder.position_at(guarded_block);
        self.bind_for_pattern(pattern, setup.i_var, Idx::INT);
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
        self.builder.terminate_jump(setup.latch_block, skip_args);
    }

    fn lower_range_body(
        &mut self,
        pattern: CanBindingPatternId,
        body: CanId,
        setup: &RangeLoopSetup,
    ) {
        self.builder.position_at(setup.body_block);
        self.bind_for_pattern(pattern, setup.i_var, Idx::INT);
        self.lower_expr(body);
        if !self.builder.is_terminated() {
            let body_args = setup
                .header_mut_params
                .iter()
                .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
                .collect();
            self.builder.terminate_jump(setup.latch_block, body_args);
        }
    }

    fn lower_range_yield_body(
        &mut self,
        pattern: CanBindingPatternId,
        body: CanId,
        list_push: Name,
        list_ptr: ArcVarId,
        elem_size: ArcVarId,
        setup: &RangeLoopSetup,
    ) {
        self.builder.position_at(setup.body_block);
        self.bind_for_pattern(pattern, setup.i_var, Idx::INT);
        let body_val = self.lower_expr(body);
        if self.builder.is_terminated() {
            return;
        }
        self.builder.emit_apply(
            Idx::UNIT,
            list_push,
            vec![list_ptr, body_val, elem_size],
            None,
            None,
        );
        let body_args = setup
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
            .collect();
        self.builder.terminate_jump(setup.latch_block, body_args);
    }

    fn emit_range_latch(&mut self, step: ArcVarId, setup: &RangeLoopSetup) {
        self.builder.position_at(setup.latch_block);
        let next = self.builder.emit_let(
            Idx::INT,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                args: vec![setup.i_var, step],
            },
            None,
        );
        let mut header_args = vec![next];
        header_args.extend(setup.latch_mut_params.iter().map(|&(_, param)| param));
        self.builder.terminate_jump(setup.header_block, header_args);
    }

    fn finish_range_loop(&mut self, setup: RangeLoopSetup) -> ArcVarId {
        self.builder.position_at(setup.exit_prep_block);
        let unit_val = self.emit_unit();
        let mut prep_args = vec![unit_val];
        prep_args.extend(setup.header_mut_params.iter().map(|&(_, _, param)| param));
        self.builder.terminate_jump(setup.exit_block, prep_args);

        self.builder.position_at(setup.exit_block);
        self.scope = setup.pre_scope;
        for &(name, param) in &setup.exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        setup
            .result_param
            .expect("ordinary range loop has a unit result parameter")
    }

    fn finish_range_yield_loop(
        &mut self,
        result_ty: Idx,
        list_ptr: ArcVarId,
        elem_ty: Idx,
        elem_size: u64,
        extent: YieldExtent,
        setup: RangeLoopSetup,
    ) -> ArcVarId {
        self.builder.position_at(setup.exit_prep_block);
        let prep_args = setup
            .header_mut_params
            .iter()
            .map(|&(_, _, param)| param)
            .collect();
        self.builder.terminate_jump(setup.exit_block, prep_args);

        self.builder.position_at(setup.exit_block);
        self.scope = setup.pre_scope;
        for &(name, param) in &setup.exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        let list_take = self.interner.intern("ori_list_take");
        let result = self
            .builder
            .emit_apply(result_ty, list_take, vec![list_ptr], None, None);
        self.builder
            .note_yield_allocation(list_ptr, result, elem_ty, elem_size, extent);
        result
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
            .emit_apply(Idx::UNIT, panic_fn, vec![msg_var], None, None);
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
