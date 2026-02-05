//! Control flow compilation: conditionals, loops, blocks.

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::ExprKind;
use ori_ir::{ExprArena, ExprId, Name, StmtRange};
use ori_types::Idx;
use tracing::instrument;

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile short-circuit logical AND (&&).
    ///
    /// Evaluates left operand first. If false, returns false without evaluating right.
    /// Otherwise, evaluates and returns the right operand.
    #[expect(clippy::too_many_arguments, reason = "matches compile_expr signature")]
    pub(crate) fn compile_short_circuit_and(
        &self,
        left: ExprId,
        right: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile left operand
        let lhs = self.compile_expr(left, arena, expr_types, locals, function, loop_ctx)?;
        let lhs_bool = lhs.into_int_value();

        // Create basic blocks
        let eval_rhs_bb = self.append_block(function, "and_rhs");
        let merge_bb = self.append_block(function, "and_merge");

        let entry_bb = self.current_block()?;

        // If left is false, short-circuit to merge; otherwise evaluate right
        self.cond_br(lhs_bool, eval_rhs_bb, merge_bb);

        // Evaluate right operand
        self.position_at_end(eval_rhs_bb);
        let rhs = self.compile_expr(right, arena, expr_types, locals, function, loop_ctx);
        let rhs_exit_bb = self.current_block()?;

        // Handle case where right operand terminates (e.g., panic)
        let rhs_terminated = rhs_exit_bb.get_terminator().is_some();
        if !rhs_terminated {
            self.br(merge_bb);
        }

        // Merge block with phi node
        self.position_at_end(merge_bb);

        let false_val = self.cx().scx.type_i1().const_int(0, false);

        if let Some(rhs_val) = rhs {
            if rhs_terminated {
                // Right side terminated, only left's false case reaches merge
                Some(false_val.into())
            } else {
                // Both paths reach merge: false from left, rhs from right
                self.build_phi_from_incoming(
                    Idx::BOOL,
                    &[(false_val.into(), entry_bb), (rhs_val, rhs_exit_bb)],
                )
            }
        } else {
            Some(false_val.into())
        }
    }

    /// Compile short-circuit logical OR (||).
    ///
    /// Evaluates left operand first. If true, returns true without evaluating right.
    /// Otherwise, evaluates and returns the right operand.
    #[expect(clippy::too_many_arguments, reason = "matches compile_expr signature")]
    pub(crate) fn compile_short_circuit_or(
        &self,
        left: ExprId,
        right: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile left operand
        let lhs = self.compile_expr(left, arena, expr_types, locals, function, loop_ctx)?;
        let lhs_bool = lhs.into_int_value();

        // Create basic blocks
        let eval_rhs_bb = self.append_block(function, "or_rhs");
        let merge_bb = self.append_block(function, "or_merge");

        let entry_bb = self.current_block()?;

        // If left is true, short-circuit to merge; otherwise evaluate right
        self.cond_br(lhs_bool, merge_bb, eval_rhs_bb);

        // Evaluate right operand
        self.position_at_end(eval_rhs_bb);
        let rhs = self.compile_expr(right, arena, expr_types, locals, function, loop_ctx);
        let rhs_exit_bb = self.current_block()?;

        // Handle case where right operand terminates (e.g., panic)
        let rhs_terminated = rhs_exit_bb.get_terminator().is_some();
        if !rhs_terminated {
            self.br(merge_bb);
        }

        // Merge block with phi node
        self.position_at_end(merge_bb);

        let true_val = self.cx().scx.type_i1().const_int(1, false);

        if let Some(rhs_val) = rhs {
            if rhs_terminated {
                // Right side terminated, only left's true case reaches merge
                Some(true_val.into())
            } else {
                // Both paths reach merge: true from left, rhs from right
                self.build_phi_from_incoming(
                    Idx::BOOL,
                    &[(true_val.into(), entry_bb), (rhs_val, rhs_exit_bb)],
                )
            }
        } else {
            Some(true_val.into())
        }
    }

    /// Compile short-circuit null coalescing (??).
    ///
    /// Evaluates left operand first. If it's Some/Ok, returns the inner value.
    /// Otherwise, evaluates and returns the right operand.
    ///
    /// Tag semantics differ between Option and Result:
    /// - Option: tag=0 (None), tag=1 (Some) — "has value" when tag != 0
    /// - Result: tag=0 (Ok), tag=1 (Err) — "has value" when tag == 0
    #[expect(clippy::too_many_arguments, reason = "matches compile_expr signature")]
    pub(crate) fn compile_short_circuit_coalesce(
        &self,
        left: ExprId,
        right: ExprId,
        result_type: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the type of the left operand to distinguish Option from Result
        let left_type = expr_types.get(left.index()).copied().unwrap_or(Idx::NONE);
        let is_result = self.cx().is_result_type(left_type);
        let is_wrapper = self.cx().is_wrapper_type(left_type);

        // Compile left operand (should be Option<T> or Result<T, E>)
        let lhs = self.compile_expr(left, arena, expr_types, locals, function, loop_ctx)?;

        // If the left operand is not a wrapper type (Option/Result), it's already
        // an unwrapped value (e.g., from a chained coalesce). Just return it.
        if !is_wrapper {
            return Some(lhs);
        }

        // Option/Result are structs: { i8 tag, payload }
        // Verify it's actually a struct before extracting
        let BasicValueEnum::StructValue(lhs_struct) = lhs else {
            // Not a struct - already unwrapped, return as-is
            return Some(lhs);
        };

        // Extract the tag (first field)
        let tag = self
            .extract_value(lhs_struct, 0, "coalesce_tag")?
            .into_int_value();

        // Determine "has value" condition based on type:
        // - Option: tag != 0 (tag=1 means Some)
        // - Result: tag == 0 (tag=0 means Ok)
        let has_value = if is_result {
            self.icmp(
                inkwell::IntPredicate::EQ,
                tag,
                self.cx().scx.type_i8().const_int(0, false),
                "is_ok",
            )
        } else {
            self.icmp(
                inkwell::IntPredicate::NE,
                tag,
                self.cx().scx.type_i8().const_int(0, false),
                "is_some",
            )
        };

        // Create basic blocks
        let has_value_bb = self.append_block(function, "coalesce_has_value");
        let no_value_bb = self.append_block(function, "coalesce_no_value");
        let merge_bb = self.append_block(function, "coalesce_merge");

        // Branch based on whether left has a value
        self.cond_br(has_value, has_value_bb, no_value_bb);

        // Has value path: extract and potentially coerce payload back to result type
        self.position_at_end(has_value_bb);
        let payload_raw = self.extract_value(lhs_struct, 1, "coalesce_payload")?;
        // If the payload is an i64, coerce it to the result type
        // If it's a struct (nested Option/Result), materialize it to avoid phi issues
        let payload = match payload_raw {
            BasicValueEnum::IntValue(i) if i.get_type().get_bit_width() == 64 => {
                self.coerce_from_i64(i, result_type)?
            }
            _ => {
                // For nested wrappers (e.g., Option<Option<T>>), the payload is a struct.
                // Materialize it to avoid LLVM JIT issues with constant structs in phi nodes.
                self.materialize_constant_struct(payload_raw)
            }
        };
        let has_value_exit_bb = self.current_block()?;
        self.br(merge_bb);

        // No value path: evaluate right operand
        self.position_at_end(no_value_bb);
        let rhs = self.compile_expr(right, arena, expr_types, locals, function, loop_ctx);

        // Materialize constant structs before the branch.
        // LLVM JIT has trouble with constant struct values in phi nodes.
        // This stores the constant to an alloca and loads it back, creating a non-constant.
        let rhs = rhs.map(|v| self.materialize_constant_struct(v));

        let no_value_exit_bb = self.current_block()?;

        // Handle case where right operand terminates (e.g., panic)
        let rhs_terminated = no_value_exit_bb.get_terminator().is_some();
        if !rhs_terminated {
            self.br(merge_bb);
        }

        // Merge block with phi node
        self.position_at_end(merge_bb);

        if let Some(rhs_val) = rhs {
            if rhs_terminated {
                // Right side terminated, only has_value case reaches merge
                Some(payload)
            } else {
                // Both paths reach merge
                self.build_phi_from_incoming(
                    result_type,
                    &[(payload, has_value_exit_bb), (rhs_val, no_value_exit_bb)],
                )
            }
        } else {
            Some(payload)
        }
    }

    /// Materialize a constant struct value to a non-constant.
    ///
    /// LLVM JIT can have trouble with constant struct values in phi nodes.
    /// This stores the constant to an alloca and loads it back.
    fn materialize_constant_struct(&self, val: BasicValueEnum<'ll>) -> BasicValueEnum<'ll> {
        let BasicValueEnum::StructValue(sv) = val else {
            return val;
        };

        if !sv.is_const() {
            return val;
        }

        // Store to alloca and load back
        let ty = sv.get_type();
        let alloca = self.alloca(ty.into(), "const_mat");
        self.store(val, alloca);
        self.load(ty.into(), alloca, "const_load")
    }
}

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile an if/else expression.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_if(
        &self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
        result_type: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile condition
        let cond_val = self.compile_expr(cond, arena, expr_types, locals, function, loop_ctx)?;
        let cond_bool = cond_val.into_int_value();

        // Create basic blocks
        let then_bb = self.append_block(function, "then");
        let else_bb = self.append_block(function, "else");
        let merge_bb = self.append_block(function, "merge");

        // Conditional branch
        self.cond_br(cond_bool, then_bb, else_bb);

        // Compile then branch
        self.position_at_end(then_bb);
        let then_val =
            self.compile_expr(then_branch, arena, expr_types, locals, function, loop_ctx);
        let then_exit_bb = self.current_block()?;
        // Only branch to merge if the block isn't already terminated (e.g., by panic/break/return)
        let then_terminated = then_exit_bb.get_terminator().is_some();
        if !then_terminated {
            self.br(merge_bb);
        }

        // Compile else branch
        self.position_at_end(else_bb);
        let else_val = if let Some(else_id) = else_branch {
            self.compile_expr(else_id, arena, expr_types, locals, function, loop_ctx)
        } else {
            // No else branch - produce default value or unit
            if result_type == Idx::UNIT {
                None
            } else {
                Some(self.cx().default_value(result_type))
            }
        };
        let else_exit_bb = self.current_block()?;
        // Only branch to merge if the block isn't already terminated
        let else_terminated = else_exit_bb.get_terminator().is_some();
        if !else_terminated {
            self.br(merge_bb);
        }

        // Merge block with phi node
        self.position_at_end(merge_bb);

        // If both branches terminated (diverged), the merge block is unreachable
        if then_terminated && else_terminated {
            self.unreachable();
            return None;
        }

        // If both branches produce values and reach merge, create a phi node
        match (then_val, else_val, then_terminated, else_terminated) {
            (Some(t), Some(e), false, false) => {
                // Both branches reach merge with values
                self.build_phi_from_incoming(result_type, &[(t, then_exit_bb), (e, else_exit_bb)])
            }
            (Some(t), _, false, true) => {
                // Only then branch reaches merge
                Some(t)
            }
            (_, Some(e), true, false) => {
                // Only else branch reaches merge
                Some(e)
            }
            _ => None,
        }
    }

    /// Compile a loop expression.
    pub(crate) fn compile_loop(
        &self,
        body: ExprId,
        result_type: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Create basic blocks
        let header_bb = self.append_block(function, "loop_header");
        let body_bb = self.append_block(function, "loop_body");
        let exit_bb = self.append_block(function, "loop_exit");

        // Jump to header
        self.br(header_bb);

        // Header block (for continue)
        self.position_at_end(header_bb);
        self.br(body_bb);

        // Body block
        self.position_at_end(body_bb);

        // Create loop context for break/continue
        let loop_ctx = LoopContext {
            header: header_bb,
            exit: exit_bb,
            break_phi: None, // TODO: set up in exit block for break-with-value
        };

        // Compile loop body
        let _body_val =
            self.compile_expr(body, arena, expr_types, locals, function, Some(&loop_ctx));

        // If we haven't branched away (no break/continue), loop back
        if self.current_block()?.get_terminator().is_none() {
            self.br(header_bb);
        }

        // Position at exit block
        self.position_at_end(exit_bb);

        // Loops with break values would need phi nodes here
        // For now, return default value for non-void results
        if result_type == Idx::UNIT {
            None
        } else {
            Some(self.cx().default_value(result_type))
        }
    }

    /// Compile a break expression.
    pub(crate) fn compile_break(
        &self,
        value: Option<ExprId>,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let ctx = loop_ctx?;

        // Compile break value if present
        if let Some(val_id) = value {
            let _val = self.compile_expr(val_id, arena, expr_types, locals, function, loop_ctx);
            // TODO: add value to phi node if loop returns values
        }

        // Jump to exit block
        self.br(ctx.exit);

        // Break doesn't produce a value (execution continues at exit)
        None
    }

    /// Compile a continue expression.
    pub(crate) fn compile_continue(
        &self,
        value: Option<ExprId>,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let ctx = loop_ctx?;

        // Compile continue value if present (for for...yield loops)
        if let Some(val_id) = value {
            let _val = self.compile_expr(val_id, arena, expr_types, locals, function, loop_ctx);
            // TODO: add value to yield accumulator for for...yield loops
        }

        // Jump back to header
        self.br(ctx.header);

        // Continue doesn't produce a value
        None
    }

    /// Compile a for loop.
    #[expect(
        clippy::too_many_arguments,
        reason = "for-loop compilation requires loop context, arena, and type state"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "handles both range and list iteration"
    )]
    pub(crate) fn compile_for(
        &self,
        binding: Name,
        iter: ExprId,
        guard: Option<ExprId>,
        body: ExprId,
        is_yield: bool,
        result_type: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Check if iterating over a range expression
        let iter_expr = arena.get_expr(iter);
        let is_range = matches!(iter_expr.kind, ExprKind::Range { .. });

        // Compile the iterable
        let iter_val = self.compile_expr(iter, arena, expr_types, locals, function, None)?;

        // Create loop blocks
        // Structure: entry -> header -> body -> latch -> header (or exit)
        // The latch block increments the index before looping back.
        // `continue` jumps to latch (to increment), `break` jumps to exit.
        let header_bb = self.append_block(function, "for_header");
        let body_bb = self.append_block(function, "for_body");
        let latch_bb = self.append_block(function, "for_latch");
        let exit_bb = self.append_block(function, "for_exit");

        // Allocate index counter
        let idx_ptr = self.alloca(self.cx().scx.type_i64().into(), "for_idx");

        // Set up iteration bounds based on type
        let (start_val, end_val, use_inclusive) = if is_range {
            // Range: { i64 start, i64 end, i1 inclusive }
            let range_struct = iter_val.into_struct_value();
            let start = self
                .extract_value(range_struct, 0, "range_start")?
                .into_int_value();
            let end = self
                .extract_value(range_struct, 1, "range_end")?
                .into_int_value();
            let inclusive = self
                .extract_value(range_struct, 2, "range_inclusive")?
                .into_int_value();
            (start, end, Some(inclusive))
        } else if let BasicValueEnum::StructValue(iter_struct) = iter_val {
            // List: { i64 len, i64 cap, ptr data }
            let len = self
                .extract_value(iter_struct, 0, "iter_len")?
                .into_int_value();
            let _data_ptr = self.extract_value(iter_struct, 2, "iter_data")?;
            let start = self.cx().scx.type_i64().const_int(0, false);
            (start, len, None)
        } else {
            // Unsupported iterable type
            return None;
        };

        // Initialize index to start
        self.store(start_val.into(), idx_ptr);

        // Jump to header
        self.br(header_bb);

        // Header: check condition
        self.position_at_end(header_bb);
        let idx = self
            .load(self.cx().scx.type_i64().into(), idx_ptr, "idx")
            .into_int_value();

        // Condition: idx < end (exclusive) or idx <= end (inclusive)
        let cond = if let Some(inclusive) = use_inclusive {
            // For ranges: use SLE if inclusive, SLT if exclusive
            let less_than = self.icmp(inkwell::IntPredicate::SLT, idx, end_val, "for_slt");
            let less_or_eq = self.icmp(inkwell::IntPredicate::SLE, idx, end_val, "for_sle");
            // Select based on inclusive flag
            self.select(inclusive, less_or_eq.into(), less_than.into(), "for_cond")
                .into_int_value()
        } else {
            // For lists: always idx < len
            self.icmp(inkwell::IntPredicate::SLT, idx, end_val, "for_cond")
        };
        self.cond_br(cond, body_bb, exit_bb);

        // Body: bind element and execute
        self.position_at_end(body_bb);

        // Bind the current index value to the binding name
        // For-loop variables are re-bound each iteration (immutable within iteration)
        locals.bind_immutable(binding, idx.into());

        // Create loop context for break/continue in for-loop body.
        // IMPORTANT: `continue` must jump to the latch block (which increments the
        // index) rather than the header, otherwise we get an infinite loop.
        let for_loop_ctx = LoopContext {
            header: latch_bb, // continue goes to latch (increment then check)
            exit: exit_bb,
            break_phi: None,
        };

        // Handle guard if present
        if let Some(guard_id) = guard {
            let guard_val = self.compile_expr(
                guard_id,
                arena,
                expr_types,
                locals,
                function,
                Some(&for_loop_ctx),
            )?;
            let guard_bool = guard_val.into_int_value();

            let guard_pass_bb = self.append_block(function, "guard_pass");

            // Guard fail: go to latch (increment and continue)
            self.cond_br(guard_bool, guard_pass_bb, latch_bb);

            self.position_at_end(guard_pass_bb);
        }

        // Compile body with loop context for break/continue support
        let _body_val = self.compile_expr(
            body,
            arena,
            expr_types,
            locals,
            function,
            Some(&for_loop_ctx),
        );

        // Fall through to latch if body didn't terminate
        if self.current_block()?.get_terminator().is_none() {
            self.br(latch_bb);
        }

        // Latch block: increment index and loop back to header
        self.position_at_end(latch_bb);
        let current_idx = self
            .load(self.cx().scx.type_i64().into(), idx_ptr, "cur_idx")
            .into_int_value();
        let next_idx = self.add(
            current_idx,
            self.cx().scx.type_i64().const_int(1, false),
            "next_idx",
        );
        self.store(next_idx.into(), idx_ptr);
        self.br(header_bb);

        // Exit
        self.position_at_end(exit_bb);

        // For yield loops, we'd return a list; for do loops, return unit
        if is_yield {
            // Return empty list for now (real impl would collect values)
            let list_type = self.cx().list_type();
            let zero = self.cx().scx.type_i64().const_int(0, false);
            let null_ptr = self.cx().scx.type_ptr().const_null();

            let list_val = self.build_struct(
                list_type,
                &[zero.into(), zero.into(), null_ptr.into()],
                "empty_list",
            );

            Some(list_val.into())
        } else if result_type == Idx::UNIT {
            None
        } else {
            Some(self.cx().default_value(result_type))
        }
    }

    /// Compile a try expression (error propagation).
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_try(
        &self,
        inner: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile inner expression (should be a Result)
        let result_val = self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?;

        // Assume result is { i8 tag, T value }
        let result_struct = result_val.into_struct_value();

        // Extract tag
        let tag = self
            .extract_value(result_struct, 0, "try_tag")?
            .into_int_value();

        // Check if Ok (tag == 0)
        let is_ok = self.icmp(
            inkwell::IntPredicate::EQ,
            tag,
            self.cx().scx.type_i8().const_int(0, false),
            "is_ok",
        );

        // Create blocks
        let ok_bb = self.append_block(function, "try_ok");
        let err_bb = self.append_block(function, "try_err");
        let merge_bb = self.append_block(function, "try_merge");

        self.cond_br(is_ok, ok_bb, err_bb);

        // Ok path: extract and return value
        self.position_at_end(ok_bb);
        let ok_val = self.extract_value(result_struct, 1, "ok_val")?;
        self.br(merge_bb);

        // Err path: propagate error (return early)
        self.position_at_end(err_bb);
        // For now, just return the error result as-is
        self.ret(result_val);

        // Merge block - only has one predecessor (ok_bb), so no phi needed
        self.position_at_end(merge_bb);

        // Return the Ok value directly (no phi needed with single predecessor)
        Some(ok_val)
    }

    /// Compile a block expression.
    pub(crate) fn compile_block(
        &self,
        stmts: StmtRange,
        result: Option<ExprId>,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        use ori_ir::ast::StmtKind;

        // Compile each statement
        let statements = arena.get_stmt_range(stmts);
        for stmt in statements {
            match &stmt.kind {
                StmtKind::Expr(expr_id) => {
                    // Evaluate for side effects
                    self.compile_expr(*expr_id, arena, expr_types, locals, function, loop_ctx);
                }
                StmtKind::Let {
                    pattern,
                    ty: _,
                    init,
                    mutable,
                } => {
                    // Compile the let binding with mutability flag
                    self.compile_let(
                        pattern, *init, *mutable, arena, expr_types, locals, function, loop_ctx,
                    );
                }
            }
        }

        // Compile the result expression if present
        if let Some(result_expr) = result {
            self.compile_expr(result_expr, arena, expr_types, locals, function, loop_ctx)
        } else {
            None
        }
    }

    /// Compile an assignment expression.
    pub(crate) fn compile_assign(
        &self,
        target: ExprId,
        value: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the value first
        let val = self.compile_expr(value, arena, expr_types, locals, function, loop_ctx)?;

        // Handle assignment target
        let target_expr = arena.get_expr(target);
        match &target_expr.kind {
            ori_ir::ast::ExprKind::Ident(name) => {
                // Simple variable assignment - store to mutable variable
                self.store_variable(*name, val, locals)?;
                Some(val)
            }
            _ => {
                // TODO: handle field/index assignment
                None
            }
        }
    }
}
