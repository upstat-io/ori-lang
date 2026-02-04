//! LLVM-based evaluator for running Ori code.
//!
//! This provides a JIT-based evaluator that compiles Ori code to LLVM IR
//! and executes it natively, as an alternative to the tree-walking interpreter.

use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use rustc_hash::FxHashMap;

use ori_ir::ast::{Module, TestDef, TypeDeclKind, Visibility};
use ori_ir::{ExprArena, ExprId, Name, StringInterner, TypeId};
use ori_types::TypeInterner;

use crate::module::ModuleCompiler;
use crate::runtime;

/// Result type for LLVM evaluation.
pub type LLVMEvalResult = Result<LLVMValue, LLVMEvalError>;

/// Values that can be returned from LLVM evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum LLVMValue {
    /// Void/unit value
    Void,
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Bool(bool),
}

/// Error during LLVM evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LLVMEvalError {
    pub message: String,
}

impl LLVMEvalError {
    pub fn new(message: impl Into<String>) -> Self {
        LLVMEvalError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LLVMEvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LLVMEvalError {}

/// LLVM-based evaluator.
///
/// Compiles Ori code to LLVM IR and JIT executes it.
pub struct LLVMEvaluator<'ctx> {
    context: &'ctx Context,
    interner: &'ctx StringInterner,
    /// Compiled functions by name
    functions: FxHashMap<Name, CompiledFunction>,
    /// Type information for expressions
    expr_types: Vec<TypeId>,
}

/// A compiled function ready for execution.
#[expect(
    dead_code,
    reason = "Fields will be used when direct function execution is implemented"
)]
struct CompiledFunction {
    /// The expression body
    body: ExprId,
    /// Parameter names
    params: Vec<Name>,
}

impl<'ctx> LLVMEvaluator<'ctx> {
    /// Create a new LLVM evaluator.
    pub fn new(context: &'ctx Context, interner: &'ctx StringInterner) -> Self {
        LLVMEvaluator {
            context,
            interner,
            functions: FxHashMap::default(),
            expr_types: Vec::new(),
        }
    }

    /// Register prelude functions.
    ///
    /// For LLVM, the prelude functions are provided by the runtime library
    /// and are automatically linked when we JIT compile.
    pub fn register_prelude(&mut self) {
        // Prelude is handled by the runtime library
    }

    /// Load a module, preparing all functions for execution.
    pub fn load_module(&mut self, module: &Module, arena: &ExprArena) -> Result<(), String> {
        // Store function info for later compilation
        for func in &module.functions {
            let params: Vec<Name> = arena
                .get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();

            self.functions.insert(
                func.name,
                CompiledFunction {
                    body: func.body,
                    params,
                },
            );
        }

        // Initialize expr_types with a reasonable size
        // In practice, this would come from type checking
        self.expr_types = vec![TypeId::INT; 1000];

        Ok(())
    }

    /// Evaluate a test expression.
    ///
    /// This compiles the entire module to LLVM IR and JIT executes the test.
    pub fn eval_test(
        &self,
        test_name: Name,
        test_body: ExprId,
        arena: &ExprArena,
        module: &Module,
    ) -> LLVMEvalResult {
        // Reset panic state
        runtime::reset_panic_state();

        // Create a fresh module compiler for this test
        let compiler = ModuleCompiler::new(self.context, self.interner, "test_module");
        compiler.declare_runtime();

        // Register user-defined struct types with actual field types
        for type_decl in &module.types {
            if let TypeDeclKind::Struct(fields) = &type_decl.kind {
                compiler.register_struct_with_types(type_decl.name, fields, arena);
            }
        }

        // Compile all functions the test might call
        for func in &module.functions {
            compiler.compile_function(func, arena, &self.expr_types);
        }

        // Compile impl block methods
        for impl_def in &module.impls {
            for method in &impl_def.methods {
                // Convert ImplMethod to Function for compilation
                let func = ori_ir::Function {
                    name: method.name,
                    generics: impl_def.generics,
                    params: method.params,
                    return_ty: None,
                    capabilities: vec![],
                    where_clauses: vec![],
                    guard: None,
                    body: method.body,
                    span: method.span,
                    visibility: Visibility::Private,
                };
                compiler.compile_function(&func, arena, &self.expr_types);
            }
        }

        // Create a wrapper test function
        let test_name_str = self.interner.lookup(test_name);
        let wrapper_name = format!("__test_{test_name_str}");
        let wrapper_name_interned = self.interner.intern(&wrapper_name);

        // Compile the test as a void function
        let test_func = ori_ir::Function {
            name: wrapper_name_interned,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            guard: None,
            body: test_body,
            span: ori_ir::Span::new(0, 0),
            visibility: Visibility::Private,
        };
        compiler.compile_function(&test_func, arena, &self.expr_types);

        // JIT compile and run
        match compiler.run_test(&wrapper_name) {
            Ok(()) => Ok(LLVMValue::Void),
            Err(msg) => Err(LLVMEvalError::new(msg)),
        }
    }

