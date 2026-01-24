//! Type checker for Sigil.
//!
//! Implements Hindley-Milner type inference with extensions for
//! Sigil's pattern system.

pub mod operators;
pub mod type_registry;

pub use type_registry::{TypeRegistry, TypeEntry, TypeKind, VariantDef};

use crate::diagnostic::Diagnostic;
use crate::ir::{
    Name, Span, ExprId, ExprArena, Module, Function, TestDef,
    ExprKind, BinaryOp, UnaryOp,
    StringInterner, TypeId,
    FunctionSeq, SeqBinding, CallArgRange,
};
use crate::parser::ParseResult;
use crate::patterns::{PatternRegistry, TypeCheckContext};
use crate::types::{Type, TypeEnv, InferenceContext, TypeError};
use crate::context::{CompilerContext, SharedRegistry};
use operators::{TypeOperatorRegistry, TypeOpResult};
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
    arena: &'a ExprArena,
    interner: &'a StringInterner,
    ctx: InferenceContext,
    env: TypeEnv,
    expr_types: HashMap<u32, Type>,
    errors: Vec<TypeCheckError>,
    /// Pattern registry for function_exp type checking.
    registry: SharedRegistry<PatternRegistry>,
    /// Type operator registry for binary operation type checking.
    type_operator_registry: TypeOperatorRegistry,
    /// Registry for user-defined types (structs, enums, aliases).
    type_registry: TypeRegistry,
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
    fn type_id_to_type(&mut self, type_id: TypeId) -> Type {
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
        let body_type = self.infer_expr(func.body);

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
        let body_type = self.infer_expr(test.body);

        // Unify with declared return type
        if let Err(e) = self.ctx.unify(&body_type, &return_type) {
            let span = self.arena.get_expr(test.body).span;
            self.report_type_error(e, span);
        }

        // Restore environment
        self.env = old_env;
    }

    /// Infer the type of an expression.
    fn infer_expr(&mut self, expr_id: ExprId) -> Type {
        let expr = self.arena.get_expr(expr_id);
        let span = expr.span;

        let ty = match &expr.kind {
            // Literals
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::String(_) => Type::Str,
            ExprKind::Char(_) => Type::Char,

            // Variable reference
            ExprKind::Ident(name) => {
                if let Some(scheme) = self.env.lookup_scheme(*name) {
                    // Instantiate the scheme to get fresh type variables
                    // This is key for let-polymorphism: each use of a polymorphic
                    // variable gets its own fresh type variables
                    self.ctx.instantiate(scheme)
                } else {
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "unknown identifier `{}`",
                            self.interner.lookup(*name)
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2003,
                    });
                    Type::Error
                }
            }

            // Binary operations
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.infer_expr(*left);
                let right_ty = self.infer_expr(*right);
                self.check_binary_op(*op, &left_ty, &right_ty, span)
            }

            // Unary operations
            ExprKind::Unary { op, operand } => {
                let operand_ty = self.infer_expr(*operand);
                self.check_unary_op(*op, &operand_ty, span)
            }

            // Function call
            ExprKind::Call { func, args } => {
                let func_ty = self.infer_expr(*func);
                let arg_ids = self.arena.get_expr_list(*args);
                let arg_types: Vec<Type> = arg_ids.iter()
                    .map(|id| self.infer_expr(*id))
                    .collect();

                self.check_call(&func_ty, &arg_types, span)
            }

            // If expression
            ExprKind::If { cond, then_branch, else_branch } => {
                let cond_ty = self.infer_expr(*cond);

                // Condition must be bool
                if let Err(e) = self.ctx.unify(&cond_ty, &Type::Bool) {
                    self.report_type_error(e, self.arena.get_expr(*cond).span);
                }

                let then_ty = self.infer_expr(*then_branch);

                if let Some(else_id) = else_branch {
                    let else_ty = self.infer_expr(*else_id);

                    // Both branches must have same type
                    if let Err(e) = self.ctx.unify(&then_ty, &else_ty) {
                        self.report_type_error(e, span);
                    }

                    then_ty
                } else {
                    // No else branch: result is unit
                    Type::Unit
                }
            }

            // Block
            ExprKind::Block { stmts, result } => {
                let block_env = self.env.child();
                let old_env = std::mem::replace(&mut self.env, block_env);

                // Type check statements
                let stmt_range = *stmts;
                for stmt in self.arena.get_stmt_range(stmt_range) {
                    match &stmt.kind {
                        crate::ir::StmtKind::Expr(e) => {
                            self.infer_expr(*e);
                        }
                        crate::ir::StmtKind::Let { pattern, ty, init, .. } => {
                            // Check for closure self-capture before type checking
                            self.check_closure_self_capture(pattern, *init, stmt.span);

                            let init_ty = self.infer_expr(*init);
                            // If type annotation present, unify with inferred type
                            let final_ty = if let Some(type_id) = ty {
                                let declared_ty = self.type_id_to_type(*type_id);
                                if let Err(e) = self.ctx.unify(&declared_ty, &init_ty) {
                                    self.report_type_error(e, self.arena.get_expr(*init).span);
                                }
                                declared_ty
                            } else {
                                init_ty
                            };
                            // Use generalization for let-polymorphism
                            self.bind_pattern_generalized(pattern, final_ty);
                        }
                    }
                }

                // Result type
                let result_ty = if let Some(result_id) = result {
                    self.infer_expr(*result_id)
                } else {
                    Type::Unit
                };

                self.env = old_env;
                result_ty
            }

            // Let binding (as expression)
            ExprKind::Let { pattern, ty, init, .. } => {
                // Check for closure self-capture before type checking
                self.check_closure_self_capture(pattern, *init, span);

                let init_ty = self.infer_expr(*init);
                // If type annotation present, unify with inferred type
                let final_ty = if let Some(type_id) = ty {
                    let declared_ty = self.type_id_to_type(*type_id);
                    if let Err(e) = self.ctx.unify(&declared_ty, &init_ty) {
                        self.report_type_error(e, self.arena.get_expr(*init).span);
                    }
                    declared_ty
                } else {
                    init_ty
                };
                // Use generalization for let-polymorphism
                self.bind_pattern_generalized(pattern, final_ty);
                Type::Unit
            }

            // Lambda
            ExprKind::Lambda { params, ret_ty, body } => {
                let params_slice = self.arena.get_params(*params);
                let param_types: Vec<Type> = params_slice
                    .iter()
                    .map(|p| {
                        match p.ty {
                            Some(type_id) => self.type_id_to_type(type_id),
                            None => self.ctx.fresh_var(),
                        }
                    })
                    .collect();

                // Create scope for lambda
                let mut lambda_env = self.env.child();
                for (param, ty) in params_slice.iter().zip(param_types.iter()) {
                    lambda_env.bind(param.name, ty.clone());
                }

                let old_env = std::mem::replace(&mut self.env, lambda_env);
                let body_ty = self.infer_expr(*body);
                self.env = old_env;

                // Use declared return type if present, otherwise inferred
                let final_ret_ty = match ret_ty {
                    Some(type_id) => {
                        let declared_ty = self.type_id_to_type(*type_id);
                        // Unify declared with inferred
                        if let Err(e) = self.ctx.unify(&declared_ty, &body_ty) {
                            self.report_type_error(e, self.arena.get_expr(*body).span);
                        }
                        declared_ty
                    }
                    None => body_ty,
                };

                Type::Function {
                    params: param_types,
                    ret: Box::new(final_ret_ty),
                }
            }

            // List
            ExprKind::List(elements) => {
                let element_ids = self.arena.get_expr_list(*elements);

                if element_ids.is_empty() {
                    // Empty list: element type is unknown
                    Type::List(Box::new(self.ctx.fresh_var()))
                } else {
                    // Infer element types and unify
                    let first_ty = self.infer_expr(element_ids[0]);
                    for id in &element_ids[1..] {
                        let elem_ty = self.infer_expr(*id);
                        if let Err(e) = self.ctx.unify(&first_ty, &elem_ty) {
                            self.report_type_error(e, self.arena.get_expr(*id).span);
                        }
                    }
                    Type::List(Box::new(first_ty))
                }
            }

            // Tuple
            ExprKind::Tuple(elements) => {
                let element_ids = self.arena.get_expr_list(*elements);
                if element_ids.is_empty() {
                    // Empty tuple is unit type
                    Type::Unit
                } else {
                    let types: Vec<Type> = element_ids.iter()
                        .map(|id| self.infer_expr(*id))
                        .collect();
                    Type::Tuple(types)
                }
            }

            // FunctionSeq: run, try, match
            ExprKind::FunctionSeq(func_seq) => {
                self.check_function_seq(func_seq, span)
            }

            // FunctionExp: map, filter, fold, etc.
            ExprKind::FunctionExp(func_exp) => {
                self.check_function_exp(func_exp, span)
            }

            // Function call with named arguments
            ExprKind::CallNamed { func, args } => {
                self.check_call_named(*func, *args, span)
            }

            // Field access
            ExprKind::Field { receiver, field: _ } => {
                let _receiver_ty = self.infer_expr(*receiver);
                // TODO: implement proper field access type checking
                self.ctx.fresh_var()
            }

            // Index access
            ExprKind::Index { receiver, index } => {
                let receiver_ty = self.infer_expr(*receiver);
                let index_ty = self.infer_expr(*index);

                match self.ctx.resolve(&receiver_ty) {
                    // List indexing: list[int] -> T (panics on out-of-bounds)
                    Type::List(elem_ty) => {
                        if let Err(e) = self.ctx.unify(&index_ty, &Type::Int) {
                            self.report_type_error(e, self.arena.get_expr(*index).span);
                        }
                        (*elem_ty).clone()
                    }
                    // Map indexing: map[K] -> Option<V> (None if key missing)
                    Type::Map { key, value } => {
                        if let Err(e) = self.ctx.unify(&index_ty, &key) {
                            self.report_type_error(e, self.arena.get_expr(*index).span);
                        }
                        Type::Option(value)
                    }
                    // String indexing: str[int] -> str (single codepoint)
                    Type::Str => {
                        if let Err(e) = self.ctx.unify(&index_ty, &Type::Int) {
                            self.report_type_error(e, self.arena.get_expr(*index).span);
                        }
                        Type::Str
                    }
                    // Type variable - defer checking
                    Type::Var(_) => self.ctx.fresh_var(),
                    // Error recovery
                    Type::Error => Type::Error,
                    // Other types - not indexable
                    other => {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "type `{}` is not indexable",
                                other.display(self.interner)
                            ),
                            span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                }
            }

            // Duration and Size literals
            ExprKind::Duration { .. } => Type::Duration,
            ExprKind::Size { .. } => Type::Size,

            // Unit
            ExprKind::Unit => Type::Unit,

            // Config reference
            ExprKind::Config(name) => {
                // TODO: implement config type lookup
                let _ = name;
                self.ctx.fresh_var()
            }

            // Self reference
            ExprKind::SelfRef => {
                // TODO: implement self type in impl blocks
                self.ctx.fresh_var()
            }

            // Function reference
            ExprKind::FunctionRef(name) => {
                // Look up function type and instantiate for polymorphism
                if let Some(scheme) = self.env.lookup_scheme(*name) {
                    self.ctx.instantiate(scheme)
                } else {
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "unknown function `@{}`",
                            self.interner.lookup(*name)
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2003,
                    });
                    Type::Error
                }
            }

            // Hash length in index context
            ExprKind::HashLength => Type::Int,

            // Method call
            ExprKind::MethodCall { receiver, method: _, args } => {
                let _receiver_ty = self.infer_expr(*receiver);
                let arg_ids = self.arena.get_expr_list(*args);
                for id in arg_ids {
                    self.infer_expr(*id);
                }
                // TODO: implement proper method resolution
                self.ctx.fresh_var()
            }

            // Match expression
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee_ty = self.infer_expr(*scrutinee);
                let match_arms = self.arena.get_arms(*arms);

                if match_arms.is_empty() {
                    // Empty match, result type is unknown
                    self.ctx.fresh_var()
                } else {
                    // All arms must have the same type
                    let first_arm_ty = self.infer_expr(match_arms[0].body);
                    for arm in &match_arms[1..] {
                        let arm_ty = self.infer_expr(arm.body);
                        if let Err(e) = self.ctx.unify(&first_arm_ty, &arm_ty) {
                            self.report_type_error(e, arm.span);
                        }
                    }
                    let _ = scrutinee_ty; // TODO: pattern matching type checking
                    first_arm_ty
                }
            }

            // For loop
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                let iter_ty = self.infer_expr(*iter);
                let resolved = self.ctx.resolve(&iter_ty);
                let elem_ty = match resolved {
                    Type::List(elem) => *elem,
                    Type::Set(elem) => *elem,
                    Type::Range(elem) => *elem,
                    Type::Str => Type::Str, // Iterating over str yields str (codepoints)
                    Type::Map { key, value: _ } => *key, // Map iteration yields keys
                    Type::Var(_) => self.ctx.fresh_var(), // Defer for type variables
                    Type::Error => Type::Error, // Error recovery
                    other => {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "`{}` is not iterable",
                                other.display(self.interner)
                            ),
                            span: self.arena.get_expr(*iter).span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                };

                // Create scope for loop body
                let mut loop_env = self.env.child();
                loop_env.bind(*binding, elem_ty);
                let old_env = std::mem::replace(&mut self.env, loop_env);

                // Type check guard if present
                if let Some(guard_id) = guard {
                    let guard_ty = self.infer_expr(*guard_id);
                    if let Err(e) = self.ctx.unify(&guard_ty, &Type::Bool) {
                        self.report_type_error(e, self.arena.get_expr(*guard_id).span);
                    }
                }

                let body_ty = self.infer_expr(*body);
                self.env = old_env;

                if *is_yield {
                    // yield collects into a list
                    Type::List(Box::new(body_ty))
                } else {
                    // do returns unit
                    Type::Unit
                }
            }

            // Loop
            ExprKind::Loop { body } => {
                let _body_ty = self.infer_expr(*body);
                // Loop result depends on break expressions
                self.ctx.fresh_var()
            }

            // Map literal
            ExprKind::Map(entries) => {
                let map_entries = self.arena.get_map_entries(*entries);
                if map_entries.is_empty() {
                    // Empty map: key and value types are unknown
                    Type::Map {
                        key: Box::new(self.ctx.fresh_var()),
                        value: Box::new(self.ctx.fresh_var()),
                    }
                } else {
                    let first_key_ty = self.infer_expr(map_entries[0].key);
                    let first_val_ty = self.infer_expr(map_entries[0].value);
                    for entry in &map_entries[1..] {
                        let key_ty = self.infer_expr(entry.key);
                        let val_ty = self.infer_expr(entry.value);
                        if let Err(e) = self.ctx.unify(&first_key_ty, &key_ty) {
                            self.report_type_error(e, entry.span);
                        }
                        if let Err(e) = self.ctx.unify(&first_val_ty, &val_ty) {
                            self.report_type_error(e, entry.span);
                        }
                    }
                    Type::Map {
                        key: Box::new(first_key_ty),
                        value: Box::new(first_val_ty),
                    }
                }
            }

            // Struct literal
            ExprKind::Struct { name: _, fields } => {
                let field_inits = self.arena.get_field_inits(*fields);
                for init in field_inits {
                    if let Some(value_id) = init.value {
                        self.infer_expr(value_id);
                    }
                }
                // TODO: return proper struct type
                self.ctx.fresh_var()
            }

            // Range
            ExprKind::Range { start, end, inclusive: _ } => {
                let elem_ty = if let Some(start_id) = start {
                    self.infer_expr(*start_id)
                } else if let Some(end_id) = end {
                    self.infer_expr(*end_id)
                } else {
                    Type::Int // unbounded range defaults to int
                };

                if let Some(_start_id) = start {
                    if let Some(end_id) = end {
                        let end_ty = self.infer_expr(*end_id);
                        if let Err(e) = self.ctx.unify(&elem_ty, &end_ty) {
                            self.report_type_error(e, self.arena.get_expr(*end_id).span);
                        }
                    }
                }
                // TODO: Range<T> type
                Type::List(Box::new(elem_ty))
            }

            // Variant constructors
            ExprKind::Ok(inner) => {
                let ok_ty = if let Some(id) = inner {
                    self.infer_expr(*id)
                } else {
                    Type::Unit
                };
                Type::Result {
                    ok: Box::new(ok_ty),
                    err: Box::new(self.ctx.fresh_var()),
                }
            }

            ExprKind::Err(inner) => {
                let err_ty = if let Some(id) = inner {
                    self.infer_expr(*id)
                } else {
                    Type::Unit
                };
                Type::Result {
                    ok: Box::new(self.ctx.fresh_var()),
                    err: Box::new(err_ty),
                }
            }

            ExprKind::Some(inner) => {
                let inner_ty = self.infer_expr(*inner);
                Type::Option(Box::new(inner_ty))
            }

            ExprKind::None => {
                Type::Option(Box::new(self.ctx.fresh_var()))
            }

            // Control flow
            ExprKind::Return(value) => {
                if let Some(id) = value {
                    self.infer_expr(*id);
                }
                Type::Never
            }

            ExprKind::Break(value) => {
                if let Some(id) = value {
                    self.infer_expr(*id);
                }
                Type::Never
            }

            ExprKind::Continue => Type::Never,

            ExprKind::Await(inner) => {
                let inner_ty = self.infer_expr(*inner);
                // TODO: handle async types properly
                let _ = inner_ty;
                self.ctx.fresh_var()
            }

            ExprKind::Try(inner) => {
                let inner_ty = self.infer_expr(*inner);
                let resolved = self.ctx.resolve(&inner_ty);
                // Try operator unwraps Result/Option
                match resolved {
                    Type::Result { ok, err: _ } => *ok,
                    Type::Option(inner) => *inner,
                    Type::Var(_) => self.ctx.fresh_var(), // Defer for type variables
                    Type::Error => Type::Error, // Error recovery
                    other => {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "the `?` operator can only be applied to `Result` or `Option`, \
                                 found `{}`",
                                other.display(self.interner)
                            ),
                            span: self.arena.get_expr(*inner).span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                }
            }

            ExprKind::Assign { target, value } => {
                let target_ty = self.infer_expr(*target);
                let value_ty = self.infer_expr(*value);
                if let Err(e) = self.ctx.unify(&target_ty, &value_ty) {
                    self.report_type_error(e, self.arena.get_expr(*value).span);
                }
                // Assignment returns the assigned value
                value_ty
            }

            // Error placeholder
            ExprKind::Error => Type::Error,
        };

        // Store the type
        self.expr_types.insert(expr_id.index() as u32, ty.clone());
        ty
    }

    /// Check a binary operation.
    ///
    /// Delegates to the TypeOperatorRegistry for type checking.
    fn check_binary_op(
        &mut self,
        op: BinaryOp,
        left: &Type,
        right: &Type,
        span: Span,
    ) -> Type {
        match self.type_operator_registry.check(
            &mut self.ctx,
            self.interner,
            op,
            left,
            right,
            span,
        ) {
            TypeOpResult::Ok(ty) => ty,
            TypeOpResult::Err(e) => {
                self.errors.push(TypeCheckError {
                    message: e.message,
                    span,
                    code: e.code,
                });
                Type::Error
            }
        }
    }

    /// Check a unary operation.
    fn check_unary_op(&mut self, op: UnaryOp, operand: &Type, span: Span) -> Type {
        match op {
            UnaryOp::Neg => {
                let resolved = self.ctx.resolve(operand);
                match resolved {
                    Type::Int | Type::Float => resolved,
                    Type::Var(_) => resolved, // Defer checking for type variables
                    _ => {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "cannot negate `{}`: negation requires a numeric type (int or float)",
                                operand.display(self.interner)
                            ),
                            span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                }
            }
            UnaryOp::Not => {
                if let Err(e) = self.ctx.unify(operand, &Type::Bool) {
                    self.report_type_error(e, span);
                }
                Type::Bool
            }
            UnaryOp::BitNot => {
                if let Err(e) = self.ctx.unify(operand, &Type::Int) {
                    self.report_type_error(e, span);
                }
                Type::Int
            }
            UnaryOp::Try => {
                // ?expr: Result<T, E> -> T (propagates E)
                let ok_ty = self.ctx.fresh_var();
                let err_ty = self.ctx.fresh_var();
                let result_ty = Type::Result {
                    ok: Box::new(ok_ty.clone()),
                    err: Box::new(err_ty),
                };
                if let Err(e) = self.ctx.unify(operand, &result_ty) {
                    self.report_type_error(e, span);
                }
                self.ctx.resolve(&ok_ty)
            }
        }
    }

    /// Check a function call.
    fn check_call(&mut self, func: &Type, args: &[Type], span: Span) -> Type {
        let result = self.ctx.fresh_var();
        let expected = Type::Function {
            params: args.to_vec(),
            ret: Box::new(result.clone()),
        };

        if let Err(e) = self.ctx.unify(func, &expected) {
            self.report_type_error(e, span);
            return Type::Error;
        }

        self.ctx.resolve(&result)
    }

    /// Check a function_seq expression (run, try, match).
    fn check_function_seq(&mut self, func_seq: &FunctionSeq, _span: Span) -> Type {
        match func_seq {
            FunctionSeq::Run { bindings, result, .. } => {
                // Create child scope for bindings
                let run_env = self.env.child();
                let old_env = std::mem::replace(&mut self.env, run_env);

                // Type check each binding/statement and add to scope
                let seq_bindings = self.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, span: binding_span, .. } => {
                            // Check for closure self-capture
                            self.check_closure_self_capture(pattern, *value, *binding_span);

                            let init_ty = self.infer_expr(*value);
                            self.bind_pattern(pattern, init_ty);
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            // Type check for side effects (e.g., assignment)
                            self.infer_expr(*expr);
                        }
                    }
                }

                // Type check result expression
                let result_ty = self.infer_expr(*result);

                // Restore parent scope
                self.env = old_env;
                result_ty
            }

            FunctionSeq::Try { bindings, result, .. } => {
                // Similar to Run, but bindings unwrap Result/Option
                let try_env = self.env.child();
                let old_env = std::mem::replace(&mut self.env, try_env);

                let seq_bindings = self.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, span: binding_span, .. } => {
                            // Check for closure self-capture
                            self.check_closure_self_capture(pattern, *value, *binding_span);

                            let init_ty = self.infer_expr(*value);
                            // Unwrap Result<T, E> or Option<T> to get T
                            let unwrapped = match &init_ty {
                                Type::Result { ok, .. } => (**ok).clone(),
                                Type::Option(some_ty) => (**some_ty).clone(),
                                other => other.clone(),
                            };
                            self.bind_pattern(pattern, unwrapped);
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            // Type check for side effects
                            self.infer_expr(*expr);
                        }
                    }
                }

                // Result expression should be Result or Option
                let result_ty = self.infer_expr(*result);

                self.env = old_env;
                result_ty
            }

            FunctionSeq::Match { scrutinee, arms, .. } => {
                let scrutinee_ty = self.infer_expr(*scrutinee);
                let match_arms = self.arena.get_arms(*arms);

                if match_arms.is_empty() {
                    self.ctx.fresh_var()
                } else {
                    // All arms must have the same type
                    let first_arm_ty = self.infer_expr(match_arms[0].body);
                    for arm in &match_arms[1..] {
                        let arm_ty = self.infer_expr(arm.body);
                        if let Err(e) = self.ctx.unify(&first_arm_ty, &arm_ty) {
                            self.report_type_error(e, arm.span);
                        }
                    }
                    let _ = scrutinee_ty; // TODO: pattern matching type checking
                    first_arm_ty
                }
            }
        }
    }

    /// Check a function_exp expression (map, filter, fold, etc.).
    ///
    /// Uses the pattern registry for Open/Closed principle compliance.
    /// Each pattern implementation is in a separate file under `patterns/`.
    fn check_function_exp(&mut self, func_exp: &crate::ir::FunctionExp, _span: Span) -> Type {
        let props = self.arena.get_named_exprs(func_exp.props);

        // Type check all property values
        let prop_types: HashMap<Name, Type> = props.iter()
            .map(|prop| (prop.name, self.infer_expr(prop.value)))
            .collect();

        // Look up pattern definition from registry
        let Some(pattern) = self.registry.get(func_exp.kind) else {
            // Unknown pattern kind - should not happen if registry is complete
            return Type::Error;
        };

        // Create type check context with property types
        let mut ctx = TypeCheckContext::new(self.interner, &mut self.ctx, prop_types);

        // Delegate to pattern's type_check implementation
        pattern.type_check(&mut ctx)
    }

    /// Check a function call with named arguments.
    fn check_call_named(&mut self, func: ExprId, args: CallArgRange, span: Span) -> Type {
        let func_ty = self.infer_expr(func);
        let call_args = self.arena.get_call_args(args);

        // Type check each argument
        let arg_types: Vec<Type> = call_args.iter()
            .map(|arg| self.infer_expr(arg.value))
            .collect();

        // Unify with function type
        match func_ty {
            Type::Function { params, ret } => {
                // Check argument count
                if params.len() != arg_types.len() {
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "expected {} arguments, found {}",
                            params.len(),
                            arg_types.len()
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2004,
                    });
                    return Type::Error;
                }

                // Unify argument types with parameter types
                for (i, (param_ty, arg_ty)) in params.iter().zip(arg_types.iter()).enumerate() {
                    if let Err(e) = self.ctx.unify(param_ty, arg_ty) {
                        let arg_span = call_args[i].span;
                        self.report_type_error(e, arg_span);
                    }
                }

                *ret
            }
            Type::Error => Type::Error,
            _ => {
                self.errors.push(TypeCheckError {
                    message: "expected function type for call".to_string(),
                    span,
                    code: crate::diagnostic::ErrorCode::E2001,
                });
                Type::Error
            }
        }
    }

    /// Bind a pattern to a type with generalization (for let-polymorphism).
    ///
    /// This is the key to Hindley-Milner let-polymorphism: we generalize
    /// the type before binding, so that `let id = x -> x` has type `∀a. a -> a`
    /// and each use of `id` gets fresh type variables.
    fn bind_pattern_generalized(&mut self, pattern: &crate::ir::BindingPattern, ty: Type) {
        use crate::ir::BindingPattern;

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
    fn bind_pattern(&mut self, pattern: &crate::ir::BindingPattern, ty: Type) {
        use crate::ir::BindingPattern;

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
                // TODO: implement proper struct type checking
                for (field_name, nested_pattern) in fields {
                    let field_ty = self.ctx.fresh_var();
                    if let Some(nested) = nested_pattern {
                        // TODO: get field type from struct type
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

    /// Report a type error.
    fn report_type_error(&mut self, err: TypeError, span: Span) {
        let diag = err.to_diagnostic(span, self.interner);
        self.errors.push(TypeCheckError {
            message: diag.message.clone(),
            span,
            code: diag.code,
        });
    }

    // =========================================================================
    // Cycle Detection
    // =========================================================================

    /// Collect free variable references from an expression.
    ///
    /// This is used for closure self-capture detection. A variable is "free"
    /// if it's referenced but not bound within the expression.
    fn collect_free_vars(&self, expr_id: ExprId, bound: &HashSet<Name>) -> HashSet<Name> {
        let mut free = HashSet::new();
        self.collect_free_vars_inner(expr_id, bound, &mut free);
        free
    }

    /// Inner recursive helper for free variable collection.
    fn collect_free_vars_inner(
        &self,
        expr_id: ExprId,
        bound: &HashSet<Name>,
        free: &mut HashSet<Name>,
    ) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Variable reference - free if not bound
            ExprKind::Ident(name) => {
                if !bound.contains(name) {
                    free.insert(*name);
                }
            }

            // Function reference - check if it refers to a local binding
            ExprKind::FunctionRef(name) => {
                // Function refs using @name syntax typically refer to top-level functions,
                // not local bindings. However, if someone writes `let f = ...; @f()`,
                // we should detect that too for completeness.
                if !bound.contains(name) {
                    free.insert(*name);
                }
            }

            // Literals - no free variables
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::Config(_)
            | ExprKind::SelfRef
            | ExprKind::HashLength
            | ExprKind::None
            | ExprKind::Continue
            | ExprKind::Error => {}

            // Binary - check both sides
            ExprKind::Binary { left, right, .. } => {
                self.collect_free_vars_inner(*left, bound, free);
                self.collect_free_vars_inner(*right, bound, free);
            }

            // Unary - check operand
            ExprKind::Unary { operand, .. } => {
                self.collect_free_vars_inner(*operand, bound, free);
            }

            // Call - check function and args
            ExprKind::Call { func, args } => {
                self.collect_free_vars_inner(*func, bound, free);
                for arg_id in self.arena.get_expr_list(*args) {
                    self.collect_free_vars_inner(*arg_id, bound, free);
                }
            }

            // Named call
            ExprKind::CallNamed { func, args } => {
                self.collect_free_vars_inner(*func, bound, free);
                for arg in self.arena.get_call_args(*args) {
                    self.collect_free_vars_inner(arg.value, bound, free);
                }
            }

            // Method call
            ExprKind::MethodCall { receiver, args, .. } => {
                self.collect_free_vars_inner(*receiver, bound, free);
                for arg_id in self.arena.get_expr_list(*args) {
                    self.collect_free_vars_inner(*arg_id, bound, free);
                }
            }

            // Field access
            ExprKind::Field { receiver, .. } => {
                self.collect_free_vars_inner(*receiver, bound, free);
            }

            // Index access
            ExprKind::Index { receiver, index } => {
                self.collect_free_vars_inner(*receiver, bound, free);
                self.collect_free_vars_inner(*index, bound, free);
            }

            // If expression
            ExprKind::If { cond, then_branch, else_branch } => {
                self.collect_free_vars_inner(*cond, bound, free);
                self.collect_free_vars_inner(*then_branch, bound, free);
                if let Some(else_id) = else_branch {
                    self.collect_free_vars_inner(*else_id, bound, free);
                }
            }

            // Match expression
            ExprKind::Match { scrutinee, arms } => {
                self.collect_free_vars_inner(*scrutinee, bound, free);
                for arm in self.arena.get_arms(*arms) {
                    // TODO: arm patterns can bind variables
                    self.collect_free_vars_inner(arm.body, bound, free);
                }
            }

            // For loop - binding is bound in body
            ExprKind::For { binding, iter, guard, body, .. } => {
                self.collect_free_vars_inner(*iter, bound, free);
                let mut body_bound = bound.clone();
                body_bound.insert(*binding);
                if let Some(guard_id) = guard {
                    self.collect_free_vars_inner(*guard_id, &body_bound, free);
                }
                self.collect_free_vars_inner(*body, &body_bound, free);
            }

            // Loop
            ExprKind::Loop { body } => {
                self.collect_free_vars_inner(*body, bound, free);
            }

            // Block - statements can introduce bindings
            ExprKind::Block { stmts, result } => {
                let mut block_bound = bound.clone();
                for stmt in self.arena.get_stmt_range(*stmts) {
                    match &stmt.kind {
                        crate::ir::StmtKind::Expr(e) => {
                            self.collect_free_vars_inner(*e, &block_bound, free);
                        }
                        crate::ir::StmtKind::Let { pattern, init, .. } => {
                            // Init is evaluated before the binding is in scope
                            self.collect_free_vars_inner(*init, &block_bound, free);
                            // Add pattern bindings for subsequent statements
                            self.add_pattern_bindings(pattern, &mut block_bound);
                        }
                    }
                }
                if let Some(result_id) = result {
                    self.collect_free_vars_inner(*result_id, &block_bound, free);
                }
            }

            // Let binding (as expression)
            ExprKind::Let { pattern: _, init, .. } => {
                // Init is evaluated before the binding
                self.collect_free_vars_inner(*init, bound, free);
                // Note: the binding itself doesn't introduce scope here,
                // that's handled by the containing block
            }

            // Lambda - params are bound in body
            ExprKind::Lambda { params, body, .. } => {
                let mut lambda_bound = bound.clone();
                for param in self.arena.get_params(*params) {
                    lambda_bound.insert(param.name);
                }
                self.collect_free_vars_inner(*body, &lambda_bound, free);
            }

            // List
            ExprKind::List(elements) => {
                for elem_id in self.arena.get_expr_list(*elements) {
                    self.collect_free_vars_inner(*elem_id, bound, free);
                }
            }

            // Map
            ExprKind::Map(entries) => {
                for entry in self.arena.get_map_entries(*entries) {
                    self.collect_free_vars_inner(entry.key, bound, free);
                    self.collect_free_vars_inner(entry.value, bound, free);
                }
            }

            // Struct literal
            ExprKind::Struct { fields, .. } => {
                for init in self.arena.get_field_inits(*fields) {
                    if let Some(value_id) = init.value {
                        self.collect_free_vars_inner(value_id, bound, free);
                    } else {
                        // Shorthand field: { x } is equivalent to { x: x }
                        if !bound.contains(&init.name) {
                            free.insert(init.name);
                        }
                    }
                }
            }

            // Tuple
            ExprKind::Tuple(elements) => {
                for elem_id in self.arena.get_expr_list(*elements) {
                    self.collect_free_vars_inner(*elem_id, bound, free);
                }
            }

            // Range
            ExprKind::Range { start, end, .. } => {
                if let Some(start_id) = start {
                    self.collect_free_vars_inner(*start_id, bound, free);
                }
                if let Some(end_id) = end {
                    self.collect_free_vars_inner(*end_id, bound, free);
                }
            }

            // Variant constructors
            ExprKind::Ok(inner) | ExprKind::Err(inner) => {
                if let Some(id) = inner {
                    self.collect_free_vars_inner(*id, bound, free);
                }
            }
            ExprKind::Some(inner) => {
                self.collect_free_vars_inner(*inner, bound, free);
            }

            // Control flow
            ExprKind::Return(value) | ExprKind::Break(value) => {
                if let Some(id) = value {
                    self.collect_free_vars_inner(*id, bound, free);
                }
            }

            ExprKind::Await(inner) | ExprKind::Try(inner) => {
                self.collect_free_vars_inner(*inner, bound, free);
            }

            ExprKind::Assign { target, value } => {
                self.collect_free_vars_inner(*target, bound, free);
                self.collect_free_vars_inner(*value, bound, free);
            }

            // FunctionSeq
            ExprKind::FunctionSeq(func_seq) => {
                self.collect_free_vars_function_seq(func_seq, bound, free);
            }

            // FunctionExp
            ExprKind::FunctionExp(func_exp) => {
                for prop in self.arena.get_named_exprs(func_exp.props) {
                    self.collect_free_vars_inner(prop.value, bound, free);
                }
            }
        }
    }

    /// Collect free variables from a FunctionSeq (run, try, match).
    fn collect_free_vars_function_seq(
        &self,
        func_seq: &FunctionSeq,
        bound: &HashSet<Name>,
        free: &mut HashSet<Name>,
    ) {
        match func_seq {
            FunctionSeq::Run { bindings, result, .. }
            | FunctionSeq::Try { bindings, result, .. } => {
                let mut seq_bound = bound.clone();
                for binding in self.arena.get_seq_bindings(*bindings) {
                    match binding {
                        SeqBinding::Let { pattern, value, .. } => {
                            self.collect_free_vars_inner(*value, &seq_bound, free);
                            self.add_pattern_bindings(pattern, &mut seq_bound);
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            self.collect_free_vars_inner(*expr, &seq_bound, free);
                        }
                    }
                }
                self.collect_free_vars_inner(*result, &seq_bound, free);
            }
            FunctionSeq::Match { scrutinee, arms, .. } => {
                self.collect_free_vars_inner(*scrutinee, bound, free);
                for arm in self.arena.get_arms(*arms) {
                    // TODO: arm patterns can bind variables
                    self.collect_free_vars_inner(arm.body, bound, free);
                }
            }
        }
    }

    /// Add bindings from a pattern to the bound set.
    fn add_pattern_bindings(&self, pattern: &crate::ir::BindingPattern, bound: &mut HashSet<Name>) {
        use crate::ir::BindingPattern;
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
    fn check_closure_self_capture(
        &mut self,
        pattern: &crate::ir::BindingPattern,
        init: ExprId,
        span: Span,
    ) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;
    use crate::ir::SharedInterner;

    fn check_source(source: &str) -> (ParseResult, TypedModule) {
        let interner = SharedInterner::default();
        let tokens = lexer::lex(source, &interner);
        let parsed = parser::parse(&tokens, &interner);
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
        // No explicit return type - let inference determine it's [int]
        let (parsed, typed) = check_source("@test () = [1, 2, 3]");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::List(Box::new(Type::Int)));
    }

    #[test]
    fn test_type_mismatch_error() {
        let (_, typed) = check_source("@test () -> int = if 42 then 1 else 2");

        // Should have error: condition must be bool
        assert!(typed.has_errors());
        assert!(typed.errors[0].message.contains("type mismatch") ||
                typed.errors[0].message.contains("expected"));
    }

    #[test]
    fn test_typed_module_salsa_traits() {
        use std::collections::HashSet;

        let (_, typed1) = check_source("@main () -> int = 42");
        let (_, typed2) = check_source("@main () -> int = 42");
        let (_, typed3) = check_source("@main () -> bool = true"); // Different return type

        // Eq
        assert_eq!(typed1, typed2);
        assert_ne!(typed1, typed3);

        // Hash
        let mut set = HashSet::new();
        set.insert(typed1.clone());
        set.insert(typed2); // duplicate
        set.insert(typed3);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_function_with_typed_params() {
        // Function with typed params should infer correctly
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
        // Calling a typed function
        let (_, typed) = check_source("@double (x: int) -> int = x * 2");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func_type = &typed.function_types[0];
        assert_eq!(func_type.return_type, Type::Int);
    }

    #[test]
    fn test_lambda_with_typed_param() {
        // Lambda with typed param
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
        // run pattern with let bindings
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
        // Direct self-capture: let f = () -> f()
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself") &&
            e.code == crate::diagnostic::ErrorCode::E2007
        ));
    }

    #[test]
    fn test_closure_self_capture_call() {
        // Self-capture with call: let f = () -> f()
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
        // Using an outer binding with the same name is NOT self-capture
        // Here f is already bound before the lambda is created
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = 42,
                let g = () -> f,
                g()
            )
        "#);

        // This should NOT be an error - g uses outer f, not itself
        assert!(!typed.errors.iter().any(|e|
            e.code == crate::diagnostic::ErrorCode::E2007
        ));
    }

    #[test]
    fn test_no_self_capture_non_lambda() {
        // Non-lambda let bindings don't have self-capture issues
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
        // Self-capture in run() context
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
        // Self-capture through nested expression
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
        // This tests that using a name from outer scope is valid
        // (the fix for actual mutual recursion would require explicit rec annotations)
        let (_, typed) = check_source(r#"
            @f (x: int) -> int = x
            @test () -> int = run(
                let g = (x: int) -> @f(x),
                g(1)
            )
        "#);

        // Using @f is valid - it's a top-level function, not self-capture
        assert!(!typed.errors.iter().any(|e|
            e.code == crate::diagnostic::ErrorCode::E2007
        ));
    }

    // =========================================================================
    // TypeRegistry Integration Tests
    // =========================================================================

    #[test]
    fn test_type_registry_in_checker() {
        // Test that TypeRegistry is properly initialized in TypeChecker
        let interner = SharedInterner::default();
        let tokens = lexer::lex("@main () -> int = 42", &interner);
        let parsed = parser::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        // Register a user type
        let point_name = interner.intern("Point");
        let x_name = interner.intern("x");
        let y_name = interner.intern("y");

        let type_id = checker.type_registry.register_struct(
            point_name,
            vec![(x_name, Type::Int), (y_name, Type::Int)],
            crate::ir::Span::new(0, 0),
            vec![],
        );

        // Verify lookup works
        assert!(checker.type_registry.contains(point_name));
        let entry = checker.type_registry.get_by_id(type_id).unwrap();
        assert_eq!(entry.name, point_name);
    }

    #[test]
    fn test_type_id_to_type_with_registry() {
        // Test that type_id_to_type uses the registry for compound types
        let interner = SharedInterner::default();
        let tokens = lexer::lex("@main () -> int = 42", &interner);
        let parsed = parser::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        // Register an alias type
        let id_name = interner.intern("UserId");
        let type_id = checker.type_registry.register_alias(
            id_name,
            Type::Int,
            crate::ir::Span::new(0, 0),
            vec![],
        );

        // Convert TypeId to Type - should resolve to the aliased type
        let resolved = checker.type_id_to_type(type_id);
        assert_eq!(resolved, Type::Int);
    }

    #[test]
    fn test_type_id_to_type_with_struct() {
        // Test struct type resolution
        let interner = SharedInterner::default();
        let tokens = lexer::lex("@main () -> int = 42", &interner);
        let parsed = parser::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        // Register a struct type
        let point_name = interner.intern("Point");
        let type_id = checker.type_registry.register_struct(
            point_name,
            vec![],
            crate::ir::Span::new(0, 0),
            vec![],
        );

        // Convert TypeId to Type - should resolve to Named(point_name)
        let resolved = checker.type_id_to_type(type_id);
        assert_eq!(resolved, Type::Named(point_name));
    }
}
