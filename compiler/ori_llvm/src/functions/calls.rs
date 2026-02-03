//! Function call compilation.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{CallArgRange, ExprArena, ExprId, ExprList, Name, TypeId};
use tracing::instrument;

use crate::builder::Builder;
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a function call with positional arguments.
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_call(
        &self,
        func: ExprId,
        args: ExprList,
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
        let arg_ids: Vec<_> = arena.iter_expr_list(args).collect();
        match fn_name {
            "str" => {
                if arg_ids.len() == 1 {
                    let arg_val = self
                        .compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_str(arg_val);
                }
            }
            "int" => {
                if arg_ids.len() == 1 {
                    let arg_val = self
                        .compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_int(arg_val);
                }
            }
            "float" => {
                if arg_ids.len() == 1 {
                    let arg_val = self
                        .compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_float(arg_val);
                }
            }
            "byte" => {
                if arg_ids.len() == 1 {
                    let arg_val = self
                        .compile_expr(arg_ids[0], arena, expr_types, locals, function, loop_ctx)?;
                    return self.compile_builtin_byte(arg_val);
                }
            }
            _ => {}
        }

        // Check if this is a closure call (variable holding a function value)
        if let Some(closure_val) = locals.get(&func_name) {
            return self.compile_closure_call(
                *closure_val,
                &arg_ids,
                arena,
                expr_types,
                locals,
                function,
                loop_ctx,
            );
        }

        // Look up the function in the module
        let callee = self.cx().llmod().get_function(fn_name)?;

        // Compile arguments
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = Vec::with_capacity(arg_ids.len());

        for arg_id in &arg_ids {
            let arg_val =
                self.compile_expr(*arg_id, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Build the call
        self.call(callee, &compiled_args, "call")
    }

    /// Compile a closure call.
    ///
    /// Closures can be stored as:
    /// - A struct (directly in locals): { i8 count, i64 `fn_ptr`, capture0, ... }
    /// - An i64 with lowest bit 0: plain function pointer (no captures)
    /// - An i64 with lowest bit 1: pointer to boxed closure (has captures)
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
            let arg_val =
                self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        match closure_val {
            BasicValueEnum::StructValue(closure_struct) => {
                // Closure struct directly in locals: { i8 count, i64 fn_ptr, capture0, ... }
                let fn_ptr_int = self
                    .extract_value(closure_struct, 1, "fn_ptr_int")?
                    .into_int_value();

                // Extract captured values (fields 2+) and append them to arguments
                let num_fields = closure_struct.get_type().count_fields();
                for i in 2..num_fields {
                    let captured =
                        self.extract_value(closure_struct, i, &format!("capture_{}", i - 2))?;
                    compiled_args.push(captured);
                }

                // Convert i64 back to function pointer
                let fn_ptr = self.int_to_ptr(fn_ptr_int, self.cx().scx.type_ptr(), "fn_ptr");
                self.call_closure_with_args(fn_ptr, &compiled_args)
            }
            BasicValueEnum::IntValue(closure_int) => {
                // Check the tag bit (lowest bit) to distinguish between:
                // - 0: plain function pointer (no captures)
                // - 1: pointer to boxed closure (has captures)
                let one = self.cx().scx.type_i64().const_int(1, false);
                let tag_bit = self.and(closure_int, one, "tag_bit");
                let is_boxed = self.icmp(
                    inkwell::IntPredicate::NE,
                    tag_bit,
                    self.cx().scx.type_i64().const_int(0, false),
                    "is_boxed",
                );

                // Create blocks for the two cases and result merge
                let boxed_bb = self.append_block(function, "closure_boxed");
                let plain_bb = self.append_block(function, "closure_plain");
                let merge_bb = self.append_block(function, "closure_merge");

                self.cond_br(is_boxed, boxed_bb, plain_bb);

                // === Boxed closure path ===
                self.position_at_end(boxed_bb);
                let boxed_result = self.call_boxed_closure(closure_int, &compiled_args)?;
                let boxed_exit_bb = self.current_block()?;
                self.br(merge_bb);

                // === Plain function pointer path ===
                self.position_at_end(plain_bb);
                let fn_ptr = self.int_to_ptr(closure_int, self.cx().scx.type_ptr(), "plain_fn_ptr");
                let plain_result = self.call_closure_with_args(fn_ptr, &compiled_args)?;
                let plain_exit_bb = self.current_block()?;
                self.br(merge_bb);

                // === Merge results ===
                self.position_at_end(merge_bb);
                self.build_phi_from_incoming(
                    TypeId::INT,
                    &[(boxed_result, boxed_exit_bb), (plain_result, plain_exit_bb)],
                )
            }
            BasicValueEnum::PointerValue(ptr) => {
                // Already a pointer - call directly
                self.call_closure_with_args(ptr, &compiled_args)
            }
            _ => {
                // Unsupported closure type
                None
            }
        }
    }

    /// Call a boxed closure (tagged pointer with captures).
    fn call_boxed_closure(
        &self,
        tagged_ptr: inkwell::values::IntValue<'ll>,
        base_args: &[BasicValueEnum<'ll>],
    ) -> Option<BasicValueEnum<'ll>> {
        // Clear the tag bit to get the real pointer
        let ptr_int = self.and(
            tagged_ptr,
            self.cx().scx.type_i64().const_int(!1u64, false),
            "ptr_untagged",
        );
        let closure_ptr = self.int_to_ptr(ptr_int, self.cx().scx.type_ptr(), "closure_ptr");

        // Load capture count from the first byte
        let capture_count = self
            .load(self.cx().scx.type_i8().into(), closure_ptr, "capture_count")
            .into_int_value();

        // Load fn_ptr from offset 8 (after i8 count aligned to 8 bytes in struct layout)
        let fn_ptr_offset = self.gep(
            self.cx().scx.type_i8().into(),
            closure_ptr,
            &[self.cx().scx.type_i64().const_int(8, false)],
            "fn_ptr_offset",
        );
        let fn_ptr_int = self
            .load(self.cx().scx.type_i64().into(), fn_ptr_offset, "fn_ptr_int")
            .into_int_value();
        let fn_ptr = self.int_to_ptr(fn_ptr_int, self.cx().scx.type_ptr(), "fn_ptr");

        // Build args: base_args + captures
        let mut all_args = base_args.to_vec();

        // Load captures from offset 16 (after i8 count + padding + i64 fn_ptr)
        // We load based on capture_count. Support up to 8 captures.
        let capture_count_i64 = self.zext(capture_count, self.cx().scx.type_i64(), "count_i64");

        for i in 0..8u64 {
            // Check if this capture exists
            let i_val = self.cx().scx.type_i64().const_int(i, false);
            let should_load = self.icmp(
                inkwell::IntPredicate::ULT,
                i_val,
                capture_count_i64,
                &format!("should_load_{i}"),
            );

            // Load the capture value (will be garbage if not used, but we won't use it)
            let capture_offset = self.gep(
                self.cx().scx.type_i8().into(),
                closure_ptr,
                &[self.cx().scx.type_i64().const_int(16 + i * 8, false)],
                &format!("capture_{i}_ptr"),
            );
            let capture_val = self.load(
                self.cx().scx.type_i64().into(),
                capture_offset,
                &format!("capture_{i}"),
            );

            // Use select to either include this capture (if i < count) or use a dummy value
            // But since we're building a static arg list, we need a different approach.
            // For simplicity, we'll just load all captures up to the actual count.
            // This requires knowing the count at compile time, which we don't.
            //
            // Alternative: use a maximum fixed capture count and always pass that many args.
            // The lambda function ignores extra args.
            //
            // For now, let's use select to conditionally add captures:
            let _ = should_load; // We'll load all 8 potential captures
            all_args.push(capture_val);
        }

        // Trim args to actual count: base_args.len() + capture_count
        // Since we can't trim at runtime, we rely on the lambda function accepting
        // extra parameters (which it ignores). This is a simplification.

        self.call_closure_with_args(fn_ptr, &all_args)
    }

    /// Call a closure function pointer with the given arguments.
    fn call_closure_with_args(
        &self,
        fn_ptr: inkwell::values::PointerValue<'ll>,
        args: &[BasicValueEnum<'ll>],
    ) -> Option<BasicValueEnum<'ll>> {
        // Create function type: all i64 params -> i64 return
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = args
            .iter()
            .map(|_| self.cx().scx.type_i64().into())
            .collect();
        let fn_type = self.cx().scx.type_i64().fn_type(&param_types, false);

        self.call_indirect(fn_type, fn_ptr, args, "closure_call")
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
            let arg_val =
                self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx)?;
            compiled_args.push(arg_val);
        }

        // Build the call
        self.call(callee, &compiled_args, "call")
    }

    /// Compile a method call: receiver.method(args)
    ///
    /// Handles both instance methods (receiver is a value) and associated functions
    /// (receiver is a type name, which won't compile to a value).
    ///
    /// Built-in methods are handled first for primitive types (int, float, bool,
    /// char, byte) before falling back to user-defined method lookup.
    pub(crate) fn compile_method_call(
        &self,
        receiver: ExprId,
        method: Name,
        args: ExprList,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Get receiver type for built-in method dispatch
        let receiver_type = expr_types
            .get(receiver.index())
            .copied()
            .unwrap_or(TypeId::INFER);

        // Try to compile receiver
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx);

        // Get argument IDs
        let arg_ids: Vec<_> = arena.iter_expr_list(args).collect();

        // Try built-in method first if we have a receiver value
        if let Some(recv) = recv_val {
            if let Some(result) = self.compile_builtin_method(
                recv,
                receiver_type,
                method,
                &arg_ids,
                arena,
                expr_types,
                locals,
                function,
                loop_ctx,
            ) {
                return Some(result);
            }
        }

        // Fall back to user method lookup
        // If receiver compiled to a value, it's an instance method - include receiver as first arg
        // If receiver is None (type name for associated function), don't include it
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = match recv_val {
            Some(val) => vec![val],
            None => vec![],
        };

        for arg_id in &arg_ids {
            if let Some(arg_val) =
                self.compile_expr(*arg_id, arena, expr_types, locals, function, loop_ctx)
            {
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
    ///
    /// Handles both instance methods (receiver is a value) and associated functions
    /// (receiver is a type name, which won't compile to a value).
    ///
    /// Built-in methods are handled first for primitive types (int, float, bool,
    /// char, byte) before falling back to user-defined method lookup.
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
        // Get receiver type for built-in method dispatch
        let receiver_type = expr_types
            .get(receiver.index())
            .copied()
            .unwrap_or(TypeId::INFER);

        // Try to compile receiver
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx);

        // Get call args and extract expression IDs for builtin method dispatch
        let call_args = arena.get_call_args(args);
        let arg_ids: Vec<ExprId> = call_args.iter().map(|a| a.value).collect();

        // Try built-in method first if we have a receiver value
        if let Some(recv) = recv_val {
            if let Some(result) = self.compile_builtin_method(
                recv,
                receiver_type,
                method,
                &arg_ids,
                arena,
                expr_types,
                locals,
                function,
                loop_ctx,
            ) {
                return Some(result);
            }
        }

        // Fall back to user method lookup
        // If receiver compiled to a value, it's an instance method - include receiver as first arg
        // If receiver is None (type name for associated function), don't include it
        let mut compiled_args: Vec<BasicValueEnum<'ll>> = match recv_val {
            Some(val) => vec![val],
            None => vec![],
        };

        for arg in call_args {
            if let Some(arg_val) =
                self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx)
            {
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
