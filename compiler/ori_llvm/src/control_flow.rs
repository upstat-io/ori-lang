//! Control flow compilation: conditionals, loops, blocks.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name, StmtRange, TypeId};
use tracing::instrument;

use crate::builder::Builder;
use crate::LoopContext;

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
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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
        self.br(merge_bb);

        // Compile else branch
        self.position_at_end(else_bb);
        let else_val = if let Some(else_id) = else_branch {
            self.compile_expr(else_id, arena, expr_types, locals, function, loop_ctx)
        } else {
            // No else branch - produce default value or unit
            if result_type == TypeId::VOID {
                None
            } else {
                Some(self.cx().default_value(result_type))
            }
        };
        let else_exit_bb = self.current_block()?;
        self.br(merge_bb);

        // Merge block with phi node
        self.position_at_end(merge_bb);

        // If both branches produce values, create a phi node
        match (then_val, else_val) {
            (Some(t), Some(e)) => {
                self.build_phi_from_incoming(result_type, &[(t, then_exit_bb), (e, else_exit_bb)])
            }
            _ => None,
        }
    }

    /// Compile a loop expression.
    pub(crate) fn compile_loop(
        &self,
        body: ExprId,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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
        if result_type == TypeId::VOID {
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
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let ctx = loop_ctx?;

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
    pub(crate) fn compile_for(
        &self,
        binding: Name,
        iter: ExprId,
        guard: Option<ExprId>,
        body: ExprId,
        is_yield: bool,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the iterable
        let iter_val = self.compile_expr(iter, arena, expr_types, locals, function, None)?;

        // For simplicity, assume iter_val is a list struct { len, cap, data }
        // Extract length and data pointer
        let iter_struct = iter_val.into_struct_value();
        let len = self
            .extract_value(iter_struct, 0, "iter_len")?
            .into_int_value();
        let _data_ptr = self.extract_value(iter_struct, 2, "iter_data")?;

        // Create loop blocks
        let header_bb = self.append_block(function, "for_header");
        let body_bb = self.append_block(function, "for_body");
        let exit_bb = self.append_block(function, "for_exit");

        // Allocate index counter
        let idx_ptr = self.alloca(self.cx().scx.type_i64().into(), "for_idx");
        self.store(self.cx().scx.type_i64().const_int(0, false).into(), idx_ptr);

        // Jump to header
        self.br(header_bb);

        // Header: check if index < len
        self.position_at_end(header_bb);
        let idx = self
            .load(self.cx().scx.type_i64().into(), idx_ptr, "idx")
            .into_int_value();
        let cond = self.icmp(inkwell::IntPredicate::SLT, idx, len, "for_cond");
        self.cond_br(cond, body_bb, exit_bb);

        // Body: bind element and execute
        self.position_at_end(body_bb);

        // For simplicity, bind the index as the element (a real impl would dereference)
        locals.insert(binding, idx.into());

        // Handle guard if present
        if let Some(guard_id) = guard {
            let guard_val =
                self.compile_expr(guard_id, arena, expr_types, locals, function, None)?;
            let guard_bool = guard_val.into_int_value();

            let guard_pass_bb = self.append_block(function, "guard_pass");
            let guard_fail_bb = self.append_block(function, "guard_fail");

            self.cond_br(guard_bool, guard_pass_bb, guard_fail_bb);

            // Guard fail: increment and continue
            self.position_at_end(guard_fail_bb);
            let next_idx = self.add(
                idx,
                self.cx().scx.type_i64().const_int(1, false),
                "next_idx",
            );
            self.store(next_idx.into(), idx_ptr);
            self.br(header_bb);

            self.position_at_end(guard_pass_bb);
        }

        // Compile body
        let _body_val = self.compile_expr(body, arena, expr_types, locals, function, None);

        // Increment index
        let current_idx = self
            .load(self.cx().scx.type_i64().into(), idx_ptr, "cur_idx")
            .into_int_value();
        let next_idx = self.add(
            current_idx,
            self.cx().scx.type_i64().const_int(1, false),
            "next_idx",
        );
        self.store(next_idx.into(), idx_ptr);

        // Loop back
        if self.current_block()?.get_terminator().is_none() {
            self.br(header_bb);
        }

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
        } else if result_type == TypeId::VOID {
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
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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
                    mutable: _,
                } => {
                    // Compile the let binding
                    self.compile_let(
                        pattern, *init, arena, expr_types, locals, function, loop_ctx,
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

    /// Compile a return expression.
    pub(crate) fn compile_return(
        &self,
        value: Option<ExprId>,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        if let Some(val_id) = value {
            let val = self.compile_expr(val_id, arena, expr_types, locals, function, loop_ctx)?;
            self.ret(val);
        } else {
            self.ret_void();
        }
        // Return doesn't produce a value (it transfers control)
        None
    }

    /// Compile an assignment expression.
    pub(crate) fn compile_assign(
        &self,
        target: ExprId,
        value: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the value first
        let val = self.compile_expr(value, arena, expr_types, locals, function, loop_ctx)?;

        // Handle assignment target
        let target_expr = arena.get_expr(target);
        match &target_expr.kind {
            ori_ir::ast::ExprKind::Ident(name) => {
                // Simple variable assignment - update locals
                locals.insert(*name, val);
                Some(val)
            }
            _ => {
                // TODO: handle field/index assignment
                None
            }
        }
    }
}
