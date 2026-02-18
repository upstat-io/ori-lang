//! Control flow lowering for V2 codegen.
//!
//! Handles if/else, blocks, let bindings, loops, break/continue,
//! assignment, and match expressions.
//!
//! For-loop lowering lives in `lower_for_loop.rs`.

use std::mem;

use ori_ir::canon::{
    CanBindingPattern, CanBindingPatternId, CanExpr, CanId, CanRange, DecisionTreeId,
};
use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::aot::debug::DebugLevel;

use super::expr_lowerer::{ExprLowerer, LoopContext};
use super::scope::ScopeBinding;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // If / else
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::If { cond, then_branch, else_branch }`.
    pub(crate) fn lower_if(
        &mut self,
        cond: CanId,
        then_branch: CanId,
        else_branch: CanId,
        expr_id: CanId,
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

    /// Lower `CanExpr::Block { stmts, result }`.
    ///
    /// In canonical IR, blocks contain only expression statements (no
    /// `StmtKind` — let bindings are `CanExpr::Let` nodes within the list).
    pub(crate) fn lower_block(&mut self, stmts: CanRange, result: CanId) -> Option<ValueId> {
        // Create a child scope for this block
        let child = self.scope.child();
        let parent = mem::replace(&mut self.scope, child);

        // Evaluate each statement expression
        let stmt_ids = self.canon.arena.get_expr_list(stmts);
        for &stmt_id in stmt_ids {
            self.lower(stmt_id);
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

    /// Lower `CanExpr::Let { pattern, init, mutable }`.
    pub(crate) fn lower_let(
        &mut self,
        pattern: CanBindingPatternId,
        init: CanId,
        mutable: bool,
    ) -> Option<ValueId> {
        let init_val = self.lower(init)?;
        let binding_pattern = self.canon.arena.get_binding_pattern(pattern);
        self.bind_pattern(binding_pattern, init_val, mutable, init);
        // Let bindings produce unit
        Some(self.builder.const_i64(0))
    }

    /// Emit debug info for a mutable binding (alloca-backed).
    fn emit_debug_mutable(&self, name_str: &str, ptr: ValueId, init_type: Idx, init_id: CanId) {
        if let Some(dc) = self.debug_context {
            if dc.level() == DebugLevel::Full {
                let init_span = self.canon.arena.span(init_id);
                if init_span != Span::DUMMY {
                    if let Some(di_ty) = dc.resolve_debug_type(init_type, self.pool) {
                        let raw = self.builder.raw_value(ptr);
                        if let (true, Some(cur_bb)) =
                            (raw.is_pointer_value(), self.builder.current_block())
                        {
                            let block = self.builder.raw_block(cur_bb);
                            dc.emit_declare_for_alloca(
                                raw.into_pointer_value(),
                                name_str,
                                di_ty,
                                init_span.start,
                                block,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Emit debug info for an immutable binding (SSA value).
    fn emit_debug_immutable(&self, name: Name, val: ValueId, init_id: CanId) {
        if let Some(dc) = self.debug_context {
            if dc.level() == DebugLevel::Full {
                let init_type = self.expr_type(init_id);
                let init_span = self.canon.arena.span(init_id);
                if init_span != Span::DUMMY {
                    if let Some(di_ty) = dc.resolve_debug_type(init_type, self.pool) {
                        let raw_val = self.builder.raw_value(val);
                        let block = self
                            .builder
                            .raw_block(self.builder.current_block().unwrap());
                        let name_str = self.resolve_name(name);
                        dc.emit_value_for_binding_at_end(
                            raw_val,
                            name_str,
                            di_ty,
                            init_span.start,
                            block,
                        );
                    }
                }
            }
        }
    }

    /// Bind a canonical binding pattern to a value, adding entries to scope.
    #[expect(
        clippy::only_used_in_recursion,
        reason = "mutable is forwarded to sub-patterns; will be consumed directly once list rest patterns are implemented"
    )]
    fn bind_pattern(
        &mut self,
        pattern: &CanBindingPattern,
        val: ValueId,
        mutable: bool,
        init_id: CanId,
    ) {
        match pattern {
            CanBindingPattern::Name {
                name,
                mutable: pat_mutable,
            } => {
                // Per-binding mutability: use the flag from the pattern itself
                // to support `let ($x, y) = ...` with mixed mutability.
                if *pat_mutable {
                    let init_type = self.expr_type(init_id);
                    let llvm_ty = self.resolve_type(init_type);
                    let name_str = self.resolve_name(*name).to_owned();
                    let ptr =
                        self.builder
                            .create_entry_alloca(self.current_function, &name_str, llvm_ty);
                    self.builder.store(val, ptr);
                    self.scope.bind_mutable(*name, ptr, llvm_ty);
                    self.emit_debug_mutable(&name_str, ptr, init_type, init_id);
                } else {
                    self.scope.bind_immutable(*name, val);
                    self.emit_debug_immutable(*name, val, init_id);
                }
            }
            CanBindingPattern::Wildcard => {
                // Discard — don't bind anything
            }
            CanBindingPattern::Tuple(elements) => {
                let elem_ids: Vec<_> = self
                    .canon
                    .arena
                    .get_binding_pattern_list(*elements)
                    .to_vec();
                for (i, sub_pat_id) in elem_ids.iter().enumerate() {
                    let sub_pattern = self.canon.arena.get_binding_pattern(*sub_pat_id);
                    if let Some(elem_val) =
                        self.builder
                            .extract_value(val, i as u32, &format!("tup.{i}"))
                    {
                        self.bind_pattern(sub_pattern, elem_val, mutable, init_id);
                    }
                }
            }
            CanBindingPattern::Struct { fields } => {
                let field_bindings: Vec<_> = self.canon.arena.get_field_bindings(*fields).to_vec();
                for (i, fb) in field_bindings.iter().enumerate() {
                    if let Some(field_val) = self.builder.extract_value(
                        val,
                        i as u32,
                        &format!("field.{}", self.resolve_name(fb.name)),
                    ) {
                        let sub_pattern = self.canon.arena.get_binding_pattern(fb.pattern);
                        self.bind_pattern(sub_pattern, field_val, mutable, init_id);
                    }
                }
            }
            CanBindingPattern::List { elements, rest } => {
                let elem_ids: Vec<_> = self
                    .canon
                    .arena
                    .get_binding_pattern_list(*elements)
                    .to_vec();
                for (i, sub_pat_id) in elem_ids.iter().enumerate() {
                    let sub_pattern = self.canon.arena.get_binding_pattern(*sub_pat_id);
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

    /// Lower `CanExpr::Loop { body }` — infinite loop with break/continue.
    pub(crate) fn lower_loop(&mut self, body: CanId, expr_id: CanId) -> Option<ValueId> {
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
    // Break / Continue
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Break(value)`.
    pub(crate) fn lower_break(&mut self, value: CanId) -> Option<ValueId> {
        let break_val = if value.is_valid() {
            self.lower(value)
                .unwrap_or_else(|| self.builder.const_i64(0))
        } else {
            self.builder.const_i64(0)
        };

        if let Some(ref mut ctx) = self.loop_ctx {
            if let Some(current_bb) = self.builder.current_block() {
                ctx.break_values.push((break_val, current_bb));
                self.builder.br(ctx.exit_block);
            } else {
                tracing::error!("break: no current block in builder");
                self.builder.record_codegen_error();
            }
        } else {
            tracing::warn!("break outside of loop in codegen");
            self.builder.record_codegen_error();
        }

        None // Break terminates the current block
    }

    /// Lower `CanExpr::Continue(value)`.
    pub(crate) fn lower_continue(&mut self, _value: CanId) -> Option<ValueId> {
        if let Some(ref ctx) = self.loop_ctx {
            self.builder.br(ctx.continue_block);
        } else {
            tracing::warn!("continue outside of loop in codegen");
            self.builder.record_codegen_error();
        }

        None // Continue terminates the current block
    }

    // -----------------------------------------------------------------------
    // Assignment
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Assign { target, value }`.
    pub(crate) fn lower_assign(&mut self, target: CanId, value: CanId) -> Option<ValueId> {
        let rhs = self.lower(value)?;

        let target_kind = *self.canon.arena.kind(target);
        match target_kind {
            CanExpr::Ident(name) => {
                if let Some(ScopeBinding::Mutable { ptr, .. }) = self.scope.lookup(name) {
                    self.builder.store(rhs, ptr);
                } else {
                    tracing::warn!(
                        name = self.resolve_name(name),
                        "assignment to non-mutable binding"
                    );
                }
            }
            CanExpr::Field { receiver, field } => {
                tracing::debug!("field assignment lowering");
                let receiver_val = self.lower(receiver);
                let _ = (receiver_val, field);
                // Full field assignment requires knowing the struct layout
            }
            CanExpr::Index { receiver, index } => {
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
    // Match (sequential if-else chain — stub for pre-decision-tree code)
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Match { scrutinee, decision_tree, arms }`.
    ///
    /// Currently uses a sequential if-else chain over the arm bodies.
    /// The decision tree is available for future upgrade to proper
    /// switch-based emission.
    pub(crate) fn lower_match(
        &mut self,
        scrutinee: CanId,
        _decision_tree: DecisionTreeId,
        arms: CanRange,
        expr_id: CanId,
    ) -> Option<ValueId> {
        let scrut_val = self.lower(scrutinee)?;
        let scrut_type = self.expr_type(scrutinee);
        let result_type = self.expr_type(expr_id);
        let result_llvm_ty = self.resolve_type(result_type);

        let arm_ids = self.canon.arena.get_expr_list(arms);
        if arm_ids.is_empty() {
            return None;
        }

        let merge_bb = self
            .builder
            .append_block(self.current_function, "match.merge");
        let mut incoming: Vec<(ValueId, super::value_id::BlockId)> = Vec::new();

        // For now, treat each arm as a body expression. The first arm that
        // matches wins. Without decision tree emission, we do literal/wildcard
        // matching on the arm body's pattern context.
        // TODO: Use decision_tree for proper switch-based emission.
        let last_idx = arm_ids.len() - 1;
        for (i, &arm_body) in arm_ids.iter().enumerate() {
            let is_last = i == last_idx;

            if is_last {
                // Last arm is the catch-all — just lower the body
                let body_val = self.lower(arm_body);
                let body_bb = self.builder.current_block();
                if let (Some(bv), Some(bb)) = (body_val, body_bb) {
                    if !self.builder.current_block_terminated() {
                        incoming.push((bv, bb));
                        self.builder.br(merge_bb);
                    }
                }
                break;
            }

            // Non-last arms need pattern tests. Since canonical IR doesn't
            // embed patterns in arm bodies, we use the decision tree for
            // proper dispatch. For now, simple literal matching via the
            // arm body (which contains the full expression including any
            // guard). This is a temporary stub.
            let next_bb = self
                .builder
                .append_block(self.current_function, &format!("match.arm{}", i + 1));

            // Check if arm body is a literal we can compare against
            let arm_kind = *self.canon.arena.kind(arm_body);
            let matches = match arm_kind {
                CanExpr::Int(n) => {
                    let pat_val = self.builder.const_i64(n);
                    let is_float = scrut_type == Idx::FLOAT;
                    if is_float {
                        Some(self.builder.fcmp_oeq(scrut_val, pat_val, "pat.eq"))
                    } else {
                        Some(self.builder.icmp_eq(scrut_val, pat_val, "pat.eq"))
                    }
                }
                CanExpr::Bool(b) => {
                    let pat_val = self.builder.const_bool(b);
                    Some(self.builder.icmp_eq(scrut_val, pat_val, "pat.eq"))
                }
                _ => None,
            };

            if let Some(test_val) = matches {
                let arm_bb = self
                    .builder
                    .append_block(self.current_function, &format!("match.body{i}"));
                self.builder.cond_br(test_val, arm_bb, next_bb);

                self.builder.position_at_end(arm_bb);
                let body_val = self.lower(arm_body);
                let body_exit = self.builder.current_block();
                if let (Some(bv), Some(bb)) = (body_val, body_exit) {
                    if !self.builder.current_block_terminated() {
                        incoming.push((bv, bb));
                        self.builder.br(merge_bb);
                    }
                }

                self.builder.position_at_end(next_bb);
            } else {
                // Cannot compile pattern test — treat as wildcard
                let body_val = self.lower(arm_body);
                let body_bb = self.builder.current_block();
                if let (Some(bv), Some(bb)) = (body_val, body_bb) {
                    if !self.builder.current_block_terminated() {
                        incoming.push((bv, bb));
                        self.builder.br(merge_bb);
                    }
                }
                break;
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
}
