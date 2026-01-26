//! Type checker core implementation.
//!
//! Contains the `TypeChecker` struct and main entry point for type checking.
//!
//! # Module Structure
//!
//! - `types`: Output types (`TypedModule`, `FunctionType`, etc.)
//! - `components`: Component structs for TypeChecker organization
//! - `scope_guards`: RAII scope guards for context management
//! - `signatures`: Function signature inference
//! - `pattern_binding`: Pattern to type binding
//! - `cycle_detection`: Closure self-capture detection
//! - `type_registration`: User-defined type registration
//! - `trait_registration`: Trait and impl registration
//! - `bound_checking`: Trait bound verification
//! - `builder`: TypeChecker builder pattern

mod builder;
pub mod bound_checking;
pub mod components;
mod cycle_detection;
mod pattern_binding;
mod scope_guards;
mod signatures;
mod trait_registration;
mod type_registration;
pub mod types;

pub use builder::TypeCheckerBuilder;
pub use components::{
    CheckContext, DiagnosticState, InferenceState, Registries, ScopeContext,
};
pub use cycle_detection::add_pattern_bindings;
pub use scope_guards::{SavedCapabilityContext, SavedImplContext};
pub use types::{FunctionType, GenericBound, TypeCheckError, TypedModule, WhereConstraint};

use sigil_diagnostic::queue::DiagnosticConfig;
use sigil_ir::{ExprArena, ExprId, Module, Function, Name, ParsedType, Span, StringInterner, TestDef, TypeId};
use sigil_types::{Type, TypeError, TypeScheme};
use std::collections::HashMap;

use crate::infer;
use crate::registry::TypeKind;

/// Type checker state.
///
/// Organized into logical components for better testability and maintainability:
/// - `context`: Immutable references to arena and interner
/// - `inference`: Mutable inference state (context, environments, expression types)
/// - `registries`: Pattern, type operator, type, and trait registries
/// - `diagnostics`: Error collection and diagnostic queue
/// - `scope`: Function signatures, impl Self type, config types, capabilities
pub struct TypeChecker<'a> {
    /// Immutable references for expression lookup.
    pub(crate) context: CheckContext<'a>,
    /// Mutable inference state.
    pub(crate) inference: InferenceState,
    /// Registry bundle for patterns, types, and traits.
    pub(crate) registries: Registries,
    /// Diagnostic collection state.
    pub(crate) diagnostics: DiagnosticState,
    /// Function and scope context state.
    pub(crate) scope: ScopeContext,
}

