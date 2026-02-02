//! Module-level LLVM compilation.
//!
//! Compiles an entire Ori module (all functions, tests) to LLVM IR.
//!
//! # Architecture
//!
//! This module provides high-level compilation orchestration using
//! [`CodegenCx`](crate::context::CodegenCx) for context management and
//! [`Builder`](crate::builder::Builder) for instruction generation.
//!
//! Two-phase codegen pattern:
//! 1. **Predefine**: Declare all symbols (functions, globals) with `declare_fn`
//! 2. **Define**: Generate function bodies with `Builder`

use inkwell::context::Context;
use inkwell::values::FunctionValue;

use ori_ir::{ExprArena, Function, Name, ParsedType, StringInterner, TestDef, TypeId};

use crate::builder::Builder;
use crate::context::CodegenCx;
use crate::functions::body::FunctionBodyConfig;

/// Convert a `ParsedType` to a `TypeId`.
///
/// Only handles primitive types for now. Returns None for complex types
/// which will fall back to INT.
fn parsed_type_to_type_id(ty: &ParsedType) -> Option<TypeId> {
    match ty {
        ParsedType::Primitive(id) => Some(*id),
        // For tuples, lists, functions, etc. we'd need more sophisticated handling
        // For now, return None to use the fallback
        _ => None,
    }
}

/// Compiler for a complete Ori module.
///
/// Wraps `CodegenCx` and provides high-level compilation methods.
/// Function and test registrations are managed by `CodegenCx` internally.
pub struct ModuleCompiler<'ll, 'tcx> {
    /// The codegen context.
    cx: CodegenCx<'ll, 'tcx>,
}

impl<'ll, 'tcx> ModuleCompiler<'ll, 'tcx> {
    /// Create a new module compiler.
    pub fn new(context: &'ll Context, interner: &'tcx StringInterner, module_name: &str) -> Self {
        let cx = CodegenCx::new(context, interner, module_name);

        Self { cx }
    }

    /// Get the codegen context.
    pub fn cx(&self) -> &CodegenCx<'ll, 'tcx> {
        &self.cx
    }

