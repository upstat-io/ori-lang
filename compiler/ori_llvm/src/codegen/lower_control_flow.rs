//! Control flow lowering for V2 codegen.
//!
//! Handles if/else, blocks, let bindings, loops, for-loops, break/continue,
//! assignment, and match expressions.

use std::mem;

use ori_ir::{ArmRange, BindingPattern, BindingPatternId, ExprId, ExprKind, StmtKind, StmtRange};
use ori_types::Idx;

use super::expr_lowerer::{ExprLowerer, LoopContext};
use super::scope::ScopeBinding;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // If / else
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::If { cond, then_branch, else_branch }`.
    ///
    /// Produces:
    /// ```text
    /// entry:
    ///   %cond = ...
    ///   cond_br %cond, then_bb, else_bb
    /// then:
    ///   %then_val = ...
    ///   br merge_bb
    /// else:
    ///   %else_val = ...
    ///   br merge_bb
    /// merge:
    ///   %result = phi [%then_val, then], [%else_val, else]
    /// ```
    pub(crate) fn lower_if(
        &mut self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let cond_val = self.lower(cond)?;

        let then_bb = self.builder.append_block(self.current_function, "if.then");
        let else_bb = self.builder.append_block(self.current_function, "if.else");
        let merge_bb = self.builder.append_block(self.current_function, "if.merge");

        self.builder.cond_br(cond_val, then_bb, else_bb);

        // Then branch
        self.builder.position_at_end(then_bb);
        let then_val = self.lower(then_branch);
        let then_exit = self.builder.current_block();
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Else branch
        self.builder.position_at_end(else_bb);
        let else_val = if else_branch.is_valid() {
            self.lower(else_branch)
        } else {
            // No else branch — produces unit
            Some(self.builder.const_i64(0))
        };
        let else_exit = self.builder.current_block();
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Merge
        self.builder.position_at_end(merge_bb);

        match (then_val, else_val, then_exit, else_exit) {
            (Some(tv), Some(ev), Some(tb), Some(eb)) => {
                let result_type = self.expr_type(expr_id);
                let result_llvm_ty = self.resolve_type(result_type);
                self.builder
                    .phi_from_incoming(result_llvm_ty, &[(tv, tb), (ev, eb)], "if.result")
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Block
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Block { stmts, result }`.
    ///
    /// Creates a child scope, evaluates statements, then the result expression.
    /// The child scope is swapped in via `mem::replace` to avoid borrow issues.
    pub(crate) fn lower_block(&mut self, stmts: StmtRange, result: ExprId) -> Option<ValueId> {
        // Create a child scope for this block
        let child = self.scope.child();
        let parent = mem::replace(&mut self.scope, child);

        // Evaluate each statement
        let stmt_slice = self.arena.get_stmt_range(stmts);
        for stmt in stmt_slice {
            match &stmt.kind {
                StmtKind::Expr(expr_id) => {
                    self.lower(*expr_id);
                }
                StmtKind::Let {
                    pattern,
                    init,
                    mutable,
                    ..
                } => {
                    self.lower_let(*pattern, *init, *mutable);
                }
            }
            // Stop processing if block is terminated (e.g., break/continue)
            if self.builder.current_block_terminated() {
                break;
            }
        }

        // Evaluate the result expression
        let result_val = if result.is_valid() && !self.builder.current_block_terminated() {
            self.lower(result)
        } else {
            None
        };

        // Restore parent scope
        self.scope = parent;

        result_val
    }

    // -----------------------------------------------------------------------
    // Let binding
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Let { pattern, init, mutable }`.
    pub(crate) fn lower_let(
        &mut self,
        pattern: BindingPatternId,
        init: ExprId,
        mutable: bool,
    ) -> Option<ValueId> {
        let init_val = self.lower(init)?;
        let binding_pattern = self.arena.get_binding_pattern(pattern);
        self.bind_pattern(binding_pattern, init_val, mutable, init);
        // Let bindings produce unit
        Some(self.builder.const_i64(0))
    }

    /// Bind a pattern to a value, adding entries to the current scope.
    ///
    /// Currently handles `Name` and `Wildcard` patterns. Destructuring
    /// patterns (Tuple, Struct, List) will be added incrementally.
    fn bind_pattern(
        &mut self,
        pattern: &BindingPattern,
        val: ValueId,
        mutable: bool,
        init_id: ExprId,
    ) {
        match pattern {
            BindingPattern::Name(name) => {
                if mutable {
                    let init_type = self.expr_type(init_id);
                    let llvm_ty = self.resolve_type(init_type);
                    let name_str = self.resolve_name(*name).to_owned();
                    let ptr =
                        self.builder
                            .create_entry_alloca(self.current_function, &name_str, llvm_ty);
                    self.builder.store(val, ptr);
                    self.scope.bind_mutable(*name, ptr, llvm_ty);
                } else {
                    self.scope.bind_immutable(*name, val);
                }
            }
            BindingPattern::Wildcard => {
                // Discard — don't bind anything
            }
            BindingPattern::Tuple(elements) => {
                for (i, sub_pattern) in elements.iter().enumerate() {
                    if let Some(elem_val) =
                        self.builder
                            .extract_value(val, i as u32, &format!("tup.{i}"))
                    {
                        self.bind_pattern(sub_pattern, elem_val, mutable, init_id);
                    }
                }
            }
            BindingPattern::Struct { fields } => {
                for (i, (field_name, sub_pattern)) in fields.iter().enumerate() {
                    if let Some(field_val) = self.builder.extract_value(
                        val,
                        i as u32,
                        &format!("field.{}", self.resolve_name(*field_name)),
                    ) {
                        if let Some(sub) = sub_pattern {
                            self.bind_pattern(sub, field_val, mutable, init_id);
                        } else {
                            // Shorthand: `let { x } = val` binds field `x` to name `x`
                            if mutable {
                                let init_type = self.expr_type(init_id);
                                let llvm_ty = self.resolve_type(init_type);
                                let name_str = self.resolve_name(*field_name).to_owned();
                                let ptr = self.builder.create_entry_alloca(
                                    self.current_function,
                                    &name_str,
                                    llvm_ty,
                                );
                                self.builder.store(field_val, ptr);
                                self.scope.bind_mutable(*field_name, ptr, llvm_ty);
                            } else {
                                self.scope.bind_immutable(*field_name, field_val);
                            }
                        }
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                // Basic list destructuring: extract elements by index
                for (i, sub_pattern) in elements.iter().enumerate() {
                    if let Some(elem_val) =
                        self.builder
                            .extract_value(val, i as u32, &format!("list.{i}"))
                    {
                        self.bind_pattern(sub_pattern, elem_val, mutable, init_id);
                    }
                }
                if let Some(_rest_name) = rest {
                    tracing::warn!(
                        "list rest pattern (`...name`) not yet implemented in V2 codegen"
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Loop
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Loop { body }` — infinite loop with break/continue.
    ///
    /// ```text
    /// loop_header:
    ///   ... body ...
    ///   br loop_header     (continue)
    /// loop_exit:
    ///   %result = phi from break values
    /// ```
    pub(crate) fn lower_loop(&mut self, body: ExprId, expr_id: ExprId) -> Option<ValueId> {
        let header_bb = self
            .builder
            .append_block(self.current_function, "loop.header");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "loop.exit");

        // Branch to header
        if !self.builder.current_block_terminated() {
            self.builder.br(header_bb);
        }

        // Save and set loop context
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: header_bb,
            break_values: Vec::new(),
        });

        // Compile loop body
        self.builder.position_at_end(header_bb);
        self.lower(body);

        // Implicit continue at end of body
        if !self.builder.current_block_terminated() {
            self.builder.br(header_bb);
        }

        // Collect break values and restore previous loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Build phi for break values at exit
        self.builder.position_at_end(exit_bb);
        if loop_ctx.break_values.is_empty() {
            // No break with value — loop result is unit
            Some(self.builder.const_i64(0))
        } else {
            let result_type = self.expr_type(expr_id);
            let result_llvm_ty = self.resolve_type(result_type);
            self.builder
                .phi_from_incoming(result_llvm_ty, &loop_ctx.break_values, "loop.result")
        }
    }

    // -----------------------------------------------------------------------
    // For loop
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::For { binding, iter, guard, body, is_yield }`.
    ///
    /// For range iteration:
    /// ```text
    /// entry:
    ///   %iter = ...  (range struct)
    ///   %start = extractvalue %iter, 0
    ///   %end = extractvalue %iter, 1
    ///   %incl = extractvalue %iter, 2
    ///   br header
    /// header:
    ///   %i = phi [%start, entry], [%next, latch]
    ///   %cond = icmp slt %i, %end  (or sle if inclusive)
    ///   cond_br %cond, body, exit
    /// body:
    ///   ... body with binding = %i ...
    ///   br latch
    /// latch:
    ///   %next = add %i, 1
    ///   br header
    /// exit:
    ///   %result = ...
    /// ```
    pub(crate) fn lower_for(
        &mut self,
        binding: ori_ir::Name,
        iter: ExprId,
        guard: ExprId,
        body: ExprId,
        _is_yield: bool,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let iter_val = self.lower(iter)?;
        let iter_type = self.expr_type(iter);
        let type_info = self.type_info.get(iter_type);

        match type_info {
            super::type_info::TypeInfo::Range => {
                self.lower_for_range(binding, iter_val, guard, body, expr_id)
            }
            super::type_info::TypeInfo::List { .. } => {
                self.lower_for_list(binding, iter_val, iter_type, guard, body, expr_id)
            }
            _ => {
                // For other iterable types, fall back to a simple range-like loop
                tracing::warn!(?iter_type, "for-loop over non-range/non-list type");
                None
            }
        }
    }

    /// For-loop over a range.
    fn lower_for_range(
        &mut self,
        binding: ori_ir::Name,
        range_val: ValueId,
        guard: ExprId,
        body: ExprId,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        // Extract range components
        let start = self.builder.extract_value(range_val, 0, "range.start")?;
        let end = self.builder.extract_value(range_val, 1, "range.end")?;
        let inclusive = self.builder.extract_value(range_val, 2, "range.incl")?;

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "for.header");
        let body_bb = self.builder.append_block(self.current_function, "for.body");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "for.latch");
        let exit_bb = self.builder.append_block(self.current_function, "for.exit");

        self.builder.br(header_bb);

        // Header: phi for induction variable + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let i_phi = self.builder.phi(i64_ty, "for.i");
        self.builder.add_phi_incoming(i_phi, &[(start, entry_bb)]);

        // Bounds check: i < end (or i <= end if inclusive)
        let cmp_lt = self.builder.icmp_slt(i_phi, end, "for.lt");
        let cmp_eq = self.builder.icmp_eq(i_phi, end, "for.eq");
        let incl_ok = self.builder.and(inclusive, cmp_eq, "for.incl_ok");
        let in_bounds = self.builder.or(cmp_lt, incl_ok, "for.inbounds");

        // Apply guard if present
        let cond = if guard.is_valid() {
            self.builder.cond_br(in_bounds, body_bb, exit_bb);
            self.builder.position_at_end(body_bb);

            // Bind the loop variable so guard can reference it
            self.scope.bind_immutable(binding, i_phi);
            let guard_val = self.lower(guard)?;

            let guarded_body_bb = self
                .builder
                .append_block(self.current_function, "for.guarded");
            self.builder.cond_br(guard_val, guarded_body_bb, latch_bb);
            self.builder.position_at_end(guarded_body_bb);
            guard_val
        } else {
            self.builder.cond_br(in_bounds, body_bb, exit_bb);
            self.builder.position_at_end(body_bb);

            // Bind the loop variable
            self.scope.bind_immutable(binding, i_phi);
            in_bounds
        };
        let _ = cond;

        // Save/set loop context — continue goes to latch
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: latch_bb,
            break_values: Vec::new(),
        });

        // Body
        self.lower(body);
        if !self.builder.current_block_terminated() {
            self.builder.br(latch_bb);
        }

        // Latch: increment and back-edge
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next = self.builder.add(i_phi, one, "for.next");
        self.builder.add_phi_incoming(i_phi, &[(next, latch_bb)]);
        self.builder.br(header_bb);

        // Restore loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Exit
        self.builder.position_at_end(exit_bb);

        if loop_ctx.break_values.is_empty() {
            Some(self.builder.const_i64(0))
        } else {
            let unit_ty = self.builder.unit_type();
            self.builder
                .phi_from_incoming(unit_ty, &loop_ctx.break_values, "for.result")
        }
    }

    /// For-loop over a list.
    fn lower_for_list(
        &mut self,
        binding: ori_ir::Name,
        list_val: ValueId,
        list_type: Idx,
        guard: ExprId,
        body: ExprId,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        // List = {i64 len, i64 cap, ptr data}
        let len = self.builder.extract_value(list_val, 0, "list.len")?;
        let data_ptr = self.builder.extract_value(list_val, 2, "list.data")?;

        // Get element type from TypeInfo
        let elem_idx = match self.type_info.get(list_type) {
            super::type_info::TypeInfo::List { element } => element,
            _ => Idx::INT,
        };
        let elem_llvm_ty = self.resolve_type(elem_idx);

        let entry_bb = self.builder.current_block()?;
        let header_bb = self
            .builder
            .append_block(self.current_function, "forlist.header");
        let body_bb = self
            .builder
            .append_block(self.current_function, "forlist.body");
        let latch_bb = self
            .builder
            .append_block(self.current_function, "forlist.latch");
        let exit_bb = self
            .builder
            .append_block(self.current_function, "forlist.exit");

        let zero = self.builder.const_i64(0);
        self.builder.br(header_bb);

        // Header: index phi + bounds check
        self.builder.position_at_end(header_bb);
        let i64_ty = self.builder.i64_type();
        let idx_phi = self.builder.phi(i64_ty, "forlist.idx");
        self.builder.add_phi_incoming(idx_phi, &[(zero, entry_bb)]);

        let in_bounds = self.builder.icmp_slt(idx_phi, len, "forlist.inbounds");
        self.builder.cond_br(in_bounds, body_bb, exit_bb);

        // Body: load element, bind, execute body
        self.builder.position_at_end(body_bb);
        let elem_ptr = self
            .builder
            .gep(elem_llvm_ty, data_ptr, &[idx_phi], "forlist.elem_ptr");
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "forlist.elem");

        // Handle guard
        if guard.is_valid() {
            self.scope.bind_immutable(binding, elem_val);
            let guard_val = self.lower(guard);
            if let Some(gv) = guard_val {
                let guarded_bb = self
                    .builder
                    .append_block(self.current_function, "forlist.guarded");
                self.builder.cond_br(gv, guarded_bb, latch_bb);
                self.builder.position_at_end(guarded_bb);
            }
        } else {
            self.scope.bind_immutable(binding, elem_val);
        }

        // Save/set loop context
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block: exit_bb,
            continue_block: latch_bb,
            break_values: Vec::new(),
        });

        self.lower(body);
        if !self.builder.current_block_terminated() {
            self.builder.br(latch_bb);
        }

        // Latch
        self.builder.position_at_end(latch_bb);
        let one = self.builder.const_i64(1);
        let next_idx = self.builder.add(idx_phi, one, "forlist.next");
        self.builder
            .add_phi_incoming(idx_phi, &[(next_idx, latch_bb)]);
        self.builder.br(header_bb);

        // Restore loop context
        let loop_ctx = self.loop_ctx.take().unwrap();
        self.loop_ctx = prev_loop;

        // Exit
        self.builder.position_at_end(exit_bb);
        if loop_ctx.break_values.is_empty() {
            Some(self.builder.const_i64(0))
        } else {
            let unit_ty = self.builder.unit_type();
            self.builder
                .phi_from_incoming(unit_ty, &loop_ctx.break_values, "forlist.result")
        }
    }

    // -----------------------------------------------------------------------
    // Break / Continue
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Break(value)`.
    pub(crate) fn lower_break(&mut self, value: ExprId) -> Option<ValueId> {
        let break_val = if value.is_valid() {
            self.lower(value)
                .unwrap_or_else(|| self.builder.const_i64(0))
        } else {
            self.builder.const_i64(0)
        };

        if let Some(ref mut ctx) = self.loop_ctx {
            let current_bb = self.builder.current_block().unwrap();
            ctx.break_values.push((break_val, current_bb));
            self.builder.br(ctx.exit_block);
        } else {
            tracing::warn!("break outside of loop in codegen");
        }

        None // Break terminates the current block
    }

    /// Lower `ExprKind::Continue(value)`.
    pub(crate) fn lower_continue(&mut self, _value: ExprId) -> Option<ValueId> {
        if let Some(ref ctx) = self.loop_ctx {
            self.builder.br(ctx.continue_block);
        } else {
            tracing::warn!("continue outside of loop in codegen");
        }

        None // Continue terminates the current block
    }

    // -----------------------------------------------------------------------
    // Assignment
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Assign { target, value }`.
    pub(crate) fn lower_assign(&mut self, target: ExprId, value: ExprId) -> Option<ValueId> {
        let rhs = self.lower(value)?;

        let target_expr = self.arena.get_expr(target);
        match &target_expr.kind {
            ExprKind::Ident(name) => {
                if let Some(ScopeBinding::Mutable { ptr, .. }) = self.scope.lookup(*name) {
                    self.builder.store(rhs, ptr);
                } else {
                    tracing::warn!(
                        name = self.resolve_name(*name),
                        "assignment to non-mutable binding"
                    );
                }
            }
            ExprKind::Field { receiver, field } => {
                // Field assignment: receiver.field = value
                // This requires computing the struct pointer and GEP
                tracing::debug!("field assignment lowering");
                let receiver_val = self.lower(*receiver);
                let _ = (receiver_val, field);
                // Full field assignment requires knowing the struct layout
                // to compute the correct GEP index — implemented in Section 10
            }
            ExprKind::Index { receiver, index } => {
                // Index assignment: receiver[index] = value
                tracing::debug!("index assignment lowering");
                let _ = (receiver, index);
                // Full index assignment requires bounds checking + element store
            }
            _ => {
                tracing::warn!("unsupported assignment target");
            }
        }

        // Assignment produces unit
        Some(self.builder.const_i64(0))
    }

    // -----------------------------------------------------------------------
    // Match (sequential if-else stub)
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Match { scrutinee, arms }` — sequential if-else chain.
    ///
    /// This is a stub implementation that handles literal and wildcard
    /// patterns. Section 10 upgrades this to decision trees.
    pub(crate) fn lower_match(
        &mut self,
        scrutinee: ExprId,
        arms: ArmRange,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let scrut_val = self.lower(scrutinee)?;
        let scrut_type = self.expr_type(scrutinee);
        let result_type = self.expr_type(expr_id);
        let result_llvm_ty = self.resolve_type(result_type);

        let arm_slice = self.arena.get_arms(arms);
        if arm_slice.is_empty() {
            return None;
        }

        let merge_bb = self
            .builder
            .append_block(self.current_function, "match.merge");
        let mut incoming: Vec<(ValueId, super::value_id::BlockId)> = Vec::new();

        for (i, arm) in arm_slice.iter().enumerate() {
            let is_last = i == arm_slice.len() - 1;

            // Check if this is a wildcard/catch-all pattern
            let is_wildcard = matches!(
                arm.pattern,
                ori_ir::MatchPattern::Wildcard | ori_ir::MatchPattern::Binding(_)
            );

            if is_wildcard {
                // Wildcard matches everything — bind if it's a Binding pattern
                if let ori_ir::MatchPattern::Binding(name) = &arm.pattern {
                    self.scope.bind_immutable(*name, scrut_val);
                }

                let body_val = self.lower(arm.body);
                let body_bb = self.builder.current_block();
                if let (Some(bv), Some(bb)) = (body_val, body_bb) {
                    if !self.builder.current_block_terminated() {
                        incoming.push((bv, bb));
                        self.builder.br(merge_bb);
                    }
                }
                break;
            }

            // For non-wildcard patterns, create test + next blocks
            let next_bb = if is_last {
                merge_bb
            } else {
                self.builder
                    .append_block(self.current_function, &format!("match.arm{}", i + 1))
            };

            // Compile pattern test (simplified: literal equality)
            let matches = self.compile_pattern_test(&arm.pattern, scrut_val, scrut_type);

            if let Some(test_val) = matches {
                let arm_bb = self
                    .builder
                    .append_block(self.current_function, &format!("match.body{i}"));
                self.builder.cond_br(test_val, arm_bb, next_bb);

                self.builder.position_at_end(arm_bb);
                let body_val = self.lower(arm.body);
                let body_exit = self.builder.current_block();
                if let (Some(bv), Some(bb)) = (body_val, body_exit) {
                    if !self.builder.current_block_terminated() {
                        incoming.push((bv, bb));
                        self.builder.br(merge_bb);
                    }
                }

                if !is_last {
                    self.builder.position_at_end(next_bb);
                }
            } else {
                // Pattern compilation failed — skip this arm
                if !is_last {
                    self.builder.position_at_end(next_bb);
                }
            }
        }

        self.builder.position_at_end(merge_bb);
        if incoming.is_empty() {
            Some(self.builder.const_i64(0))
        } else {
            self.builder
                .phi_from_incoming(result_llvm_ty, &incoming, "match.result")
        }
    }

    /// Compile a simple pattern test (literal equality or variant tag check).
    fn compile_pattern_test(
        &mut self,
        pattern: &ori_ir::MatchPattern,
        scrutinee: ValueId,
        scrut_type: Idx,
    ) -> Option<ValueId> {
        match pattern {
            ori_ir::MatchPattern::Literal(expr_id) => {
                let pat_val = self.lower(*expr_id)?;
                let is_float = scrut_type == Idx::FLOAT;
                if is_float {
                    Some(self.builder.fcmp_oeq(scrutinee, pat_val, "pat.eq"))
                } else {
                    Some(self.builder.icmp_eq(scrutinee, pat_val, "pat.eq"))
                }
            }
            ori_ir::MatchPattern::Wildcard => {
                // Always matches
                Some(self.builder.const_bool(true))
            }
            ori_ir::MatchPattern::Binding(_name) => {
                // Always matches, binding handled by caller
                Some(self.builder.const_bool(true))
            }
            _ => {
                tracing::debug!(?pattern, "complex match pattern — stub implementation");
                None
            }
        }
    }
}
