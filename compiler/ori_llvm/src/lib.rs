//! LLVM Backend for Ori
//!
//! This crate provides native code generation via LLVM.
//!
//! # Modules
//! - [`LLVMCodegen`] - Low-level expression compilation
//! - [`module`] - Module-level compilation (full programs)
//! - [`runtime`] - Runtime functions for JIT execution
//! - [`evaluator`] - JIT-based evaluator for running tests

pub mod evaluator;
pub mod module;
pub mod runtime;

use std::collections::HashMap;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PhiValue};
use inkwell::OptimizationLevel;

use ori_ir::{
    ast::{patterns::{BindingPattern, MatchPattern, FunctionSeq, FunctionExp, SeqBinding}, BinaryOp, ExprKind, UnaryOp, ArmRange, CallArgRange},
    DurationUnit, SizeUnit,
    ExprArena, ExprId, ExprRange, Name, StringInterner, TypeId, StmtRange,
};

/// Loop context for break/continue.
#[derive(Clone)]
struct LoopContext<'ctx> {
    /// Block to jump to on continue.
    header: inkwell::basic_block::BasicBlock<'ctx>,
    /// Block to jump to on break.
    exit: inkwell::basic_block::BasicBlock<'ctx>,
    /// Phi node for break values (if any). TODO: use for break-with-value.
    _break_phi: Option<PhiValue<'ctx>>,
}

/// LLVM code generator.
pub struct LLVMCodegen<'ctx> {
    /// The LLVM context.
    pub context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    interner: &'ctx StringInterner,
}

impl<'ctx> LLVMCodegen<'ctx> {
    /// Create a new LLVM code generator.
    pub fn new(context: &'ctx Context, interner: &'ctx StringInterner, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        Self {
            context,
            module,
            builder,
            interner,
        }
    }

