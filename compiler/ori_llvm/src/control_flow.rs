//! Control flow compilation: conditionals, loops, blocks.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue, PhiValue};
use ori_ir::{ExprArena, ExprId, Name, StmtRange, TypeId};

use crate::{LLVMCodegen, LoopContext};

impl<'ctx> LLVMCodegen<'ctx> {
    /// Compile an if/else expression.
    pub(crate) fn compile_if(
        &self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile condition
        let cond_val = self.compile_expr(cond, arena, expr_types, locals, function, loop_ctx)?;
        let cond_bool = cond_val.into_int_value();

        // Create basic blocks
        let then_bb = self.context.append_basic_block(function, "then");
        let else_bb = self.context.append_basic_block(function, "else");
        let merge_bb = self.context.append_basic_block(function, "merge");

        // Conditional branch
        self.builder
            .build_conditional_branch(cond_bool, then_bb, else_bb)
            .ok()?;

        // Compile then branch
        self.builder.position_at_end(then_bb);
        let then_val = self.compile_expr(then_branch, arena, expr_types, locals, function, loop_ctx);
        let then_exit_bb = self.builder.get_insert_block()?;
        self.builder.build_unconditional_branch(merge_bb).ok()?;

        // Compile else branch
        self.builder.position_at_end(else_bb);
        let else_val = if let Some(else_id) = else_branch {
            self.compile_expr(else_id, arena, expr_types, locals, function, loop_ctx)
        } else {
            // No else branch - produce default value or unit
            if result_type == TypeId::VOID {
                None
            } else {
                Some(self.default_value(result_type))
            }
        };
        let else_exit_bb = self.builder.get_insert_block()?;
        self.builder.build_unconditional_branch(merge_bb).ok()?;

        // Merge block with phi node
        self.builder.position_at_end(merge_bb);

        // If both branches produce values, create a phi node
        match (then_val, else_val) {
            (Some(t), Some(e)) => {
                let phi = self.build_phi(result_type, &[
                    (t, then_exit_bb),
                    (e, else_exit_bb),
                ])?;
                Some(phi.as_basic_value())
            }
            _ => None,
        }
    }

    /// Build a phi node for the given incoming values.
    pub(crate) fn build_phi(
        &self,
        type_id: TypeId,
        incoming: &[(BasicValueEnum<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)],
    ) -> Option<PhiValue<'ctx>> {
        let llvm_type = self.llvm_type(type_id);
        let phi = self.builder.build_phi(llvm_type, "phi").ok()?;

        for (val, bb) in incoming {
            phi.add_incoming(&[(val, *bb)]);
        }

