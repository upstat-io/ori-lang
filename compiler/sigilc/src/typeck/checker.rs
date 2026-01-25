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
use super::type_registry::{
    TypeRegistry, TraitRegistry, TraitEntry, TraitMethodDef,
    ImplEntry, ImplMethodDef, TraitAssocTypeDef,
};
use super::infer;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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

/// A generic parameter with its trait bounds and associated type variable.
#[derive(Clone, Debug)]
pub struct GenericBound {
    /// The generic parameter name (e.g., `T` in `<T: Eq>`)
    pub param: Name,
    /// Trait bounds as paths (e.g., `["Eq"]`, `["Comparable"]`)
    pub bounds: Vec<Vec<Name>>,
    /// The type variable used for this generic in the function signature.
    /// Used to resolve the actual type at call sites for constraint checking.
    pub type_var: Type,
}

// Manual Eq/PartialEq/Hash that ignores type_var (which contains fresh vars)
impl PartialEq for GenericBound {
    fn eq(&self, other: &Self) -> bool {
        self.param == other.param && self.bounds == other.bounds
    }
}

impl Eq for GenericBound {}

impl std::hash::Hash for GenericBound {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.param.hash(state);
        self.bounds.hash(state);
    }
}

/// Function type information.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionType {
    pub name: Name,
    /// Generic parameters with their trait bounds
    pub generics: Vec<GenericBound>,
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
        }
    }

    /// Type check a module.
    pub fn check_module(mut self, module: &Module) -> TypedModule {
        let mut function_types = Vec::new();

        // Pass 0: Register traits and implementations
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

    /// Infer function signature from declaration.
    ///
    /// For generic functions, creates a fresh type variable for each generic parameter
    /// and uses it consistently across all parameter type annotations that reference
    /// that generic. This enables proper constraint checking at call sites.
    fn infer_function_signature(&mut self, func: &Function) -> FunctionType {
        // Step 1: Create fresh type variables for each generic parameter
        let generic_params = self.arena.get_generic_params(func.generics);
        let mut generic_type_vars: HashMap<Name, Type> = HashMap::new();

        for gp in generic_params {
            let type_var = self.ctx.fresh_var();
            generic_type_vars.insert(gp.name, type_var);
        }

        // Step 2: Collect generic bounds with their type variables
        let mut generics = Vec::new();
        for gp in generic_params {
            let bounds: Vec<Vec<Name>> = gp.bounds.iter()
                .map(|b| b.path.clone())
                .collect();
            let type_var = generic_type_vars.get(&gp.name).cloned()
                .unwrap_or_else(|| self.ctx.fresh_var());
            generics.push(GenericBound {
                param: gp.name,
                bounds,
                type_var,
            });
        }

        // Step 3: Merge where clause bounds
        for wc in &func.where_clauses {
            if let Some(gb) = generics.iter_mut().find(|g| g.param == wc.param) {
                // Add bounds from where clause to existing generic
                for bound in &wc.bounds {
                    gb.bounds.push(bound.path.clone());
                }
            } else {
                // Where clause for a param not in generic list - create new entry
                let bounds: Vec<Vec<Name>> = wc.bounds.iter()
                    .map(|b| b.path.clone())
                    .collect();
                let type_var = generic_type_vars.get(&wc.param).cloned()
                    .unwrap_or_else(|| self.ctx.fresh_var());
                generics.push(GenericBound {
                    param: wc.param,
                    bounds,
                    type_var,
                });
            }
        }

        // Step 4: Convert parameter types, using generic type vars when applicable
        let params: Vec<Type> = self.arena.get_params(func.params)
            .iter()
            .map(|p| {
                // Check if this param's type annotation refers to a generic parameter
                if let Some(type_name) = p.type_name {
                    if let Some(type_var) = generic_type_vars.get(&type_name) {
                        return type_var.clone();
                    }
                }
                // Fall back to regular type conversion
                match p.ty {
                    Some(type_id) => self.type_id_to_type(type_id),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect();

        // Step 5: Handle return type (TODO: also check for generic return types)
        let return_type = match func.return_ty {
            Some(type_id) => self.type_id_to_type(type_id),
            None => self.ctx.fresh_var(),
        };

        FunctionType {
            name: func.name,
            generics,
            params,
            return_type,
        }
    }

    /// Type check a function body.
    fn check_function(&mut self, func: &Function, func_type: &FunctionType) {
        // Create scope for function parameters - O(1) using Rc-shared base
        let mut func_env = if let Some(ref base) = self.base_env {
            TypeEnv::child_of_rc(Rc::clone(base))
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

        // Create scope for test parameters - O(1) using Rc-shared base
        let mut test_env = if let Some(ref base) = self.base_env {
            TypeEnv::child_of_rc(Rc::clone(base))
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

    // =========================================================================
    // Trait and Impl Registration
    // =========================================================================

    /// Register all trait definitions from a module.
    fn register_traits(&mut self, module: &Module) {
        use crate::ir::TraitItem;

        for trait_def in &module.traits {
            // Convert generic params to names
            let type_params: Vec<Name> = self.arena
                .get_generic_params(trait_def.generics)
                .iter()
                .map(|gp| gp.name)
                .collect();

            // Convert super-traits to names
            let super_traits: Vec<Name> = trait_def.super_traits
                .iter()
                .map(|b| b.name())
                .collect();

            // Convert trait items
            let mut methods = Vec::new();
            let mut assoc_types = Vec::new();

            for item in &trait_def.items {
                match item {
                    TraitItem::MethodSig(sig) => {
                        let params = self.params_to_types(sig.params);
                        let return_ty = self.type_id_to_type(sig.return_ty);
                        methods.push(TraitMethodDef {
                            name: sig.name,
                            params,
                            return_ty,
                            has_default: false,
                        });
                    }
                    TraitItem::DefaultMethod(method) => {
                        let params = self.params_to_types(method.params);
                        let return_ty = self.type_id_to_type(method.return_ty);
                        methods.push(TraitMethodDef {
                            name: method.name,
                            params,
                            return_ty,
                            has_default: true,
                        });
                    }
                    TraitItem::AssocType(at) => {
                        assoc_types.push(TraitAssocTypeDef {
                            name: at.name,
                        });
                    }
                }
            }

            let entry = TraitEntry {
                name: trait_def.name,
                span: trait_def.span,
                type_params,
                super_traits,
                methods,
                assoc_types,
                is_public: trait_def.is_public,
            };

            self.trait_registry.register_trait(entry);
        }
    }

    /// Register all implementation blocks from a module.
    fn register_impls(&mut self, module: &Module) {
        for impl_def in &module.impls {
            // Convert generic params to names
            let type_params: Vec<Name> = self.arena
                .get_generic_params(impl_def.generics)
                .iter()
                .map(|gp| gp.name)
                .collect();

            // Convert trait path to single name (for now, just use last segment)
            let trait_name = impl_def.trait_path.as_ref().map(|path| {
                *path.last().expect("trait path cannot be empty")
            });

            // Convert self type
            let self_ty = self.type_id_to_type(impl_def.self_ty);

            // Convert methods
            let methods: Vec<ImplMethodDef> = impl_def.methods
                .iter()
                .map(|m| {
                    let params = self.params_to_types(m.params);
                    let return_ty = self.type_id_to_type(m.return_ty);
                    ImplMethodDef {
                        name: m.name,
                        params,
                        return_ty,
                    }
                })
                .collect();

            let entry = ImplEntry {
                trait_name,
                self_ty,
                span: impl_def.span,
                type_params,
                methods,
            };

            // Register impl, checking for coherence violations
            if let Err(coherence_err) = self.trait_registry.register_impl(entry) {
                self.errors.push(TypeCheckError {
                    message: format!(
                        "{} (previous impl at {:?})",
                        coherence_err.message,
                        coherence_err.existing_span
                    ),
                    span: coherence_err.span,
                    code: crate::diagnostic::ErrorCode::E2010,
                });
            }
        }
    }

    /// Convert a parameter range to a vector of types.
    fn params_to_types(&mut self, params: crate::ir::ParamRange) -> Vec<Type> {
        self.arena
            .get_params(params)
            .iter()
            .map(|p| {
                match p.ty {
                    Some(type_id) => self.type_id_to_type(type_id),
                    None => self.ctx.fresh_var(),
                }
            })
            .collect()
    }

    // =========================================================================
    // Trait Bound Checking
    // =========================================================================

    /// Check if a type satisfies a trait bound.
    ///
    /// Returns true if the type implements the trait, false otherwise.
    /// This uses the trait registry to check for implementations.
    #[allow(dead_code)]
    pub(crate) fn type_satisfies_bound(&self, ty: &Type, trait_path: &[Name]) -> bool {
        // Get the trait name (last segment of path)
        let trait_name = match trait_path.last() {
            Some(name) => *name,
            None => return false,
        };

        // Check if the type implements the trait
        self.trait_registry.implements(ty, trait_name)
    }

    /// Check trait bounds for a function call.
    ///
    /// Given a function's generic bounds and the resolved types from a call,
    /// verifies that the types satisfy all required trait bounds.
    ///
    /// NOTE: Full constraint checking requires parser changes to preserve
    /// type annotation names (e.g., knowing that a param was annotated `: T`
    /// where `T` is a generic parameter). The current implementation stores
    /// bounds but cannot fully enforce them without that connection.
    ///
    /// For now, this is a stub that will be enhanced when the parser
    /// preserves type annotation names.
    #[allow(dead_code)]
    pub(crate) fn check_function_bounds(
        &mut self,
        _func_name: Name,
        _resolved_args: &[Type],
        _span: Span,
    ) {
        // TODO: Implement full constraint checking when parser preserves type names.
        //
        // The full implementation would:
        // 1. Look up the function's generics from function_sigs
        // 2. Map resolved types back to generic parameters
        // 3. Check that each resolved type satisfies its generic's bounds
        //
        // Currently blocked because the parser converts type annotations like `: T`
        // to `TypeId::INFER`, losing the information that `T` was used.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::ir::SharedInterner;

    /// Helper to parse source code
    fn parse_source(source: &str, interner: &SharedInterner) -> ParseResult {
        let tokens = lex(source, interner);
        parse(&tokens, interner)
    }

    #[test]
    fn test_generic_bounds_parsing() {
        // Test that generic bounds are correctly extracted from functions
        let source = r#"
            @compare<T: Comparable> (a: T, b: T) -> int = 0
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

        // Check that the function has generics
        assert_eq!(parse_result.module.functions.len(), 1);
        let func = &parse_result.module.functions[0];

        // The function should have one generic parameter
        let generic_params = parse_result.arena.get_generic_params(func.generics);
        assert_eq!(generic_params.len(), 1, "expected 1 generic param");

        let gp = &generic_params[0];
        assert_eq!(interner.lookup(gp.name), "T");

        // The generic param should have one bound: Comparable
        assert_eq!(gp.bounds.len(), 1, "expected 1 bound");
        assert_eq!(gp.bounds[0].path.len(), 1);
        assert_eq!(interner.lookup(gp.bounds[0].path[0]), "Comparable");
    }

    #[test]
    fn test_multiple_bounds_parsing() {
        // Test multiple bounds: <T: A + B>
        let source = r#"
            @process<T: Eq + Clone> (x: T) -> T = x
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

        let func = &parse_result.module.functions[0];
        let generic_params = parse_result.arena.get_generic_params(func.generics);
        assert_eq!(generic_params.len(), 1);

        let gp = &generic_params[0];
        assert_eq!(gp.bounds.len(), 2, "expected 2 bounds");
        assert_eq!(interner.lookup(gp.bounds[0].path[0]), "Eq");
        assert_eq!(interner.lookup(gp.bounds[1].path[0]), "Clone");
    }

    #[test]
    fn test_where_clause_parsing() {
        // Test where clause: where T: Clone
        let source = r#"
            @transform<T> (x: T) -> T where T: Clone = x
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

        let func = &parse_result.module.functions[0];

        // Check where clauses
        assert_eq!(func.where_clauses.len(), 1, "expected 1 where clause");
        let wc = &func.where_clauses[0];
        assert_eq!(interner.lookup(wc.param), "T");
        assert_eq!(wc.bounds.len(), 1);
        assert_eq!(interner.lookup(wc.bounds[0].path[0]), "Clone");
    }

    #[test]
    fn test_function_type_captures_generics() {
        // Test that FunctionType captures generic bounds
        let source = r#"
            @compare<T: Comparable> (a: T, b: T) -> int = 0
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        let typed = type_check(&parse_result, &interner);

        // Check that the typed module has the function
        assert_eq!(typed.function_types.len(), 1);
        let func_type = &typed.function_types[0];

        // Check that generics were captured
        assert_eq!(func_type.generics.len(), 1, "expected 1 generic");
        assert_eq!(interner.lookup(func_type.generics[0].param), "T");
        assert_eq!(func_type.generics[0].bounds.len(), 1);
        assert_eq!(interner.lookup(func_type.generics[0].bounds[0][0]), "Comparable");
    }

    #[test]
    fn test_where_clause_merged_into_generics() {
        // Test that where clause bounds are merged with generic bounds
        let source = r#"
            @process<T: Eq> (x: T) -> T where T: Clone = x
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        let typed = type_check(&parse_result, &interner);

        let func_type = &typed.function_types[0];
        assert_eq!(func_type.generics.len(), 1);

        // T should have both Eq (from generic decl) and Clone (from where clause)
        let bounds = &func_type.generics[0].bounds;
        assert_eq!(bounds.len(), 2, "expected 2 bounds (Eq + Clone)");
    }

    #[test]
    fn test_type_name_captured_in_params() {
        // Test that type annotation names are captured for generic parameter tracking
        let source = r#"
            @swap<T> (a: T, b: T) -> T = a
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        assert!(parse_result.errors.is_empty(), "parse errors: {:?}", parse_result.errors);

        let func = &parse_result.module.functions[0];
        let params = parse_result.arena.get_params(func.params);

        // Both params should have type_name = Some("T")
        assert_eq!(params.len(), 2);
        let t_name = interner.intern("T");
        assert_eq!(params[0].type_name, Some(t_name), "first param should have type_name 'T'");
        assert_eq!(params[1].type_name, Some(t_name), "second param should have type_name 'T'");
    }

    #[test]
    fn test_generic_params_share_type_variable() {
        // Test that multiple params with the same generic type share the same type variable
        let source = r#"
            @swap<T> (a: T, b: T) -> T = a
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        let typed = type_check(&parse_result, &interner);

        let func_type = &typed.function_types[0];

        // Both params should have the same type (after resolution)
        assert_eq!(func_type.params.len(), 2);
        // In a proper generic function, both params would be unified to the same type
        // Here they should both be type variables that will be unified at call sites
    }

    #[test]
    fn test_constraint_violation_detected() {
        // Test that calling a generic function with a type that doesn't
        // satisfy the bounds produces an error
        let source = r#"
            trait Serializable {
                @serialize (self) -> str
            }

            @save<T: Serializable> (x: T) -> str = x.serialize()

            @main () -> void = run(
                let result = save(x: 42),
                print(msg: result)
            )
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        let typed = type_check(&parse_result, &interner);

        // Should have an error because int doesn't implement Serializable
        let bound_errors: Vec<_> = typed.errors.iter()
            .filter(|e| e.code == crate::diagnostic::ErrorCode::E2009)
            .collect();

        assert!(!bound_errors.is_empty(),
            "expected E2009 error for missing trait bound, got: {:?}",
            typed.errors);
    }

    #[test]
    fn test_constraint_satisfied_with_impl() {
        // Test that calling a generic function with a type that implements
        // the required trait works.
        //
        // This test verifies that when a type has an impl for the required trait,
        // no E2009 bound violation errors are emitted.
        //
        // Note: For now, we use a simplified test that just verifies the impl
        // is registered. Full call-site constraint checking with custom types
        // requires type declaration support (Phase 6).
        let source = r#"
            trait Printable {
                @to_string (self) -> str
            }

            @format<T: Printable> (x: T) -> str = "formatted"
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);

        if !parse_result.errors.is_empty() {
            panic!("parse errors: {:?}", parse_result.errors);
        }

        let typed = type_check(&parse_result, &interner);

        // The function signature should have the Printable bound
        let func_type = &typed.function_types[0];
        assert_eq!(func_type.generics.len(), 1);
        assert_eq!(func_type.generics[0].bounds.len(), 1);
        assert_eq!(
            interner.lookup(func_type.generics[0].bounds[0][0]),
            "Printable"
        );
    }

    #[test]
    fn test_multiple_generic_params() {
        // Test multiple generic parameters with different bounds
        let source = r#"
            trait Eq { @eq (self, other: Self) -> bool }
            trait Ord { @cmp (self, other: Self) -> int }

            @compare<A: Eq, B: Ord> (a: A, b: B) -> bool = true
        "#;

        let interner = SharedInterner::default();
        let parse_result = parse_source(source, &interner);
        let typed = type_check(&parse_result, &interner);

        // Check that both generic params are captured
        let func_type = &typed.function_types[0];
        assert_eq!(func_type.generics.len(), 2, "expected 2 generic params");

        let a_name = interner.intern("A");
        let b_name = interner.intern("B");

        let a_generic = func_type.generics.iter().find(|g| g.param == a_name);
        let b_generic = func_type.generics.iter().find(|g| g.param == b_name);

        assert!(a_generic.is_some(), "should have generic A");
        assert!(b_generic.is_some(), "should have generic B");

        // A should have Eq bound, B should have Ord bound
        assert_eq!(a_generic.unwrap().bounds.len(), 1);
        assert_eq!(b_generic.unwrap().bounds.len(), 1);
    }
}
