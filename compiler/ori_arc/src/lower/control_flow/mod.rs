//! Control flow lowering — block, let, if/else, loop, for, break,
//! continue, match, and assign.
//!
//! These are the expression variants that create multiple basic blocks
//! in the ARC IR. The key challenge is SSA merge: when mutable variables
//! are reassigned in divergent branches (if/else, match, loop), block
//! parameters serve as phi nodes at the merge point.

use ori_ir::canon::{CanBindingPatternId, CanExpr, CanId, CanRange, DecisionTreeId};
use ori_ir::{Name, Span};
use ori_types::Idx;
use rustc_hash::FxHashMap;

mod for_yield;
#[cfg(test)]
pub(crate) use for_yield::pool_type_store_size;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::expr::{ArcLowerer, LoopContext};
use super::scope::merge_mutable_vars;

impl ArcLowerer<'_> {
    // Block

    /// Lower `Block { stmts, result }`.
    ///
    /// Creates a child scope for the block body. Statements are lowered
    /// sequentially. The result expression (if present) is the block's value.
    ///
    /// Local `let` bindings inside the block don't leak to the parent scope,
    /// but mutable variable reassignments (`x = expr`) DO propagate — they
    /// must survive so that loop headers see updated values.
    pub(crate) fn lower_block(&mut self, stmts: CanRange, result: CanId, _ty: Idx) -> ArcVarId {
        let parent_scope = self.scope.clone();
        let stmt_ids: Vec<_> = self.arena.get_expr_list(stmts).to_vec();
        tracing::debug!(
            stmts = stmt_ids.len(),
            has_result = result.is_valid(),
            mutable_count = parent_scope.mutable_bindings().count(),
            "block: enter"
        );

        // Save and reset block_let_names for this block's scope.
        // bind_pattern will populate it with names introduced by `let`.
        let parent_let_names = std::mem::take(&mut self.block_let_names);

        for &stmt_id in &stmt_ids {
            if self.builder.is_terminated() {
                break;
            }
            self.lower_expr(stmt_id);
        }

        let result_var = if result.is_valid() && !self.builder.is_terminated() {
            self.lower_expr(result)
        } else if !self.builder.is_terminated() {
            self.emit_unit()
        } else {
            ArcVarId::new(0)
        };

        // Carry forward mutable var reassignments from the inner scope.
        // Local `let` bindings (shadows) die with the block, but `x = expr`
        // on an outer mutable variable must propagate so loop headers see
        // updates. Skip names that were freshly `let`-bound in this block —
        // those are shadows, not reassignments.
        let inner_scope = self.scope.clone();
        self.scope = parent_scope;
        let mut propagated = 0u32;
        for (name, var) in inner_scope.mutable_bindings() {
            // Skip names that were introduced by `let` in this block — they
            // are shadows of outer variables, not reassignments.
            if self.block_let_names.contains(&name) {
                tracing::trace!(
                    name = self.name_str(name),
                    var = var.raw(),
                    "block: skipping shadow (let-bound in this block)"
                );
                continue;
            }
            if self.scope.is_mutable(name) {
                let old = self.scope.lookup(name);
                if old != Some(var) {
                    tracing::trace!(
                        name = self.name_str(name),
                        old_var = old.map(ArcVarId::raw),
                        new_var = var.raw(),
                        "block: propagating mutable var"
                    );
                    propagated += 1;
                }
                self.scope.bind_mutable(name, var);
            }
        }

        // Restore parent's block_let_names.
        self.block_let_names = parent_let_names;

        tracing::debug!(
            result = result_var.raw(),
            propagated,
            terminated = self.builder.is_terminated(),
            "block: exit"
        );
        result_var
    }

    // Let

    /// Lower `Let { pattern, init, mutable }`.
    ///
    /// Evaluates the initializer, then binds the pattern in the current scope.
    /// Returns unit (let bindings are statements, not value-producing).
    pub(crate) fn lower_let(
        &mut self,
        pattern: ori_ir::canon::CanBindingPatternId,
        init: CanId,
    ) -> ArcVarId {
        let init_val = self.lower_expr(init);
        let binding = self.arena.get_binding_pattern(pattern);
        tracing::trace!(init_var = init_val.raw(), "let: bind pattern");
        self.bind_pattern(binding, init_val, init);
        self.emit_unit()
    }

    // If / Else

    /// Lower `If { cond, then_branch, else_branch }`.
    ///
    /// Produces 4 blocks: entry (cond), then, else, merge.
    /// Mutable variables that diverge get SSA-merged via block parameters.
    pub(crate) fn lower_if(
        &mut self,
        cond: CanId,
        then_branch: CanId,
        else_branch: CanId,
        ty: Idx,
        _span: Span,
    ) -> ArcVarId {
        let cond_var = self.lower_expr(cond);

        let then_block = self.builder.new_block();
        let else_block = self.builder.new_block();
        let merge_block = self.builder.new_block();
        tracing::debug!(
            then_bb = then_block.index(),
            else_bb = else_block.index(),
            merge_bb = merge_block.index(),
            "if: enter"
        );

        self.builder
            .terminate_branch(cond_var, then_block, else_block);

        let pre_scope = self.scope.clone();

        let mut mutable_var_types = FxHashMap::default();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
            mutable_var_types.insert(name, var_ty);
        }

        // Then branch.
        self.builder.position_at(then_block);
        self.scope = pre_scope.clone();
        let then_val = self.lower_expr(then_branch);
        let then_scope = self.scope.clone();
        let then_terminated = self.builder.is_terminated();
        let then_exit = self.builder.current_block();

        // Else branch.
        self.builder.position_at(else_block);
        self.scope = pre_scope.clone();
        let else_val = if else_branch.is_valid() {
            self.lower_expr(else_branch)
        } else {
            self.emit_unit()
        };
        let else_scope = self.scope.clone();
        let else_terminated = self.builder.is_terminated();
        let else_exit = self.builder.current_block();

        // Add SSA merge parameters.
        let result_param = self.builder.add_block_param(merge_block, ty);
        let rebindings = merge_mutable_vars(
            self.builder,
            merge_block,
            &pre_scope,
            &[then_scope.clone(), else_scope.clone()],
            &mutable_var_types,
        );

        tracing::debug!(
            then_terminated,
            else_terminated,
            rebindings = rebindings.len(),
            "if: merge"
        );
        for (name, merge_var) in &rebindings {
            tracing::trace!(
                name = self.name_str(*name),
                merge_var = merge_var.raw(),
                "if: rebinding"
            );
        }

        if !then_terminated {
            self.builder.position_at(then_exit);
            let mut jump_args = vec![then_val];
            for (name, _) in &rebindings {
                let var = then_scope.lookup(*name).unwrap_or(then_val);
                jump_args.push(var);
            }
            self.builder.terminate_jump(merge_block, jump_args);
        }

        if !else_terminated {
            self.builder.position_at(else_exit);
            let mut jump_args = vec![else_val];
            for (name, _) in &rebindings {
                let var = else_scope.lookup(*name).unwrap_or(else_val);
                jump_args.push(var);
            }
            self.builder.terminate_jump(merge_block, jump_args);
        }

        self.builder.position_at(merge_block);
        self.scope = pre_scope;
        for (name, merge_var) in &rebindings {
            self.scope.bind_mutable(*name, *merge_var);
        }

        result_param
    }

    // Loop

    /// Lower `Loop { body }` — infinite loop with break/continue.
    pub(crate) fn lower_loop(&mut self, body: CanId, ty: Idx) -> ArcVarId {
        let header_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        let pre_scope = self.scope.clone();
        let mut mutable_var_names = Vec::new();
        let mut header_params = Vec::new();

        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
            mutable_var_names.push(name);
            let param_var = self.builder.add_block_param(header_block, var_ty);
            header_params.push((name, var, param_var));
        }

        tracing::debug!(
            header_bb = header_block.index(),
            exit_bb = exit_block.index(),
            mutable_vars = header_params.len(),
            "loop: enter"
        );
        for &(name, pre_var, param_var) in &header_params {
            tracing::trace!(
                name = self.name_str(name),
                entry_var = pre_var.raw(),
                header_param = param_var.raw(),
                "loop: header param"
            );
        }

        let entry_args: Vec<_> = header_params.iter().map(|(_, var, _)| *var).collect();
        self.builder.terminate_jump(header_block, entry_args);

        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for (name, _, param_var) in &header_params {
            self.scope.bind_mutable(*name, *param_var);
        }

        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: header_block,
            mutable_vars: mutable_var_names,
        });

        self.lower_expr(body);

        if self.builder.is_terminated() {
            tracing::debug!("loop: body terminated (all paths break/continue)");
        } else {
            let continue_args: Vec<_> = header_params
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            for (i, &(name, _, param)) in header_params.iter().enumerate() {
                tracing::trace!(
                    name = self.name_str(name),
                    header_param = param.raw(),
                    updated_var = continue_args[i].raw(),
                    changed = (param.raw() != continue_args[i].raw()),
                    "loop: fall-through arg"
                );
            }
            self.builder.terminate_jump(header_block, continue_args);
        }

        self.loop_ctx = prev_loop;

        // Exit block: result value + mutable var params so post-loop code
        // sees the final values of mutable variables (not pre-loop values).
        self.builder.position_at(exit_block);
        let result_param = self.builder.add_block_param(exit_block, ty);
        self.scope = pre_scope;
        for &(name, pre_var, _) in &header_params {
            let var_ty = if pre_var.index() < self.builder.var_types.len() {
                self.builder.var_types[pre_var.index()]
            } else {
                Idx::UNIT
            };
            let exit_param = self.builder.add_block_param(exit_block, var_ty);
            tracing::trace!(
                name = self.name_str(name),
                exit_param = exit_param.raw(),
                "loop: exit param"
            );
            self.scope.bind_mutable(name, exit_param);
        }
        tracing::debug!(
            result = result_param.raw(),
            exit_bb = exit_block.index(),
            "loop: exit"
        );
        result_param
    }

    // For

    /// Lower `For { binding, iter, guard, body }` — range iteration.
    ///
    /// Produces 4+ blocks: header, body, latch, exit (plus guard blocks).
    /// Mutable variables from the enclosing scope flow through the loop
    /// as block parameters on header and latch blocks, ensuring SSA
    /// dominance at the exit point.
    ///
    /// Block param layout:
    /// - header: `[i_var, mut0, mut1, ...]`
    /// - latch:  `[mut0, mut1, ...]` (`i_var` from header dominates latch)
    pub(crate) fn lower_for(
        &mut self,
        pattern: CanBindingPatternId,
        iter: CanId,
        guard: CanId,
        body: CanId,
        ty: Idx,
        is_yield: bool,
    ) -> ArcVarId {
        let iter_val = self.lower_expr(iter);
        let iter_ty = self.expr_type(iter);
        let tag = self.pool.tag(iter_ty);
        tracing::debug!(
            pattern = ?pattern,
            ?tag,
            is_yield,
            has_guard = guard.is_valid(),
            "for: enter"
        );

        if is_yield {
            return self.lower_for_yield(pattern, iter_val, iter_ty, tag, guard, body, ty);
        }

        if tag == ori_types::Tag::Range {
            // Direct counter-based loop for ranges.
            self.lower_for_range(pattern, iter_val, iter_ty, guard, body)
        } else if tag == ori_types::Tag::Option {
            // Direct 0-or-1 iteration — cheaper than allocating an iterator.
            let elem_ty = self.pool.option_inner(iter_ty);
            self.lower_for_option(pattern, iter_val, elem_ty, guard, body)
        } else if tag.is_iterator() {
            // Already an iterator — use __iter_next loop.
            let elem_ty = self.pool.iterator_elem(iter_ty);
            self.lower_for_iterator(pattern, iter_val, elem_ty, guard, body)
        } else {
            // List, Map, Set, Str, etc. — convert to iterator via .iter(),
            // then use the iterator-based loop.
            let elem_ty = self.extract_iterable_elem_type(tag, iter_ty);
            let iter_name = self.interner.intern("iter");
            // Use INT for the iterator handle — it's an opaque pointer with no
            // RC semantics (cleanup is via ori_iter_drop, not RC dec).
            let iter_result = self
                .builder
                .emit_apply(Idx::INT, iter_name, vec![iter_val], None);
            self.lower_for_iterator(pattern, iter_result, elem_ty, guard, body)
        }
    }

    /// Extract the element type from an iterable's type tag.
    ///
    /// Used for types that go through `.iter()` → `__iter_next`.
    /// Option and Range are handled by dedicated lowering paths.
    fn extract_iterable_elem_type(&self, tag: ori_types::Tag, iter_ty: Idx) -> Idx {
        match tag {
            ori_types::Tag::List => self.pool.list_elem(iter_ty),
            ori_types::Tag::Set => self.pool.set_elem(iter_ty),
            ori_types::Tag::Str => Idx::CHAR,
            ori_types::Tag::Map => {
                // Map iteration yields (key, value) tuples.
                // The type checker already created this tuple type during
                // `infer_for` — look it up in the pool without mutating.
                let key_ty = self.pool.map_key(iter_ty);
                let val_ty = self.pool.map_value(iter_ty);
                self.pool.find_tuple(&[key_ty, val_ty]).unwrap_or(Idx::INT)
            }
            _ => Idx::INT,
        }
    }

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
    fn lower_for_iterator(
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

        // Collect mutable bindings for SSA merge.
        let pre_scope = self.scope.clone();
        let mut mutable_var_names = Vec::new();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
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

        let iter_next_name = self.interner.intern("__iter_next");
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
                .terminate_branch(has_more, guarded_block, exit_block);

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
                .terminate_branch(has_more, body_block, exit_block);
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

        // Exit: restore scope with mutable vars from header params.
        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        for &(name, _, param_var) in &header_mut_params {
            self.scope.bind_mutable(name, param_var);
        }
        self.emit_unit()
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
    fn lower_for_option(
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
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
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
    fn lower_for_range(
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

        // Collect mutable bindings for SSA merge through the loop.
        let pre_scope = self.scope.clone();
        let mut mutable_var_names = Vec::new();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();

        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
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
                .terminate_branch(in_bounds, guarded_block, exit_block);

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
                .terminate_branch(in_bounds, body_block, exit_block);
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

        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        for &(name, _, param_var) in &header_mut_params {
            self.scope.bind_mutable(name, param_var);
        }
        self.emit_unit()
    }

    // Break / Continue

    /// Lower a `break` expression to ARC IR.
    ///
    /// The exit block expects: `[break_value, mut_var_0, mut_var_1, ...]`
    /// in the same order as the header params (matching `LoopContext::mutable_vars`).
    pub(crate) fn lower_break(&mut self, value: CanId) -> ArcVarId {
        let break_val = if value.is_valid() {
            self.lower_expr(value)
        } else {
            self.emit_unit()
        };

        if let Some(ref ctx) = self.loop_ctx {
            let exit_block = ctx.exit_block;
            let mut args = vec![break_val];
            for name in &ctx.mutable_vars {
                if let Some(var) = self.scope.lookup(*name) {
                    args.push(var);
                }
            }
            tracing::debug!(
                exit_bb = exit_block.index(),
                break_val = break_val.raw(),
                mutable_args = args.len() - 1,
                "break: jump to exit"
            );
            self.builder.terminate_jump(exit_block, args);
        } else {
            tracing::warn!("break outside of loop in ARC IR lowering");
        }

        self.emit_unit()
    }

    /// Lower a `continue` expression to ARC IR.
    pub(crate) fn lower_continue(&mut self, _value: CanId) -> ArcVarId {
        if let Some(ref ctx) = self.loop_ctx {
            let continue_block = ctx.continue_block;
            let args: Vec<_> = ctx
                .mutable_vars
                .iter()
                .filter_map(|name| self.scope.lookup(*name))
                .collect();
            tracing::debug!(
                continue_bb = continue_block.index(),
                mutable_args = args.len(),
                "continue: jump to header"
            );
            self.builder.terminate_jump(continue_block, args);
        } else {
            tracing::warn!("continue outside of loop in ARC IR lowering");
        }

        self.emit_unit()
    }

    // Assign

    /// Lower `Assign { target, value }` — SSA rebinding for mutable variables.
    pub(crate) fn lower_assign(&mut self, target: CanId, value: CanId, span: Span) -> ArcVarId {
        let rhs = self.lower_expr(value);
        let target_kind = *self.arena.kind(target);

        match target_kind {
            CanExpr::Ident(name) => {
                if self.scope.is_mutable(name) {
                    let ty = self.expr_type(value);
                    let old_var = self.scope.lookup(name);
                    let new_var = self.builder.emit_let(ty, ArcValue::Var(rhs), Some(span));
                    tracing::trace!(
                        name = self.name_str(name),
                        old_var = old_var.map(ArcVarId::raw),
                        new_var = new_var.raw(),
                        "assign: rebind mutable"
                    );
                    self.scope.bind_mutable(name, new_var);
                } else {
                    tracing::warn!(
                        name = ?name,
                        "assignment to non-mutable binding in ARC IR"
                    );
                }
            }
            CanExpr::Field { receiver, field: _ } => {
                let recv = self.lower_expr(receiver);
                let setter_fn = self.interner.intern("__set_field");
                self.builder
                    .emit_apply(Idx::UNIT, setter_fn, vec![recv, rhs], Some(span));
            }
            CanExpr::Index { receiver, index } => {
                let recv = self.lower_expr(receiver);
                let idx_var = self.lower_expr(index);
                let setter_fn = self.interner.intern("__set_index");
                self.builder
                    .emit_apply(Idx::UNIT, setter_fn, vec![recv, idx_var, rhs], Some(span));
            }
            _ => {
                tracing::warn!("unsupported assignment target in ARC IR");
            }
        }

        self.emit_unit()
    }

    // Match

    /// Lower `Match { scrutinee, decision_tree, arms }` via pre-compiled decision tree.
    ///
    /// The canonicalization pass already compiled the pattern matrix into a
    /// `DecisionTree`. We read it from `CanonResult.decision_trees` and
    /// walk it to emit ARC IR blocks.
    pub(crate) fn lower_match(
        &mut self,
        scrutinee: CanId,
        tree_id: DecisionTreeId,
        arms: CanRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let scrut_var = self.lower_expr(scrutinee);

        let arm_ids: Vec<_> = self.arena.get_expr_list(arms).to_vec();
        if arm_ids.is_empty() {
            return self.emit_unit();
        }

        tracing::debug!(
            scrutinee = scrut_var.raw(),
            arms = arm_ids.len(),
            "match: enter"
        );

        let merge_block = self.builder.new_block();
        let result_param = self.builder.add_block_param(merge_block, ty);

        // Save pre-match scope and add merge block params for mutable
        // variables. Each arm resets to this scope before lowering, and
        // passes its final mutable variable values as jump arguments —
        // same SSA merge pattern that `lower_if` uses.
        let pre_scope = self.scope.clone();
        let mut mutable_var_merge: Vec<(Name, ArcVarId)> = Vec::new();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
            let merge_var = self.builder.add_block_param(merge_block, var_ty);
            mutable_var_merge.push((name, merge_var));
        }
        let mutable_var_names: Vec<Name> = mutable_var_merge.iter().map(|(n, _)| *n).collect();

        tracing::debug!(
            mutable_vars = mutable_var_names.len(),
            "match: mutable var merge setup"
        );

        // O(1) Arc clone instead of deep-cloning the recursive tree structure.
        let tree = self.canon.decision_trees.get_shared(tree_id);

        let mut ctx = crate::decision_tree::emit::EmitContext {
            root_scrutinee: scrut_var,
            merge_block,
            arm_bodies: arm_ids,
            span,
            pre_scope: pre_scope.clone(),
            mutable_var_names,
        };

        crate::decision_tree::emit::emit_tree(self, &tree, &mut ctx);

        // Restore pre-match scope and rebind mutable variables from
        // merge block params (SSA phi outputs).
        self.builder.position_at(merge_block);
        self.scope = pre_scope;
        for (name, merge_var) in &mutable_var_merge {
            self.scope.bind_mutable(*name, *merge_var);
        }

        result_param
    }
}

// Tests

#[cfg(test)]
mod tests;