    /// Evaluate an expression directly.
    ///
    /// This is a simplified version that wraps the expression in a test function.
    pub fn eval(&self, expr: ExprId, arena: &ExprArena) -> LLVMEvalResult {
        runtime::reset_panic_state();

        let compiler = ModuleCompiler::new(self.context, self.interner, "eval_module");
        compiler.declare_runtime();

        // Create a wrapper function for the expression
        let wrapper_name = self.interner.intern("__eval_wrapper");
        let wrapper_func = ori_ir::Function {
            name: wrapper_name,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            guard: None,
            body: expr,
            span: ori_ir::Span::new(0, 0),
            visibility: Visibility::Private,
        };
        compiler.compile_function(&wrapper_func, arena, &self.expr_types);

        match compiler.run_test("__eval_wrapper") {
            Ok(()) => Ok(LLVMValue::Void),
            Err(msg) => Err(LLVMEvalError::new(msg)),
        }
    }
}

/// Function type signature for LLVM compilation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionSig {
    /// Parameter types
    pub params: Vec<TypeId>,
    /// Return type
    pub return_type: TypeId,
    /// Whether this function is generic (has type parameters).
    /// Generic functions cannot be directly compiled to LLVM without monomorphization.
    pub is_generic: bool,
}

/// A compiled module with JIT engine ready for test execution.
///
/// All functions and tests are compiled once, then tests can be run multiple times
/// from the same engine. This avoids the O(n²) recompilation problem where each test
/// would otherwise recompile all module functions.
///
/// # Lifetime
///
/// The execution engine owns the compiled machine code and must outlive any
/// function calls. The `'ll` lifetime ties to the LLVM context.
pub struct CompiledTestModule<'ll> {
    /// The JIT execution engine (owns the compiled code).
    engine: ExecutionEngine<'ll>,
    /// Test wrapper function names for lookup.
    /// Maps test `Name` to the wrapper function name string (e.g., `__test_my_test`).
    test_wrappers: FxHashMap<Name, String>,
}

impl CompiledTestModule<'_> {
    /// Run a single test from this compiled module.
    ///
    /// # Safety
    ///
    /// The test function must exist in the compiled module and have signature `() -> void`.
    #[allow(unsafe_code)]
    pub fn run_test(&self, test_name: Name) -> LLVMEvalResult {
        // Reset panic state before running
        runtime::reset_panic_state();

        // Look up the wrapper function name
        let wrapper_name = self.test_wrappers.get(&test_name).ok_or_else(|| {
            LLVMEvalError::new(format!("Test wrapper not found for test: {test_name:?}"))
        })?;

        // Get function pointer and execute
        // SAFETY: We compiled this test wrapper with signature () -> void
        unsafe {
            let test_fn = self
                .engine
                .get_function::<unsafe extern "C" fn()>(wrapper_name)
                .map_err(|e| LLVMEvalError::new(format!("Test function not found: {e}")))?;

            test_fn.call();
        }

        // Check if panic occurred
        if runtime::did_panic() {
            let msg = runtime::get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            Err(LLVMEvalError::new(msg))
        } else {
            Ok(LLVMValue::Void)
        }
    }
}

