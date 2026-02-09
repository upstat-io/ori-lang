//! Control flow lowering — block, let, if/else, loop, for, break,
//! continue, match, and assign.
//!
//! These are the expression variants that create multiple basic blocks
//! in the ARC IR. The key challenge is SSA merge: when mutable variables
//! are reassigned in divergent branches (if/else, match, loop), block
//! parameters serve as phi nodes at the merge point.

use ori_ir::ast::{ExprKind, StmtKind};
use ori_ir::{ArmRange, BindingPatternId, ExprId, Name, Span, StmtRange};
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::expr::{ArcLowerer, LoopContext};
use super::scope::merge_mutable_vars;

impl ArcLowerer<'_> {
    // ── Block ──────────────────────────────────────────────────

    /// Lower `Block { stmts, result }`.
    ///
    /// Creates a child scope for the block body. Statements are lowered
    /// sequentially. The result expression (if present) is the block's value.
    pub(crate) fn lower_block(&mut self, stmts: StmtRange, result: ExprId, _ty: Idx) -> ArcVarId {
        let parent_scope = self.scope.clone();

        let stmt_slice: Vec<_> = self.arena.get_stmt_range(stmts).to_vec();
        for stmt in &stmt_slice {
            if self.builder.is_terminated() {
                break;
            }
            match &stmt.kind {
                StmtKind::Expr(expr_id) => {
                    self.lower_expr(*expr_id);
                }
                StmtKind::Let {
                    pattern,
                    ty: _,
                    init,
                    mutable,
                } => {
                    self.lower_let(*pattern, *init, *mutable);
                }
            }
        }

        let result_var = if result.is_valid() && !self.builder.is_terminated() {
            self.lower_expr(result)
        } else if !self.builder.is_terminated() {
            self.emit_unit()
        } else {
            // Block is terminated (e.g., by break/continue/return) —
            // the result doesn't matter, but we still need a var.
            ArcVarId::new(0)
        };

        self.scope = parent_scope;
        result_var
    }

    // ── Let ────────────────────────────────────────────────────

    /// Lower `Let { pattern, init, mutable }`.
    ///
    /// Evaluates the initializer, then binds the pattern in the current scope.
    /// Returns unit (let bindings are statements, not value-producing).
    pub(crate) fn lower_let(
        &mut self,
        pattern: BindingPatternId,
        init: ExprId,
        mutable: bool,
    ) -> ArcVarId {
        let init_val = self.lower_expr(init);
        let binding = self.arena.get_binding_pattern(pattern);
        self.bind_pattern(binding, init_val, mutable, init);
        self.emit_unit()
    }

    // ── If / Else ──────────────────────────────────────────────

    /// Lower `If { cond, then_branch, else_branch }`.
    ///
    /// Produces 4 blocks: entry (cond), then, else, merge.
    /// Mutable variables that diverge get SSA-merged via block parameters.
    ///
    /// ```text
    /// entry:
    ///   let cond = ...
    ///   branch cond, then_block, else_block
    ///
    /// then_block:
    ///   let then_val = ...
    ///   jump merge_block(then_val, ...mutable_vars)
    ///
    /// else_block:
    ///   let else_val = ...
    ///   jump merge_block(else_val, ...mutable_vars)
    ///
    /// merge_block(result, ...merged_vars):
    ///   ...
    /// ```
    pub(crate) fn lower_if(
        &mut self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
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

        // Collect mutable variable types for merge.
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

        // Add SSA merge parameters.
        let result_param = self.builder.add_block_param(merge_block, ty);
        let rebindings = merge_mutable_vars(
            self.builder,
            merge_block,
            &pre_scope,
            &[then_scope.clone(), else_scope.clone()],
            &mutable_var_types,
        );

        // Terminate then/else with jumps to merge (passing values).
        if !then_terminated {
            self.builder.position_at(then_block);
            let mut jump_args = vec![then_val];
            for (name, _) in &rebindings {
                let var = then_scope.lookup(*name).unwrap_or(then_val);
                jump_args.push(var);
            }
            self.builder.terminate_jump(merge_block, jump_args);
        }

        if !else_terminated {
            self.builder.position_at(else_block);
            let mut jump_args = vec![else_val];
            for (name, _) in &rebindings {
                let var = else_scope.lookup(*name).unwrap_or(else_val);
                jump_args.push(var);
            }
            self.builder.terminate_jump(merge_block, jump_args);
        }

        // Continue at merge block.
        self.builder.position_at(merge_block);
        self.scope = pre_scope;
        for (name, merge_var) in &rebindings {
            self.scope.bind_mutable(*name, *merge_var);
        }

        result_param
    }

    // ── Loop ───────────────────────────────────────────────────

    /// Lower `Loop { body }` — infinite loop with break/continue.
    ///
    /// ```text
    /// header:
    ///   ...mutable vars as block params...
    ///   <body>
    ///   jump header(updated_vars)    // implicit continue
    ///
    /// exit:
    ///   ...result from break...
    /// ```
    pub(crate) fn lower_loop(&mut self, body: ExprId, ty: Idx) -> ArcVarId {
        let header_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Collect mutable variables for the loop header params.
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

        // Jump from entry to header, passing current mutable var values.
        let entry_args: Vec<_> = header_params.iter().map(|(_, var, _)| *var).collect();
        self.builder.terminate_jump(header_block, entry_args);

        // Set up scope for loop body using header params.
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for (name, _, param_var) in &header_params {
            self.scope.bind_mutable(*name, *param_var);
        }

        // Set loop context.
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: header_block,
            mutable_var_types: mutable_var_types.clone(),
        });

        // Lower the body.
        self.lower_expr(body);

        // Implicit continue at end of body.
        if !self.builder.is_terminated() {
            let continue_args: Vec<_> = header_params
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            self.builder.terminate_jump(header_block, continue_args);
        }

        // Restore.
        self.loop_ctx = prev_loop;

        // Exit block — result is unit for infinite loops (break provides value).
        self.builder.position_at(exit_block);
        self.scope = pre_scope;
        // Add a result parameter for break values.
        self.builder.add_block_param(exit_block, ty)
    }

    // ── For ────────────────────────────────────────────────────

    /// Lower `For { binding, iter, guard, body }` — range iteration.
    ///
    /// Desugars to header/body/latch/exit pattern with an induction variable.
    pub(crate) fn lower_for(
        &mut self,
        binding: Name,
        iter: ExprId,
        guard: ExprId,
        body: ExprId,
        _ty: Idx,
    ) -> ArcVarId {
        let iter_val = self.lower_expr(iter);

        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let latch_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Extract range components: project start, end, step from tuple.
        let start = self.builder.emit_project(Idx::INT, iter_val, 0, None);
        let end = self.builder.emit_project(Idx::INT, iter_val, 1, None);

        // Jump to header with initial value.
        self.builder.terminate_jump(header_block, vec![start]);

        // Header: induction variable as block param.
        self.builder.position_at(header_block);
        let i_var = self.builder.add_block_param(header_block, Idx::INT);

        // Bounds check: i < end.
        let in_bounds = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Lt),
                args: vec![i_var, end],
            },
            None,
        );

        // Guard check if present.
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

        // Body.
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

        // Latch: increment and back-edge.
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

        // Exit.
        self.builder.position_at(exit_block);
        self.emit_unit()
    }

    // ── Break / Continue ───────────────────────────────────────

    pub(crate) fn lower_break(&mut self, value: ExprId) -> ArcVarId {
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

        // After break, current block is terminated. Return a dummy.
        self.emit_unit()
    }

    pub(crate) fn lower_continue(&mut self, _value: ExprId) -> ArcVarId {
        if let Some(ref ctx) = self.loop_ctx {
            let continue_block = ctx.continue_block;
            // Collect current mutable var values for the loop header.
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

    // ── Assign ─────────────────────────────────────────────────

    /// Lower `Assign { target, value }` — SSA rebinding for mutable variables.
    pub(crate) fn lower_assign(&mut self, target: ExprId, value: ExprId, span: Span) -> ArcVarId {
        let rhs = self.lower_expr(value);
        let target_expr = self.arena.get_expr(target);

        match &target_expr.kind {
            ExprKind::Ident(name) => {
                if self.scope.is_mutable(*name) {
                    // SSA rebinding: create a fresh var and rebind.
                    let ty = self.expr_type(value);
                    let new_var = self.builder.emit_let(ty, ArcValue::Var(rhs), Some(span));
                    self.scope.bind_mutable(*name, new_var);
                } else {
                    tracing::warn!(
                        name = ?name,
                        "assignment to non-mutable binding in ARC IR"
                    );
                }
            }
            ExprKind::Field { receiver, field: _ } => {
                // Field assignment: emit Apply to setter function.
                let recv = self.lower_expr(*receiver);
                let setter_fn = self.interner.intern("__set_field");
                self.builder
                    .emit_apply(Idx::UNIT, setter_fn, vec![recv, rhs], Some(span));
            }
            ExprKind::Index { receiver, index } => {
                // Index assignment: emit Apply to setter function.
                let recv = self.lower_expr(*receiver);
                let idx_var = self.lower_expr(*index);
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

    // ── Match ──────────────────────────────────────────────────

    /// Lower `Match { scrutinee, arms }` via Maranget decision trees.
    ///
    /// Pipeline: flatten patterns → build matrix → compile tree → emit IR.
    ///
    /// 1. Each arm's `MatchPattern` is flattened to a `FlatPattern`.
    /// 2. The flat patterns form a `PatternMatrix` (one row per arm).
    /// 3. `compile::compile` produces a `DecisionTree`.
    /// 4. `emit::emit_tree` walks the tree and emits `Switch`/`Branch` terminators.
    pub(crate) fn lower_match(
        &mut self,
        scrutinee: ExprId,
        arms: ArmRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let scrut_var = self.lower_expr(scrutinee);
        let scrut_ty = self.expr_type(scrutinee);

        let arm_slice: Vec<_> = self.arena.get_arms(arms).to_vec();
        if arm_slice.is_empty() {
            return self.emit_unit();
        }

        let merge_block = self.builder.new_block();
        let result_param = self.builder.add_block_param(merge_block, ty);

        // Step 1 & 2: Flatten patterns and build the pattern matrix.
        let matrix = self.build_pattern_matrix(&arm_slice, scrut_ty);

        // Step 3: Compile the matrix into a decision tree.
        let tree = crate::decision_tree::compile::compile(matrix, vec![vec![]]);

        // Step 4: Emit ARC IR blocks from the decision tree.
        let arm_bodies: Vec<ExprId> = arm_slice.iter().map(|a| a.body).collect();

        let mut ctx = crate::decision_tree::emit::EmitContext {
            root_scrutinee: scrut_var,
            scrutinee_ty: scrut_ty,
            merge_block,
            arm_bodies,
            span,
        };

        crate::decision_tree::emit::emit_tree(self, &tree, &mut ctx);

        self.builder.position_at(merge_block);
        result_param
    }

    /// Build a `PatternMatrix` from match arms.
    ///
    /// Each arm's `MatchPattern` is flattened into a `FlatPattern` using
    /// type information from the pool. The result is a single-column
    /// matrix (the root scrutinee column); the Maranget algorithm
    /// expands columns as it specializes on constructors.
    fn build_pattern_matrix(
        &self,
        arms: &[ori_ir::ast::patterns::MatchArm],
        scrut_ty: Idx,
    ) -> crate::decision_tree::PatternMatrix {
        arms.iter()
            .enumerate()
            .map(|(i, arm)| {
                let flat = crate::decision_tree::flatten::flatten_pattern(
                    &arm.pattern,
                    self.arena,
                    scrut_ty,
                    self.pool,
                );
                crate::decision_tree::PatternRow {
                    patterns: vec![flat],
                    arm_index: i,
                    guard: arm.guard.map(ori_ir::canon::CanId::from_expr_id),
                }
            })
            .collect()
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::ast::{Expr, ExprKind, Stmt, StmtKind};
    use ori_ir::{BindingPattern, ExprArena, Name, Span, StringInterner};
    use ori_types::Idx;
    use ori_types::Pool;

    use crate::ir::ArcTerminator;

    #[test]
    fn lower_block_with_let() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        // { let x = 1; x + 2 }
        let lit1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(10, 11)));
        let x_name = Name::from_raw(100);
        let pat = arena.alloc_binding_pattern(BindingPattern::Name(x_name));

        let let_stmt = Stmt::new(
            StmtKind::Let {
                pattern: pat,
                ty: ori_ir::ParsedTypeId::INVALID,
                init: lit1,
                mutable: false,
            },
            Span::new(2, 12),
        );
        let stmt_id = arena.alloc_stmt(let_stmt);
        #[allow(clippy::cast_possible_truncation)] // Test code: index always fits u32.
        let stmts = arena.alloc_stmt_range(stmt_id.index() as u32, 1);

        let x_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(x_name), Span::new(14, 15)));
        let lit2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(18, 19)));
        let add = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: ori_ir::BinaryOp::Add,
                left: x_ref,
                right: lit2,
            },
            Span::new(14, 19),
        ));

        let block = arena.alloc_expr(Expr::new(
            ExprKind::Block { stmts, result: add },
            Span::new(0, 20),
        ));

        let max_id = block.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[lit1.index()] = Idx::INT;
        expr_types[x_ref.index()] = Idx::INT;
        expr_types[lit2.index()] = Idx::INT;
        expr_types[add.index()] = Idx::INT;
        expr_types[block.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            block,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty(), "problems: {problems:?}");
        // Should produce: let v0=1, unit, Var(v0), let v2=2, PrimOp(Add), return
        assert!(func.blocks[0].body.len() >= 3);
    }

    #[test]
    fn lower_if_else_produces_four_blocks() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(3, 7)));
        let then_val = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(10, 11)));
        let else_val = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(17, 18)));
        let if_expr = arena.alloc_expr(Expr::new(
            ExprKind::If {
                cond,
                then_branch: then_val,
                else_branch: else_val,
            },
            Span::new(0, 19),
        ));

        let max_id = if_expr.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[cond.index()] = Idx::BOOL;
        expr_types[then_val.index()] = Idx::INT;
        expr_types[else_val.index()] = Idx::INT;
        expr_types[if_expr.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            if_expr,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty());
        // 4 blocks: entry, then, else, merge.
        assert_eq!(func.blocks.len(), 4);

        // Entry terminates with Branch.
        assert!(matches!(
            func.blocks[0].terminator,
            ArcTerminator::Branch { .. }
        ));

        // Merge block has at least 1 param (the result).
        assert!(!func.blocks[3].params.is_empty());
    }

    #[test]
    fn lower_loop_produces_header_and_exit() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        // loop { break 42 }
        let lit42 = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(14, 16)));
        let break_expr = arena.alloc_expr(Expr::new(ExprKind::Break(lit42), Span::new(8, 16)));
        let loop_expr = arena.alloc_expr(Expr::new(
            ExprKind::Loop { body: break_expr },
            Span::new(0, 18),
        ));

        let max_id = loop_expr.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[lit42.index()] = Idx::INT;
        expr_types[break_expr.index()] = Idx::UNIT;
        expr_types[loop_expr.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            loop_expr,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty(), "problems: {problems:?}");
        // Should have: entry, header, exit (at least 3 blocks).
        assert!(func.blocks.len() >= 3);
    }
}
