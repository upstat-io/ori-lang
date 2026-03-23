//! For-yield Option lowering — `for x in option yield body`.
//!
//! Produces a 0-or-1 element list. Extracted from `for_yield.rs` to stay
//! under the 500-line file limit.

use ori_ir::canon::CanBindingPatternId;
use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::super::expr::{ArcLowerer, ForYieldContext, LoopContext};

impl ArcLowerer<'_> {
    /// Lower `for x in <option> yield body` — 0-or-1 element list.
    ///
    /// Checks the Option tag: Some → evaluate body, push to list. None → empty list.
    ///
    /// ```text
    /// entry:     list_ptr = ori_list_new(8, elem_size)
    ///            tag = project(option, 0)
    ///            branch(tag == 0, some, none)
    /// some:      elem = project(option, 1)
    ///            [guard check]
    ///            body_val = lower(body)
    ///            ori_list_push(list_ptr, body_val, elem_size)
    ///            jump → exit(mut0', mut1', ...)
    /// none:      jump → exit(mut0, mut1, ...)
    /// exit:      result = ori_list_take(list_ptr)
    /// ```
    #[expect(
        clippy::too_many_lines,
        reason = "option for-yield lowering with mutable-var SSA merge and LoopContext setup is inherently sequential"
    )]
    pub(super) fn lower_for_yield_option(
        &mut self,
        pattern: CanBindingPatternId,
        option_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
        result_ty: Idx,
    ) -> ArcVarId {
        let some_block = self.builder.new_block();
        let none_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Collect outer mutable bindings for SSA merge through break/continue,
        // matching the iterator path in `lower_for_yield_iterator()`.
        let pre_scope = self.scope.clone();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mut_info.push((name, var, var_ty));
        }

        // Determine the body result element type from the list result type.
        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            elem_ty
        };

        tracing::debug!(
            pattern = ?pattern,
            exit_bb = exit_block.index(),
            mutable_vars = mut_info.len(),
            has_guard = guard.is_valid(),
            "for_yield_option: enter"
        );

        // Allocate growable list: ori_list_new(initial_cap=8, elem_size).
        let list_new = self.interner.intern("ori_list_new");
        let eight = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(8)), None);
        let body_elem_size = self.compute_elem_size(body_elem_ty);
        let elem_size_var = self.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(body_elem_size)),
            None,
        );
        let list_ptr =
            self.builder
                .emit_apply(Idx::INT, list_new, vec![eight, elem_size_var], None);

        // Exit block params: mutable vars.
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

        // None path: empty list — jump to exit with unmodified mutable vars.
        self.builder.position_at(none_block);
        let none_args: Vec<_> = mut_info.iter().map(|&(_, pre_var, _)| pre_var).collect();
        self.builder.terminate_jump(exit_block, none_args);

        // Some path: extract element, optionally check guard, evaluate body.
        self.builder.position_at(some_block);
        let elem = self.builder.emit_project(elem_ty, option_val, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        if guard.is_valid() {
            let body_block = self.builder.new_block();
            let guard_val = self.lower_expr(guard);

            // Guard skip → jump to exit (element filtered out, list stays empty).
            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            self.builder.position_at(guard_skip);
            // Guard doesn't modify mutable vars — use pre-captured values
            // (same pattern as none_block above, avoids fragile scope lookup).
            let skip_args: Vec<_> = mut_info.iter().map(|&(_, pre_var, _)| pre_var).collect();
            self.builder.terminate_jump(exit_block, skip_args);

            self.builder.position_at(body_block);
        }

        // Intern list_push before setting up LoopContext (break/continue need it).
        let list_push = self.interner.intern("ori_list_push");

        // Set up LoopContext so break/continue work inside the yield body.
        // Option is not a loop — continue_block = exit_block (both break and
        // continue exit immediately since there's only one element to visit).
        let mutable_var_entries: Vec<_> = mut_info
            .iter()
            .map(|&(name, pre_var, _)| (name, pre_var))
            .collect();
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: exit_block,
            mutable_vars: mutable_var_entries,
            yield_ctx: Some(ForYieldContext {
                list_ptr,
                elem_size: elem_size_var,
                list_push_name: list_push,
            }),
        });

        let body_val = self.lower_expr(body);

        if !self.builder.is_terminated() {
            // Normal body completion: push result and jump to exit.
            self.builder.emit_apply(
                Idx::UNIT,
                list_push,
                vec![list_ptr, body_val, elem_size_var],
                None,
            );

            let exit_args: Vec<_> = mut_info
                .iter()
                .map(|&(name, pre_var, _)| {
                    // Body may have modified mutable vars — look up current value.
                    // Fall back to pre_var if scope was restructured (should not happen).
                    self.scope.lookup(name).unwrap_or(pre_var)
                })
                .collect();
            self.builder.terminate_jump(exit_block, exit_args);
        }

        self.loop_ctx = prev_loop;

        // Exit: restore scope with final mutable var values, extract result.
        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }

        let list_take = self.interner.intern("ori_list_take");
        self.builder
            .emit_apply(result_ty, list_take, vec![list_ptr], None)
    }
}
