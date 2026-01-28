//! Function call compilation.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{CallArgRange, ExprArena, ExprId, ExprRange, Name, TypeId};
use tracing::instrument;

use crate::builder::Builder;
use crate::LoopContext;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a function call with positional arguments.
    #[instrument(skip(self, arena, expr_types, locals, function, loop_ctx), level = "debug")]
    pub(crate) fn compile_call(
        &self,
        func: ExprId,
        args: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the function being called
        let func_expr = arena.get_expr(func);

        // For now, only handle direct function calls (Ident expressions)
        let func_name = match &func_expr.kind {
            ori_ir::ast::ExprKind::Ident(name) => *name,
            _ => {
                // Check if it's a closure call (variable holding a function)
                // TODO: handle closure calls properly
                return None;
            }
        };

        let fn_name = self.cx().interner.lookup(func_name);

        // Handle built-in type conversion functions
        let arg_ids = arena.get_expr_list(args);
        match fn_name {
            "str" => {
                if arg_ids.len() == 1 {
                    let arg_val = self.compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_str(arg_val);
                }
            }
            "int" => {
                if arg_ids.len() == 1 {
                    let arg_val = self.compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_int(arg_val);
                }
            }
            "float" => {
                if arg_ids.len() == 1 {
                    let arg_val = self.compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_float(arg_val);
                }
            }
            "byte" => {
                if arg_ids.len() == 1 {
                    let arg_val = self.compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_byte(arg_val);
                }
            }
            _ => {}
        }

        // Check if this is a closure call (variable holding a function value)
        if let Some(closure_val) = locals.get(&func_name) {
            return self.compile_closure_call(*closure_val, arg_ids, arena, expr_types, locals, function, loop_ctx);
        }

        // Look up the function in the module
        let callee = self.cx().llmod().get_function(fn_name)?;

        // Compile arguments
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = Vec::with_capacity(arg_ids.len());

        for &arg_id in arg_ids {
            let arg_val = self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Build the call
        self.call(callee, &compiled_args, "call")
    }

    /// Compile a closure call.
    ///
    /// Closures can be stored as:
    /// - A struct: { i8 tag, i64 fn_ptr, capture0, capture1, ... } (closures with captures)
    /// - An i64: function pointer (simple function references or closures without captures)
    fn compile_closure_call(
        &self,
        closure_val: BasicValueEnum<'ll>,
        arg_ids: &[ExprId],
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile regular arguments first
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = Vec::with_capacity(arg_ids.len());
        for &arg_id in arg_ids {
            let arg_val = self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Handle different closure representations
        let fn_ptr = match closure_val {
            BasicValueEnum::StructValue(closure_struct) => {
                // Closure with captures: { i8 tag, i64 fn_ptr, capture0, ... }
                let fn_ptr_int = self.extract_value(closure_struct, 1, "fn_ptr_int").into_int_value();

                // Extract captured values (fields 2+) and append them to arguments
                let num_fields = closure_struct.get_type().count_fields();
                for i in 2..num_fields {
                    let captured = self.extract_value(closure_struct, i, &format!("capture_{}", i - 2));
                    compiled_args.push(captured);
                }

                // Convert i64 back to function pointer
                self.int_to_ptr(fn_ptr_int, self.cx().scx.type_ptr(), "fn_ptr")
            }
            BasicValueEnum::IntValue(fn_ptr_int) => {
                // Simple function pointer (no captures)
                self.int_to_ptr(fn_ptr_int, self.cx().scx.type_ptr(), "fn_ptr")
            }
            BasicValueEnum::PointerValue(ptr) => {
                // Already a pointer
                ptr
            }
            _ => {
                // Unsupported closure type
                return None;
            }
        };

        // Build the indirect call
        // Create function type: all i64 params -> i64 return
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = compiled_args
            .iter()
            .map(|_| self.cx().scx.type_i64().into())
            .collect();
        let fn_type = self.cx().scx.type_i64().fn_type(&param_types, false);

        self.call_indirect(fn_type, fn_ptr, &compiled_args, "closure_call")
    }

    /// Compile a function call with named arguments.
    pub(crate) fn compile_call_named(
        &self,
        func: ExprId,
        args: CallArgRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get the function being called
        let func_expr = arena.get_expr(func);

        // For now, only handle direct function calls (Ident expressions)
        let func_name = match &func_expr.kind {
            ori_ir::ast::ExprKind::Ident(name) => *name,
            _ => {
                return None;
            }
        };

        // Look up the function in the module
        let fn_name = self.cx().interner.lookup(func_name);
        let callee = self.cx().llmod().get_function(fn_name)?;

        // For named args, we just use the order they appear
        // (proper named arg handling would reorder based on parameter names)
        let call_args = arena.get_call_args(args);
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = Vec::with_capacity(call_args.len());

        for arg in call_args {
            let arg_val = self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Build the call
        self.call(callee, &compiled_args, "call")
    }

    /// Compile a method call: receiver.method(args)
    pub(crate) fn compile_method_call(
        &self,
        receiver: ExprId,
        method: Name,
        args: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile receiver
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Compile arguments
        let arg_ids = arena.get_expr_list(args);
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = vec![recv_val]; // receiver is first arg

        for &arg_id in arg_ids {
            if let Some(arg_val) = self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx) {
                compiled_args.push(arg_val);
            }
        }

        // Look up method function
        let method_name = self.cx().interner.lookup(method);
        if let Some(callee) = self.cx().llmod().get_function(method_name) {
            self.call(callee, &compiled_args, "method_call")
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
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile receiver
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Compile arguments (receiver first)
        let call_args = arena.get_call_args(args);
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = vec![recv_val];

        for arg in call_args {
            if let Some(arg_val) = self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx) {
                compiled_args.push(arg_val);
            }
        }

        // Look up method function
        let method_name = self.cx().interner.lookup(method);
        if let Some(callee) = self.cx().llmod().get_function(method_name) {
            self.call(callee, &compiled_args, "method_call")
        } else {
            None
        }
    }
}
