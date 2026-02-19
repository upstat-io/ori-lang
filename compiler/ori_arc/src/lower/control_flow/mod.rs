//! Control flow lowering — block, let, if/else, loop, for, break,
//! continue, match, and assign.
//!
//! These are the expression variants that create multiple basic blocks
//! in the ARC IR. The key challenge is SSA merge: when mutable variables
//! are reassigned in divergent branches (if/else, match, loop), block
//! parameters serve as phi nodes at the merge point.

use ori_ir::canon::{CanExpr, CanId, CanRange, DecisionTreeId};
use ori_ir::{Name, Span};
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::expr::{ArcLowerer, LoopContext};
use super::scope::merge_mutable_vars;

impl ArcLowerer<'_> {
    // Block

    /// Lower `Block { stmts, result }`.
    ///
    /// Creates a child scope for the block body. Statements are lowered
    /// sequentially. The result expression (if present) is the block's value.
    pub(crate) fn lower_block(&mut self, stmts: CanRange, result: CanId, _ty: Idx) -> ArcVarId {
        let parent_scope = self.scope.clone();

        let stmt_ids: Vec<_> = self.arena.get_expr_list(stmts).to_vec();
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

        self.scope = parent_scope;
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
        mutable: bool,
    ) -> ArcVarId {
        let init_val = self.lower_expr(init);
        let binding = self.arena.get_binding_pattern(pattern);
        self.bind_pattern(binding, init_val, mutable, init);
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
        // After lowering, the current block may differ from `then_block`
        // if the branch contains invoke calls (which terminate the entry
        // block and create continuation blocks). We must jump from wherever
        // lowering left us, not from the original entry block.
        self.builder.position_at(then_block);
        self.scope = pre_scope.clone();
        let then_val = self.lower_expr(then_branch);
        let then_scope = self.scope.clone();
        let then_terminated = self.builder.is_terminated();
        let then_exit = self.builder.current_block();

        // Else branch (same continuation-block reasoning as above).
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
        let mut mutable_var_types = FxHashMap::default();
        let mut header_params = Vec::new();

        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = if (var.index()) < self.builder.var_types.len() {
                self.builder.var_types[var.index()]
            } else {
                Idx::UNIT
            };
            mutable_var_types.insert(name, var_ty);
            let param_var = self.builder.add_block_param(header_block, var_ty);
            header_params.push((name, var, param_var));
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
            mutable_var_types: mutable_var_types.clone(),
        });

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            let continue_args: Vec<_> = header_params
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            self.builder.terminate_jump(header_block, continue_args);
        }

        self.loop_ctx = prev_loop;

        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        self.builder.add_block_param(exit_block, ty)
    }

    // For

    /// Lower `For { binding, iter, guard, body }` — range iteration.
    pub(crate) fn lower_for(
        &mut self,
        binding: Name,
        iter: CanId,
        guard: CanId,
        body: CanId,
        _ty: Idx,
    ) -> ArcVarId {
        let iter_val = self.lower_expr(iter);

        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let latch_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        let start = self.builder.emit_project(Idx::INT, iter_val, 0, None);
        let end = self.builder.emit_project(Idx::INT, iter_val, 1, None);

        self.builder.terminate_jump(header_block, vec![start]);

        self.builder.position_at(header_block);
        let i_var = self.builder.add_block_param(header_block, Idx::INT);

        let in_bounds = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Lt),
                args: vec![i_var, end],
            },
            None,
        );

        if guard.is_valid() {
            let guarded_block = self.builder.new_block();
            self.builder
                .terminate_branch(in_bounds, guarded_block, exit_block);
            self.builder.position_at(guarded_block);
            self.scope.bind(binding, i_var);
            let guard_val = self.lower_expr(guard);
            self.builder
                .terminate_branch(guard_val, body_block, latch_block);
        } else {
            self.builder
                .terminate_branch(in_bounds, body_block, exit_block);
        }

        self.builder.position_at(body_block);
        self.scope.bind(binding, i_var);

        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: latch_block,
            mutable_var_types: FxHashMap::default(),
        });

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            self.builder.terminate_jump(latch_block, vec![]);
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
        self.builder.terminate_jump(header_block, vec![next]);

        self.builder.position_at(exit_block);
        self.emit_unit()
    }

    // Break / Continue

    /// Lower a `break` expression to ARC IR.
    pub(crate) fn lower_break(&mut self, value: CanId) -> ArcVarId {
        let break_val = if value.is_valid() {
            self.lower_expr(value)
        } else {
            self.emit_unit()
        };

        if let Some(ref ctx) = self.loop_ctx {
            let exit_block = ctx.exit_block;
            self.builder.terminate_jump(exit_block, vec![break_val]);
        } else {
            tracing::warn!("break outside of loop in ARC IR lowering");
        }

        self.emit_unit()
    }

    /// Lower a `continue` expression to ARC IR.
    pub(crate) fn lower_continue(&mut self, _value: CanId) -> ArcVarId {
        if let Some(ref ctx) = self.loop_ctx {
            let continue_block = ctx.continue_block;
            let mut args = Vec::new();
            for name in ctx.mutable_var_types.keys() {
                if let Some(var) = self.scope.lookup(*name) {
                    args.push(var);
                }
            }
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
                    let new_var = self.builder.emit_let(ty, ArcValue::Var(rhs), Some(span));
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

        let merge_block = self.builder.new_block();
        let result_param = self.builder.add_block_param(merge_block, ty);

        // O(1) Arc clone instead of deep-cloning the recursive tree structure.
        let tree = self.canon.decision_trees.get_shared(tree_id);

        let mut ctx = crate::decision_tree::emit::EmitContext {
            root_scrutinee: scrut_var,
            merge_block,
            arm_bodies: arm_ids,
            span,
        };

        crate::decision_tree::emit::emit_tree(self, &tree, &mut ctx);

        self.builder.position_at(merge_block);
        result_param
    }
}

// Tests

#[cfg(test)]
mod tests;
