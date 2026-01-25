//! Type checker core implementation.
//!
//! Contains the TypeChecker struct and main entry point for type checking.
//!
//! # Module Structure
//!
//! - `types`: Output types (TypedModule, FunctionType, etc.)
//! - `signatures`: Function signature inference
//! - `pattern_binding`: Pattern to type binding
//! - `cycle_detection`: Closure self-capture detection
//! - `trait_registration`: Trait and impl registration
//! - `bound_checking`: Trait bound verification

mod types;
mod signatures;
mod pattern_binding;
mod cycle_detection;
mod type_registration;
mod trait_registration;
mod bound_checking;

pub use types::{TypedModule, GenericBound, FunctionType, TypeCheckError};

use crate::ir::{
    Name, Span, ExprId, ExprArena, Module, Function, TestDef,
    StringInterner, TypeId,
};
use crate::parser::ParseResult;
use crate::patterns::PatternRegistry;
use crate::types::{Type, TypeEnv, InferenceContext, TypeError};
use crate::context::{CompilerContext, SharedRegistry};
use crate::diagnostic::queue::{DiagnosticQueue, DiagnosticConfig};
use super::operators::TypeOperatorRegistry;
use super::type_registry::{TypeRegistry, TraitRegistry};
use super::infer;
use std::collections::HashMap;
use std::rc::Rc;

/// Type checker state.
pub struct TypeChecker<'a> {
    pub(crate) arena: &'a ExprArena,
    pub(crate) interner: &'a StringInterner,
    pub(crate) ctx: InferenceContext,
    pub(crate) env: TypeEnv,
    /// Shared base environment for O(1) child scope creation.
    /// Set after first pass to avoid O(n²) cloning.
    pub(crate) base_env: Option<Rc<TypeEnv>>,
    pub(crate) expr_types: HashMap<u32, Type>,
    pub(crate) errors: Vec<TypeCheckError>,
    /// Pattern registry for function_exp type checking.
    pub(crate) registry: SharedRegistry<PatternRegistry>,
    /// Type operator registry for binary operation type checking.
    pub(crate) type_operator_registry: TypeOperatorRegistry,
    /// Registry for user-defined types (structs, enums, aliases).
    pub(crate) type_registry: TypeRegistry,
    /// Registry for traits and implementations.
    pub(crate) trait_registry: TraitRegistry,
    /// Function signatures for constraint checking during calls.
    pub(crate) function_sigs: HashMap<Name, FunctionType>,
    /// Diagnostic queue for deduplication and error limits.
    /// Only active when source is provided.
    diagnostic_queue: Option<DiagnosticQueue>,
    /// Source code for line/column computation.
    source: Option<String>,
}