    /// Get the LLVM module.
    pub fn module(&self) -> &Module<'ctx> {
        &self.module
    }

    /// Print LLVM IR to string.
    pub fn print_to_string(&self) -> String {
        self.module.print_to_string().to_string()
    }

    /// Write LLVM IR to a file.
    pub fn write_ir_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        self.module.print_to_file(path).map_err(|e| e.to_string())
    }

    /// Write object file.
    pub fn write_object_file(&self, path: &std::path::Path) -> Result<(), String> {
        use inkwell::targets::{
            CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
        };

        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| e.to_string())?;

        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).map_err(|e| e.to_string())?;

        let cpu = TargetMachine::get_host_cpu_name();
        let features = TargetMachine::get_host_cpu_features();

        let target_machine = target
            .create_target_machine(
                &triple,
                cpu.to_str().map_err(|e| e.to_string())?,
                features.to_str().map_err(|e| e.to_string())?,
                OptimizationLevel::Default,
                RelocMode::PIC,
                CodeModel::Default,
            )
            .ok_or("Failed to create target machine")?;

        target_machine
            .write_to_file(&self.module, FileType::Object, path)
            .map_err(|e| e.to_string())
    }

    // ========================================================================
    // Type Mapping
    // ========================================================================

    /// Map a Ori TypeId to an LLVM type.
    fn llvm_type(&self, type_id: TypeId) -> BasicTypeEnum<'ctx> {
        match type_id {
            TypeId::INT => self.context.i64_type().into(),
            TypeId::FLOAT => self.context.f64_type().into(),
            TypeId::BOOL => self.context.bool_type().into(),
            TypeId::CHAR => self.context.i32_type().into(), // Unicode codepoint
            TypeId::BYTE => self.context.i8_type().into(),
            // For now, other types become opaque pointers
            _ => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
        }
    }

    /// Map a Ori TypeId to an LLVM metadata type (for function params).
    fn llvm_metadata_type(&self, type_id: TypeId) -> BasicMetadataTypeEnum<'ctx> {
        self.llvm_type(type_id).into()
    }

    // ========================================================================
    // Function Codegen
    // ========================================================================

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

    /// Get a default value for a type.
    fn default_value(&self, type_id: TypeId) -> BasicValueEnum<'ctx> {
        match type_id {
            TypeId::INT => self.context.i64_type().const_int(0, false).into(),
            TypeId::FLOAT => self.context.f64_type().const_float(0.0).into(),
            TypeId::BOOL => self.context.bool_type().const_int(0, false).into(),
            TypeId::CHAR => self.context.i32_type().const_int(0, false).into(),
            TypeId::BYTE => self.context.i8_type().const_int(0, false).into(),
            _ => self.context.ptr_type(inkwell::AddressSpace::default()).const_null().into(),
        }
    }

    // ========================================================================
    // Expression Codegen
    // ========================================================================

    /// Compile an expression.
    fn compile_expr(
        &self,
        id: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let expr = arena.get_expr(id);
        let type_id = expr_types.get(id.index()).copied().unwrap_or(TypeId::INFER);

        match &expr.kind {
            // Literals
            ExprKind::Int(n) => {
                Some(self.context.i64_type().const_int(*n as u64, true).into())
            }

            ExprKind::Float(bits) => {
                Some(self.context.f64_type().const_float(f64::from_bits(*bits)).into())
            }

            ExprKind::Bool(b) => {
                Some(self.context.bool_type().const_int(u64::from(*b), false).into())
            }

            ExprKind::Char(c) => {
                Some(self.context.i32_type().const_int(u64::from(*c), false).into())
            }

            // String literal
            ExprKind::String(name) => {
                self.compile_string(*name)
            }

            // Variables
            ExprKind::Ident(name) => {
                locals.get(name).copied()
            }

            // Binary operations
            ExprKind::Binary { op, left, right } => {
                let lhs = self.compile_expr(*left, arena, expr_types, locals, function, loop_ctx)?;
                let rhs = self.compile_expr(*right, arena, expr_types, locals, function, loop_ctx)?;
                self.compile_binary_op(*op, lhs, rhs, type_id)
            }

            // Unary operations
            ExprKind::Unary { op, operand } => {
                let val = self.compile_expr(*operand, arena, expr_types, locals, function, loop_ctx)?;
                self.compile_unary_op(*op, val, type_id)
            }

            // Let binding
            ExprKind::Let { pattern, init, .. } => {
                self.compile_let(pattern, *init, arena, expr_types, locals, function, loop_ctx)
            }

            // If/else expression
            ExprKind::If { cond, then_branch, else_branch } => {
                self.compile_if(
                    *cond,
                    *then_branch,
                    *else_branch,
                    type_id,
                    arena,
                    expr_types,
                    locals,
                    function,
                    loop_ctx,
                )
            }

            // Loop
            ExprKind::Loop { body } => {
                self.compile_loop(*body, type_id, arena, expr_types, locals, function)
            }

            // Break
            ExprKind::Break(value) => {
                self.compile_break(*value, arena, expr_types, locals, function, loop_ctx)
            }

            // Continue
            ExprKind::Continue => {
                self.compile_continue(loop_ctx)
            }

            // Tuple
            ExprKind::Tuple(range) => {
                self.compile_tuple(*range, arena, expr_types, locals, function, loop_ctx)
            }

            // Struct literal
            ExprKind::Struct { name, fields } => {
                self.compile_struct(*name, *fields, arena, expr_types, locals, function, loop_ctx)
            }

            // Field access
            ExprKind::Field { receiver, field } => {
                self.compile_field_access(*receiver, *field, arena, expr_types, locals, function, loop_ctx)
            }

            // Option type constructors
            ExprKind::Some(inner) => {
                self.compile_some(*inner, type_id, arena, expr_types, locals, function, loop_ctx)
            }

            ExprKind::None => {
                self.compile_none(type_id)
            }

            // Result type constructors
            ExprKind::Ok(inner) => {
                self.compile_ok(*inner, type_id, arena, expr_types, locals, function, loop_ctx)
            }

            ExprKind::Err(inner) => {
                self.compile_err(*inner, type_id, arena, expr_types, locals, function, loop_ctx)
            }

            // Match expression
            ExprKind::Match { scrutinee, arms } => {
                self.compile_match(*scrutinee, *arms, type_id, arena, expr_types, locals, function, loop_ctx)
            }

            // Function call (positional args)
            ExprKind::Call { func, args } => {
                self.compile_call(*func, *args, arena, expr_types, locals, function, loop_ctx)
            }

            // Function call (named args) - treat same as positional for now
            ExprKind::CallNamed { func, args } => {
                self.compile_call_named(*func, *args, arena, expr_types, locals, function, loop_ctx)
            }

            // Unit
            ExprKind::Unit => None,

            // Config variable (compile-time constant)
            ExprKind::Config(name) => {
                self.compile_config(*name, locals)
            }

            // Self reference (for recursion)
            ExprKind::SelfRef => {
                // Return pointer to current function
                Some(function.as_global_value().as_pointer_value().into())
            }

            // Function reference: @name
            ExprKind::FunctionRef(name) => {
                self.compile_function_ref(*name)
            }

            // Hash length: # (refers to length in index context)
            ExprKind::HashLength => {
                // This should be resolved during evaluation to the actual length
                // For now, return 0 as placeholder (context-dependent)
                Some(self.context.i64_type().const_int(0, false).into())
            }

            // Duration literal: 100ms, 5s
            ExprKind::Duration { value, unit } => {
                self.compile_duration(*value, *unit)
            }

            // Size literal: 4kb, 10mb
            ExprKind::Size { value, unit } => {
                self.compile_size(*value, *unit)
            }

            // Block: { stmts; result }
            ExprKind::Block { stmts, result } => {
                self.compile_block(*stmts, *result, arena, expr_types, locals, function, loop_ctx)
            }

            // Return from function
            ExprKind::Return(value) => {
                self.compile_return(*value, arena, expr_types, locals, function, loop_ctx)
            }

            // Assignment: target = value
            ExprKind::Assign { target, value } => {
                self.compile_assign(*target, *value, arena, expr_types, locals, function, loop_ctx)
            }

            // List literal: [a, b, c]
            ExprKind::List(range) => {
                self.compile_list(*range, arena, expr_types, locals, function, loop_ctx)
            }

            // Map literal: {k: v, ...}
            ExprKind::Map(entries) => {
                self.compile_map(*entries, arena, expr_types, locals, function, loop_ctx)
            }

            // Range: start..end
            ExprKind::Range { start, end, inclusive } => {
                self.compile_range(*start, *end, *inclusive, arena, expr_types, locals, function, loop_ctx)
            }

            // Index access: receiver[index]
            ExprKind::Index { receiver, index } => {
                self.compile_index(*receiver, *index, arena, expr_types, locals, function, loop_ctx)
            }

            // Method call: receiver.method(args)
            ExprKind::MethodCall { receiver, method, args } => {
                self.compile_method_call(*receiver, *method, *args, arena, expr_types, locals, function, loop_ctx)
            }

            // Method call with named args
            ExprKind::MethodCallNamed { receiver, method, args } => {
                self.compile_method_call_named(*receiver, *method, *args, arena, expr_types, locals, function, loop_ctx)
            }

            // Lambda: params -> body
            ExprKind::Lambda { params, ret_ty: _, body } => {
                self.compile_lambda(*params, *body, arena, expr_types, locals, function)
            }

            // For loop: for x in iter do/yield body
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                self.compile_for(*binding, *iter, *guard, *body, *is_yield, type_id, arena, expr_types, locals, function)
            }

            // Await (no-op for sync runtime)
            ExprKind::Await(inner) => {
                // Just compile the inner expression - no async support yet
                self.compile_expr(*inner, arena, expr_types, locals, function, loop_ctx)
            }

            // Try expression: expr?
            ExprKind::Try(inner) => {
                self.compile_try(*inner, arena, expr_types, locals, function, loop_ctx)
            }

            // With capability provision
            ExprKind::WithCapability { capability: _, provider: _, body } => {
                // For now, just compile the body (capability system not yet implemented)
                self.compile_expr(*body, arena, expr_types, locals, function, loop_ctx)
            }

            // Sequential expression patterns (run, try, match)
            ExprKind::FunctionSeq(seq) => {
                self.compile_function_seq(seq, type_id, arena, expr_types, locals, function, loop_ctx)
            }

            // Named expression patterns (recurse, parallel, etc.)
            ExprKind::FunctionExp(exp) => {
                self.compile_function_exp(exp, type_id, arena, expr_types, locals, function, loop_ctx)
            }

            // Error placeholder - should not be reached at runtime
            ExprKind::Error => None,
        }
    }

    /// Compile a let binding.
    fn compile_let(
        &self,
        pattern: &BindingPattern,
        init: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the initializer
        let value = self.compile_expr(init, arena, expr_types, locals, function, loop_ctx)?;

        // Bind the value based on the pattern
        match pattern {
            BindingPattern::Name(name) => {
                locals.insert(*name, value);
            }
            BindingPattern::Wildcard => {
                // Discard the value
            }
            _ => {
                // TODO: destructuring patterns
            }
        }

        // Let bindings produce the bound value
        Some(value)
    }

    /// Compile an if/else expression.
    fn compile_if(
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
    fn build_phi(
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

    // ========================================================================
    // Phase 3: Loops
    // ========================================================================

    /// Compile a loop expression.
    fn compile_loop(
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
    fn compile_break(
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
    fn compile_continue(
        &self,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let ctx = loop_ctx?;

        // Jump back to header
        self.builder.build_unconditional_branch(ctx.header).ok()?;

        // Continue doesn't produce a value
        None
    }

    // ========================================================================
    // Phase 4: Compound Types
    // ========================================================================

    /// Compile a tuple expression.
    fn compile_tuple(
        &self,
        range: ori_ir::ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get tuple elements
        let element_ids = arena.get_expr_list(range);

        if element_ids.is_empty() {
            // Empty tuple = unit
            return None;
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut types: Vec<BasicTypeEnum<'ctx>> = Vec::new();

        for &elem_id in element_ids {
            if let Some(val) = self.compile_expr(elem_id, arena, expr_types, locals, function, loop_ctx) {
                types.push(val.get_type());
                values.push(val);
            } else {
                // Element doesn't produce a value (unit element)
                // Skip for now, or could use void placeholder
                return None;
            }
        }

        // Create a struct type for the tuple
        let struct_type = self.context.struct_type(&types, false);

        // Build the struct value
        let mut struct_val = struct_type.get_undef();
        for (i, val) in values.into_iter().enumerate() {
            struct_val = self.builder
                .build_insert_value(struct_val, val, i as u32, "tuple_elem")
                .ok()?
                .into_struct_value();
        }

        Some(struct_val.into())
    }

    /// Compile a struct literal.
    ///
    /// For now, structs are represented as LLVM struct types with fields
    /// in declaration order. We need type information to know field order.
    fn compile_struct(
        &self,
        _name: Name,
        fields: ori_ir::ast::FieldInitRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get field initializers
        let field_inits = arena.get_field_inits(fields);

        if field_inits.is_empty() {
            // Empty struct = unit-like
            return None;
        }

        // Compile each field value
        let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut types: Vec<BasicTypeEnum<'ctx>> = Vec::new();

        for init in field_inits {
            // Get the value - either explicit or shorthand (variable with same name)
            let value_id = init.value.unwrap_or_else(|| {
                // Shorthand: `Point { x, y }` - look up variable `x`
                // We need to find an expression for this name
                // For now, assume it's in locals
                panic!("Struct shorthand not yet supported in LLVM backend")
            });

            if let Some(val) = self.compile_expr(value_id, arena, expr_types, locals, function, loop_ctx) {
                types.push(val.get_type());
                values.push(val);
            } else {
                return None;
            }
        }

        // Create a struct type
        let struct_type = self.context.struct_type(&types, false);

        // Build the struct value
        let mut struct_val = struct_type.get_undef();
        for (i, val) in values.into_iter().enumerate() {
            struct_val = self.builder
                .build_insert_value(struct_val, val, i as u32, "struct_field")
                .ok()?
                .into_struct_value();
        }

        Some(struct_val.into())
    }

    /// Compile field access on a struct.
    ///
    /// For now, we need to know the field index from the type system.
    /// This is a simplified version that assumes field order matches init order.
    fn compile_field_access(
        &self,
        receiver: ExprId,
        field: Name,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the receiver (the struct value)
        let struct_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;

        // Get as struct value
        let struct_val = struct_val.into_struct_value();

        // For proper field access, we need the type definition to know field indices.
        // For now, use a heuristic: look up field name to get index.
        // This is a placeholder - real implementation needs type context.

        // Get field name for error messages
        let field_name = self.interner.lookup(field);

        // Try common field names (x=0, y=1, z=2, etc.)
        // This is a hack - real implementation should use type info
        let field_index = match field_name {
            "x" | "first" | "0" | "a" => 0,
            "y" | "second" | "1" | "b" => 1,
            "z" | "third" | "2" | "c" => 2,
            "w" | "fourth" | "3" | "d" => 3,
            _ => {
                // Try to parse as number
                field_name.parse::<u32>().unwrap_or(0)
            }
        };

        // Extract the field value
        self.builder
            .build_extract_value(struct_val, field_index, &format!("field_{field_name}"))
            .ok()
    }

    // ========================================================================
    // Phase 5: Option/Result (Tagged Unions)
    // ========================================================================

    /// Create an Option type (tag i8 + payload).
    ///
    /// Layout: { i8 tag, T value }
    /// - tag = 0: None
    /// - tag = 1: Some(value)
    fn option_type(&self, payload_type: BasicTypeEnum<'ctx>) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[self.context.i8_type().into(), payload_type],
            false,
        )
    }

    /// Create a Result type (tag i8 + payload).
    ///
    /// Layout: { i8 tag, max(T, E) value }
    /// - tag = 0: Ok(value)
    /// - tag = 1: Err(value)
    ///
    /// For simplicity, we use the same payload type for both Ok and Err.
    /// A more sophisticated implementation would use a union.
    fn result_type(&self, payload_type: BasicTypeEnum<'ctx>) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[self.context.i8_type().into(), payload_type],
            false,
        )
    }

    /// Compile Some(value).
    fn compile_some(
        &self,
        inner: ExprId,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the inner value
        let inner_val = self.compile_expr(inner, arena, expr_types, locals, function, loop_ctx)?;

        // Create Option type with this payload type
        let opt_type = self.option_type(inner_val.get_type());

        // Build the struct: { tag = 1, value = inner_val }
        let tag = self.context.i8_type().const_int(1, false); // 1 = Some

        let mut struct_val = opt_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "some_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, inner_val, 1, "some_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile None.
    fn compile_none(&self, type_id: TypeId) -> Option<BasicValueEnum<'ctx>> {
        // For None, we need to know the inner type to create the right struct.
        // Since we don't have that info easily, use i64 as default payload.
        let payload_type = self.llvm_type(type_id);

        // If we got a pointer type (unknown), use i64 as default
        let payload_type = if payload_type.is_pointer_type() {
            self.context.i64_type().into()
        } else {
            payload_type
        };

        let opt_type = self.option_type(payload_type);

        // Build the struct: { tag = 0, value = undef }
        let tag = self.context.i8_type().const_int(0, false); // 0 = None
        let default_val = self.default_value_for_type(payload_type);

        let mut struct_val = opt_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "none_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, default_val, 1, "none_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile Ok(value).
    fn compile_ok(
        &self,
        inner: Option<ExprId>,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get the inner value (or use unit if None)
        let inner_val = if let Some(inner_id) = inner {
            self.compile_expr(inner_id, arena, expr_types, locals, function, loop_ctx)?
        } else {
            // Ok() with no value - use a dummy i64
            self.context.i64_type().const_int(0, false).into()
        };

        // Create Result type with this payload type
        let result_type = self.result_type(inner_val.get_type());

        // Build the struct: { tag = 0, value = inner_val }
        let tag = self.context.i8_type().const_int(0, false); // 0 = Ok

        let mut struct_val = result_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "ok_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, inner_val, 1, "ok_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Compile Err(value).
    fn compile_err(
        &self,
        inner: Option<ExprId>,
        _type_id: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Get the inner value (or use unit if None)
        let inner_val = if let Some(inner_id) = inner {
            self.compile_expr(inner_id, arena, expr_types, locals, function, loop_ctx)?
        } else {
            // Err() with no value - use a dummy i64
            self.context.i64_type().const_int(0, false).into()
        };

        // Create Result type with this payload type
        let result_type = self.result_type(inner_val.get_type());

        // Build the struct: { tag = 1, value = inner_val }
        let tag = self.context.i8_type().const_int(1, false); // 1 = Err

        let mut struct_val = result_type.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, tag, 0, "err_tag")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, inner_val, 1, "err_val")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    // ========================================================================
    // Phase 5: Match Expressions
    // ========================================================================

    /// Compile a match expression.
    ///
    /// Match expressions are compiled as a series of conditional branches:
    /// 1. Evaluate scrutinee
    /// 2. For each arm: check pattern, if match execute body, else try next arm
    /// 3. Use phi node to merge results from all arms
    fn compile_match(
        &self,
        scrutinee: ExprId,
        arms: ArmRange,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the scrutinee
        let scrutinee_val = self.compile_expr(scrutinee, arena, expr_types, locals, function, loop_ctx)?;

        // Get the arms
        let arms = arena.get_arms(arms);

        if arms.is_empty() {
            // No arms - return default value
            return if result_type == TypeId::VOID {
                None
            } else {
                Some(self.default_value(result_type))
            };
        }

        // Create merge block for all arms
        let merge_bb = self.context.append_basic_block(function, "match_merge");

        // Track incoming values for the phi node
        let mut incoming: Vec<(BasicValueEnum<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        // Process each arm
        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;

            // Create blocks for this arm
            let arm_body_bb = self.context.append_basic_block(function, &format!("match_arm_{i}"));
            let next_bb = if is_last {
                merge_bb // Last arm falls through to merge (or unreachable)
            } else {
                self.context.append_basic_block(function, &format!("match_next_{i}"))
            };

            // Check the pattern
            let matches = self.compile_pattern_check(&arm.pattern, scrutinee_val, arena, expr_types);

            if let Some(cond) = matches {
                // Conditional branch based on pattern match
                self.builder.build_conditional_branch(cond, arm_body_bb, next_bb).ok()?;
            } else {
                // Pattern always matches (wildcard, binding)
                self.builder.build_unconditional_branch(arm_body_bb).ok()?;
            }

            // Compile arm body
            self.builder.position_at_end(arm_body_bb);

            // Bind pattern variables
            self.bind_pattern_vars(&arm.pattern, scrutinee_val, locals);

            // Compile guard if present
            if let Some(guard) = arm.guard {
                let guard_val = self.compile_expr(guard, arena, expr_types, locals, function, loop_ctx)?;
                let guard_bool = guard_val.into_int_value();

                // If guard fails, go to next arm
                let guard_pass_bb = self.context.append_basic_block(function, &format!("guard_pass_{i}"));
                self.builder.build_conditional_branch(guard_bool, guard_pass_bb, next_bb).ok()?;
                self.builder.position_at_end(guard_pass_bb);
            }

            // Compile arm body
            let body_val = self.compile_expr(arm.body, arena, expr_types, locals, function, loop_ctx);

            // Jump to merge block
            let arm_exit_bb = self.builder.get_insert_block()?;
            if arm_exit_bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb).ok()?;
            }

            // Track incoming value for phi
            if let Some(val) = body_val {
                incoming.push((val, arm_exit_bb));
            }

            // Position at next arm's check block
            if !is_last {
                self.builder.position_at_end(next_bb);
            }
        }

        // Build merge block
        self.builder.position_at_end(merge_bb);

        // Create phi node if we have values
        if incoming.is_empty() {
            None
        } else if incoming.len() == 1 {
            // Single arm - just use the value
            Some(incoming[0].0)
        } else {
            // Multiple arms - need phi node
            let phi = self.build_phi(result_type, &incoming)?;
            Some(phi.as_basic_value())
        }
    }

    /// Check if a pattern matches the scrutinee.
    /// Returns Some(condition) if a runtime check is needed, None if always matches.
    fn compile_pattern_check(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ctx>,
        arena: &ExprArena,
        _expr_types: &[TypeId],
    ) -> Option<inkwell::values::IntValue<'ctx>> {
        match pattern {
            MatchPattern::Wildcard | MatchPattern::Binding(_) => {
                // Always matches
                None
            }

            MatchPattern::Literal(expr_id) => {
                // Compare with literal value
                let literal_expr = arena.get_expr(*expr_id);
                match &literal_expr.kind {
                    ExprKind::Int(n) => {
                        let expected = self.context.i64_type().const_int(*n as u64, true);
                        let actual = scrutinee.into_int_value();
                        Some(self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            actual,
                            expected,
                            "lit_match",
                        ).ok()?)
                    }
                    ExprKind::Bool(b) => {
                        let expected = self.context.bool_type().const_int(u64::from(*b), false);
                        let actual = scrutinee.into_int_value();
                        Some(self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            actual,
                            expected,
                            "bool_match",
                        ).ok()?)
                    }
                    _ => {
                        // Unsupported literal type - treat as always match for now
                        None
                    }
                }
            }

            MatchPattern::Variant { name, inner: _ } => {
                // For Option/Result, check the tag
                // Scrutinee should be a struct { i8 tag, T value }
                let struct_val = match scrutinee {
                    BasicValueEnum::StructValue(sv) => sv,
                    _ => return None, // Can't match variant on non-struct
                };

                // Extract tag
                let tag = self.builder.build_extract_value(struct_val, 0, "tag").ok()?;
                let tag_int = tag.into_int_value();

                // Get expected tag based on variant name
                let variant_name = self.interner.lookup(*name);
                let expected_tag = match variant_name {
                    "None" => 0,
                    "Some" => 1,
                    "Ok" => 0,
                    "Err" => 1,
                    _ => 0, // Unknown variant - assume tag 0
                };

                let expected = self.context.i8_type().const_int(expected_tag, false);
                Some(self.builder.build_int_compare(
                    inkwell::IntPredicate::EQ,
                    tag_int,
                    expected,
                    "variant_match",
                ).ok()?)
            }

            // Other patterns - treat as always match for now
            _ => None,
        }
    }

    /// Bind pattern variables to the scrutinee value.
    fn bind_pattern_vars(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ctx>,
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
    ) {
        match pattern {
            MatchPattern::Binding(name) => {
                locals.insert(*name, scrutinee);
            }

            MatchPattern::Variant { name: _, inner } => {
                // For variants like Some(x), extract the inner value and bind it
                if let Some(inner_pattern) = inner {
                    // Extract the payload from the tagged union
                    if let BasicValueEnum::StructValue(struct_val) = scrutinee {
                        if let Ok(payload) = self.builder.build_extract_value(struct_val, 1, "payload") {
                            self.bind_pattern_vars(inner_pattern, payload, locals);
                        }
                    }
                }
            }

            MatchPattern::At { name, pattern } => {
                // Bind the whole value to name, then process inner pattern
                locals.insert(*name, scrutinee);
                self.bind_pattern_vars(pattern, scrutinee, locals);
            }

            MatchPattern::Tuple(patterns) => {
                // Bind each tuple element
                if let BasicValueEnum::StructValue(struct_val) = scrutinee {
                    for (i, pat) in patterns.iter().enumerate() {
                        if let Ok(elem) = self.builder.build_extract_value(struct_val, i as u32, &format!("tuple_{i}")) {
                            self.bind_pattern_vars(pat, elem, locals);
                        }
                    }
                }
            }

            _ => {
                // Other patterns don't bind variables (Wildcard, Literal, etc.)
            }
        }
    }

    // ========================================================================
    // Phase 6: Function Calls
    // ========================================================================

    /// Compile a function call with positional arguments.
    fn compile_call(
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
            ExprKind::Ident(name) => *name,
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
    fn compile_call_named(
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
            ExprKind::Ident(name) => *name,
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

    // ========================================================================
    // Phase 7: Strings
    // ========================================================================

    /// Compile a string literal.
    ///
    /// Creates a global constant string and returns a pointer to it.
    /// For now, strings are represented as { i64 len, i8* data } structs.
    fn compile_string(&self, name: Name) -> Option<BasicValueEnum<'ctx>> {
        let string_content = self.interner.lookup(name);

        // Create a unique global name for this string based on a hash
        // (We can't use name.0 directly since it's private)
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        string_content.hash(&mut hasher);
        let global_name = format!(".str.{:x}", hasher.finish());

        // Check if we already have this string as a global
        if let Some(global) = self.module.get_global(&global_name) {
            // Return pointer to existing string data
            let ptr = global.as_pointer_value();

            // Create string struct { len, data_ptr }
            let len = self.context.i64_type().const_int(string_content.len() as u64, false);
            let string_struct = self.string_type();

            let mut struct_val = string_struct.get_undef();
            struct_val = self.builder
                .build_insert_value(struct_val, len, 0, "str_len")
                .ok()?
                .into_struct_value();
            struct_val = self.builder
                .build_insert_value(struct_val, ptr, 1, "str_data")
                .ok()?
                .into_struct_value();

            return Some(struct_val.into());
        }

        // Create a null-terminated string constant
        let string_bytes: Vec<u8> = string_content.bytes().chain(std::iter::once(0)).collect();
        let string_const = self.context.const_string(&string_bytes, false);

        // Create global variable for the string data
        let global = self.module.add_global(string_const.get_type(), None, &global_name);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_constant(true);
        global.set_initializer(&string_const);

        // Get pointer to the string data
        let ptr = global.as_pointer_value();

        // Create string struct { len, data_ptr }
        let len = self.context.i64_type().const_int(string_content.len() as u64, false);
        let string_struct = self.string_type();

        let mut struct_val = string_struct.get_undef();
        struct_val = self.builder
            .build_insert_value(struct_val, len, 0, "str_len")
            .ok()?
            .into_struct_value();
        struct_val = self.builder
            .build_insert_value(struct_val, ptr, 1, "str_data")
            .ok()?
            .into_struct_value();

        Some(struct_val.into())
    }

    /// Get the string type: { i64 len, i8* data }
    fn string_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Get a default value for an LLVM type.
    fn default_value_for_type(&self, ty: BasicTypeEnum<'ctx>) -> BasicValueEnum<'ctx> {
        match ty {
            BasicTypeEnum::IntType(t) => t.const_int(0, false).into(),
            BasicTypeEnum::FloatType(t) => t.const_float(0.0).into(),
            BasicTypeEnum::PointerType(t) => t.const_null().into(),
            BasicTypeEnum::StructType(t) => t.get_undef().into(),
            BasicTypeEnum::ArrayType(t) => t.get_undef().into(),
            BasicTypeEnum::VectorType(t) => t.get_undef().into(),
            BasicTypeEnum::ScalableVectorType(t) => t.get_undef().into(),
        }
    }

    /// Compile a binary operation.
    fn compile_binary_op(
        &self,
        op: BinaryOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
        _result_type: TypeId,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Determine the operand type - both must be the same type for binary ops
        let lhs_is_struct = matches!(lhs, BasicValueEnum::StructValue(_));
        let rhs_is_struct = matches!(rhs, BasicValueEnum::StructValue(_));
        let both_struct = lhs_is_struct && rhs_is_struct;
        let is_float = matches!(lhs, BasicValueEnum::FloatValue(_));
        // If one is struct and the other isn't, we can't do the operation
        let is_struct = lhs_is_struct || rhs_is_struct;

        match op {
            // Arithmetic
            BinaryOp::Add => {
                if both_struct {
                    // String concatenation - call runtime function
                    self.compile_str_concat(lhs, rhs)
                } else if is_struct {
                    // Mixed types - not supported
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_add(l, r, "fadd").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_add(l, r, "iadd").ok()?.into())
                }
            }

            BinaryOp::Sub => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_sub(l, r, "fsub").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_sub(l, r, "isub").ok()?.into())
                }
            }

            BinaryOp::Mul => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_mul(l, r, "fmul").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_mul(l, r, "imul").ok()?.into())
                }
            }

            BinaryOp::Div => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_div(l, r, "fdiv").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_signed_div(l, r, "idiv").ok()?.into())
                }
            }

            BinaryOp::Mod => {
                if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_rem(l, r, "frem").ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_signed_rem(l, r, "irem").ok()?.into())
                }
            }

            // Comparisons
            BinaryOp::Eq => {
                if both_struct {
                    // String equality - call runtime function
                    self.compile_str_eq(lhs, rhs)
                } else if is_struct {
                    // Mixed types - not comparable
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OEQ, l, r, "feq"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::EQ, l, r, "ieq"
                    ).ok()?.into())
                }
            }

            BinaryOp::NotEq => {
                if both_struct {
                    // String inequality - call runtime function
                    self.compile_str_ne(lhs, rhs)
                } else if is_struct {
                    // Mixed types - not comparable
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::ONE, l, r, "fne"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::NE, l, r, "ine"
                    ).ok()?.into())
                }
            }

            BinaryOp::Lt => {
                if is_struct {
                    // String ordering not yet implemented
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OLT, l, r, "flt"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SLT, l, r, "ilt"
                    ).ok()?.into())
                }
            }

            BinaryOp::LtEq => {
                if is_struct {
                    // String ordering not yet implemented
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OLE, l, r, "fle"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SLE, l, r, "ile"
                    ).ok()?.into())
                }
            }

            BinaryOp::Gt => {
                if is_struct {
                    // String ordering not yet implemented
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OGT, l, r, "fgt"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SGT, l, r, "igt"
                    ).ok()?.into())
                }
            }

            BinaryOp::GtEq => {
                if is_struct {
                    // String ordering not yet implemented
                    None
                } else if is_float {
                    let l = lhs.into_float_value();
                    let r = rhs.into_float_value();
                    Some(self.builder.build_float_compare(
                        inkwell::FloatPredicate::OGE, l, r, "fge"
                    ).ok()?.into())
                } else {
                    let l = lhs.into_int_value();
                    let r = rhs.into_int_value();
                    Some(self.builder.build_int_compare(
                        inkwell::IntPredicate::SGE, l, r, "ige"
                    ).ok()?.into())
                }
            }

            // Logical
            BinaryOp::And => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_and(l, r, "and").ok()?.into())
            }

            BinaryOp::Or => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_or(l, r, "or").ok()?.into())
            }

            // Bitwise
            BinaryOp::BitAnd => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_and(l, r, "bitand").ok()?.into())
            }

            BinaryOp::BitOr => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_or(l, r, "bitor").ok()?.into())
            }

            BinaryOp::BitXor => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_xor(l, r, "bitxor").ok()?.into())
            }

            BinaryOp::Shl => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_left_shift(l, r, "shl").ok()?.into())
            }

            BinaryOp::Shr => {
                let l = lhs.into_int_value();
                let r = rhs.into_int_value();
                Some(self.builder.build_right_shift(l, r, true, "shr").ok()?.into())
            }

            // Not yet implemented
            _ => None,
        }
    }

    /// Compile string concatenation by calling runtime function.
    fn compile_str_concat(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let str_concat = self.module.get_function("ori_str_concat")?;

        // Get the string struct type { i64, ptr }
        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        // Allocate stack space for lhs and rhs strings
        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        // Store the struct values
        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        // Call ori_str_concat(ptr, ptr) -> OriStr
        let result = self.builder.build_call(
            str_concat,
            &[lhs_ptr.into(), rhs_ptr.into()],
            "str_concat_result"
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile string equality by calling runtime function.
    fn compile_str_eq(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let str_eq = self.module.get_function("ori_str_eq")?;

        // Get the string struct type { i64, ptr }
        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        // Allocate stack space for lhs and rhs strings
        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        // Store the struct values
        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        // Call ori_str_eq(ptr, ptr) -> bool
        let result = self.builder.build_call(
            str_eq,
            &[lhs_ptr.into(), rhs_ptr.into()],
            "str_eq_result"
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile string inequality by calling runtime function.
    fn compile_str_ne(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let str_ne = self.module.get_function("ori_str_ne")?;

        // Get the string struct type { i64, ptr }
        let i64_type = self.context.i64_type();
        let ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let str_type = self.context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        // Allocate stack space for lhs and rhs strings
        let lhs_ptr = self.builder.build_alloca(str_type, "lhs_str").ok()?;
        let rhs_ptr = self.builder.build_alloca(str_type, "rhs_str").ok()?;

        // Store the struct values
        self.builder.build_store(lhs_ptr, lhs.into_struct_value()).ok()?;
        self.builder.build_store(rhs_ptr, rhs.into_struct_value()).ok()?;

        // Call ori_str_ne(ptr, ptr) -> bool
        let result = self.builder.build_call(
            str_ne,
            &[lhs_ptr.into(), rhs_ptr.into()],
            "str_ne_result"
        ).ok()?;

        result.try_as_basic_value().basic()
    }

    /// Compile a unary operation.
    fn compile_unary_op(
        &self,
        op: UnaryOp,
        val: BasicValueEnum<'ctx>,
        _result_type: TypeId,
    ) -> Option<BasicValueEnum<'ctx>> {
        match op {
            UnaryOp::Neg => {
                match val {
                    BasicValueEnum::IntValue(i) => {
                        Some(self.builder.build_int_neg(i, "neg").ok()?.into())
                    }
                    BasicValueEnum::FloatValue(f) => {
                        Some(self.builder.build_float_neg(f, "fneg").ok()?.into())
                    }
                    _ => None,
                }
            }

            UnaryOp::Not => {
                let i = val.into_int_value();
                Some(self.builder.build_not(i, "not").ok()?.into())
            }

            UnaryOp::BitNot => {
                let i = val.into_int_value();
                Some(self.builder.build_not(i, "bitnot").ok()?.into())
            }

            UnaryOp::Try => {
                // Try operator needs special handling (error propagation)
                // For now, just return the value
                Some(val)
            }
        }
    }

    // ========================================================================
    // Phase 1: Simple Literals & References
    // ========================================================================

    /// Compile a config variable reference.
    /// Config variables are compile-time constants stored in locals.
    fn compile_config(
        &self,
        name: Name,
        locals: &HashMap<Name, BasicValueEnum<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Config variables should be pre-populated in locals by the caller
        locals.get(&name).copied()
    }

    /// Compile a function reference (@name).
    fn compile_function_ref(&self, name: Name) -> Option<BasicValueEnum<'ctx>> {
        let fn_name = self.interner.lookup(name);
        let func = self.module.get_function(fn_name)?;
        Some(func.as_global_value().as_pointer_value().into())
    }

    /// Compile a duration literal.
    /// Durations are stored as i64 milliseconds.
    fn compile_duration(&self, value: u64, unit: DurationUnit) -> Option<BasicValueEnum<'ctx>> {
        let millis = unit.to_millis(value);
        Some(self.context.i64_type().const_int(millis, false).into())
    }

    /// Compile a size literal.
    /// Sizes are stored as i64 bytes.
    fn compile_size(&self, value: u64, unit: SizeUnit) -> Option<BasicValueEnum<'ctx>> {
        let bytes = unit.to_bytes(value);
        Some(self.context.i64_type().const_int(bytes, false).into())
    }

    // ========================================================================
    // Phase 2: Block, Return, Assign
    // ========================================================================

    /// Compile a block expression.
    fn compile_block(
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
    fn compile_return(
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
    fn compile_assign(
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
            ExprKind::Ident(name) => {
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

    // ========================================================================
    // Phase 3: Collections
    // ========================================================================

    /// Compile a list literal.
    /// Lists are represented as { i64 len, i64 cap, ptr data }.
    fn compile_list(
        &self,
        range: ExprRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let elements = arena.get_expr_list(range);

        if elements.is_empty() {
            // Empty list - return struct with zeros
            let list_type = self.list_type();
            let zero = self.context.i64_type().const_int(0, false);
            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

            let mut list_val = list_type.get_undef();
            list_val = self.builder.build_insert_value(list_val, zero, 0, "list_len").ok()?.into_struct_value();
            list_val = self.builder.build_insert_value(list_val, zero, 1, "list_cap").ok()?.into_struct_value();
            list_val = self.builder.build_insert_value(list_val, null_ptr, 2, "list_data").ok()?.into_struct_value();

            return Some(list_val.into());
        }

        // Compile each element
        let mut values: Vec<BasicValueEnum<'ctx>> = Vec::new();
        for &elem_id in elements {
            if let Some(val) = self.compile_expr(elem_id, arena, expr_types, locals, function, loop_ctx) {
                values.push(val);
            }
        }

        if values.is_empty() {
            return None;
        }

        // Get element type from first value
        let elem_type = values[0].get_type();
        let len = values.len() as u64;

        // Create array type for storage
        let array_type = elem_type.array_type(len as u32);

        // Allocate array on stack (for now - runtime would use heap)
        let array_ptr = self.builder.build_alloca(array_type, "list_storage").ok()?;

        // Store each element
        for (i, val) in values.iter().enumerate() {
            let indices = [
                self.context.i64_type().const_int(0, false),
                self.context.i64_type().const_int(i as u64, false),
            ];
            // SAFETY: GEP with constant indices into an array we just allocated
            #[allow(unsafe_code)]
            let elem_ptr = unsafe {
                self.builder.build_gep(array_type, array_ptr, &indices, "elem_ptr").ok()?
            };
            self.builder.build_store(elem_ptr, *val).ok()?;
        }

        // Create list struct
        let list_type = self.list_type();
        let len_val = self.context.i64_type().const_int(len, false);

        let mut list_val = list_type.get_undef();
        list_val = self.builder.build_insert_value(list_val, len_val, 0, "list_len").ok()?.into_struct_value();
        list_val = self.builder.build_insert_value(list_val, len_val, 1, "list_cap").ok()?.into_struct_value();
        list_val = self.builder.build_insert_value(list_val, array_ptr, 2, "list_data").ok()?.into_struct_value();

        Some(list_val.into())
    }

    /// Get the list type: { i64 len, i64 cap, ptr data }
    fn list_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Compile a map literal.
    fn compile_map(
        &self,
        entries: ori_ir::ast::MapEntryRange,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let map_entries = arena.get_map_entries(entries);

        if map_entries.is_empty() {
            // Empty map - return struct with zeros
            let map_type = self.map_type();
            let zero = self.context.i64_type().const_int(0, false);
            let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

            let mut map_val = map_type.get_undef();
            map_val = self.builder.build_insert_value(map_val, zero, 0, "map_len").ok()?.into_struct_value();
            map_val = self.builder.build_insert_value(map_val, zero, 1, "map_cap").ok()?.into_struct_value();
            map_val = self.builder.build_insert_value(map_val, null_ptr, 2, "map_keys").ok()?.into_struct_value();
            map_val = self.builder.build_insert_value(map_val, null_ptr, 3, "map_vals").ok()?.into_struct_value();

            return Some(map_val.into());
        }

        // Compile each key-value pair
        let mut keys: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut vals: Vec<BasicValueEnum<'ctx>> = Vec::new();

        for entry in map_entries {
            if let Some(key) = self.compile_expr(entry.key, arena, expr_types, locals, function, loop_ctx) {
                if let Some(val) = self.compile_expr(entry.value, arena, expr_types, locals, function, loop_ctx) {
                    keys.push(key);
                    vals.push(val);
                }
            }
        }

        if keys.is_empty() {
            return None;
        }

        let len = keys.len() as u64;

        // For simplicity, create a map struct with the length
        // A real implementation would use a hash table
        let map_type = self.map_type();
        let len_val = self.context.i64_type().const_int(len, false);
        let null_ptr = self.context.ptr_type(inkwell::AddressSpace::default()).const_null();

        let mut map_val = map_type.get_undef();
        map_val = self.builder.build_insert_value(map_val, len_val, 0, "map_len").ok()?.into_struct_value();
        map_val = self.builder.build_insert_value(map_val, len_val, 1, "map_cap").ok()?.into_struct_value();
        map_val = self.builder.build_insert_value(map_val, null_ptr, 2, "map_keys").ok()?.into_struct_value();
        map_val = self.builder.build_insert_value(map_val, null_ptr, 3, "map_vals").ok()?.into_struct_value();

        Some(map_val.into())
    }

    /// Get the map type: { i64 len, i64 cap, ptr keys, ptr vals }
    fn map_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
                self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            ],
            false,
        )
    }

    /// Compile a range expression.
    /// Ranges are represented as { i64 start, i64 end, i1 inclusive }.
    fn compile_range(
        &self,
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile start (default to 0)
        let start_val = if let Some(start_id) = start {
            self.compile_expr(start_id, arena, expr_types, locals, function, loop_ctx)?
                .into_int_value()
        } else {
            self.context.i64_type().const_int(0, false)
        };

        // Compile end (default to i64::MAX for unbounded)
        let end_val = if let Some(end_id) = end {
            self.compile_expr(end_id, arena, expr_types, locals, function, loop_ctx)?
                .into_int_value()
        } else {
            self.context.i64_type().const_int(i64::MAX as u64, false)
        };

        // Create range struct
        let range_type = self.range_type();
        let inclusive_val = self.context.bool_type().const_int(u64::from(inclusive), false);

        let mut range_val = range_type.get_undef();
        range_val = self.builder.build_insert_value(range_val, start_val, 0, "range_start").ok()?.into_struct_value();
        range_val = self.builder.build_insert_value(range_val, end_val, 1, "range_end").ok()?.into_struct_value();
        range_val = self.builder.build_insert_value(range_val, inclusive_val, 2, "range_incl").ok()?.into_struct_value();

        Some(range_val.into())
    }

    /// Get the range type: { i64 start, i64 end, i1 inclusive }
    fn range_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context.i64_type().into(),
                self.context.i64_type().into(),
                self.context.bool_type().into(),
            ],
            false,
        )
    }

    // ========================================================================
    // Phase 4: Index & Method Calls
    // ========================================================================

    /// Compile an index expression: receiver[index]
    fn compile_index(
        &self,
        receiver: ExprId,
        index: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx)?;
        let idx_val = self.compile_expr(index, arena, expr_types, locals, function, loop_ctx)?;

        // Handle different receiver types
        match recv_val {
            BasicValueEnum::StructValue(struct_val) => {
                // Could be a tuple - use index as field number
                let idx = idx_val.into_int_value();
                if let Some(const_idx) = idx.get_zero_extended_constant() {
                    self.builder
                        .build_extract_value(struct_val, const_idx as u32, "index")
                        .ok()
                } else {
                    // Dynamic index not supported for tuples
                    None
                }
            }
            _ => {
                // For lists/arrays, would need GEP or runtime call
                // Return None for now
                None
            }
        }
    }

    /// Compile a method call: receiver.method(args)
    fn compile_method_call(
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
    fn compile_method_call_named(
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

    // ========================================================================
    // Phase 5: Lambdas
    // ========================================================================

    /// Compile a lambda expression.
    /// Lambdas are compiled as closures: { fn_ptr, captures_ptr }.
    fn compile_lambda(
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

    // ========================================================================
    // Phase 6: For, Try, Await
    // ========================================================================

    /// Compile a for loop.
    #[allow(clippy::too_many_arguments)]
    fn compile_for(
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
    fn compile_try(
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

    // ========================================================================
    // Phase 7: Patterns (FunctionSeq, FunctionExp)
    // ========================================================================

    /// Compile a FunctionSeq (run, try, match).
    fn compile_function_seq(
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
    fn compile_function_exp(
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

    // ========================================================================
    // JIT Execution
    // ========================================================================

    /// JIT compile and execute a function that returns i64.
    ///
    /// # Safety
    /// This uses LLVM's JIT execution engine which requires unsafe code.
    /// The caller must ensure the function exists and has the correct signature.
    #[allow(unsafe_code)]
    pub fn jit_execute_i64(&self, fn_name: &str) -> Result<i64, String> {
        let ee = self.module
            .create_jit_execution_engine(OptimizationLevel::None)
            .map_err(|e| e.to_string())?;

        // SAFETY: We trust LLVM's JIT to correctly compile and execute the function.
        // The function must exist with signature () -> i64.
        unsafe {
            let func = ee
                .get_function::<unsafe extern "C" fn() -> i64>(fn_name)
                .map_err(|e| e.to_string())?;

            Ok(func.call())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::ast::Expr;

    #[test]
    fn test_simple_add() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create a simple function: fn add() -> i64 { 2 + 3 }
        let mut arena = ExprArena::new();

        // Build: 2 + 3
        let two = arena.alloc_expr(Expr {
            kind: ExprKind::Int(2),
            span: ori_ir::Span::new(0, 1),
        });
        let three = arena.alloc_expr(Expr {
            kind: ExprKind::Int(3),
            span: ori_ir::Span::new(0, 1),
        });
        let add_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: two,
                right: three,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_add");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            add_expr,
            &arena,
            &expr_types,
        );

        // Print IR for debugging
        println!("Generated LLVM IR:\n{}", codegen.print_to_string());

        // JIT execute
        let result = codegen.jit_execute_i64("test_add").expect("JIT failed");
        assert_eq!(result, 5);
    }

    #[test]
    fn test_function_with_params() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn add(a: int, b: int) -> int { a + b }
        let mut arena = ExprArena::new();

        let a_name = interner.intern("a");
        let b_name = interner.intern("b");

        let a_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(a_name),
            span: ori_ir::Span::new(0, 1),
        });
        let b_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(b_name),
            span: ori_ir::Span::new(0, 1),
        });
        let add_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: a_ref,
                right: b_ref,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("add");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[a_name, b_name],
            &[TypeId::INT, TypeId::INT],
            TypeId::INT,
            add_expr,
            &arena,
            &expr_types,
        );

        println!("Generated LLVM IR:\n{}", codegen.print_to_string());

        // We can't easily JIT a function with params without a wrapper,
        // but we can verify the IR is valid
        assert!(codegen.print_to_string().contains("define i64 @add(i64"));
    }

    // ========================================================================
    // Phase 2: Let Bindings
    // ========================================================================

    #[test]
    fn test_let_binding() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { let x = 10; let y = 20; x + y }
        let mut arena = ExprArena::new();

        // let x = 10
        let ten = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let x_name = interner.intern("x");
        let let_x = arena.alloc_expr(Expr {
            kind: ExprKind::Let {
                pattern: BindingPattern::Name(x_name),
                ty: None,
                init: ten,
                mutable: false,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // let y = 20
        let twenty = arena.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });
        let y_name = interner.intern("y");
        let _let_y = arena.alloc_expr(Expr {
            kind: ExprKind::Let {
                pattern: BindingPattern::Name(y_name),
                ty: None,
                init: twenty,
                mutable: false,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // x + y
        let x_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(x_name),
            span: ori_ir::Span::new(0, 1),
        });
        let y_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(y_name),
            span: ori_ir::Span::new(0, 1),
        });
        let _add_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: x_ref,
                right: y_ref,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // Create a sequence using FunctionSeq (run pattern)
        // For now, we'll simulate it by having a nested structure
        // Actually, we need to handle sequences properly. Let me use the Block pattern.
        // But Block uses StmtRange which needs more infrastructure.
        // For the test, let me manually thread the let bindings.

        // We need to compile this as a series of expressions where the final
        // one is the result. For now, let's do it inline in compile_function
        // by extending the test to manually compile each expression.

        // Actually, let me just test that a single let works first.
        // fn test() -> int { let x = 42; x }
        let forty_two = arena.alloc_expr(Expr {
            kind: ExprKind::Int(42),
            span: ori_ir::Span::new(0, 1),
        });
        let x_name2 = interner.intern("x2");
        let _let_x2 = arena.alloc_expr(Expr {
            kind: ExprKind::Let {
                pattern: BindingPattern::Name(x_name2),
                ty: None,
                init: forty_two,
                mutable: false,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // For this test, we'll verify the IR contains the expected structure
        let fn_name = interner.intern("test_let");
        // The let binding returns the value, so the body is just the let
        // which should return 42
        let expr_types = vec![
            TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT,
            TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT,
        ];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            let_x, // Use the first let which returns the value
            &arena,
            &expr_types,
        );

        println!("Let Binding IR:\n{}", codegen.print_to_string());

        // JIT execute - let x = 10 returns 10
        let result = codegen.jit_execute_i64("test_let").expect("JIT failed");
        assert_eq!(result, 10);
    }

    // ========================================================================
    // Phase 2: If/Else Expressions
    // ========================================================================

    #[test]
    fn test_if_else() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { if true then 10 else 20 }
        let mut arena = ExprArena::new();

        let cond = arena.alloc_expr(Expr {
            kind: ExprKind::Bool(true),
            span: ori_ir::Span::new(0, 1),
        });
        let then_val = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let else_val = arena.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });
        let if_expr = arena.alloc_expr(Expr {
            kind: ExprKind::If {
                cond,
                then_branch: then_val,
                else_branch: Some(else_val),
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_if_true");
        let expr_types = vec![TypeId::BOOL, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            if_expr,
            &arena,
            &expr_types,
        );

        println!("If/Else IR:\n{}", codegen.print_to_string());

        // JIT execute - if true then 10 else 20 = 10
        let result = codegen.jit_execute_i64("test_if_true").expect("JIT failed");
        assert_eq!(result, 10);
    }

    #[test]
    fn test_if_else_false() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { if false then 10 else 20 }
        let mut arena = ExprArena::new();

        let cond = arena.alloc_expr(Expr {
            kind: ExprKind::Bool(false),
            span: ori_ir::Span::new(0, 1),
        });
        let then_val = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let else_val = arena.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });
        let if_expr = arena.alloc_expr(Expr {
            kind: ExprKind::If {
                cond,
                then_branch: then_val,
                else_branch: Some(else_val),
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_if_false");
        let expr_types = vec![TypeId::BOOL, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            if_expr,
            &arena,
            &expr_types,
        );

        // JIT execute - if false then 10 else 20 = 20
        let result = codegen.jit_execute_i64("test_if_false").expect("JIT failed");
        assert_eq!(result, 20);
    }

    // ========================================================================
    // Phase 3: Loops
    // ========================================================================

    #[test]
    fn test_loop_with_break() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int {
        //     let mut x = 0
        //     loop {
        //         x = x + 1
        //         if x == 5 then break else ()
        //     }
        //     x
        // }
        // But since we don't have assignment or mutable vars fully working,
        // let's test a simpler case: loop { break } which should just exit
        // The result type for void loop is void, so let's test IR generation.

        let mut arena = ExprArena::new();

        // break expression
        let break_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Break(None),
            span: ori_ir::Span::new(0, 1),
        });

        // loop { break }
        let loop_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Loop { body: break_expr },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_loop");
        let expr_types = vec![TypeId::VOID, TypeId::VOID];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::VOID,
            loop_expr,
            &arena,
            &expr_types,
        );

        println!("Loop IR:\n{}", codegen.print_to_string());

        // Verify IR contains loop structure
        let ir = codegen.print_to_string();
        assert!(ir.contains("loop_header"));
        assert!(ir.contains("loop_body"));
        assert!(ir.contains("loop_exit"));
    }

    #[test]
    fn test_loop_ir_structure() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Test that a loop with conditional break generates proper control flow:
        // fn test() -> int {
        //     loop {
        //         if true then break else ()
        //     }
        //     42
        // }
        // Simplified: since we can't easily sequence expressions,
        // just verify the loop + break structure is correct

        let mut arena = ExprArena::new();

        // true condition
        let cond = arena.alloc_expr(Expr {
            kind: ExprKind::Bool(true),
            span: ori_ir::Span::new(0, 1),
        });

        // break
        let break_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Break(None),
            span: ori_ir::Span::new(0, 1),
        });

        // unit
        let unit = arena.alloc_expr(Expr {
            kind: ExprKind::Unit,
            span: ori_ir::Span::new(0, 1),
        });

        // if true then break else ()
        let if_expr = arena.alloc_expr(Expr {
            kind: ExprKind::If {
                cond,
                then_branch: break_expr,
                else_branch: Some(unit),
            },
            span: ori_ir::Span::new(0, 1),
        });

        // loop { if true then break else () }
        let loop_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Loop { body: if_expr },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_loop_cond");
        let expr_types = vec![
            TypeId::BOOL, TypeId::VOID, TypeId::VOID, TypeId::VOID, TypeId::VOID,
        ];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::VOID,
            loop_expr,
            &arena,
            &expr_types,
        );

        println!("Loop with conditional IR:\n{}", codegen.print_to_string());

        // Verify proper branching
        let ir = codegen.print_to_string();
        assert!(ir.contains("br i1")); // Conditional branch
        assert!(ir.contains("br label")); // Unconditional branch to exit
    }

    // ========================================================================
    // Phase 4: Tuples
    // ========================================================================

    #[test]
    fn test_tuple_creation() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> (int, int) { (10, 20) }
        let mut arena = ExprArena::new();

        // Tuple elements
        let elem1 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let elem2 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });

        // Allocate the tuple elements in the arena's expr_list
        let range = arena.alloc_expr_list([elem1, elem2]);

        // Create tuple expression
        let tuple_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Tuple(range),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_tuple");
        // The result type would be a tuple, but we're using opaque ptr for now
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Placeholder - we'd need a tuple TypeId
            tuple_expr,
            &arena,
            &expr_types,
        );

        println!("Tuple IR:\n{}", codegen.print_to_string());

        // Verify IR contains struct type
        // Note: LLVM may optimize insertvalue to a constant struct
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i64, i64 }")); // Struct type for tuple
        assert!(ir.contains("i64 10")); // First element
        assert!(ir.contains("i64 20")); // Second element
    }

    // ========================================================================
    // Phase 5: Structs
    // ========================================================================

    #[test]
    fn test_struct_creation() {
        use ori_ir::ast::FieldInit;

        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> { x: int, y: int } { Point { x: 10, y: 20 } }
        let mut arena = ExprArena::new();

        // Field values
        let val_x = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let val_y = arena.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });

        // Field initializers
        let x_name = interner.intern("x");
        let y_name = interner.intern("y");

        let field_range = arena.alloc_field_inits([
            FieldInit {
                name: x_name,
                value: Some(val_x),
                span: ori_ir::Span::new(0, 1),
            },
            FieldInit {
                name: y_name,
                value: Some(val_y),
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        // Struct literal: Point { x: 10, y: 20 }
        let point_name = interner.intern("Point");
        let struct_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Struct {
                name: point_name,
                fields: field_range,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_struct");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Placeholder
            struct_expr,
            &arena,
            &expr_types,
        );

        println!("Struct IR:\n{}", codegen.print_to_string());

        // Verify IR contains struct construction
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i64, i64 }")); // Struct type
    }

    #[test]
    fn test_field_access() {
        use ori_ir::ast::FieldInit;

        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { Point { x: 10, y: 20 }.x }
        let mut arena = ExprArena::new();

        // Build struct
        let val_x = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let val_y = arena.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });

        let x_name = interner.intern("x");
        let y_name = interner.intern("y");

        let field_range = arena.alloc_field_inits([
            FieldInit {
                name: x_name,
                value: Some(val_x),
                span: ori_ir::Span::new(0, 1),
            },
            FieldInit {
                name: y_name,
                value: Some(val_y),
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        let point_name = interner.intern("Point");
        let struct_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Struct {
                name: point_name,
                fields: field_range,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // Field access: .x
        let field_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Field {
                receiver: struct_expr,
                field: x_name,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_field");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            field_expr,
            &arena,
            &expr_types,
        );

        println!("Field Access IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 10 (the x field)
        let result = codegen.jit_execute_i64("test_field").expect("JIT failed");
        assert_eq!(result, 10);
    }

    // ========================================================================
    // Phase 5: Option/Result (Tagged Unions)
    // ========================================================================

    #[test]
    fn test_some() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> Option<int> { Some(42) }
        let mut arena = ExprArena::new();

        // Inner value
        let inner = arena.alloc_expr(Expr {
            kind: ExprKind::Int(42),
            span: ori_ir::Span::new(0, 1),
        });

        // Some(42)
        let some_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Some(inner),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_some");
        let expr_types = vec![TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Placeholder - actual type would be Option<int>
            some_expr,
            &arena,
            &expr_types,
        );

        println!("Some IR:\n{}", codegen.print_to_string());

        // Verify IR contains tagged union structure
        // Note: LLVM constant-folds the struct, so we see the final value
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
        assert!(ir.contains("i8 1")); // Tag = 1 (Some)
        assert!(ir.contains("i64 42")); // Payload = 42
    }

    #[test]
    fn test_none() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> Option<int> { None }
        let mut arena = ExprArena::new();

        // None
        let none_expr = arena.alloc_expr(Expr {
            kind: ExprKind::None,
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_none");
        let expr_types = vec![TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Placeholder
            none_expr,
            &arena,
            &expr_types,
        );

        println!("None IR:\n{}", codegen.print_to_string());

        // Verify IR contains tagged union with tag = 0
        // Note: LLVM optimizes { i8 0, i64 0 } to zeroinitializer
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
        // None produces zeroinitializer (tag=0, value=0)
        assert!(ir.contains("zeroinitializer") || ir.contains("i8 0"));
    }

    #[test]
    fn test_ok() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> Result<int, int> { Ok(100) }
        let mut arena = ExprArena::new();

        // Inner value
        let inner = arena.alloc_expr(Expr {
            kind: ExprKind::Int(100),
            span: ori_ir::Span::new(0, 1),
        });

        // Ok(100)
        let ok_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Ok(Some(inner)),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_ok");
        let expr_types = vec![TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Placeholder
            ok_expr,
            &arena,
            &expr_types,
        );

        println!("Ok IR:\n{}", codegen.print_to_string());

        // Verify IR contains tagged union structure
        // Note: LLVM constant-folds the struct, so we see the final value
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
        assert!(ir.contains("i8 0")); // Tag = 0 (Ok)
        assert!(ir.contains("i64 100")); // Payload = 100
    }

    #[test]
    fn test_err() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> Result<int, int> { Err(999) }
        let mut arena = ExprArena::new();

        // Inner value
        let inner = arena.alloc_expr(Expr {
            kind: ExprKind::Int(999),
            span: ori_ir::Span::new(0, 1),
        });

        // Err(999)
        let err_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Err(Some(inner)),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_err");
        let expr_types = vec![TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Placeholder
            err_expr,
            &arena,
            &expr_types,
        );

        println!("Err IR:\n{}", codegen.print_to_string());

        // Verify IR contains tagged union structure
        // Note: LLVM constant-folds the struct, so we see the final value
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
        assert!(ir.contains("i8 1")); // Tag = 1 (Err)
        assert!(ir.contains("i64 999")); // Payload = 999
    }

    // ========================================================================
    // Phase 5: Match Expressions
    // ========================================================================

    #[test]
    fn test_match_literal() {
        use ori_ir::ast::patterns::MatchArm;

        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int {
        //     match 1 {
        //         1 -> 100,
        //         _ -> 200,
        //     }
        // }
        let mut arena = ExprArena::new();

        // Scrutinee: 1
        let scrutinee = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });

        // Arm 1: 1 -> 100
        let lit_1 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let body_1 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(100),
            span: ori_ir::Span::new(0, 1),
        });

        // Arm 2: _ -> 200
        let body_2 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(200),
            span: ori_ir::Span::new(0, 1),
        });

        let arms = arena.alloc_arms([
            MatchArm {
                pattern: MatchPattern::Literal(lit_1),
                guard: None,
                body: body_1,
                span: ori_ir::Span::new(0, 1),
            },
            MatchArm {
                pattern: MatchPattern::Wildcard,
                guard: None,
                body: body_2,
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        // Match expression
        let match_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Match { scrutinee, arms },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_match_lit");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            match_expr,
            &arena,
            &expr_types,
        );

        println!("Match Literal IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 100 (matches first arm)
        let result = codegen.jit_execute_i64("test_match_lit").expect("JIT failed");
        assert_eq!(result, 100);
    }

    #[test]
    fn test_match_wildcard() {
        use ori_ir::ast::patterns::MatchArm;

        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int {
        //     match 5 {
        //         1 -> 100,
        //         _ -> 200,
        //     }
        // }
        let mut arena = ExprArena::new();

        // Scrutinee: 5
        let scrutinee = arena.alloc_expr(Expr {
            kind: ExprKind::Int(5),
            span: ori_ir::Span::new(0, 1),
        });

        // Arm 1: 1 -> 100
        let lit_1 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let body_1 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(100),
            span: ori_ir::Span::new(0, 1),
        });

        // Arm 2: _ -> 200
        let body_2 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(200),
            span: ori_ir::Span::new(0, 1),
        });

        let arms = arena.alloc_arms([
            MatchArm {
                pattern: MatchPattern::Literal(lit_1),
                guard: None,
                body: body_1,
                span: ori_ir::Span::new(0, 1),
            },
            MatchArm {
                pattern: MatchPattern::Wildcard,
                guard: None,
                body: body_2,
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        // Match expression
        let match_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Match { scrutinee, arms },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_match_wild");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            match_expr,
            &arena,
            &expr_types,
        );

        println!("Match Wildcard IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 200 (matches wildcard)
        let result = codegen.jit_execute_i64("test_match_wild").expect("JIT failed");
        assert_eq!(result, 200);
    }

    #[test]
    fn test_match_binding() {
        use ori_ir::ast::patterns::MatchArm;

        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int {
        //     match 42 {
        //         x -> x + 1,
        //     }
        // }
        let mut arena = ExprArena::new();

        let x_name = interner.intern("x");

        // Scrutinee: 42
        let scrutinee = arena.alloc_expr(Expr {
            kind: ExprKind::Int(42),
            span: ori_ir::Span::new(0, 1),
        });

        // x + 1
        let x_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(x_name),
            span: ori_ir::Span::new(0, 1),
        });
        let one = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let body = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: x_ref,
                right: one,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let arms = arena.alloc_arms([
            MatchArm {
                pattern: MatchPattern::Binding(x_name),
                guard: None,
                body,
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        // Match expression
        let match_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Match { scrutinee, arms },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_match_bind");
        let expr_types = vec![TypeId::INT; 10];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            match_expr,
            &arena,
            &expr_types,
        );

        println!("Match Binding IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 43 (42 + 1)
        let result = codegen.jit_execute_i64("test_match_bind").expect("JIT failed");
        assert_eq!(result, 43);
    }

    // ========================================================================
    // Phase 6: Function Calls
    // ========================================================================

    #[test]
    fn test_function_call_simple() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create:
        //   fn add(a: int, b: int) -> int { a + b }
        //   fn main() -> int { add(10, 20) }

        let mut arena = ExprArena::new();

        // First, create the add function
        let a_name = interner.intern("a");
        let b_name = interner.intern("b");

        let a_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(a_name),
            span: ori_ir::Span::new(0, 1),
        });
        let b_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(b_name),
            span: ori_ir::Span::new(0, 1),
        });
        let add_body = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: a_ref,
                right: b_ref,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let add_fn_name = interner.intern("add");
        let add_expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            add_fn_name,
            &[a_name, b_name],
            &[TypeId::INT, TypeId::INT],
            TypeId::INT,
            add_body,
            &arena,
            &add_expr_types,
        );

        // Now create the main function that calls add(10, 20)
        let mut arena2 = ExprArena::new();

        // Arguments: 10, 20
        let arg1 = arena2.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let arg2 = arena2.alloc_expr(Expr {
            kind: ExprKind::Int(20),
            span: ori_ir::Span::new(0, 1),
        });
        let args = arena2.alloc_expr_list([arg1, arg2]);

        // Function reference: add
        let func = arena2.alloc_expr(Expr {
            kind: ExprKind::Ident(add_fn_name),
            span: ori_ir::Span::new(0, 1),
        });

        // Call: add(10, 20)
        let call_expr = arena2.alloc_expr(Expr {
            kind: ExprKind::Call { func, args },
            span: ori_ir::Span::new(0, 1),
        });

        let main_fn_name = interner.intern("main");
        let main_expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            main_fn_name,
            &[],
            &[],
            TypeId::INT,
            call_expr,
            &arena2,
            &main_expr_types,
        );

        println!("Function Call IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 30 (10 + 20)
        let result = codegen.jit_execute_i64("main").expect("JIT failed");
        assert_eq!(result, 30);
    }

    #[test]
    fn test_function_call_nested() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create:
        //   fn double(x: int) -> int { x + x }
        //   fn main() -> int { double(double(5)) }
        //   Should return 20

        let mut arena = ExprArena::new();
        let x_name = interner.intern("x");

        // double function: x + x
        let x_ref1 = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(x_name),
            span: ori_ir::Span::new(0, 1),
        });
        let x_ref2 = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(x_name),
            span: ori_ir::Span::new(0, 1),
        });
        let double_body = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: x_ref1,
                right: x_ref2,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let double_fn_name = interner.intern("double");
        let double_expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            double_fn_name,
            &[x_name],
            &[TypeId::INT],
            TypeId::INT,
            double_body,
            &arena,
            &double_expr_types,
        );

        // main function: double(double(5))
        let mut arena2 = ExprArena::new();

        // Inner call: double(5)
        let five = arena2.alloc_expr(Expr {
            kind: ExprKind::Int(5),
            span: ori_ir::Span::new(0, 1),
        });
        let inner_args = arena2.alloc_expr_list([five]);
        let double_ref_inner = arena2.alloc_expr(Expr {
            kind: ExprKind::Ident(double_fn_name),
            span: ori_ir::Span::new(0, 1),
        });
        let inner_call = arena2.alloc_expr(Expr {
            kind: ExprKind::Call { func: double_ref_inner, args: inner_args },
            span: ori_ir::Span::new(0, 1),
        });

        // Outer call: double(double(5))
        let outer_args = arena2.alloc_expr_list([inner_call]);
        let double_ref_outer = arena2.alloc_expr(Expr {
            kind: ExprKind::Ident(double_fn_name),
            span: ori_ir::Span::new(0, 1),
        });
        let outer_call = arena2.alloc_expr(Expr {
            kind: ExprKind::Call { func: double_ref_outer, args: outer_args },
            span: ori_ir::Span::new(0, 1),
        });

        let main_fn_name = interner.intern("main");
        let main_expr_types = vec![TypeId::INT; 10];

        codegen.compile_function(
            main_fn_name,
            &[],
            &[],
            TypeId::INT,
            outer_call,
            &arena2,
            &main_expr_types,
        );

        println!("Nested Call IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 20 (double(double(5)) = double(10) = 20)
        let result = codegen.jit_execute_i64("main").expect("JIT failed");
        assert_eq!(result, 20);
    }

    #[test]
    fn test_recursive_function() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create factorial:
        //   fn factorial(n: int) -> int {
        //       if n <= 1 then 1 else n * factorial(n - 1)
        //   }
        //   fn main() -> int { factorial(5) }
        //   Should return 120

        let mut arena = ExprArena::new();
        let n_name = interner.intern("n");
        let factorial_fn_name = interner.intern("factorial");

        // n <= 1
        let n_ref1 = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(n_name),
            span: ori_ir::Span::new(0, 1),
        });
        let one_cond = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let cond = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::LtEq,
                left: n_ref1,
                right: one_cond,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // then branch: 1
        let then_branch = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });

        // n - 1
        let n_ref2 = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(n_name),
            span: ori_ir::Span::new(0, 1),
        });
        let one_sub = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let n_minus_1 = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Sub,
                left: n_ref2,
                right: one_sub,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // factorial(n - 1)
        let rec_args = arena.alloc_expr_list([n_minus_1]);
        let factorial_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(factorial_fn_name),
            span: ori_ir::Span::new(0, 1),
        });
        let rec_call = arena.alloc_expr(Expr {
            kind: ExprKind::Call { func: factorial_ref, args: rec_args },
            span: ori_ir::Span::new(0, 1),
        });

        // n * factorial(n - 1)
        let n_ref3 = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(n_name),
            span: ori_ir::Span::new(0, 1),
        });
        let else_branch = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Mul,
                left: n_ref3,
                right: rec_call,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // if n <= 1 then 1 else n * factorial(n - 1)
        let factorial_body = arena.alloc_expr(Expr {
            kind: ExprKind::If {
                cond,
                then_branch,
                else_branch: Some(else_branch),
            },
            span: ori_ir::Span::new(0, 1),
        });

        let factorial_expr_types = vec![TypeId::INT; 20];

        codegen.compile_function(
            factorial_fn_name,
            &[n_name],
            &[TypeId::INT],
            TypeId::INT,
            factorial_body,
            &arena,
            &factorial_expr_types,
        );

        // main function: factorial(5)
        let mut arena2 = ExprArena::new();

        let five = arena2.alloc_expr(Expr {
            kind: ExprKind::Int(5),
            span: ori_ir::Span::new(0, 1),
        });
        let args = arena2.alloc_expr_list([five]);
        let factorial_ref_main = arena2.alloc_expr(Expr {
            kind: ExprKind::Ident(factorial_fn_name),
            span: ori_ir::Span::new(0, 1),
        });
        let call = arena2.alloc_expr(Expr {
            kind: ExprKind::Call { func: factorial_ref_main, args },
            span: ori_ir::Span::new(0, 1),
        });

        let main_fn_name = interner.intern("main");
        let main_expr_types = vec![TypeId::INT; 5];

        codegen.compile_function(
            main_fn_name,
            &[],
            &[],
            TypeId::INT,
            call,
            &arena2,
            &main_expr_types,
        );

        println!("Factorial IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 120 (5! = 120)
        let result = codegen.jit_execute_i64("main").expect("JIT failed");
        assert_eq!(result, 120);
    }

    // ========================================================================
    // Phase 7: Strings
    // ========================================================================

    #[test]
    fn test_string_literal() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> str { "hello" }
        let mut arena = ExprArena::new();

        // String literal "hello"
        let hello = interner.intern("hello");
        let str_expr = arena.alloc_expr(Expr {
            kind: ExprKind::String(hello),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_str");
        let expr_types = vec![TypeId::STR];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::STR,
            str_expr,
            &arena,
            &expr_types,
        );

        println!("String IR:\n{}", codegen.print_to_string());

        // Verify IR contains string constant
        let ir = codegen.print_to_string();
        assert!(ir.contains("hello")); // String content
        assert!(ir.contains("{ i64, ptr }")); // String struct type
        assert!(ir.contains("i64 5")); // Length = 5
    }

    #[test]
    fn test_string_multiple() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create two functions that use the same string literal
        // Should reuse the global constant
        let mut arena = ExprArena::new();

        let hello = interner.intern("world");
        let str_expr1 = arena.alloc_expr(Expr {
            kind: ExprKind::String(hello),
            span: ori_ir::Span::new(0, 1),
        });
        let str_expr2 = arena.alloc_expr(Expr {
            kind: ExprKind::String(hello),
            span: ori_ir::Span::new(0, 1),
        });

        let fn1_name = interner.intern("test_str1");
        let fn2_name = interner.intern("test_str2");
        let expr_types = vec![TypeId::STR];

        codegen.compile_function(
            fn1_name,
            &[],
            &[],
            TypeId::STR,
            str_expr1,
            &arena,
            &expr_types,
        );

        codegen.compile_function(
            fn2_name,
            &[],
            &[],
            TypeId::STR,
            str_expr2,
            &arena,
            &expr_types,
        );

        println!("Multiple Strings IR:\n{}", codegen.print_to_string());

        // Verify only one global string constant declaration
        let ir = codegen.print_to_string();
        // Count the actual global declarations (lines starting with @.str)
        let global_count = ir.lines()
            .filter(|line| line.trim_start().starts_with("@.str."))
            .count();
        // Should have exactly 1 global string (reused between functions)
        assert_eq!(global_count, 1, "Expected 1 global string constant, found {global_count}");
    }

    #[test]
    fn test_string_empty() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> str { "" }
        let mut arena = ExprArena::new();

        // Empty string
        let empty = interner.intern("");
        let str_expr = arena.alloc_expr(Expr {
            kind: ExprKind::String(empty),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_empty_str");
        let expr_types = vec![TypeId::STR];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::STR,
            str_expr,
            &arena,
            &expr_types,
        );

        println!("Empty String IR:\n{}", codegen.print_to_string());

        // Verify IR contains zero length
        let ir = codegen.print_to_string();
        assert!(ir.contains("i64 0")); // Length = 0
    }

    // ========================================================================
    // Phase 8: New Expression Types
    // ========================================================================

    #[test]
    fn test_duration_literal() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { 5s } -> 5000 (milliseconds)
        let mut arena = ExprArena::new();

        let duration_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Duration {
                value: 5,
                unit: ori_ir::DurationUnit::Seconds,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_duration");
        let expr_types = vec![TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            duration_expr,
            &arena,
            &expr_types,
        );

        println!("Duration IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 5000 (5 seconds in milliseconds)
        let result = codegen.jit_execute_i64("test_duration").expect("JIT failed");
        assert_eq!(result, 5000);
    }

    #[test]
    fn test_size_literal() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { 2kb } -> 2048 (bytes)
        let mut arena = ExprArena::new();

        let size_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Size {
                value: 2,
                unit: ori_ir::SizeUnit::Kilobytes,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_size");
        let expr_types = vec![TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            size_expr,
            &arena,
            &expr_types,
        );

        println!("Size IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 2048 (2 * 1024)
        let result = codegen.jit_execute_i64("test_size").expect("JIT failed");
        assert_eq!(result, 2048);
    }

    #[test]
    fn test_range_literal() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> Range { 1..10 }
        let mut arena = ExprArena::new();

        let start = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let end = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });

        let range_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Range {
                start: Some(start),
                end: Some(end),
                inclusive: false,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_range");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Returns struct, but we test IR generation
            range_expr,
            &arena,
            &expr_types,
        );

        println!("Range IR:\n{}", codegen.print_to_string());

        // Verify IR contains range struct type
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i64, i64, i1 }")); // Range struct type
    }

    #[test]
    fn test_list_literal_empty() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> List { [] }
        let mut arena = ExprArena::new();

        let range = arena.alloc_expr_list([]);
        let list_expr = arena.alloc_expr(Expr {
            kind: ExprKind::List(range),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_empty_list");
        let expr_types = vec![TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            list_expr,
            &arena,
            &expr_types,
        );

        println!("Empty List IR:\n{}", codegen.print_to_string());

        // Verify IR contains list struct type
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i64, i64, ptr }")); // List struct type
    }

    #[test]
    fn test_list_literal_with_elements() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> List { [1, 2, 3] }
        let mut arena = ExprArena::new();

        let elem1 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let elem2 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(2),
            span: ori_ir::Span::new(0, 1),
        });
        let elem3 = arena.alloc_expr(Expr {
            kind: ExprKind::Int(3),
            span: ori_ir::Span::new(0, 1),
        });

        let range = arena.alloc_expr_list([elem1, elem2, elem3]);
        let list_expr = arena.alloc_expr(Expr {
            kind: ExprKind::List(range),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_list_elems");
        let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            list_expr,
            &arena,
            &expr_types,
        );

        println!("List with Elements IR:\n{}", codegen.print_to_string());

        // Verify IR contains list construction
        let ir = codegen.print_to_string();
        assert!(ir.contains("{ i64, i64, ptr }")); // List struct type
        assert!(ir.contains("i64 3")); // Length = 3
    }

    #[test]
    fn test_assign() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> int { let mut x = 10; x = 20; x }
        // Simplified: let x = 10, then return 10 (since let returns the value)
        let mut arena = ExprArena::new();

        let x_name = interner.intern("x");

        // let x = 10
        let ten = arena.alloc_expr(Expr {
            kind: ExprKind::Int(10),
            span: ori_ir::Span::new(0, 1),
        });
        let let_x = arena.alloc_expr(Expr {
            kind: ExprKind::Let {
                pattern: BindingPattern::Name(x_name),
                ty: None,
                init: ten,
                mutable: true,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_assign");
        let expr_types = vec![TypeId::INT, TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT,
            let_x,
            &arena,
            &expr_types,
        );

        println!("Assign IR:\n{}", codegen.print_to_string());

        // JIT execute - should return 10
        let result = codegen.jit_execute_i64("test_assign").expect("JIT failed");
        assert_eq!(result, 10);
    }

    #[test]
    fn test_function_ref() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create a helper function first
        let mut arena = ExprArena::new();

        let x = arena.alloc_expr(Expr {
            kind: ExprKind::Int(42),
            span: ori_ir::Span::new(0, 1),
        });

        let helper_name = interner.intern("helper");
        let expr_types = vec![TypeId::INT];

        codegen.compile_function(
            helper_name,
            &[],
            &[],
            TypeId::INT,
            x,
            &arena,
            &expr_types,
        );

        // Now test FunctionRef
        let mut arena2 = ExprArena::new();

        let func_ref_expr = arena2.alloc_expr(Expr {
            kind: ExprKind::FunctionRef(helper_name),
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_func_ref");
        let expr_types2 = vec![TypeId::INT];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Returns pointer, but we test IR generation
            func_ref_expr,
            &arena2,
            &expr_types2,
        );

        println!("Function Ref IR:\n{}", codegen.print_to_string());

        // Verify IR contains reference to helper function
        let ir = codegen.print_to_string();
        assert!(ir.contains("@helper")); // Reference to helper function
    }

    #[test]
    fn test_lambda_simple() {
        let context = Context::create();
        let interner = StringInterner::new();
        let codegen = LLVMCodegen::new(&context, &interner, "test");

        // Create: fn test() -> fn { x -> x + 1 }
        let mut arena = ExprArena::new();

        let x_name = interner.intern("x");

        // Parameter list with one param
        let params = arena.alloc_params([
            ori_ir::ast::Param {
                name: x_name,
                ty: None,
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        // Body: x + 1
        let x_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(x_name),
            span: ori_ir::Span::new(0, 1),
        });
        let one = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let body = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: x_ref,
                right: one,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let lambda_expr = arena.alloc_expr(Expr {
            kind: ExprKind::Lambda {
                params,
                ret_ty: None,
                body,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let fn_name = interner.intern("test_lambda");
        let expr_types = vec![TypeId::INT; 5];

        codegen.compile_function(
            fn_name,
            &[],
            &[],
            TypeId::INT, // Returns function pointer
            lambda_expr,
            &arena,
            &expr_types,
        );

        println!("Lambda IR:\n{}", codegen.print_to_string());

        // Verify IR contains lambda function
        let ir = codegen.print_to_string();
        assert!(ir.contains("__lambda_")); // Lambda function name
    }
}
