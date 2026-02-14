//! Control flow lowering for V2 codegen.
//!
//! Handles if/else, blocks, let bindings, loops, for-loops, break/continue,
//! assignment, and match expressions.

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
    // For loop
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::For { binding, iter, guard, body, is_yield }`.
    pub(crate) fn lower_for(
        &mut self,
        binding: Name,
        iter: CanId,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        let iter_val = self.lower(iter)?;
        let iter_type = self.expr_type(iter);
        let type_info = self.type_info.get(iter_type);

        match type_info {
            super::type_info::TypeInfo::Range => {
                self.lower_for_range(binding, iter_val, guard, body, is_yield, expr_id)
            }
            super::type_info::TypeInfo::List { .. } => {
                self.lower_for_list(binding, iter_val, iter_type, guard, body, is_yield, expr_id)
            }
            _ => {
                tracing::warn!(?iter_type, "for-loop over non-range/non-list type");
                self.builder.record_codegen_error();
                None
            }
        }
    }

    /// For-loop over a range.
    fn lower_for_range(
        &mut self,
        binding: Name,
        range_val: ValueId,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
    ) -> Option<ValueId> {
        // Extract range components
        let start = self.builder.extract_value(range_val, 0, "range.start")?;
        let end = self.builder.extract_value(range_val, 1, "range.end")?;
        let inclusive = self.builder.extract_value(range_val, 2, "range.incl")?;

        // Yield setup: allocate list buffer and write index
        let yield_ctx = if is_yield {
            Some(self.setup_yield_context(start, end, inclusive, expr_id)?)
        } else {
            None
        };

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
        let body_val = self.lower(body);

        // Yield: store body value into the output list
        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

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

        // Yield: build and return the list struct
        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

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
        binding: Name,
        list_val: ValueId,
        list_type: Idx,
        guard: CanId,
        body: CanId,
        is_yield: bool,
        expr_id: CanId,
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

        // Yield setup: allocate output list using source list length as capacity
        let yield_ctx = if is_yield {
            Some(self.setup_yield_context_with_capacity(len, expr_id)?)
        } else {
            None
        };

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

        let body_val = self.lower(body);

        // Yield: store body value into the output list
        if let (Some(ref yc), Some(bv)) = (&yield_ctx, body_val) {
            self.emit_yield_store(yc, bv);
        }

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

        // Yield: build and return the list struct
        if let Some(yc) = yield_ctx {
            return self.finish_yield_list(&yc, expr_id);
        }

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

    // -----------------------------------------------------------------------
    // For-yield helpers
    // -----------------------------------------------------------------------

    /// Context for a for-yield loop: the allocated buffer and write index.
    fn setup_yield_context(
        &mut self,
        start: ValueId,
        end: ValueId,
        inclusive: ValueId,
        expr_id: CanId,
    ) -> Option<YieldContext> {
        // Compute capacity: end - start + (inclusive ? 1 : 0)
        // Clamp to 0 if negative (start > end).
        let diff = self.builder.sub(end, start, "yield.diff");
        let one = self.builder.const_i64(1);
        let zero = self.builder.const_i64(0);
        let incl_extra = self
            .builder
            .select(inclusive, one, zero, "yield.incl_extra");
        let raw_cap = self.builder.add(diff, incl_extra, "yield.raw_cap");
        let is_neg = self.builder.icmp_slt(raw_cap, zero, "yield.neg");
        let cap = self.builder.select(is_neg, zero, raw_cap, "yield.cap");

        self.setup_yield_context_with_capacity(cap, expr_id)
    }

    /// Setup yield context with a pre-computed capacity value.
    fn setup_yield_context_with_capacity(
        &mut self,
        cap: ValueId,
        expr_id: CanId,
    ) -> Option<YieldContext> {
        let result_type = self.expr_type(expr_id);
        let type_info = self.type_info.get(result_type);
        let elem_idx = match &type_info {
            super::type_info::TypeInfo::List { element } => *element,
            _ => ori_types::Idx::INT,
        };
        let elem_llvm_ty = self.resolve_type(elem_idx);
        let elem_size = self.type_info.get(elem_idx).size().unwrap_or(8);

        // Allocate raw data buffer: ori_list_alloc_data(capacity, elem_size) -> *mut u8
        let esize = self.builder.const_i64(elem_size as i64);
        let i64_ty = self.builder.i64_type();
        let i64_ty2 = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let alloc_data =
            self.builder
                .get_or_declare_function("ori_list_alloc_data", &[i64_ty, i64_ty2], ptr_ty);
        let data_ptr = self.builder.call(alloc_data, &[cap, esize], "yield.data")?;

        // Write index alloca at function entry (for mem2reg promotion)
        let i64_llvm = self.builder.i64_type();
        let write_idx =
            self.builder
                .create_entry_alloca(self.current_function, "yield.widx", i64_llvm);
        let zero = self.builder.const_i64(0);
        self.builder.store(zero, write_idx);

        Some(YieldContext {
            data_ptr,
            write_idx,
            cap,
            elem_llvm_ty,
        })
    }

    /// Store a body value into the yield output list and increment write index.
    fn emit_yield_store(&mut self, yc: &YieldContext, body_val: ValueId) {
        let i64_ty = self.builder.i64_type();
        let widx = self.builder.load(i64_ty, yc.write_idx, "yield.widx_cur");

        // Store element at data[write_idx]
        let elem_ptr = self
            .builder
            .gep(yc.elem_llvm_ty, yc.data_ptr, &[widx], "yield.elem_ptr");
        self.builder.store(body_val, elem_ptr);

        // Increment write index
        let one = self.builder.const_i64(1);
        let next_widx = self.builder.add(widx, one, "yield.widx_next");
        self.builder.store(next_widx, yc.write_idx);
    }

    /// Build the final list struct from yield context after the loop completes.
    fn finish_yield_list(&mut self, yc: &YieldContext, expr_id: CanId) -> Option<ValueId> {
        let i64_ty = self.builder.i64_type();
        let final_len = self.builder.load(i64_ty, yc.write_idx, "yield.final_len");
        let result_type = self.expr_type(expr_id);
        let list_ty = self.resolve_type(result_type);
        Some(
            self.builder
                .build_struct(list_ty, &[final_len, yc.cap, yc.data_ptr], "yield.list"),
        )
    }
}

/// Temporary state for for-yield list construction.
struct YieldContext {
    /// Pointer to the allocated data buffer.
    data_ptr: ValueId,
    /// Alloca holding the current write index (mutable counter).
    write_idx: ValueId,
    /// Allocated capacity of the buffer.
    cap: ValueId,
    /// LLVM type of each element (for GEP sizing).
    elem_llvm_ty: super::value_id::LLVMTypeId,
}