impl<'a> TypeChecker<'a> {
    /// Create a new type checker with default registries.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        TypeChecker {
            arena,
            interner,
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            base_env: None,
            expr_types: HashMap::new(),
            errors: Vec::new(),
            registry: SharedRegistry::new(PatternRegistry::new()),
            type_operator_registry: TypeOperatorRegistry::new(),
            type_registry: TypeRegistry::new(),
            trait_registry: TraitRegistry::new(),
            function_sigs: HashMap::new(),
            diagnostic_queue: None,
            source: None,
        }
    }

    /// Create a type checker with source code for diagnostic queue features.
    ///
    /// When source is provided, error deduplication and limits are enabled.
    pub fn with_source(arena: &'a ExprArena, interner: &'a StringInterner, source: String) -> Self {
        TypeChecker {
            arena,
            interner,
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            base_env: None,
            expr_types: HashMap::new(),
            errors: Vec::new(),
            registry: SharedRegistry::new(PatternRegistry::new()),
            type_operator_registry: TypeOperatorRegistry::new(),
            type_registry: TypeRegistry::new(),
            trait_registry: TraitRegistry::new(),
            function_sigs: HashMap::new(),
            diagnostic_queue: Some(DiagnosticQueue::new()),
            source: Some(source),
        }
    }

    /// Create a type checker with a custom compiler context.
    ///
    /// This enables dependency injection for testing with mock registries.
    pub fn with_context(
        arena: &'a ExprArena,
        interner: &'a StringInterner,
        context: &CompilerContext,
    ) -> Self {
        TypeChecker {
            arena,
            interner,
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            base_env: None,
            expr_types: HashMap::new(),
            errors: Vec::new(),
            registry: context.pattern_registry.clone(),
            type_operator_registry: TypeOperatorRegistry::new(),
            type_registry: TypeRegistry::new(),
            trait_registry: TraitRegistry::new(),
            function_sigs: HashMap::new(),
            diagnostic_queue: None,
            source: None,
        }
    }

    /// Create a type checker with source and custom diagnostic configuration.
    pub fn with_source_and_config(
        arena: &'a ExprArena,
        interner: &'a StringInterner,
        source: String,
        config: DiagnosticConfig,
    ) -> Self {
        TypeChecker {
            arena,
            interner,
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            base_env: None,
            expr_types: HashMap::new(),
            errors: Vec::new(),
            registry: SharedRegistry::new(PatternRegistry::new()),
            type_operator_registry: TypeOperatorRegistry::new(),
            type_registry: TypeRegistry::new(),
            trait_registry: TraitRegistry::new(),
            function_sigs: HashMap::new(),
            diagnostic_queue: Some(DiagnosticQueue::with_config(config)),
            source: Some(source),
        }
    }

    /// Type check a module.
    pub fn check_module(mut self, module: &Module) -> TypedModule {
        let mut function_types = Vec::new();

        // Pass 0a: Register user-defined types (structs, enums, newtypes)
        // Must be done before traits, as traits/impls may reference these types.
        self.register_types(module);

        // Pass 0b: Register traits and implementations
        self.register_traits(module);
        self.register_impls(module);

        // First pass: collect function signatures
        for func in &module.functions {
            let func_type = self.infer_function_signature(func);
            function_types.push(func_type.clone());

            // Store signature for constraint checking during calls
            self.function_sigs.insert(func.name, func_type.clone());

            // Bind function name to its type
            let fn_type = Type::Function {
                params: func_type.params.clone(),
                ret: Box::new(func_type.return_type.clone()),
            };
            self.env.bind(func.name, fn_type);
        }

        // Freeze the base environment into an Rc for O(1) child scope creation.
        // This avoids O(n²) cloning when checking many functions.
        self.base_env = Some(Rc::new(std::mem::take(&mut self.env)));

        // Second pass: type check function bodies
        for (func, func_type) in module.functions.iter().zip(function_types.iter()) {
            self.check_function(func, func_type);
        }

        // Third pass: type check test bodies
        for test in &module.tests {
            self.check_test(test);
        }

        // Build expression types vector with resolved types
        let max_expr = self.expr_types.keys().max().copied().unwrap_or(0);
        let mut expr_types = vec![Type::Error; (max_expr + 1) as usize];
        for (id, ty) in self.expr_types {
            expr_types[id as usize] = self.ctx.resolve(&ty);
        }

        // Resolve function types
        let resolved_function_types: Vec<FunctionType> = function_types
            .into_iter()
            .map(|ft| FunctionType {
                name: ft.name,
                generics: ft.generics,
                params: ft.params.iter().map(|t| self.ctx.resolve(t)).collect(),
                return_type: self.ctx.resolve(&ft.return_type),
            })
            .collect();

        TypedModule {
            expr_types,
            function_types: resolved_function_types,
            errors: self.errors,
        }
    }

    /// Convert a TypeId to a Type.
    ///
    /// TypeId is the parsed type annotation representation.
    /// Type is the type checker's internal representation.
    pub(crate) fn type_id_to_type(&mut self, type_id: TypeId) -> Type {
        match type_id {
            TypeId::INT => Type::Int,
            TypeId::FLOAT => Type::Float,
            TypeId::BOOL => Type::Bool,
            TypeId::STR => Type::Str,
            TypeId::CHAR => Type::Char,
            TypeId::BYTE => Type::Byte,
            TypeId::VOID => Type::Unit,
            TypeId::NEVER => Type::Never,
            TypeId::INFER => self.ctx.fresh_var(),
            _ => {
                // Look up compound types in the type registry
                if let Some(ty) = self.type_registry.to_type(type_id) {
                    ty
                } else {
                    // Unknown compound type - use a fresh var for error recovery
                    self.ctx.fresh_var()
                }
            }
        }
    }

    /// Type check a function body.
    fn check_function(&mut self, func: &Function, func_type: &FunctionType) {
        // Create scope for function parameters
        let mut func_env = if let Some(ref base) = self.base_env {
            base.child()
        } else {
            self.env.child()
        };

        // Bind parameters
        let params = self.arena.get_params(func.params);
        for (param, param_type) in params.iter().zip(func_type.params.iter()) {
            func_env.bind(param.name, param_type.clone());
        }

        // Save current env and switch to function env
        let old_env = std::mem::replace(&mut self.env, func_env);

        // Infer body type
        let body_type = infer::infer_expr(self, func.body);

        // Unify with declared return type
        if let Err(e) = self.ctx.unify(&body_type, &func_type.return_type) {
            let span = self.arena.get_expr(func.body).span;
            self.report_type_error(e, span);
        }

        // Restore environment
        self.env = old_env;
    }

    /// Type check a test body.
    fn check_test(&mut self, test: &TestDef) {
        // Infer parameter types
        let params: Vec<Type> = self.arena.get_params(test.params)
            .iter()
            .map(|p| {
                match p.ty {
                    Some(type_id) => self.type_id_to_type(type_id),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect();

        // Infer return type
        let return_type = match test.return_ty {
            Some(type_id) => self.type_id_to_type(type_id),
            None => self.ctx.fresh_var(),
        };

        // Create scope for test parameters
        let mut test_env = if let Some(ref base) = self.base_env {
            base.child()
        } else {
            self.env.child()
        };

        // Bind parameters
        let param_defs = self.arena.get_params(test.params);
        for (param, param_type) in param_defs.iter().zip(params.iter()) {
            test_env.bind(param.name, param_type.clone());
        }

        // Save current env and switch to test env
        let old_env = std::mem::replace(&mut self.env, test_env);

        // Infer body type
        let body_type = infer::infer_expr(self, test.body);

        // Unify with declared return type
        if let Err(e) = self.ctx.unify(&body_type, &return_type) {
            let span = self.arena.get_expr(test.body).span;
            self.report_type_error(e, span);
        }

        // Restore environment
        self.env = old_env;
    }

    /// Report a type error.
    pub(crate) fn report_type_error(&mut self, err: TypeError, span: Span) {
        let diag = err.to_diagnostic(span, self.interner);
        let error = TypeCheckError {
            message: diag.message.clone(),
            span,
            code: diag.code,
        };

        // If we have a diagnostic queue, use it for deduplication/limits
        if let (Some(ref mut queue), Some(ref source)) = (&mut self.diagnostic_queue, &self.source) {
            let is_soft = error.is_soft();
            // Add to queue - it will handle deduplication and limits
            if queue.add_with_source(diag, source, is_soft) {
                self.errors.push(error);
            }
        } else {
            // No queue - add directly
            self.errors.push(error);
        }
    }

    /// Check if the error limit has been reached.
    ///
    /// When source is provided, the diagnostic queue tracks error limits.
    /// Returns false if no source/queue is configured.
    pub fn limit_reached(&self) -> bool {
        self.diagnostic_queue.as_ref().map_or(false, |q| q.limit_reached())
    }

    /// Store the type for an expression.
    pub(crate) fn store_type(&mut self, expr_id: ExprId, ty: Type) {
        self.expr_types.insert(expr_id.index() as u32, ty);
    }
}

/// Type check a parsed module.
pub fn type_check(
    parse_result: &ParseResult,
    interner: &StringInterner,
) -> TypedModule {
    let checker = TypeChecker::new(&parse_result.arena, interner);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with source code for diagnostic queue features.
///
/// When source is provided, error deduplication and limits are enabled.
pub fn type_check_with_source(
    parse_result: &ParseResult,
    interner: &StringInterner,
    source: String,
) -> TypedModule {
    let checker = TypeChecker::with_source(&parse_result.arena, interner, source);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with source and custom diagnostic configuration.
pub fn type_check_with_config(
    parse_result: &ParseResult,
    interner: &StringInterner,
    source: String,
    config: DiagnosticConfig,
) -> TypedModule {
    let checker = TypeChecker::with_source_and_config(&parse_result.arena, interner, source, config);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with a custom compiler context.
///
/// This allows dependency injection of custom registries for testing.
pub fn type_check_with_context(
    parse_result: &ParseResult,
    interner: &StringInterner,
    context: &CompilerContext,
) -> TypedModule {
    let checker = TypeChecker::with_context(&parse_result.arena, interner, context);
    checker.check_module(&parse_result.module)
}

#[cfg(test)]
mod tests;
