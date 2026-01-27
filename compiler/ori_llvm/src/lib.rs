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

mod collections;
mod control_flow;
mod functions;
mod matching;
mod operators;
mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, FunctionValue, PhiValue};
use inkwell::OptimizationLevel;

use ori_ir::ast::{patterns::BindingPattern, ExprKind};
use ori_ir::{ExprArena, ExprId, Name, StringInterner, TypeId};

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

    /// Compile an expression, dispatching to the appropriate module.
    pub(crate) fn compile_expr(
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