    /// Get the LLVM module.
    pub fn module(&self) -> &inkwell::module::Module<'ll> {
        self.cx.llmod()
    }

    /// Declare runtime functions that Ori code can call.
    pub fn declare_runtime(&self) {
        self.cx.declare_runtime_functions();
    }

    /// Register a user-defined struct type.
    ///
    /// Creates an LLVM struct type with the given field names.
    /// For now, all fields are mapped to i64 (matching the default fallback).
    pub fn register_struct(&self, name: Name, field_names: Vec<Name>) {
        // For now, all fields are i64 (matching the INT fallback)
        let field_types: Vec<_> = field_names
            .iter()
            .map(|_| self.cx.scx.type_i64().into())
            .collect();

        self.cx.register_struct(name, field_names, &field_types);
    }

    /// Compile a function definition (legacy - uses hardcoded INT types).
    pub fn compile_function(&self, func: &Function, arena: &ExprArena, expr_types: &[TypeId]) {
        self.compile_function_with_sig(func, arena, expr_types, None);
    }

    /// Compile a function definition with type signature from the type checker.
    pub fn compile_function_with_sig(
        &self,
        func: &Function,
        arena: &ExprArena,
        expr_types: &[TypeId],
        sig: Option<&crate::evaluator::FunctionSig>,
    ) {
        // Get parameter names
        let params = arena.get_params(func.params);
        let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();

        // Use signature if provided, otherwise extract from AST declarations
        // Clone params to avoid lifetime complexity with the else branch.
        // Cost is O(n) where n = param count, typically small (<10).
        let (param_types, return_type) = if let Some(sig) = sig {
            (sig.params.clone(), sig.return_type)
        } else {
            // Extract types from parameter declarations
            let param_types: Vec<TypeId> = params
                .iter()
                .map(|p| {
                    p.ty.as_ref()
                        .and_then(parsed_type_to_type_id)
                        .unwrap_or(TypeId::INT)
                })
                .collect();

            // Extract return type from function declaration
            let return_type = func
                .return_ty
                .as_ref()
                .and_then(parsed_type_to_type_id)
                .unwrap_or(TypeId::VOID);

            (param_types, return_type)
        };

        // Phase 1: Declare the function
        let llvm_func = self.cx.declare_fn(func.name, &param_types, return_type);

        // Phase 2: Define the function body
        // Create entry block
        let entry_bb = self.cx.llcx().append_basic_block(llvm_func, "entry");

        // Create builder positioned at entry
        let builder = Builder::build(&self.cx, entry_bb);

        // Compile the body
        builder.compile_function_body(&FunctionBodyConfig {
            param_names: &param_names,
            return_type,
            body: func.body,
            arena,
            expr_types,
            function: llvm_func,
        });
    }

    /// Compile a test definition.
    ///
    /// Tests are compiled as void functions that call assertions.
    pub fn compile_test(&self, test: &TestDef, arena: &ExprArena, expr_types: &[TypeId]) {
        // Tests are void -> void functions
        // Phase 1: Declare
        let llvm_func = self.cx.declare_fn(test.name, &[], TypeId::VOID);

        // Register as test
        self.cx.register_test(test.name, llvm_func);

        // Phase 2: Define
        let entry_bb = self.cx.llcx().append_basic_block(llvm_func, "entry");
        let builder = Builder::build(&self.cx, entry_bb);

        builder.compile_function_body(&FunctionBodyConfig {
            param_names: &[],
            return_type: TypeId::VOID,
            body: test.body,
            arena,
            expr_types,
            function: llvm_func,
        });
    }

    /// Get a compiled function by name.
    pub fn get_function(&self, name: Name) -> Option<FunctionValue<'ll>> {
        self.cx.get_function(name)
    }

    /// Get a compiled test by name.
    pub fn get_test(&self, name: Name) -> Option<FunctionValue<'ll>> {
        self.cx.get_test(name)
    }

    /// Get all compiled tests.
    pub fn tests(&self) -> std::collections::HashMap<Name, FunctionValue<'ll>> {
        self.cx.all_tests()
    }

    /// Print LLVM IR to string.
    pub fn print_to_string(&self) -> String {
        self.cx.llmod().print_to_string().to_string()
    }

    /// Create JIT execution engine and run a test.
    ///
    /// Returns Ok(()) if test passed, Err(message) if failed.
    #[allow(unsafe_code)]
    pub fn run_test(&self, test_name: &str) -> Result<(), String> {
        use inkwell::OptimizationLevel;

        // Reset panic state before running
        crate::runtime::reset_panic_state();

        // Debug: print IR before JIT compilation if ORI_DEBUG_LLVM is set
        if std::env::var("ORI_DEBUG_LLVM").is_ok() {
            eprintln!("=== LLVM IR for {test_name} ===");
            eprintln!("{}", self.cx.llmod().print_to_string().to_string());
            eprintln!("=== END IR ===");
        }

        // Create JIT execution engine
        let ee = self
            .cx
            .llmod()
            .create_jit_execution_engine(OptimizationLevel::None)
            .map_err(|e| e.to_string())?;

        // Add runtime function mappings
        self.add_runtime_mappings(&ee)?;

        // Find the test function
        // SAFETY: We trust LLVM's JIT to correctly compile and execute.
        // The function must exist with signature () -> void.
        unsafe {
            let test_fn = ee
                .get_function::<unsafe extern "C" fn()>(test_name)
                .map_err(|e| format!("Test function '{test_name}' not found: {e}"))?;

            // Run the test
            test_fn.call();
        }

        // Check if panic occurred
        if crate::runtime::did_panic() {
            let msg =
                crate::runtime::get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            Err(msg)
        } else {
            Ok(())
        }
    }

    /// Add runtime function mappings to the execution engine.
    ///
    /// Returns an error if a runtime function is not declared.
    fn add_runtime_mappings(
        &self,
        ee: &inkwell::execution_engine::ExecutionEngine<'ll>,
    ) -> Result<(), String> {
        use crate::runtime;

        let mappings: &[(&str, usize)] = &[
            ("ori_print", runtime::ori_print as *const () as usize),
            (
                "ori_print_int",
                runtime::ori_print_int as *const () as usize,
            ),
            (
                "ori_print_float",
                runtime::ori_print_float as *const () as usize,
            ),
            (
                "ori_print_bool",
                runtime::ori_print_bool as *const () as usize,
            ),
            ("ori_panic", runtime::ori_panic as *const () as usize),
            (
                "ori_panic_cstr",
                runtime::ori_panic_cstr as *const () as usize,
            ),
            ("ori_assert", runtime::ori_assert as *const () as usize),
            (
                "ori_assert_eq_int",
                runtime::ori_assert_eq_int as *const () as usize,
            ),
            (
                "ori_assert_eq_bool",
                runtime::ori_assert_eq_bool as *const () as usize,
            ),
            ("ori_list_new", runtime::ori_list_new as *const () as usize),
            (
                "ori_list_free",
                runtime::ori_list_free as *const () as usize,
            ),
            ("ori_list_len", runtime::ori_list_len as *const () as usize),
            (
                "ori_compare_int",
                runtime::ori_compare_int as *const () as usize,
            ),
            ("ori_min_int", runtime::ori_min_int as *const () as usize),
            ("ori_max_int", runtime::ori_max_int as *const () as usize),
            (
                "ori_str_concat",
                runtime::ori_str_concat as *const () as usize,
            ),
            ("ori_str_eq", runtime::ori_str_eq as *const () as usize),
            ("ori_str_ne", runtime::ori_str_ne as *const () as usize),
            (
                "ori_assert_eq_str",
                runtime::ori_assert_eq_str as *const () as usize,
            ),
            // Type conversion functions
            (
                "ori_str_from_int",
                runtime::ori_str_from_int as *const () as usize,
            ),
            (
                "ori_str_from_bool",
                runtime::ori_str_from_bool as *const () as usize,
            ),
            (
                "ori_str_from_float",
                runtime::ori_str_from_float as *const () as usize,
            ),
            // Closure boxing
            (
                "ori_closure_box",
                runtime::ori_closure_box as *const () as usize,
            ),
        ];

        let module = self.cx.llmod();
        for &(name, addr) in mappings {
            let func = module.get_function(name).ok_or_else(|| {
                format!(
                    "{name} not declared - call declare_runtime_functions() before running tests"
                )
            })?;
            ee.add_global_mapping(&func, addr);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::ast::{BinaryOp, Expr, ExprKind};
    use ori_ir::{GenericParamRange, Param, Visibility};

    #[test]
    fn test_module_compiler_basic() {
        let context = Context::create();
        let interner = StringInterner::new();
        let compiler = ModuleCompiler::new(&context, &interner, "test_module");

        // Declare runtime functions
        compiler.declare_runtime();

        // Create a simple function: fn add(a: int, b: int) -> int { a + b }
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
        let add_body = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: a_ref,
                right: b_ref,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let add_name = interner.intern("add");
        let params = arena.alloc_params([
            Param {
                name: a_name,
                ty: None,
                span: ori_ir::Span::new(0, 1),
            },
            Param {
                name: b_name,
                ty: None,
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        let func = Function {
            name: add_name,
            generics: GenericParamRange::EMPTY,
            params,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            body: add_body,
            span: ori_ir::Span::new(0, 1),
            visibility: Visibility::Private,
        };

        let expr_types = vec![TypeId::INT; 10];
        compiler.compile_function(&func, &arena, &expr_types);

        if std::env::var("ORI_DEBUG_LLVM").is_ok() {
            println!("Module IR:\n{}", compiler.print_to_string());
        }

        // Verify function was compiled
        assert!(compiler.get_function(add_name).is_some());
    }

    #[test]
    fn test_module_with_test() {
        let context = Context::create();
        let interner = StringInterner::new();
        let compiler = ModuleCompiler::new(&context, &interner, "test_module");

        // Declare runtime functions
        compiler.declare_runtime();

        // Create a test: @test_add () -> void = run(assert_eq(actual: 1 + 1, expected: 2))
        // Simplified: just assert 1 == 1
        let mut arena = ExprArena::new();

        // true literal (represents 1 == 1 simplified)
        let condition = arena.alloc_expr(Expr {
            kind: ExprKind::Bool(true),
            span: ori_ir::Span::new(0, 1),
        });

        // Call ori_assert(true)
        // For now, just use the condition directly as the body
        let test_body = condition;

        let test_name = interner.intern("test_simple");
        let empty_params = arena.alloc_params([]);
        let test_def = TestDef {
            name: test_name,
            targets: vec![],
            params: empty_params,
            return_ty: None,
            body: test_body,
            span: ori_ir::Span::new(0, 1),
            skip_reason: None,
            expected_errors: vec![],
            fail_expected: None,
        };

        let expr_types = vec![TypeId::BOOL];
        compiler.compile_test(&test_def, &arena, &expr_types);

        if std::env::var("ORI_DEBUG_LLVM").is_ok() {
            println!("Test Module IR:\n{}", compiler.print_to_string());
        }

        // Verify test was compiled
        assert!(compiler.get_test(test_name).is_some());
    }
}