        Some(phi)
    }

    /// Compile a loop expression.
    pub(crate) fn compile_loop(
        &self,
        body: ExprId,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Create basic blocks
        let header_bb = self.context.append_basic_block(function, "loop_header");
        let body_bb = self.context.append_basic_block(function, "loop_body");
        let exit_bb = self.context.append_basic_block(function, "loop_exit");

        // Jump to header
        self.builder.build_unconditional_branch(header_bb).ok()?;

        // Header block (for continue)
        self.builder.position_at_end(header_bb);
        self.builder.build_unconditional_branch(body_bb).ok()?;

        // Body block
        self.builder.position_at_end(body_bb);

        // Create loop context for break/continue
        let loop_ctx = LoopContext {
            header: header_bb,
            exit: exit_bb,
            _break_phi: None, // TODO: set up in exit block for break-with-value
        };

        // Compile loop body
        let _body_val = self.compile_expr(body, arena, expr_types, locals, function, Some(&loop_ctx));

        // If we haven't branched away (no break/continue), loop back
        if self.builder.get_insert_block()?.get_terminator().is_none() {
            self.builder.build_unconditional_branch(header_bb).ok()?;
        }

        // Position at exit block
        self.builder.position_at_end(exit_bb);

        // Loops with break values would need phi nodes here
        // For now, return default value for non-void results
        if result_type == TypeId::VOID {
            None
        } else {
            Some(self.default_value(result_type))
        }
    }

    /// Compile a break expression.
    pub(crate) fn compile_break(
        &self,
        value: Option<ExprId>,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let ctx = loop_ctx?;

        // Compile break value if present
        if let Some(val_id) = value {
            let _val = self.compile_expr(val_id, arena, expr_types, locals, function, loop_ctx);
            // TODO: add value to phi node if loop returns values
        }

        // Jump to exit block
        self.builder.build_unconditional_branch(ctx.exit).ok()?;

        // Break doesn't produce a value (execution continues at exit)
        None
    }

    /// Compile a continue expression.
    pub(crate) fn compile_continue(
        &self,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let ctx = loop_ctx?;

        // Jump back to header
        self.builder.build_unconditional_branch(ctx.header).ok()?;

        // Continue doesn't produce a value
        None
    }

    /// Compile a for loop.
    #[expect(clippy::too_many_arguments, reason = "for-loop compilation requires loop context, arena, and type state")]
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
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the iterable
        let iter_val = self.compile_expr(iter, arena, expr_types, locals, function, None)?;

        // For simplicity, assume iter_val is a list struct { len, cap, data }
        // Extract length and data pointer
        let iter_struct = iter_val.into_struct_value();
        let len = self.builder.build_extract_value(iter_struct, 0, "iter_len").ok()?.into_int_value();
        let _data_ptr = self.builder.build_extract_value(iter_struct, 2, "iter_data").ok()?;

        // Create loop blocks
        let header_bb = self.context.append_basic_block(function, "for_header");
        let body_bb = self.context.append_basic_block(function, "for_body");
        let exit_bb = self.context.append_basic_block(function, "for_exit");

        // Allocate index counter
        let idx_ptr = self.builder.build_alloca(self.context.i64_type(), "for_idx").ok()?;
        self.builder.build_store(idx_ptr, self.context.i64_type().const_int(0, false)).ok()?;

        // Jump to header
        self.builder.build_unconditional_branch(header_bb).ok()?;

        // Header: check if index < len
        self.builder.position_at_end(header_bb);
        let idx = self.builder.build_load(self.context.i64_type(), idx_ptr, "idx").ok()?.into_int_value();
        let cond = self.builder.build_int_compare(inkwell::IntPredicate::SLT, idx, len, "for_cond").ok()?;
        self.builder.build_conditional_branch(cond, body_bb, exit_bb).ok()?;

        // Body: bind element and execute
        self.builder.position_at_end(body_bb);

        // For simplicity, bind the index as the element (a real impl would dereference)
        locals.insert(binding, idx.into());

        // Handle guard if present
        if let Some(guard_id) = guard {
            let guard_val = self.compile_expr(guard_id, arena, expr_types, locals, function, None)?;
            let guard_bool = guard_val.into_int_value();

            let guard_pass_bb = self.context.append_basic_block(function, "guard_pass");
            let guard_fail_bb = self.context.append_basic_block(function, "guard_fail");

            self.builder.build_conditional_branch(guard_bool, guard_pass_bb, guard_fail_bb).ok()?;

            // Guard fail: increment and continue
            self.builder.position_at_end(guard_fail_bb);
            let next_idx = self.builder.build_int_add(idx, self.context.i64_type().const_int(1, false), "next_idx").ok()?;
            self.builder.build_store(idx_ptr, next_idx).ok()?;
            self.builder.build_unconditional_branch(header_bb).ok()?;

            self.builder.position_at_end(guard_pass_bb);
        }

        // Compile body
        let _body_val = self.compile_expr(body, arena, expr_types, locals, function, None);

        // Increment index
        let current_idx = self.builder.build_load(self.context.i64_type(), idx_ptr, "cur_idx").ok()?.into_int_value();
        let next_idx = self.builder.build_int_add(current_idx, self.context.i64_type().const_int(1, false), "next_idx").ok()?;
        self.builder.build_store(idx_ptr, next_idx).ok()?;

        // Loop back
        if self.builder.get_insert_block()?.get_terminator().is_none() {
            self.builder.build_unconditional_branch(header_bb).ok()?;
        }

        // Exit
        self.builder.position_at_end(exit_bb);

        // For yield loops, we'd return a list; for do loops, return unit
        if is_yield {
            // Return empty list for now (real impl would collect values)
            let list_type = self.list_type();
            let zero = self.context.i64_type().const_int(0, false);
            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

            let mut list_val = list_type.get_undef();
            list_val = self.builder.build_insert_value(list_val, zero, 0, "list_len").ok()?.into_struct_value();
            list_val = self.builder.build_insert_value(list_val, zero, 1, "list_cap").ok()?.into_struct_value();
            list_val = self.builder.build_insert_value(list_val, null_ptr, 2, "list_data").ok()?.into_struct_value();

            Some(list_val.into())
        } else if result_type == TypeId::VOID {
            None
        } else {
            Some(self.default_value(result_type))
        }
    }

    /// Compile a try expression (error propagation).
    pub(crate) fn compile_try(
        &self,
        inner: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile inner expression (should be a Result)
        let result_val = self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?;

        // Assume result is { i8 tag, T value }
        let result_struct = result_val.into_struct_value();

        // Extract tag
        let tag = self.builder.build_extract_value(result_struct, 0, "try_tag").ok()?.into_int_value();

        // Check if Ok (tag == 0)
        let is_ok = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.context.i8_type().const_int(0, false),
            "is_ok",
        ).ok()?;

        // Create blocks
        let ok_bb = self.context.append_basic_block(function, "try_ok");
        let err_bb = self.context.append_basic_block(function, "try_err");
        let merge_bb = self.context.append_basic_block(function, "try_merge");

        self.builder.build_conditional_branch(is_ok, ok_bb, err_bb).ok()?;

        // Ok path: extract and return value
        self.builder.position_at_end(ok_bb);
        let ok_val = self.builder.build_extract_value(result_struct, 1, "ok_val").ok()?;
        self.builder.build_unconditional_branch(merge_bb).ok()?;
        let ok_exit = self.builder.get_insert_block()?;

        // Err path: propagate error (return early)
        self.builder.position_at_end(err_bb);
        // For now, just return the error result as-is
        self.builder.build_return(Some(&result_val)).ok()?;

        // Merge block
        self.builder.position_at_end(merge_bb);

        // Return the Ok value
        let phi = self.builder.build_phi(ok_val.get_type(), "try_result").ok()?;
        phi.add_incoming(&[(&ok_val, ok_exit)]);

        Some(phi.as_basic_value())
    }

    /// Compile a block expression.
    pub(crate) fn compile_block(
        &self,
        stmts: StmtRange,
        result: Option<ExprId>,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        use ori_ir::ast::StmtKind;

        // Compile each statement
        let statements = arena.get_stmt_range(stmts);
        for stmt in statements {
            match &stmt.kind {
                StmtKind::Expr(expr_id) => {
                    // Evaluate for side effects
                    self.compile_expr(*expr_id, arena, expr_types, locals, function, loop_ctx);
                }
                StmtKind::Let { pattern, ty: _, init, mutable: _ } => {
                    // Compile the let binding
                    self.compile_let(pattern, *init, arena, expr_types, locals, function, loop_ctx);
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
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        if let Some(val_id) = value {
            let val = self.compile_expr(val_id, arena, expr_types, locals, function, loop_ctx)?;
            self.builder.build_return(Some(&val)).ok()?;
        } else {
            self.builder.build_return(None).ok()?;
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
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
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
