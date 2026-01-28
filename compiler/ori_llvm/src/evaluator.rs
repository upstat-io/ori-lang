//! LLVM-based evaluator for running Ori code.
//!
//! This provides a JIT-based evaluator that compiles Ori code to LLVM IR
//! and executes it natively, as an alternative to the tree-walking interpreter.

use std::collections::HashMap;

use inkwell::context::Context;

use ori_ir::ast::Module;
use ori_ir::{ExprArena, ExprId, Name, StringInterner, TypeId};

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
#[derive(Debug, Clone)]
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
    functions: HashMap<Name, CompiledFunction>,
    /// Type information for expressions
    expr_types: Vec<TypeId>,
}

/// A compiled function ready for execution.
#[allow(dead_code)]
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
            functions: HashMap::new(),
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
        let mut compiler = ModuleCompiler::new(self.context, self.interner, "test_module");
        compiler.declare_runtime();

        // Compile all functions the test might call
        for func in &module.functions {
            compiler.compile_function(func, arena, &self.expr_types);
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
            body: test_body,
            span: ori_ir::Span::new(0, 0),
            is_public: false,
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

        let mut compiler = ModuleCompiler::new(self.context, self.interner, "eval_module");
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
            body: expr,
            span: ori_ir::Span::new(0, 0),
            is_public: false,
        };
        compiler.compile_function(&wrapper_func, arena, &self.expr_types);

        match compiler.run_test("__eval_wrapper") {
            Ok(()) => Ok(LLVMValue::Void),
            Err(msg) => Err(LLVMEvalError::new(msg)),
        }
    }
}

/// Function type signature for LLVM compilation.
#[derive(Debug, Clone)]
pub struct FunctionSig {
    /// Parameter types
    pub params: Vec<TypeId>,
    /// Return type
    pub return_type: TypeId,
}

/// LLVM-based evaluator that owns its context.
///
/// This is the recommended evaluator for use in applications that don't
/// want to manage the LLVM context lifetime themselves.
pub struct OwnedLLVMEvaluator {
    context: Context,
    /// Compiled functions by name
    functions: HashMap<Name, CompiledFunction>,
}

impl OwnedLLVMEvaluator {
    /// Create a new owned LLVM evaluator.
    pub fn new() -> Self {
        OwnedLLVMEvaluator {
            context: Context::create(),
            functions: HashMap::new(),
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
    /// - `expr_types`: Type of each expression (indexed by ExprId)
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
        let mut compiler = ModuleCompiler::new(&self.context, interner, "test_module");
        compiler.declare_runtime();

        // Compile all functions the test might call
        for (i, func) in module.functions.iter().enumerate() {
            let sig = function_sigs.get(i);
            compiler.compile_function_with_sig(func, arena, expr_types, sig);
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
            body: test_body,
            span: ori_ir::Span::new(0, 0),
            is_public: false,
        };
        let void_sig = FunctionSig {
            params: vec![],
            return_type: TypeId::VOID,
        };
        compiler.compile_function_with_sig(&test_func, arena, expr_types, Some(&void_sig));

        // JIT compile and run
        match compiler.run_test(&wrapper_name) {
            Ok(()) => Ok(LLVMValue::Void),
            Err(msg) => Err(LLVMEvalError::new(msg)),
        }
    }
}

impl Default for OwnedLLVMEvaluator {
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
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
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
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    }
}
