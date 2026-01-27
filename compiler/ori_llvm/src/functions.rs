//! Function compilation, calls, lambdas, and expression patterns.

use std::collections::HashMap;

use inkwell::types::{BasicMetadataTypeEnum, BasicType};
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::patterns::{FunctionExp, FunctionSeq, SeqBinding};
use ori_ir::{CallArgRange, DurationUnit, ExprArena, ExprId, ExprRange, Name, SizeUnit, TypeId};

use crate::{LLVMCodegen, LoopContext};

impl<'ctx> LLVMCodegen<'ctx> {
    /// Compile a function.
    pub fn compile_function(
        &self,
        name: Name,
        param_names: &[Name],
        param_types: &[TypeId],
        return_type: TypeId,
        body: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
    ) -> FunctionValue<'ctx> {
        let fn_name = self.interner.lookup(name);

        // Build function type
        let param_llvm_types: Vec<BasicMetadataTypeEnum> = param_types
            .iter()
            .map(|&t| self.llvm_metadata_type(t))
            .collect();

        let fn_type = if return_type == TypeId::VOID {
            self.context.void_type().fn_type(&param_llvm_types, false)
        } else {
            self.llvm_type(return_type).fn_type(&param_llvm_types, false)
        };

        // Create function
        let function = self.module.add_function(fn_name, fn_type, None);

        // Create entry block
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        // Build parameter map
        let mut locals: HashMap<Name, BasicValueEnum<'ctx>> = HashMap::new();

        for (i, &param_name) in param_names.iter().enumerate() {
            let param_value = function.get_nth_param(i as u32).unwrap_or_else(|| {
                panic!("Missing parameter {i}")
            });
            param_value.set_name(self.interner.lookup(param_name));
            locals.insert(param_name, param_value);
        }

        // Compile body (no loop context at top level)
        let result = self.compile_expr(body, arena, expr_types, &mut locals, function, None);

        // Return
        if return_type == TypeId::VOID {
            self.builder.build_return(None).unwrap_or_else(|e| {
                panic!("Failed to build return: {e}")
            });
        } else if let Some(val) = result {
            self.builder.build_return(Some(&val)).unwrap_or_else(|e| {
                panic!("Failed to build return: {e}")
            });
        } else {
            // Fallback: return default value
            let default = self.default_value(return_type);
            self.builder.build_return(Some(&default)).unwrap_or_else(|e| {
                panic!("Failed to build return: {e}")
            });
        }

