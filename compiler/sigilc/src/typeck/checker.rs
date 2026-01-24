//! Type checker core implementation.
//!
//! Contains the TypeChecker struct and main entry point for type checking.

use crate::diagnostic::Diagnostic;
use crate::ir::{
    Name, Span, ExprId, ExprArena, Module, Function, TestDef,
    StringInterner, TypeId, BindingPattern,
};
use crate::parser::ParseResult;
use crate::patterns::PatternRegistry;
use crate::types::{Type, TypeEnv, InferenceContext, TypeError};
use crate::context::{CompilerContext, SharedRegistry};
use super::operators::TypeOperatorRegistry;
use super::type_registry::TypeRegistry;
use super::infer;
use std::collections::{HashMap, HashSet};

/// Type-checked module.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypedModule {
    /// Type of each expression (indexed by ExprId).
    pub expr_types: Vec<Type>,
    /// Type of each function.
    pub function_types: Vec<FunctionType>,
    /// Type checking errors.
    pub errors: Vec<TypeCheckError>,
}

impl TypedModule {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Function type information.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionType {
    pub name: Name,
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// Type checking error with location.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeCheckError {
    pub message: String,
    pub span: Span,
    pub code: crate::diagnostic::ErrorCode,
}

impl TypeCheckError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.code)
            .with_message(&self.message)
            .with_label(self.span, "type error here")
    }
}

/// Type checker state.
pub struct TypeChecker<'a> {
    pub(crate) arena: &'a ExprArena,
    pub(crate) interner: &'a StringInterner,
    pub(crate) ctx: InferenceContext,
    pub(crate) env: TypeEnv,
    pub(crate) expr_types: HashMap<u32, Type>,
    pub(crate) errors: Vec<TypeCheckError>,
    /// Pattern registry for function_exp type checking.
    pub(crate) registry: SharedRegistry<PatternRegistry>,
    /// Type operator registry for binary operation type checking.
    pub(crate) type_operator_registry: TypeOperatorRegistry,
    /// Registry for user-defined types (structs, enums, aliases).
    pub(crate) type_registry: TypeRegistry,
}

impl<'a> TypeChecker<'a> {
    /// Create a new type checker with default registries.
    pub fn new(arena: &'a ExprArena, interner: &'a StringInterner) -> Self {
        TypeChecker {
            arena,
            interner,
            ctx: InferenceContext::new(),
            env: TypeEnv::new(),
            expr_types: HashMap::new(),
            errors: Vec::new(),
            registry: SharedRegistry::new(PatternRegistry::new()),
            type_operator_registry: TypeOperatorRegistry::new(),
            type_registry: TypeRegistry::new(),
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
            expr_types: HashMap::new(),
            errors: Vec::new(),
            registry: context.pattern_registry.clone(),
            type_operator_registry: TypeOperatorRegistry::new(),
            type_registry: TypeRegistry::new(),
        }
    }