/// LLVM-based evaluator that owns its context.
///
/// This is the recommended evaluator for use in applications that don't
/// want to manage the LLVM context lifetime themselves.
pub struct OwnedLLVMEvaluator<'tcx> {
    context: Context,
    /// Compiled functions by name
    functions: FxHashMap<Name, CompiledFunction>,
    /// Type interner for resolving compound types (List, Map, etc.)
    type_interner: Option<&'tcx TypeInterner>,
}

impl<'tcx> OwnedLLVMEvaluator<'tcx> {
    /// Create a new owned LLVM evaluator.
    #[must_use]
    pub fn new() -> Self {
        OwnedLLVMEvaluator {
            context: Context::create(),
            functions: FxHashMap::default(),
            type_interner: None,
        }
    }

    /// Create an evaluator with a type interner for compound type resolution.
    ///
    /// The type interner allows proper LLVM type generation for compound types
    /// like List, Map, Tuple, etc., which require looking up `TypeId` -> `TypeData`.
    #[must_use]
    pub fn with_type_interner(type_interner: &'tcx TypeInterner) -> Self {
        OwnedLLVMEvaluator {
            context: Context::create(),
            functions: FxHashMap::default(),
            type_interner: Some(type_interner),
        }
    }

    /// Load a module, preparing all functions for execution.
    pub fn load_module(&mut self, module: &Module, arena: &ExprArena) -> Result<(), String> {
        // Store function info for later compilation
        for func in &module.functions {
            let params: Vec<Name> = arena
                .get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();

            self.functions.insert(
                func.name,
                CompiledFunction {
                    body: func.body,
                    params,
                },
            );
        }

        Ok(())
    }

    /// Evaluate a test expression.
    ///
    /// This compiles the entire module to LLVM IR and JIT executes the test.
    ///
    /// # Arguments
    /// - `test_name`: Name of the test
    /// - `test_body`: Expression ID of the test body
    /// - `arena`: Expression arena
    /// - `module`: The module containing functions the test may call
    /// - `interner`: String interner
    /// - `expr_types`: Type of each expression (indexed by `ExprId`)
    /// - `function_sigs`: Signature of each function (indexed same as module.functions)
    pub fn eval_test(
        &self,
        test_name: Name,
        test_body: ExprId,
        arena: &ExprArena,
        module: &Module,
        interner: &StringInterner,
        expr_types: &[TypeId],
        function_sigs: &[FunctionSig],
    ) -> LLVMEvalResult {
        // Reset panic state
        runtime::reset_panic_state();

        // Create a fresh module compiler for this test
        // Use type interner if available for proper compound type handling
        let compiler = if let Some(type_interner) = self.type_interner {
            ModuleCompiler::with_type_interner(
                &self.context,
                interner,
                type_interner,
                "test_module",
            )
        } else {
            ModuleCompiler::new(&self.context, interner, "test_module")
        };
        compiler.declare_runtime();

        // Register user-defined struct types with actual field types
        for type_decl in &module.types {
            if let TypeDeclKind::Struct(fields) = &type_decl.kind {
                compiler.register_struct_with_types(type_decl.name, fields, arena);
            }
        }

        // Compile all functions the test might call
        // Skip generic functions - they require monomorphization which isn't implemented yet
        for (i, func) in module.functions.iter().enumerate() {
            let sig = function_sigs.get(i);
            // Skip generic functions - they have unresolved type variables
            if sig.is_some_and(|s| s.is_generic) {
                continue;
            }
            compiler.compile_function_with_sig(func, arena, expr_types, sig);
        }

        // Compile impl block methods
        for impl_def in &module.impls {
            for method in &impl_def.methods {
                // Convert ImplMethod to Function for compilation
                let func = ori_ir::Function {
                    name: method.name,
                    generics: impl_def.generics,
                    params: method.params,
                    return_ty: None, // Type info comes from expr_types
                    capabilities: vec![],
                    where_clauses: vec![],
                    guard: None,
                    body: method.body,
                    span: method.span,
                    visibility: Visibility::Private,
                };
                // Compile without signature - uses fallback INT types
                // TODO: Get proper signatures for impl methods
                compiler.compile_function(&func, arena, expr_types);
            }
        }

        // Create a wrapper test function
        let test_name_str = interner.lookup(test_name);
        let wrapper_name = format!("__test_{test_name_str}");
        let wrapper_name_interned = interner.intern(&wrapper_name);

        // Compile the test as a void function
        let test_func = ori_ir::Function {
            name: wrapper_name_interned,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            guard: None,
            body: test_body,
            span: ori_ir::Span::new(0, 0),
            visibility: Visibility::Private,
        };
        let void_sig = FunctionSig {
            params: vec![],
            return_type: TypeId::VOID,
            is_generic: false,
        };
        compiler.compile_function_with_sig(&test_func, arena, expr_types, Some(&void_sig));

        // JIT compile and run
        match compiler.run_test(&wrapper_name) {
            Ok(()) => Ok(LLVMValue::Void),
            Err(msg) => Err(LLVMEvalError::new(msg)),
        }
    }