        function
    }

    /// Compile a function call with positional arguments.
    pub(crate) fn compile_call(
        &self,
        func: ExprId,
        args: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get the function being called
        let func_expr = arena.get_expr(func);

        // For now, only handle direct function calls (Ident expressions)
        let func_name = match &func_expr.kind {
            ori_ir::ast::ExprKind::Ident(name) => *name,
            _ => {
                // TODO: handle function references, closures, etc.
                return None;
            }
        };

        // Look up the function in the module
        let fn_name = self.interner.lookup(func_name);
        let callee = self.module.get_function(fn_name)?;

        // Compile arguments
        let arg_ids = arena.get_expr_list(args);
        let mut compiled_args: Vec<BasicValueEnum<'ctx>> = Vec::with_capacity(arg_ids.len());

        for &arg_id in arg_ids {
            let arg_val = self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Convert to BasicMetadataValueEnum for the call
        let args_meta: Vec<inkwell::values::BasicMetadataValueEnum> = compiled_args
            .iter()
            .map(|v| (*v).into())
            .collect();

        // Build the call
        let call_val = self.builder.build_call(callee, &args_meta, "call").ok()?;

        // Get the return value (may be void)
        call_val.try_as_basic_value().basic()
    }

    /// Compile a function call with named arguments.
    pub(crate) fn compile_call_named(
        &self,
        func: ExprId,
        args: CallArgRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get the function being called
        let func_expr = arena.get_expr(func);

        // For now, only handle direct function calls (Ident expressions)
        let func_name = match &func_expr.kind {
            ori_ir::ast::ExprKind::Ident(name) => *name,
            _ => {
                // TODO: handle function references, closures, etc.
                return None;
            }
        };

        // Look up the function in the module
        let fn_name = self.interner.lookup(func_name);
        let callee = self.module.get_function(fn_name)?;

        // For named args, we just use the order they appear
        // (proper named arg handling would reorder based on parameter names)
        let call_args = arena.get_call_args(args);
        let mut compiled_args: Vec<BasicValueEnum<'ctx>> = Vec::with_capacity(call_args.len());

        for arg in call_args {
            let arg_val = self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Convert to BasicMetadataValueEnum for the call
        let args_meta: Vec<inkwell::values::BasicMetadataValueEnum> = compiled_args
            .iter()
            .map(|v| (*v).into())
            .collect();

        // Build the call
        let call_val = self.builder.build_call(callee, &args_meta, "call").ok()?;

        // Get the return value (may be void)
        call_val.try_as_basic_value().basic()
    }

    /// Compile a method call: receiver.method(args)
    pub(crate) fn compile_method_call(
        &self,
        receiver: ExprId,
        method: Name,
        args: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile receiver
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Compile arguments
        let arg_ids = arena.get_expr_list(args);
        let mut compiled_args: Vec<BasicValueEnum<'ctx>> = vec![recv_val]; // receiver is first arg

        for &arg_id in arg_ids {
            if let Some(arg_val) = self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx) {
                compiled_args.push(arg_val);
            }
        }

        // Look up method function
        let method_name = self.interner.lookup(method);
        if let Some(callee) = self.module.get_function(method_name) {
            let args_meta: Vec<inkwell::values::BasicMetadataValueEnum> = compiled_args
                .iter()
                .map(|v| (*v).into())
                .collect();

            let call_val = self.builder.build_call(callee, &args_meta, "method_call").ok()?;
            call_val.try_as_basic_value().basic()
        } else {
            None
        }
    }

    /// Compile a method call with named args.
    pub(crate) fn compile_method_call_named(
        &self,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile receiver
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Compile arguments (receiver first)
        let call_args = arena.get_call_args(args);
        let mut compiled_args: Vec<BasicValueEnum<'ctx>> = vec![recv_val];

        for arg in call_args {
            if let Some(arg_val) = self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx) {
                compiled_args.push(arg_val);
            }
        }

        // Look up method function
        let method_name = self.interner.lookup(method);
        if let Some(callee) = self.module.get_function(method_name) {
            let args_meta: Vec<inkwell::values::BasicMetadataValueEnum> = compiled_args
                .iter()
                .map(|v| (*v).into())
                .collect();

            let call_val = self.builder.build_call(callee, &args_meta, "method_call").ok()?;
            call_val.try_as_basic_value().basic()
        } else {
            None
        }
    }

    /// Compile a lambda expression.
    /// Lambdas are compiled as closures: { fn_ptr, captures_ptr }.
    pub(crate) fn compile_lambda(
        &self,
        params: ori_ir::ast::ParamRange,
        body: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &HashMap<Name, BasicValueEnum<'ctx>>,
        _parent_function: FunctionValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let parameters = arena.get_params(params);

        // Create a unique name for this lambda
        static LAMBDA_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let lambda_id = LAMBDA_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let lambda_name = format!("__lambda_{lambda_id}");

        // Build parameter types (all as i64 for simplicity)
        let param_types: Vec<BasicMetadataTypeEnum> = parameters
            .iter()
            .map(|_| self.context.i64_type().into())
            .collect();

        // Create function type
        let fn_type = self.context.i64_type().fn_type(&param_types, false);
        let lambda_fn = self.module.add_function(&lambda_name, fn_type, None);

        // Create entry block for lambda
        let entry = self.context.append_basic_block(lambda_fn, "entry");

        // Save current builder position
        let saved_block = self.builder.get_insert_block();

        // Position at lambda entry
        self.builder.position_at_end(entry);

        // Build parameter map for lambda body
        let mut lambda_locals: HashMap<Name, BasicValueEnum<'ctx>> = locals.clone();
        for (i, param) in parameters.iter().enumerate() {
            if let Some(param_val) = lambda_fn.get_nth_param(i as u32) {
                param_val.set_name(self.interner.lookup(param.name));
                lambda_locals.insert(param.name, param_val);
            }
        }

        // Compile lambda body
        let result = self.compile_expr(body, arena, expr_types, &mut lambda_locals, lambda_fn, None);

        // Return result
        if let Some(val) = result {
            self.builder.build_return(Some(&val)).ok()?;
        } else {
            let zero = self.context.i64_type().const_int(0, false);
            self.builder.build_return(Some(&zero)).ok()?;
        }

        // Restore builder position
        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }

        // Return function pointer as closure
        // For a full implementation, we'd create a closure struct with captures
        Some(lambda_fn.as_global_value().as_pointer_value().into())
    }

    /// Compile a FunctionSeq (run, try, match).
    pub(crate) fn compile_function_seq(
        &self,
        seq: &FunctionSeq,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        match seq {
            FunctionSeq::Run { bindings, result, .. } => {
                // Execute bindings sequentially
                let seq_bindings = arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    self.compile_seq_binding(binding, arena, expr_types, locals, function, loop_ctx);
                }
                // Return result
                self.compile_expr(*result, arena, expr_types, locals, function, loop_ctx)
            }

            FunctionSeq::Try { bindings, result, .. } => {
                // Execute bindings with error propagation
                let seq_bindings = arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    // Each binding should check for errors and propagate
                    self.compile_seq_binding(binding, arena, expr_types, locals, function, loop_ctx);
                }
                self.compile_expr(*result, arena, expr_types, locals, function, loop_ctx)
            }

            FunctionSeq::Match { scrutinee, arms, .. } => {
                // Delegate to existing match compilation
                self.compile_match(*scrutinee, *arms, result_type, arena, expr_types, locals, function, loop_ctx)
            }

            FunctionSeq::ForPattern { over, map, arm: _, default, .. } => {
                // Compile the for pattern
                let iter_val = self.compile_expr(*over, arena, expr_types, locals, function, loop_ctx)?;

                // Apply map if present
                let _mapped = if let Some(map_fn) = map {
                    self.compile_expr(*map_fn, arena, expr_types, locals, function, loop_ctx)?
                } else {
                    iter_val
                };

                // For now, just return the default
                self.compile_expr(*default, arena, expr_types, locals, function, loop_ctx)
            }
        }
    }

    /// Compile a SeqBinding (let or stmt).
    fn compile_seq_binding(
        &self,
        binding: &SeqBinding,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        match binding {
            SeqBinding::Let { pattern, value, .. } => {
                self.compile_let(pattern, *value, arena, expr_types, locals, function, loop_ctx)
            }
            SeqBinding::Stmt { expr, .. } => {
                self.compile_expr(*expr, arena, expr_types, locals, function, loop_ctx)
            }
        }
    }

    /// Compile a FunctionExp (recurse, parallel, etc.).
    pub(crate) fn compile_function_exp(
        &self,
        exp: &FunctionExp,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        use ori_ir::ast::patterns::FunctionExpKind;

        let named_exprs = arena.get_named_exprs(exp.props);

        match exp.kind {
            FunctionExpKind::Recurse => {
                // Find condition, base, and step
                let mut condition = None;
                let mut base = None;
                let mut step = None;

                for ne in named_exprs {
                    let name = self.interner.lookup(ne.name);
                    match name {
                        "condition" => condition = Some(ne.value),
                        "base" => base = Some(ne.value),
                        "step" => step = Some(ne.value),
                        _ => {}
                    }
                }

                // Implement as a simple conditional for now
                if let (Some(cond), Some(base_expr), Some(_step_expr)) = (condition, base, step) {
                    let cond_val = self.compile_expr(cond, arena, expr_types, locals, function, loop_ctx)?;
                    let cond_bool = cond_val.into_int_value();

                    let then_bb = self.context.append_basic_block(function, "recurse_base");
                    let else_bb = self.context.append_basic_block(function, "recurse_step");
                    let merge_bb = self.context.append_basic_block(function, "recurse_merge");

                    self.builder.build_conditional_branch(cond_bool, then_bb, else_bb).ok()?;

                    self.builder.position_at_end(then_bb);
                    let base_val = self.compile_expr(base_expr, arena, expr_types, locals, function, loop_ctx);
                    let then_exit = self.builder.get_insert_block()?;
                    self.builder.build_unconditional_branch(merge_bb).ok()?;

                    self.builder.position_at_end(else_bb);
                    // For step, would need to call self - for now return default
                    let step_val = self.default_value(result_type);
                    let else_exit = self.builder.get_insert_block()?;
                    self.builder.build_unconditional_branch(merge_bb).ok()?;

                    self.builder.position_at_end(merge_bb);

                    if let Some(bv) = base_val {
                        let phi = self.build_phi(result_type, &[(bv, then_exit), (step_val, else_exit)])?;
                        Some(phi.as_basic_value())
                    } else {
                        Some(step_val)
                    }
                } else {
                    None
                }
            }

            FunctionExpKind::Print => {
                // Find msg parameter
                for ne in named_exprs {
                    let name = self.interner.lookup(ne.name);
                    if name == "msg" {
                        // Compile the message (but we don't have a runtime print yet)
                        let _msg = self.compile_expr(ne.value, arena, expr_types, locals, function, loop_ctx);
                        // Would call runtime print function here
                    }
                }
                None // print returns void
            }

            FunctionExpKind::Panic => {
                // Compile panic - would call runtime panic function
                // For now, just create unreachable
                self.builder.build_unreachable().ok()?;
                None
            }

            _ => {
                // Patterns without custom LLVM codegen — return default for now.
                // Avoids coupling: new FunctionExpKind variants don't require changes here.
                if result_type == TypeId::VOID {
                    None
                } else {
                    Some(self.default_value(result_type))
                }
            }
        }
    }

    /// Compile a config variable reference.
    /// Config variables are compile-time constants stored in locals.
    pub(crate) fn compile_config(
        &self,
        name: Name,
        locals: &HashMap<Name, BasicValueEnum<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Config variables should be pre-populated in locals by the caller
        locals.get(&name).copied()
    }

    /// Compile a function reference (@name).
    pub(crate) fn compile_function_ref(&self, name: Name) -> Option<BasicValueEnum<'ctx>> {
        let fn_name = self.interner.lookup(name);
        let func = self.module.get_function(fn_name)?;
        Some(func.as_global_value().as_pointer_value().into())
    }

    /// Compile a duration literal.
    /// Durations are stored as i64 milliseconds.
    pub(crate) fn compile_duration(&self, value: u64, unit: DurationUnit) -> Option<BasicValueEnum<'ctx>> {
        let millis = unit.to_millis(value);
        Some(self.context.i64_type().const_int(millis, false).into())
    }

    /// Compile a size literal.
    /// Sizes are stored as i64 bytes.
    pub(crate) fn compile_size(&self, value: u64, unit: SizeUnit) -> Option<BasicValueEnum<'ctx>> {
        let bytes = unit.to_bytes(value);
        Some(self.context.i64_type().const_int(bytes, false).into())
    }
}