    /// Type check a module.
    pub fn check_module(mut self, module: &Module) -> TypedModule {
        let mut function_types = Vec::new();

        // First pass: collect function signatures
        for func in &module.functions {
            let func_type = self.infer_function_signature(func);
            function_types.push(func_type.clone());

            // Bind function name to its type
            let fn_type = Type::Function {
                params: func_type.params.clone(),
                ret: Box::new(func_type.return_type.clone()),
            };
            self.env.bind(func.name, fn_type);
        }

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

    /// Infer function signature from declaration.
    fn infer_function_signature(&mut self, func: &Function) -> FunctionType {
        let params: Vec<Type> = self.arena.get_params(func.params)
            .iter()
            .map(|p| {
                match p.ty {
                    Some(type_id) => self.type_id_to_type(type_id),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect();

        let return_type = match func.return_ty {
            Some(type_id) => self.type_id_to_type(type_id),
            None => self.ctx.fresh_var(),
        };

        FunctionType {
            name: func.name,
            params,
            return_type,
        }
    }

    /// Type check a function body.
    fn check_function(&mut self, func: &Function, func_type: &FunctionType) {
        // Create scope for function parameters
        let mut func_env = self.env.child();

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
        let mut test_env = self.env.child();

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
        self.errors.push(TypeCheckError {
            message: diag.message.clone(),
            span,
            code: diag.code,
        });
    }

    /// Store the type for an expression.
    pub(crate) fn store_type(&mut self, expr_id: ExprId, ty: Type) {
        self.expr_types.insert(expr_id.index() as u32, ty);
    }

    // =========================================================================
    // Pattern Binding
    // =========================================================================

    /// Bind a pattern to a type with generalization (for let-polymorphism).
    ///
    /// This is the key to Hindley-Milner let-polymorphism: we generalize
    /// the type before binding, so that `let id = x -> x` has type `∀a. a -> a`
    /// and each use of `id` gets fresh type variables.
    pub(crate) fn bind_pattern_generalized(&mut self, pattern: &BindingPattern, ty: Type) {
        // Collect free vars in the environment to avoid generalizing over them
        let env_free_vars = self.env.free_vars(&self.ctx);

        match pattern {
            BindingPattern::Name(name) => {
                // Generalize the type: quantify over free vars not in environment
                let scheme = self.ctx.generalize(&ty, &env_free_vars);
                self.env.bind_scheme(*name, scheme);
            }
            BindingPattern::Wildcard => {
                // Wildcard doesn't bind anything
            }
            BindingPattern::Tuple(patterns) => {
                // For tuple destructuring, each element gets generalized separately
                if let Type::Tuple(elem_types) = ty {
                    if patterns.len() == elem_types.len() {
                        for (pat, elem_ty) in patterns.iter().zip(elem_types) {
                            self.bind_pattern_generalized(pat, elem_ty);
                        }
                    }
                }
            }
            BindingPattern::Struct { fields } => {
                // For struct destructuring, bind each field with generalization
                for (field_name, nested_pattern) in fields {
                    let field_ty = self.ctx.fresh_var();
                    if let Some(nested) = nested_pattern {
                        self.bind_pattern_generalized(nested, field_ty);
                    } else {
                        let scheme = self.ctx.generalize(&field_ty, &env_free_vars);
                        self.env.bind_scheme(*field_name, scheme);
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                // For list destructuring, each element gets generalized
                if let Type::List(elem_ty) = &ty {
                    for elem_pat in elements {
                        self.bind_pattern_generalized(elem_pat, (**elem_ty).clone());
                    }
                    if let Some(rest_name) = rest {
                        let scheme = self.ctx.generalize(&ty, &env_free_vars);
                        self.env.bind_scheme(*rest_name, scheme);
                    }
                }
            }
        }
    }

    /// Bind a pattern to a type (for let bindings with destructuring).
    /// This is the non-generalizing version used for function parameters.
    pub(crate) fn bind_pattern(&mut self, pattern: &BindingPattern, ty: Type) {
        match pattern {
            BindingPattern::Name(name) => {
                self.env.bind(*name, ty);
            }
            BindingPattern::Wildcard => {
                // Wildcard doesn't bind anything
            }
            BindingPattern::Tuple(patterns) => {
                // For tuple destructuring, we need to unify with a tuple type
                let resolved = self.ctx.resolve(&ty);
                match resolved {
                    Type::Tuple(elem_types) => {
                        if patterns.len() == elem_types.len() {
                            for (pat, elem_ty) in patterns.iter().zip(elem_types) {
                                self.bind_pattern(pat, elem_ty);
                            }
                        } else {
                            self.errors.push(TypeCheckError {
                                message: format!(
                                    "tuple pattern has {} elements, but type has {}",
                                    patterns.len(),
                                    elem_types.len()
                                ),
                                span: Span::default(),
                                code: crate::diagnostic::ErrorCode::E2001,
                            });
                        }
                    }
                    Type::Var(_) => {
                        // Type variable - bind patterns to fresh vars
                        for pat in patterns {
                            let fresh_ty = self.ctx.fresh_var();
                            self.bind_pattern(pat, fresh_ty);
                        }
                    }
                    Type::Error => {} // Error recovery - don't cascade errors
                    other => {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "cannot destructure `{}` as a tuple",
                                other.display(self.interner)
                            ),
                            span: Span::default(),
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                    }
                }
            }
            BindingPattern::Struct { fields } => {
                // For struct destructuring, bind each field
                for (field_name, nested_pattern) in fields {
                    let field_ty = self.ctx.fresh_var();
                    if let Some(nested) = nested_pattern {
                        self.bind_pattern(nested, field_ty);
                    } else {
                        // Shorthand: { x } binds x to the field type
                        self.env.bind(*field_name, field_ty);
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                // For list destructuring, bind each element
                let resolved = self.ctx.resolve(&ty);
                match resolved {
                    Type::List(elem_ty) => {
                        for elem_pat in elements {
                            self.bind_pattern(elem_pat, (*elem_ty).clone());
                        }
                        if let Some(rest_name) = rest {
                            self.env.bind(*rest_name, ty.clone());
                        }
                    }
                    Type::Var(_) => {
                        // Type variable - bind patterns to fresh vars
                        let elem_ty = self.ctx.fresh_var();
                        for elem_pat in elements {
                            self.bind_pattern(elem_pat, elem_ty.clone());
                        }
                        if let Some(rest_name) = rest {
                            self.env.bind(*rest_name, Type::List(Box::new(elem_ty)));
                        }
                    }
                    Type::Error => {} // Error recovery - don't cascade errors
                    other => {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "cannot destructure `{}` as a list",
                                other.display(self.interner)
                            ),
                            span: Span::default(),
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                    }
                }
            }
        }
    }

    // =========================================================================
    // Cycle Detection
    // =========================================================================

    /// Collect free variable references from an expression.
    ///
    /// This is used for closure self-capture detection. A variable is "free"
    /// if it's referenced but not bound within the expression.
    pub(crate) fn collect_free_vars(&self, expr_id: ExprId, bound: &HashSet<Name>) -> HashSet<Name> {
        let mut free = HashSet::new();
        infer::collect_free_vars_inner(self, expr_id, bound, &mut free);
        free
    }

    /// Add bindings from a pattern to the bound set.
    pub(crate) fn add_pattern_bindings(&self, pattern: &BindingPattern, bound: &mut HashSet<Name>) {
        match pattern {
            BindingPattern::Name(name) => {
                bound.insert(*name);
            }
            BindingPattern::Wildcard => {}
            BindingPattern::Tuple(patterns) => {
                for p in patterns {
                    self.add_pattern_bindings(p, bound);
                }
            }
            BindingPattern::Struct { fields } => {
                for (field_name, nested) in fields {
                    if let Some(nested_pattern) = nested {
                        self.add_pattern_bindings(nested_pattern, bound);
                    } else {
                        // Shorthand: { x } binds x
                        bound.insert(*field_name);
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                for p in elements {
                    self.add_pattern_bindings(p, bound);
                }
                if let Some(rest_name) = rest {
                    bound.insert(*rest_name);
                }
            }
        }
    }

    /// Check for closure self-capture in a let binding.
    ///
    /// Detects patterns like: `let f = () -> f()` where a closure captures itself.
    /// This would create a reference cycle and must be rejected at compile time.
    pub(crate) fn check_closure_self_capture(
        &mut self,
        pattern: &BindingPattern,
        init: ExprId,
        span: Span,
    ) {
        use crate::ir::ExprKind;

        // Get the names being bound
        let mut bound_names = HashSet::new();
        self.add_pattern_bindings(pattern, &mut bound_names);

        // Check if init is a lambda that references any of the bound names
        let expr = self.arena.get_expr(init);
        if let ExprKind::Lambda { body, params, .. } = &expr.kind {
            // The lambda's parameters are bound in its body
            let mut lambda_bound = HashSet::new();
            for param in self.arena.get_params(*params) {
                lambda_bound.insert(param.name);
            }

            // Collect free variables from the lambda body
            let free_vars = self.collect_free_vars(*body, &lambda_bound);

            // Check if any bound name is in the free variables
            for name in &bound_names {
                if free_vars.contains(name) {
                    let name_str = self.interner.lookup(*name);
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "closure cannot capture itself: `{}` references itself in its body",
                            name_str
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2007,
                    });
                }
            }
        }
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