    /// Compile an entire module with all its tests.
    ///
    /// This is the recommended way to run multiple tests from the same module.
    /// It compiles all functions and test wrappers ONCE, then returns a
    /// `CompiledTestModule` that can run individual tests without recompilation.
    ///
    /// # Performance
    ///
    /// For a module with N functions and M tests:
    /// - Old approach: O(M × N) function compilations (each test recompiles all)
    /// - This approach: O(N + M) function compilations (compile once, run many)
    ///
    /// For files with many tests (like `benchmark_10k.ori`), this prevents LLVM
    /// resource exhaustion from repeated compilations into the same context.
    ///
    /// # Arguments
    ///
    /// - `module`: The parsed module containing functions and type declarations
    /// - `tests`: The tests to compile wrappers for
    /// - `arena`: Expression arena for looking up AST nodes
    /// - `interner`: String interner for name resolution
    /// - `expr_types`: Type of each expression (indexed by `ExprId`)
    /// - `function_sigs`: Signature of each function (indexed same as module.functions)
    pub fn compile_module_with_tests<'a>(
        &'a self,
        module: &Module,
        tests: &[&TestDef],
        arena: &ExprArena,
        interner: &StringInterner,
        expr_types: &[TypeId],
        function_sigs: &[FunctionSig],
    ) -> Result<CompiledTestModule<'a>, LLVMEvalError> {
        use inkwell::OptimizationLevel;

        // Create a single module compiler for all functions and tests
        let compiler = if let Some(type_interner) = self.type_interner {
            ModuleCompiler::with_type_interner(
                &self.context,
                interner,
                type_interner,
                "test_module",
            )
        } else {
            ModuleCompiler::new(&self.context, interner, "test_module")
        };
        compiler.declare_runtime();

        // Register user-defined struct types with actual field types
        for type_decl in &module.types {
            if let TypeDeclKind::Struct(fields) = &type_decl.kind {
                compiler.register_struct_with_types(type_decl.name, fields, arena);
            }
        }

        // Compile ALL functions once
        for (i, func) in module.functions.iter().enumerate() {
            let sig = function_sigs.get(i);
            // Skip generic functions - they require monomorphization
            if sig.is_some_and(|s| s.is_generic) {
                continue;
            }
            compiler.compile_function_with_sig(func, arena, expr_types, sig);
        }

        // Compile impl block methods
        for impl_def in &module.impls {
            for method in &impl_def.methods {
                let func = ori_ir::Function {
                    name: method.name,
                    generics: impl_def.generics,
                    params: method.params,
                    return_ty: None,
                    capabilities: vec![],
                    where_clauses: vec![],
                    guard: None,
                    body: method.body,
                    span: method.span,
                    visibility: Visibility::Private,
                };
                compiler.compile_function(&func, arena, expr_types);
            }
        }

        // Compile ALL test wrappers upfront
        let mut test_wrappers = FxHashMap::default();
        let void_sig = FunctionSig {
            params: vec![],
            return_type: TypeId::VOID,
            is_generic: false,
        };

        for test in tests {
            let test_name_str = interner.lookup(test.name);
            let wrapper_name = format!("__test_{test_name_str}");
            let wrapper_name_interned = interner.intern(&wrapper_name);

            let test_func = ori_ir::Function {
                name: wrapper_name_interned,
                generics: ori_ir::GenericParamRange::EMPTY,
                params: ori_ir::ParamRange::EMPTY,
                return_ty: None,
                capabilities: vec![],
                where_clauses: vec![],
                guard: None,
                body: test.body,
                span: ori_ir::Span::new(0, 0),
                visibility: Visibility::Private,
            };
            compiler.compile_function_with_sig(&test_func, arena, expr_types, Some(&void_sig));

            test_wrappers.insert(test.name, wrapper_name);
        }

        // Debug: print IR if requested
        if std::env::var("ORI_DEBUG_LLVM").is_ok() {
            eprintln!("=== LLVM IR for compiled module ===");
            eprintln!("{}", compiler.module().print_to_string().to_string());
            eprintln!("=== END IR ===");
        }

        // Create JIT execution engine ONCE for all tests
        let engine = compiler
            .module()
            .create_jit_execution_engine(OptimizationLevel::None)
            .map_err(|e| LLVMEvalError::new(e.to_string()))?;

        // Add runtime function mappings
        Self::add_runtime_mappings_to_engine(&engine, compiler.module())?;

        Ok(CompiledTestModule {
            engine,
            test_wrappers,
        })
    }

    /// Add runtime function mappings to an execution engine.
    fn add_runtime_mappings_to_engine(
        engine: &ExecutionEngine<'_>,
        module: &inkwell::module::Module<'_>,
    ) -> Result<(), LLVMEvalError> {
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
            (
                "ori_closure_box",
                runtime::ori_closure_box as *const () as usize,
            ),
        ];

        for &(name, addr) in mappings {
            let func = module.get_function(name).ok_or_else(|| {
                LLVMEvalError::new(format!(
                    "{name} not declared - call declare_runtime_functions() before compiling"
                ))
            })?;
            engine.add_global_mapping(&func, addr);
        }

        Ok(())
    }
}