impl<'a> TypeChecker<'a> {
    /// Create a new type checker with default registries.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        TypeCheckerBuilder::new(arena, interner).build()
    }

    /// Create a type checker with source code for diagnostic queue features.
    ///
    /// When source is provided, error deduplication and limits are enabled.
    pub fn with_source(arena: &'a ExprArena, interner: &'a StringInterner, source: String) -> Self {
        TypeCheckerBuilder::new(arena, interner)
            .with_source(source)
            .build()
    }

    /// Create a type checker with source and custom diagnostic configuration.
    pub fn with_source_and_config(
        arena: &'a ExprArena,
        interner: &'a StringInterner,
        source: String,
        config: DiagnosticConfig,
    ) -> Self {
        TypeCheckerBuilder::new(arena, interner)
            .with_source(source)
            .with_diagnostic_config(config)
            .build()
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
        crate::derives::register_derived_impls(module, &mut self.registries.traits, self.context.interner);

        // Pass 0d: Register config variables
        self.register_configs(module);

        // First pass: collect function signatures
        for func in &module.functions {
            let func_type = self.infer_function_signature(func);
            function_types.push(func_type.clone());

            // Validate capabilities in uses clause
            self.validate_capabilities(func);

            // Store signature for constraint checking during calls
            self.scope.function_sigs.insert(func.name, func_type.clone());

            // Bind function name to its type
            // For generic functions, create a polymorphic type scheme
            // so each call site gets fresh type variables
            let fn_type = Type::Function {
                params: func_type.params.clone(),
                ret: Box::new(func_type.return_type.clone()),
            };

            // Extract type vars from generic parameters
            let type_vars: Vec<_> = func_type.generics.iter()
                .filter_map(|g| {
                    if let Type::Var(tv) = &g.type_var {
                        Some(tv.clone())
                    } else {
                        None
                    }
                })
                .collect();

            if type_vars.is_empty() {
                self.inference.env.bind(func.name, fn_type);
            } else {
                let scheme = TypeScheme::poly(type_vars, fn_type);
                self.inference.env.bind_scheme(func.name, scheme);
            }
        }

        // Freeze the base environment for child scope creation.
        // This avoids modifying the base during function checking.
        self.inference.base_env = Some(std::mem::take(&mut self.inference.env));

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
        let max_expr = self.inference.expr_types.keys().max().copied().unwrap_or(0);
        let mut expr_types = vec![Type::Error; max_expr + 1];
        for (id, ty) in self.inference.expr_types {
            expr_types[id] = self.inference.ctx.resolve(&ty);
        }

        // Resolve function types
        let resolved_function_types: Vec<FunctionType> = function_types
            .into_iter()
            .map(|ft| FunctionType {
                name: ft.name,
                generics: ft.generics,
                where_constraints: ft.where_constraints,
                params: ft.params.iter().map(|t| self.inference.ctx.resolve(t)).collect(),
                return_type: self.inference.ctx.resolve(&ft.return_type),
                capabilities: ft.capabilities,
            })
            .collect();

        TypedModule {
            expr_types,
            function_types: resolved_function_types,
            errors: self.diagnostics.errors,
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
            TypeId::INFER => self.inference.ctx.fresh_var(),
            _ => {
                // Look up compound types in the type registry
                if let Some(ty) = self.registries.types.to_type(type_id) {
                    ty
                } else {
                    // Unknown compound type - use a fresh var for error recovery
                    self.inference.ctx.fresh_var()
                }
            }
        }
    }

    /// Convert a `ParsedType` to a Type.
    ///
    /// `ParsedType` captures the full structure of type annotations as parsed.
    /// This method resolves them into the type checker's internal representation.
    pub(crate) fn parsed_type_to_type(&mut self, parsed: &ParsedType) -> Type {
        self.resolve_parsed_type_internal(parsed, None)
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
        self.resolve_parsed_type_internal(parsed, Some(generic_type_vars))
    }

    /// Internal type resolution with optional generic substitutions.
    ///
    /// Consolidates the logic from `parsed_type_to_type` and `resolve_parsed_type_with_generics`
    /// to eliminate code duplication.
    fn resolve_parsed_type_internal(
        &mut self,
        parsed: &ParsedType,
        generic_type_vars: Option<&HashMap<Name, Type>>,
    ) -> Type {
        match parsed {
            ParsedType::Primitive(type_id) => self.type_id_to_type(*type_id),
            ParsedType::Infer => self.inference.ctx.fresh_var(),
            ParsedType::SelfType => {
                // Self type resolution is handled during impl checking.
                self.inference.ctx.fresh_var()
            }
            ParsedType::Named { name, type_args } => {
                // Check if this name refers to a generic parameter (when resolving with generics)
                if type_args.is_empty() {
                    if let Some(vars) = generic_type_vars {
                        if let Some(type_var) = vars.get(name) {
                            return type_var.clone();
                        }
                    }
                }
                // Handle well-known generic types
                self.resolve_well_known_generic(*name, type_args, generic_type_vars)
            }
            ParsedType::List(inner) => {
                let elem_ty = self.resolve_parsed_type_internal(inner, generic_type_vars);
                Type::List(Box::new(elem_ty))
            }
            ParsedType::Tuple(elems) => {
                let types: Vec<Type> = elems
                    .iter()
                    .map(|e| self.resolve_parsed_type_internal(e, generic_type_vars))
                    .collect();
                Type::Tuple(types)
            }
            ParsedType::Function { params, ret } => {
                let param_types: Vec<Type> = params
                    .iter()
                    .map(|p| self.resolve_parsed_type_internal(p, generic_type_vars))
                    .collect();
                let ret_ty = self.resolve_parsed_type_internal(ret, generic_type_vars);
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_ty),
                }
            }
            ParsedType::Map { key, value } => {
                let key_ty = self.resolve_parsed_type_internal(key, generic_type_vars);
                let value_ty = self.resolve_parsed_type_internal(value, generic_type_vars);
                Type::Map {
                    key: Box::new(key_ty),
                    value: Box::new(value_ty),
                }
            }
            ParsedType::AssociatedType { base, assoc_name } => {
                self.make_projection_type(base, *assoc_name, generic_type_vars)
            }
        }
    }

    /// Resolve a well-known generic type (Option, Result, Set, Range, Channel).
    ///
    /// Returns the appropriate Type for known generic types, or a Named type for
    /// user-defined types and type parameters.
    fn resolve_well_known_generic(
        &mut self,
        name: Name,
        type_args: &[ParsedType],
        generic_type_vars: Option<&HashMap<Name, Type>>,
    ) -> Type {
        let name_str = self.context.interner.lookup(name);
        match name_str {
            "Option" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_parsed_type_internal(&type_args[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Option(Box::new(inner))
            }
            "Result" => {
                let (ok, err) = if type_args.len() == 2 {
                    (
                        self.resolve_parsed_type_internal(&type_args[0], generic_type_vars),
                        self.resolve_parsed_type_internal(&type_args[1], generic_type_vars),
                    )
                } else {
                    (self.inference.ctx.fresh_var(), self.inference.ctx.fresh_var())
                };
                Type::Result {
                    ok: Box::new(ok),
                    err: Box::new(err),
                }
            }
            "Set" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_parsed_type_internal(&type_args[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Set(Box::new(inner))
            }
            "Range" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_parsed_type_internal(&type_args[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Range(Box::new(inner))
            }
            "Channel" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_parsed_type_internal(&type_args[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Channel(Box::new(inner))
            }
            _ => {
                // User-defined type or type parameter
                // Treat as a named type reference - resolution happens during unification
                Type::Named(name)
            }
        }
    }

    /// Create a projection type for an associated type (e.g., Self.Item or T.Item).
    ///
    /// Resolves the base type and creates a Projection type.
    fn make_projection_type(
        &mut self,
        base: &ParsedType,
        assoc_name: Name,
        generic_type_vars: Option<&HashMap<Name, Type>>,
    ) -> Type {
        // Associated type projection like Self.Item or T.Item
        // The base type is converted, and we create a Projection type.
        // The trait_name is not known at parse time in general; we use
        // a placeholder that will be resolved during impl checking or
        // when we have more context about which trait defines this associated type.
        let base_ty = self.resolve_parsed_type_internal(base, generic_type_vars);

        // For now, use a placeholder trait name. In a more complete implementation,
        // we would look up which trait defines this associated type based on
        // the context (current trait definition or trait bounds on the base type).
        // Using the assoc_name as the trait_name placeholder for now.
        Type::Projection {
            base: Box::new(base_ty),
            trait_name: assoc_name, // Placeholder - resolved during impl checking
            assoc_name,
        }
    }

    /// Type check a function body.
    fn check_function(&mut self, func: &Function, func_type: &FunctionType) {
        // Create scope for function parameters
        let mut func_env = if let Some(ref base) = self.inference.base_env {
            base.child()
        } else {
            self.inference.env.child()
        };

        // Bind parameters
        let params = self.context.arena.get_params(func.params);
        for (param, param_type) in params.iter().zip(func_type.params.iter()) {
            func_env.bind(param.name, param_type.clone());
        }

        // Save current env and switch to function env
        let old_env = std::mem::replace(&mut self.inference.env, func_env);

        // Set current function's capabilities for propagation checking
        let new_caps = func.capabilities.iter().map(|c| c.name).collect();
        self.with_capability_scope(new_caps, |checker| {
            // Infer body type
            let body_type = infer::infer_expr(checker, func.body);

            // Unify with declared return type
            if let Err(e) = checker.inference.ctx.unify(&body_type, &func_type.return_type) {
                let span = checker.context.arena.get_expr(func.body).span;
                checker.report_type_error(&e, span);
            }
        });

        // Restore environment
        self.inference.env = old_env;
    }

    /// Type check a test body.
    fn check_test(&mut self, test: &TestDef) {
        // Infer parameter types
        let params: Vec<Type> = self.context.arena.get_params(test.params)
            .iter()
            .map(|p| {
                match &p.ty {
                    Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
                    None => self.inference.ctx.fresh_var(),
                }
            })
            .collect();

        // Infer return type
        let return_type = match &test.return_ty {
            Some(parsed_ty) => self.parsed_type_to_type(parsed_ty),
            None => self.inference.ctx.fresh_var(),
        };

        // Create scope for test parameters
        let mut test_env = if let Some(ref base) = self.inference.base_env {
            base.child()
        } else {
            self.inference.env.child()
        };

        // Bind parameters
        let param_defs = self.context.arena.get_params(test.params);
        for (param, param_type) in param_defs.iter().zip(params.iter()) {
            test_env.bind(param.name, param_type.clone());
        }

        // Save current env and switch to test env
        let old_env = std::mem::replace(&mut self.inference.env, test_env);

        // Tests don't declare capabilities, so we start with empty capability context
        // but must still track provided capabilities from with...in expressions
        self.with_empty_capability_scope(|checker| {
            // Infer body type
            let body_type = infer::infer_expr(checker, test.body);

            // Unify with declared return type
            if let Err(e) = checker.inference.ctx.unify(&body_type, &return_type) {
                let span = checker.context.arena.get_expr(test.body).span;
                checker.report_type_error(&e, span);
            }
        });

        // Restore environment
        self.inference.env = old_env;
    }

    /// Type check all methods in an impl block.
    fn check_impl_methods(&mut self, impl_def: &sigil_ir::ImplDef) {
        let self_ty = self.parsed_type_to_type(&impl_def.self_ty);

        self.with_impl_scope(self_ty.clone(), |checker| {
            for method in &impl_def.methods {
                checker.check_impl_method(method, &self_ty);
            }
        });
    }

    /// Type check a single impl method.
    fn check_impl_method(&mut self, method: &sigil_ir::ImplMethod, self_ty: &Type) {
        // Create scope for method parameters
        let mut method_env = if let Some(ref base) = self.inference.base_env {
            base.child()
        } else {
            self.inference.env.child()
        };

        // Bind parameters (first param is typically `self`)
        let params = self.context.arena.get_params(method.params);
        for param in params {
            let param_ty = if let Some(ref parsed_ty) = param.ty {
                self.parsed_type_to_type(parsed_ty)
            } else {
                // If first param is named `self`, bind to Self type
                let self_name = self.context.interner.intern("self");
                if param.name == self_name {
                    self_ty.clone()
                } else {
                    self.inference.ctx.fresh_var()
                }
            };
            method_env.bind(param.name, param_ty);
        }

        // Save current env and switch to method env
        let old_env = std::mem::replace(&mut self.inference.env, method_env);

        // Infer body type
        let body_type = infer::infer_expr(self, method.body);

        // Unify with declared return type
        let return_type = self.parsed_type_to_type(&method.return_ty);
        if let Err(e) = self.inference.ctx.unify(&body_type, &return_type) {
            let span = self.context.arena.get_expr(method.body).span;
            self.report_type_error(&e, span);
        }

        // Restore environment
        self.inference.env = old_env;
    }

    /// Validate that capabilities in a function's `uses` clause refer to valid traits.
    ///
    /// For each capability in the `uses` clause, checks that a trait with that name exists
    /// in the trait registry. If not, reports an error.
    fn validate_capabilities(&mut self, func: &Function) {
        for cap_ref in &func.capabilities {
            if !self.registries.traits.has_trait(cap_ref.name) {
                let cap_name = self.context.interner.lookup(cap_ref.name);
                self.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown capability `{cap_name}`: capabilities must be defined traits"),
                    span: cap_ref.span,
                    code: sigil_diagnostic::ErrorCode::E2012,
                });
            }
        }
    }

    /// Resolve a type through any alias chain.
    ///
    /// If the type is a named type that refers to an alias, returns the
    /// underlying type. Otherwise returns the type unchanged.
    pub(crate) fn resolve_through_aliases(&self, ty: &Type) -> Type {
        match ty {
            Type::Named(name) => {
                if let Some(entry) = self.registries.types.get_by_name(*name) {
                    if let TypeKind::Alias { target } = &entry.kind {
                        return self.resolve_through_aliases(target);
                    }
                }
                ty.clone()
            }
            _ => ty.clone(),
        }
    }

    /// Register config variable types.
    ///
    /// Infers the type of each config value and stores it for $name references.
    fn register_configs(&mut self, module: &Module) {
        for config in &module.configs {
            let config_ty = infer::infer_expr(self, config.value);
            self.scope.config_types.insert(config.name, config_ty);
        }
    }

    /// Report a type error.
    pub(crate) fn report_type_error(&mut self, err: &TypeError, span: Span) {
        let diag = err.to_diagnostic(span, self.context.interner);
        let error = TypeCheckError {
            message: diag.message.clone(),
            span,
            code: diag.code,
        };

        // If we have a diagnostic queue, use it for deduplication/limits
        if let (Some(ref mut queue), Some(ref source)) = (&mut self.diagnostics.queue, &self.diagnostics.source) {
            let is_soft = error.is_soft();
            // Add to queue - it will handle deduplication and limits
            if queue.add_with_source(diag, source, is_soft) {
                self.diagnostics.errors.push(error);
            }
        } else {
            // No queue - add directly
            self.diagnostics.errors.push(error);
        }
    }

    /// Check if the error limit has been reached.
    ///
    /// When source is provided, the diagnostic queue tracks error limits.
    /// Returns false if no source/queue is configured.
    pub fn limit_reached(&self) -> bool {
        self.diagnostics.queue.as_ref().is_some_and(sigil_diagnostic::queue::DiagnosticQueue::limit_reached)
    }

    /// Store the type for an expression.
    pub(crate) fn store_type(&mut self, expr_id: ExprId, ty: Type) {
        self.inference.expr_types.insert(expr_id.index(), ty);
    }
}

/// Type check a parsed module.
pub fn type_check(
    parse_result: &sigil_parse::ParseResult,
    interner: &StringInterner,
) -> TypedModule {
    let checker = TypeChecker::new(&parse_result.arena, interner);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with source code for diagnostic queue features.
///
/// When source is provided, error deduplication and limits are enabled.
pub fn type_check_with_source(
    parse_result: &sigil_parse::ParseResult,
    interner: &StringInterner,
    source: String,
) -> TypedModule {
    let checker = TypeChecker::with_source(&parse_result.arena, interner, source);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with source and custom diagnostic configuration.
pub fn type_check_with_config(
    parse_result: &sigil_parse::ParseResult,
    interner: &StringInterner,
    source: String,
    config: DiagnosticConfig,
) -> TypedModule {
    let checker = TypeChecker::with_source_and_config(&parse_result.arena, interner, source, config);
    checker.check_module(&parse_result.module)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use sigil_ir::SharedInterner;
    use sigil_types::Type;

    fn check_source(source: &str) -> (sigil_parse::ParseResult, TypedModule) {
        let interner = SharedInterner::default();
        let tokens = sigil_lexer::lex(source, &interner);
        let parsed = sigil_parse::parse(&tokens, &interner);
        let typed = type_check(&parsed, &interner);
        (parsed, typed)
    }

    #[test]
    fn test_literal_types() {
        let (parsed, typed) = check_source("@main () -> int = 42");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Int);
    }

    #[test]
    fn test_binary_arithmetic() {
        let (parsed, typed) = check_source("@add () -> int = 1 + 2");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Int);
    }

    #[test]
    fn test_comparison() {
        let (parsed, typed) = check_source("@cmp () -> bool = 1 < 2");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Bool);
    }

    #[test]
    fn test_if_expression() {
        let (parsed, typed) = check_source("@test () -> int = if true then 1 else 2");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Int);
    }

    #[test]
    fn test_list_type() {
        let (parsed, typed) = check_source("@test () = [1, 2, 3]");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::List(Box::new(Type::Int)));
    }

    #[test]
    fn test_type_mismatch_error() {
        let (_, typed) = check_source("@test () -> int = if 42 then 1 else 2");

        assert!(typed.has_errors());
        assert!(typed.errors[0].message.contains("type mismatch") ||
                typed.errors[0].message.contains("expected"));
    }

    #[test]
    fn test_typed_module_salsa_traits() {
        use std::collections::HashSet;

        let (_, typed1) = check_source("@main () -> int = 42");
        let (_, typed2) = check_source("@main () -> int = 42");
        let (_, typed3) = check_source("@main () -> bool = true");

        assert_eq!(typed1, typed2);
        assert_ne!(typed1, typed3);

        let mut set = HashSet::new();
        set.insert(typed1.clone());
        set.insert(typed2);
        set.insert(typed3);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_function_with_typed_params() {
        let (_, typed) = check_source("@add (a: int, b: int) -> int = a + b");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func_type = &typed.function_types[0];
        assert_eq!(func_type.params.len(), 2);
        assert_eq!(func_type.params[0], Type::Int);
        assert_eq!(func_type.params[1], Type::Int);
        assert_eq!(func_type.return_type, Type::Int);
    }

    #[test]
    fn test_function_call_type_inference() {
        let (_, typed) = check_source("@double (x: int) -> int = x * 2");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func_type = &typed.function_types[0];
        assert_eq!(func_type.return_type, Type::Int);
    }

    #[test]
    fn test_lambda_with_typed_param() {
        let (_, typed) = check_source("@test () = (x: int) -> x + 1");

        assert!(!typed.has_errors());
    }

    #[test]
    fn test_tuple_type() {
        let (parsed, typed) = check_source("@test () = (1, true, \"hello\")");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(
            *body_type,
            Type::Tuple(vec![Type::Int, Type::Bool, Type::Str])
        );
    }

    #[test]
    fn test_nested_if_type() {
        let (_, typed) = check_source(r#"
            @test (x: int) -> int =
                if x > 0 then
                    if x > 10 then 100 else 10
                else
                    0
        "#);

        assert!(!typed.has_errors());
    }

    #[test]
    fn test_run_pattern_type() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let x: int = 1,
                let y: int = 2,
                x + y
            )
        "#);

        assert!(!typed.has_errors());
    }

    // =========================================================================
    // Closure Self-Capture Detection Tests
    // =========================================================================

    #[test]
    fn test_closure_self_capture_direct() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself") &&
            e.code == sigil_diagnostic::ErrorCode::E2007
        ));
    }

    #[test]
    fn test_closure_self_capture_call() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = (x: int) -> f(x + 1),
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself")
        ));
    }

    #[test]
    fn test_no_self_capture_uses_outer_binding() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = 42,
                let g = () -> f,
                g()
            )
        "#);

        assert!(!typed.errors.iter().any(|e|
            e.code == sigil_diagnostic::ErrorCode::E2007
        ));
    }

    #[test]
    fn test_no_self_capture_non_lambda() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let x = 1 + 2,
                x
            )
        "#);

        assert!(!typed.has_errors());
    }

    #[test]
    fn test_closure_self_capture_in_run() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself")
        ));
    }

    #[test]
    fn test_closure_self_capture_nested_expression() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> if true then f else f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself")
        ));
    }

    #[test]
    fn test_valid_mutual_recursion_via_outer_scope() {
        let (_, typed) = check_source(r#"
            @f (x: int) -> int = x
            @test () -> int = run(
                let g = (x: int) -> @f(x),
                g(1)
            )
        "#);

        assert!(!typed.errors.iter().any(|e|
            e.code == sigil_diagnostic::ErrorCode::E2007
        ));
    }

    // =========================================================================
    // TypeRegistry Integration Tests
    // =========================================================================

    #[test]
    fn test_type_registry_in_checker() {
        let interner = SharedInterner::default();
        let tokens = sigil_lexer::lex("@main () -> int = 42", &interner);
        let parsed = sigil_parse::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        let point_name = interner.intern("Point");
        let x_name = interner.intern("x");
        let y_name = interner.intern("y");

        let type_id = checker.registries.types.register_struct(
            point_name,
            vec![(x_name, Type::Int), (y_name, Type::Int)],
            sigil_ir::Span::new(0, 0),
            vec![],
        );

        assert!(checker.registries.types.contains(point_name));
        let entry = checker.registries.types.get_by_id(type_id).unwrap();
        assert_eq!(entry.name, point_name);
    }

    #[test]
    fn test_type_id_to_type_with_registry() {
        let interner = SharedInterner::default();
        let tokens = sigil_lexer::lex("@main () -> int = 42", &interner);
        let parsed = sigil_parse::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        let id_name = interner.intern("UserId");
        let type_id = checker.registries.types.register_alias(
            id_name,
            Type::Int,
            sigil_ir::Span::new(0, 0),
            vec![],
        );

        let resolved = checker.type_id_to_type(type_id);
        assert_eq!(resolved, Type::Int);
    }

    #[test]
    fn test_type_id_to_type_with_struct() {
        let interner = SharedInterner::default();
        let tokens = sigil_lexer::lex("@main () -> int = 42", &interner);
        let parsed = sigil_parse::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        let point_name = interner.intern("Point");
        let type_id = checker.registries.types.register_struct(
            point_name,
            vec![],
            sigil_ir::Span::new(0, 0),
            vec![],
        );

        let resolved = checker.type_id_to_type(type_id);
        assert_eq!(resolved, Type::Named(point_name));
    }
}
