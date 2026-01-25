//! Type checker core implementation.
//!
//! Contains the `TypeChecker` struct and main entry point for type checking.
//!
//! # Module Structure
//!
//! - `types`: Output types (`TypedModule`, `FunctionType`, etc.)
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
pub(crate) mod bound_checking;

pub use types::{TypedModule, GenericBound, FunctionType, TypeCheckError};
pub(crate) use cycle_detection::add_pattern_bindings;

use crate::ir::{
    Name, Span, ExprId, ExprArena, Module, Function, TestDef,
    StringInterner, TypeId, ParsedType,
};
use crate::parser::ParseResult;
use sigil_patterns::PatternRegistry;
use crate::types::{Type, TypeEnv, InferenceContext, TypeError};
use crate::context::{CompilerContext, SharedRegistry};
use crate::diagnostic::queue::{DiagnosticQueue, DiagnosticConfig};
use super::operators::TypeOperatorRegistry;
use super::type_registry::{TypeRegistry, TraitRegistry};
use super::infer;
use std::collections::HashMap;

/// Type checker state.
pub struct TypeChecker<'a> {
    pub(crate) arena: &'a ExprArena,
    pub(crate) interner: &'a StringInterner,
    pub(crate) ctx: InferenceContext,
    pub(crate) env: TypeEnv,
    /// Frozen base environment for child scope creation.
    /// Set after first pass to avoid modifying the base during function checking.
    pub(crate) base_env: Option<TypeEnv>,
    pub(crate) expr_types: HashMap<usize, Type>,
    pub(crate) errors: Vec<TypeCheckError>,
    /// Pattern registry for `function_exp` type checking.
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
    /// The Self type when inside an impl block.
    pub(crate) current_impl_self: Option<Type>,
    /// Config variable types for $name references.
    pub(crate) config_types: HashMap<Name, Type>,
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
            current_impl_self: None,
            config_types: HashMap::new(),
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
            current_impl_self: None,
            config_types: HashMap::new(),
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
            current_impl_self: None,
            config_types: HashMap::new(),
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
            current_impl_self: None,
            config_types: HashMap::new(),
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

        // Pass 0c: Register derived trait implementations
        // Must be done after register_types so we know the type structure,
        // but after register_impls so explicit impls take precedence.
        super::derives::register_derived_impls(module, &mut self.trait_registry, self.interner);

        // Pass 0d: Register config variables
        self.register_configs(module);

        // First pass: collect function signatures
        for func in &module.functions {
            let func_type = self.infer_function_signature(func);
            function_types.push(func_type.clone());

            // Validate capabilities in uses clause
            self.validate_capabilities(func);

            // Store signature for constraint checking during calls
            self.function_sigs.insert(func.name, func_type.clone());

            // Bind function name to its type
            let fn_type = Type::Function {
                params: func_type.params.clone(),
                ret: Box::new(func_type.return_type.clone()),
            };
            self.env.bind(func.name, fn_type);
        }

        // Freeze the base environment for child scope creation.
        // This avoids modifying the base during function checking.
        self.base_env = Some(std::mem::take(&mut self.env));

        // Second pass: type check function bodies
        for (func, func_type) in module.functions.iter().zip(function_types.iter()) {
            self.check_function(func, func_type);
        }

        // Third pass: type check test bodies
        for test in &module.tests {
            self.check_test(test);
        }

        // Fourth pass: type check impl method bodies
        for impl_def in &module.impls {
            self.check_impl_methods(impl_def);
        }

        // Build expression types vector with resolved types
        let max_expr = self.expr_types.keys().max().copied().unwrap_or(0);
        let mut expr_types = vec![Type::Error; max_expr + 1];
        for (id, ty) in self.expr_types {
            expr_types[id] = self.ctx.resolve(&ty);
        }

        // Resolve function types
        let resolved_function_types: Vec<FunctionType> = function_types
            .into_iter()
            .map(|ft| FunctionType {
                name: ft.name,
                generics: ft.generics,
                params: ft.params.iter().map(|t| self.ctx.resolve(t)).collect(),
                return_type: self.ctx.resolve(&ft.return_type),
                capabilities: ft.capabilities,
            })
            .collect();

        TypedModule {
            expr_types,
            function_types: resolved_function_types,
            errors: self.errors,
        }
    }

    /// Convert a `TypeId` to a Type.
    ///
    /// `TypeId` is the parsed type annotation representation for primitives.
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

    /// Convert a `ParsedType` to a Type.
    ///
    /// `ParsedType` captures the full structure of type annotations as parsed.
    /// This method resolves them into the type checker's internal representation.
    pub(crate) fn parsed_type_to_type(&mut self, parsed: &ParsedType) -> Type {
        match parsed {
            ParsedType::Primitive(type_id) => self.type_id_to_type(*type_id),
            ParsedType::Infer => self.ctx.fresh_var(),
            ParsedType::SelfType => {
                // Self type resolution is handled during impl checking.
                self.ctx.fresh_var()
            }
            ParsedType::Named { name, type_args } => {
                // Handle well-known generic types
                let name_str = self.interner.lookup(*name);
                match name_str {
                    "Option" => {
                        if type_args.len() == 1 {
                            let inner = self.parsed_type_to_type(&type_args[0]);
                            Type::Option(Box::new(inner))
                        } else {
                            Type::Option(Box::new(self.ctx.fresh_var()))
                        }
                    }
                    "Result" => {
                        if type_args.len() == 2 {
                            let ok = self.parsed_type_to_type(&type_args[0]);
                            let err = self.parsed_type_to_type(&type_args[1]);
                            Type::Result { ok: Box::new(ok), err: Box::new(err) }
                        } else {
                            Type::Result {
                                ok: Box::new(self.ctx.fresh_var()),
                                err: Box::new(self.ctx.fresh_var()),
                            }
                        }
                    }
                    "Set" => {
                        if type_args.len() == 1 {
                            let inner = self.parsed_type_to_type(&type_args[0]);
                            Type::Set(Box::new(inner))
                        } else {
                            Type::Set(Box::new(self.ctx.fresh_var()))
                        }
                    }
                    "Range" => {
                        if type_args.len() == 1 {
                            let inner = self.parsed_type_to_type(&type_args[0]);
                            Type::Range(Box::new(inner))
                        } else {
                            Type::Range(Box::new(self.ctx.fresh_var()))
                        }
                    }
                    "Channel" => {
                        if type_args.len() == 1 {
                            let inner = self.parsed_type_to_type(&type_args[0]);
                            Type::Channel(Box::new(inner))
                        } else {
                            Type::Channel(Box::new(self.ctx.fresh_var()))
                        }
                    }
                    _ => {
                        // User-defined type or type parameter
                        // Treat as a named type reference - resolution happens during unification
                        Type::Named(*name)
                    }
                }
            }
            ParsedType::List(inner) => {
                let elem_ty = self.parsed_type_to_type(inner);
                Type::List(Box::new(elem_ty))
            }
            ParsedType::Tuple(elems) => {
                let types: Vec<Type> = elems.iter()
                    .map(|e| self.parsed_type_to_type(e))
                    .collect();
                Type::Tuple(types)
            }
            ParsedType::Function { params, ret } => {
                let param_types: Vec<Type> = params.iter()
                    .map(|p| self.parsed_type_to_type(p))
                    .collect();
                let ret_ty = self.parsed_type_to_type(ret);
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_ty),
                }
            }
            ParsedType::Map { key, value } => {
                let key_ty = self.parsed_type_to_type(key);
                let value_ty = self.parsed_type_to_type(value);
                Type::Map {
                    key: Box::new(key_ty),
                    value: Box::new(value_ty),
                }
            }
            ParsedType::AssociatedType { base, assoc_name } => {
                // Associated type projection like Self.Item or T.Item
                // The base type is converted, and we create a Projection type.
                // The trait_name is not known at parse time in general; we use
                // a placeholder that will be resolved during impl checking or
                // when we have more context about which trait defines this associated type.
                let base_ty = self.parsed_type_to_type(base);

                // For now, use a placeholder trait name. In a more complete implementation,
                // we would look up which trait defines this associated type based on
                // the context (current trait definition or trait bounds on the base type).
                // Using the assoc_name as the trait_name placeholder for now.
                Type::Projection {
                    base: Box::new(base_ty),
                    trait_name: *assoc_name, // Placeholder - resolved during impl checking
                    assoc_name: *assoc_name,
                }
            }
        }
    }

    /// Resolve a `ParsedType` to a Type, substituting generic type variables.
    ///
    /// This is used when inferring function signatures where type annotations
    /// may refer to generic parameters (e.g., `T` in `@foo<T>(x: T) -> T`).
    pub(crate) fn resolve_parsed_type_with_generics(
        &mut self,
        parsed: &ParsedType,
        generic_type_vars: &HashMap<Name, Type>,
    ) -> Type {
        match parsed {
            ParsedType::Named { name, type_args } if type_args.is_empty() => {
                // Check if this name refers to a generic parameter
                if let Some(type_var) = generic_type_vars.get(name) {
                    return type_var.clone();
                }
                // Fall through to regular resolution
                self.parsed_type_to_type(parsed)
            }
            ParsedType::Named { name, type_args } => {
                // Handle generic types like Option<T> where T might be a generic param
                let name_str = self.interner.lookup(*name);
                match name_str {
                    "Option" if type_args.len() == 1 => {
                        let inner = self.resolve_parsed_type_with_generics(&type_args[0], generic_type_vars);
                        Type::Option(Box::new(inner))
                    }
                    "Result" if type_args.len() == 2 => {
                        let ok = self.resolve_parsed_type_with_generics(&type_args[0], generic_type_vars);
                        let err = self.resolve_parsed_type_with_generics(&type_args[1], generic_type_vars);
                        Type::Result { ok: Box::new(ok), err: Box::new(err) }
                    }
                    _ => self.parsed_type_to_type(parsed)
                }
            }
            ParsedType::List(inner) => {
                let elem_ty = self.resolve_parsed_type_with_generics(inner, generic_type_vars);
                Type::List(Box::new(elem_ty))
            }
            ParsedType::Function { params, ret } => {
                let param_types: Vec<Type> = params.iter()
                    .map(|p| self.resolve_parsed_type_with_generics(p, generic_type_vars))
                    .collect();
                let ret_ty = self.resolve_parsed_type_with_generics(ret, generic_type_vars);
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_ty),
                }
            }
            ParsedType::AssociatedType { base, assoc_name } => {
                // Resolve the base type with generic substitutions
                let base_ty = self.resolve_parsed_type_with_generics(base, generic_type_vars);
                Type::Projection {
                    base: Box::new(base_ty),
                    trait_name: *assoc_name, // Placeholder
                    assoc_name: *assoc_name,
                }
            }
            _ => self.parsed_type_to_type(parsed),
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
            self.report_type_error(&e, span);
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
                match &p.ty {
                    Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect();

        // Infer return type
        let return_type = match &test.return_ty {
            Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
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
            self.report_type_error(&e, span);
        }

        // Restore environment
        self.env = old_env;
    }

    /// Type check all methods in an impl block.
    fn check_impl_methods(&mut self, impl_def: &crate::ir::ImplDef) {
        let self_ty = self.parsed_type_to_type(&impl_def.self_ty);
        let prev_self = self.enter_impl(self_ty.clone());

        for method in &impl_def.methods {
            self.check_impl_method(method, &self_ty);
        }

        self.exit_impl(prev_self);
    }

    /// Type check a single impl method.
    fn check_impl_method(&mut self, method: &crate::ir::ImplMethod, self_ty: &Type) {
        // Create scope for method parameters
        let mut method_env = if let Some(ref base) = self.base_env {
            base.child()
        } else {
            self.env.child()
        };

        // Bind parameters (first param is typically `self`)
        let params = self.arena.get_params(method.params);
        for param in params {
            let param_ty = if let Some(ref parsed_ty) = param.ty {
                self.parsed_type_to_type(parsed_ty)
            } else {
                // If first param is named `self`, bind to Self type
                let self_name = self.interner.intern("self");
                if param.name == self_name {
                    self_ty.clone()
                } else {
                    self.ctx.fresh_var()
                }
            };
            method_env.bind(param.name, param_ty);
        }

        // Save current env and switch to method env
        let old_env = std::mem::replace(&mut self.env, method_env);

        // Infer body type
        let body_type = infer::infer_expr(self, method.body);

        // Unify with declared return type
        let return_type = self.parsed_type_to_type(&method.return_ty);
        if let Err(e) = self.ctx.unify(&body_type, &return_type) {
            let span = self.arena.get_expr(method.body).span;
            self.report_type_error(&e, span);
        }

        // Restore environment
        self.env = old_env;
    }

    /// Validate that capabilities in a function's `uses` clause refer to valid traits.
    ///
    /// For each capability in the `uses` clause, checks that a trait with that name exists
    /// in the trait registry. If not, reports an error.
    fn validate_capabilities(&mut self, func: &Function) {
        for cap_ref in &func.capabilities {
            if !self.trait_registry.has_trait(cap_ref.name) {
                let cap_name = self.interner.lookup(cap_ref.name);
                self.errors.push(TypeCheckError {
                    message: format!("unknown capability `{cap_name}`: capabilities must be defined traits"),
                    span: cap_ref.span,
                    code: crate::diagnostic::ErrorCode::E2012,
                });
            }
        }
    }

    /// Resolve a type through any alias chain.
    ///
    /// If the type is a named type that refers to an alias, returns the
    /// underlying type. Otherwise returns the type unchanged.
    pub(crate) fn resolve_through_aliases(&self, ty: &Type) -> Type {
        use crate::typeck::type_registry::TypeKind;

        match ty {
            Type::Named(name) => {
                if let Some(entry) = self.type_registry.get_by_name(*name) {
                    if let TypeKind::Alias { target } = &entry.kind {
                        return self.resolve_through_aliases(target);
                    }
                }
                ty.clone()
            }
            _ => ty.clone(),
        }
    }

    /// Enter an impl block context, setting the Self type.
    ///
    /// Returns the previous Self type to restore later.
    pub(crate) fn enter_impl(&mut self, self_ty: Type) -> Option<Type> {
        self.current_impl_self.replace(self_ty)
    }

    /// Exit an impl block context, restoring the previous Self type.
    pub(crate) fn exit_impl(&mut self, prev: Option<Type>) {
        self.current_impl_self = prev;
    }

    /// Register config variable types.
    ///
    /// Infers the type of each config value and stores it for $name references.
    fn register_configs(&mut self, module: &Module) {
        for config in &module.configs {
            let config_ty = infer::infer_expr(self, config.value);
            self.config_types.insert(config.name, config_ty);
        }
    }

    /// Report a type error.
    pub(crate) fn report_type_error(&mut self, err: &TypeError, span: Span) {
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
        self.diagnostic_queue.as_ref().is_some_and(sigil_diagnostic::queue::DiagnosticQueue::limit_reached)
    }

    /// Store the type for an expression.
    pub(crate) fn store_type(&mut self, expr_id: ExprId, ty: Type) {
        self.expr_types.insert(expr_id.index(), ty);
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