impl Default for OwnedLLVMEvaluator<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::ast::{BinaryOp, Expr, ExprKind};

    #[test]
    fn test_llvm_evaluator_simple() {
        let context = Context::create();
        let interner = StringInterner::new();
        let evaluator = LLVMEvaluator::new(&context, &interner);

        // Create a simple expression: 1 + 2
        let mut arena = ExprArena::new();
        let one = arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: ori_ir::Span::new(0, 1),
        });
        let two = arena.alloc_expr(Expr {
            kind: ExprKind::Int(2),
            span: ori_ir::Span::new(0, 1),
        });
        let add = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: one,
                right: two,
            },
            span: ori_ir::Span::new(0, 1),
        });

        // Evaluate - since we're returning void from the wrapper,
        // we just check it doesn't panic
        let result = evaluator.eval(add, &arena);
        if let Err(e) = &result {
            eprintln!("Error: {}", e.message);
        }
        assert!(result.is_ok(), "Expected Ok, got {result:?}");
    }

    #[test]
    fn test_llvm_evaluator_with_assertion() {
        let context = Context::create();
        let interner = StringInterner::new();
        let evaluator = LLVMEvaluator::new(&context, &interner);

        // Create: assert(condition: true)
        let mut arena = ExprArena::new();
        let true_val = arena.alloc_expr(Expr {
            kind: ExprKind::Bool(true),
            span: ori_ir::Span::new(0, 1),
        });

        // For now just test that we can evaluate a bool
        let result = evaluator.eval(true_val, &arena);
        if let Err(e) = &result {
            eprintln!("Error: {}", e.message);
        }
        assert!(result.is_ok(), "Expected Ok, got {result:?}");
    }
}
