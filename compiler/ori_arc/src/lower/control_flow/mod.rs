//! Control flow lowering — block, let, if/else, break, continue,
//! match, and assign.
//!
//! These are the expression variants that create multiple basic blocks
//! in the ARC IR. The key challenge is SSA merge: when mutable variables
//! are reassigned in divergent branches (if/else, match, loop), block
//! parameters serve as phi nodes at the merge point.
//!
//! Loop constructs (`loop`, `for`) live in the [`loops`] submodule.

use ori_ir::canon::{CanExpr, CanId, CanRange, DecisionTreeId};
use ori_ir::{Name, Span};
use ori_types::Idx;
use rustc_hash::FxHashMap;

mod for_loops;
mod for_yield;
mod for_yield_option;
mod loops;
mod type_layout;
#[cfg(test)]
pub(crate) use type_layout::pool_type_store_size;

use crate::ir::{ArcValue, ArcVarId};

use super::expr::ArcLowerer;
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
            mutable_var_types.insert(name, self.builder.var_type_or_unit(var));
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

    // Break / Continue

    /// Lower a `break` expression to ARC IR.
    ///
    /// For-do: exit block expects `[break_value, mut_var_0, mut_var_1, ...]`.
    /// For-yield: optionally pushes break value to list, then jumps to exit
    /// with `[coll_param?, mut_var_0, mut_var_1, ...]`.
    pub(crate) fn lower_break(&mut self, value: CanId) -> ArcVarId {
        // Extract for-yield info before mutable borrows (lower_expr needs &mut self).
        let yield_info = self
            .loop_ctx
            .as_ref()
            .and_then(|ctx| ctx.yield_ctx.as_ref())
            .map(|yc| (yc.list_ptr, yc.elem_size, yc.list_push_name, yc.coll_param));

        if let Some((list_ptr, elem_size, push_name, coll_param)) = yield_info {
            // For-yield break: optionally push value, then jump to exit.
            if value.is_valid() {
                let val = self.lower_expr(value);
                self.builder
                    .emit_apply(Idx::UNIT, push_name, vec![list_ptr, val, elem_size], None);
            }
            // Re-borrow loop_ctx for jump args (mutable borrows are done).
            if let Some(ref ctx) = self.loop_ctx {
                let exit_block = ctx.exit_block;
                let mut args: Vec<_> = coll_param.into_iter().collect();
                for &(name, fallback) in &ctx.mutable_vars {
                    args.push(self.scope.lookup(name).unwrap_or(fallback));
                }
                tracing::debug!(
                    exit_bb = exit_block.index(),
                    has_value = value.is_valid(),
                    mutable_args = ctx.mutable_vars.len(),
                    "for-yield break: jump to exit"
                );
                self.builder.terminate_jump(exit_block, args);
            }
        } else {
            // For-do break: send break value + mutable vars to exit.
            let break_val = if value.is_valid() {
                self.lower_expr(value)
            } else {
                self.emit_unit()
            };

            if let Some(ref ctx) = self.loop_ctx {
                let exit_block = ctx.exit_block;
                let mut args = vec![break_val];
                for &(name, fallback) in &ctx.mutable_vars {
                    args.push(self.scope.lookup(name).unwrap_or(fallback));
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
        }

        self.emit_unit()
    }

    /// Lower a `continue` expression to ARC IR.
    ///
    /// For-do: jumps to header with `[mut_var_0, mut_var_1, ...]`.
    /// For-yield: optionally pushes value to list, then jumps to header
    /// with `[coll_param?, mut_var_0, mut_var_1, ...]`.
    pub(crate) fn lower_continue(&mut self, value: CanId) -> ArcVarId {
        // Extract for-yield info before mutable borrows (lower_expr needs &mut self).
        let yield_info = self
            .loop_ctx
            .as_ref()
            .and_then(|ctx| ctx.yield_ctx.as_ref())
            .map(|yc| (yc.list_ptr, yc.elem_size, yc.list_push_name, yc.coll_param));

        if let Some((list_ptr, elem_size, push_name, coll_param)) = yield_info {
            // For-yield continue: optionally push value, then jump to header.
            if value.is_valid() {
                let val = self.lower_expr(value);
                self.builder
                    .emit_apply(Idx::UNIT, push_name, vec![list_ptr, val, elem_size], None);
            }
            // Re-borrow loop_ctx for jump args (mutable borrows are done).
            if let Some(ref ctx) = self.loop_ctx {
                let continue_block = ctx.continue_block;
                let mut args: Vec<_> = coll_param.into_iter().collect();
                for &(name, fallback) in &ctx.mutable_vars {
                    args.push(self.scope.lookup(name).unwrap_or(fallback));
                }
                tracing::debug!(
                    continue_bb = continue_block.index(),
                    has_value = value.is_valid(),
                    mutable_args = ctx.mutable_vars.len(),
                    "for-yield continue: jump to header"
                );
                self.builder.terminate_jump(continue_block, args);
            }
        } else if let Some(ref ctx) = self.loop_ctx {
            // For-do continue: jump to header with mutable vars only.
            let continue_block = ctx.continue_block;
            let args: Vec<_> = ctx
                .mutable_vars
                .iter()
                .map(|&(name, fallback)| self.scope.lookup(name).unwrap_or(fallback))
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
            CanExpr::Field { .. } => {
                self.problems.push(crate::lower::ArcProblem::InternalError {
                    message: "field assignment reached ARC lowering before desugaring".to_string(),
                    span,
                });
            }
            CanExpr::Index { .. } => {
                self.problems.push(crate::lower::ArcProblem::InternalError {
                    message: "index assignment reached ARC lowering before desugaring".to_string(),
                    span,
                });
            }
            _ => {
                self.problems.push(crate::lower::ArcProblem::InternalError {
                    message: "unexpected assignment target in ARC lowering".to_string(),
                    span,
                });
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
            let var_ty = self.builder.var_type_or_unit(var);
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

        let scrut_ty = self.builder.var_type(scrut_var);
        let mut ctx = crate::decision_tree::emit::EmitContext::new(
            scrut_var,
            scrut_ty,
            merge_block,
            arm_ids,
            span,
            pre_scope.clone(),
            mutable_var_names,
        );

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
