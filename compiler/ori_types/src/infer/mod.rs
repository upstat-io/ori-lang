//! Type inference engine for Types V2.
//!
//! This module provides the main orchestrator for Hindley-Milner type inference,
//! connecting the Pool, `UnifyEngine`, and error system into a unified inference API.
//!
//! # Architecture
//!
//! `InferEngine` wraps `UnifyEngine` and adds:
//! - Expression type storage (`expr_types`)
//! - Type environment management (`TypeEnvV2`)
//! - Context-aware error reporting
//! - Bidirectional type checking (`infer` vs `check`)
//!
//! # Usage
//!
//! ```ignore
//! let mut pool = Pool::new();
//! let mut engine = InferEngine::new(&mut pool);
//!
//! // Infer type of expression (bottom-up)
//! let ty = engine.infer_literal_int();
//! assert_eq!(ty, Idx::INT);
//!
//! // Check expression against expected type (top-down)
//! engine.check(expr_id, Expected::from_type(Idx::INT))?;
//! ```
//!
//! # Design Notes
//!
//! This is part of the Types V2 migration. The engine uses:
//! - `Idx` as the canonical type handle (not `Type` or `TypeId`)
//! - `UnifyEngine` for O(α(n)) unification
//! - `Pool` for O(1) type equality
//! - Rich error context for helpful diagnostic messages

mod env;
mod expr;

pub use env::TypeEnvV2;
pub use expr::infer_expr;

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::{
    diff_types, ContextKind, ErrorContext, Expected, Idx, Pool, Suggestion, TypeCheckError,
    TypeErrorKind, TypeProblem, UnifyEngine, UnifyError,
};

/// Expression ID type (mirrors `ori_ir::ExprId`).
///
/// Using a simple usize to avoid dependency on `ori_ir` for the core types module.
/// When integrating with `ori_typeck`, convert `ExprId` to/from this.
pub type ExprIndex = usize;

/// The type inference engine.
///
/// Orchestrates Hindley-Milner type inference using the Types V2 infrastructure:
/// - `Pool` for type storage and interning
/// - `UnifyEngine` for unification with path compression
/// - `TypeEnvV2` for name bindings
/// - Error accumulation for comprehensive diagnostics
///
/// # Component Structure
///
/// ```text
/// InferEngine
/// ├── UnifyEngine (unification, resolution, generalization)
/// │   └── Pool (type storage, interning, flags)
/// ├── TypeEnvV2 (name → type scheme bindings)
/// ├── expr_types (expression → inferred type)
/// ├── context_stack (error context tracking)
/// └── errors (accumulated type errors)
/// ```
pub struct InferEngine<'pool> {
    /// The unification engine (owns mutable pool access).
    unify: UnifyEngine<'pool>,

    /// Type environment for name bindings.
    env: TypeEnvV2,

    /// Inferred types for expressions (expr index → type).
    expr_types: FxHashMap<ExprIndex, Idx>,

    /// Context stack for error reporting.
    context_stack: Vec<ContextKind>,

    /// Accumulated type check errors.
    errors: Vec<TypeCheckError>,
}

impl<'pool> InferEngine<'pool> {
    /// Create a new inference engine.
    pub fn new(pool: &'pool mut Pool) -> Self {
        Self {
            unify: UnifyEngine::new(pool),
            env: TypeEnvV2::new(),
            expr_types: FxHashMap::default(),
            context_stack: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Create a new inference engine with an existing environment.
    ///
    /// Use this when you need to share type bindings across inference sessions.
    pub fn with_env(pool: &'pool mut Pool, env: TypeEnvV2) -> Self {
        Self {
            unify: UnifyEngine::new(pool),
            env,
            expr_types: FxHashMap::default(),
            context_stack: Vec::new(),
            errors: Vec::new(),
        }
    }

    // ========================================
    // Pool Access
    // ========================================

    /// Get read-only access to the pool.
    #[inline]
    pub fn pool(&self) -> &Pool {
        self.unify.pool()
    }

    /// Get mutable access to the pool (through the unify engine).
    #[inline]
    pub fn pool_mut(&mut self) -> &mut Pool {
        self.unify.pool_mut()
    }

    /// Get access to the unification engine.
    #[inline]
    pub fn unify(&mut self) -> &mut UnifyEngine<'pool> {
        &mut self.unify
    }

    // ========================================
    // Environment Access
    // ========================================

    /// Get the type environment.
    #[inline]
    pub fn env(&self) -> &TypeEnvV2 {
        &self.env
    }

    /// Get mutable access to the type environment.
    #[inline]
    pub fn env_mut(&mut self) -> &mut TypeEnvV2 {
        &mut self.env
    }

    /// Enter a new scope (for let bindings, lambdas, etc.).
    ///
    /// This:
    /// 1. Increases the unification rank (for generalization)
    /// 2. Creates a child environment scope
    pub fn enter_scope(&mut self) {
        self.unify.enter_scope();
        self.env = self.env.child();
    }

    /// Exit the current scope.
    ///
    /// This:
    /// 1. Decreases the unification rank
    /// 2. Restores the parent environment
    ///
    /// Call `generalize()` on relevant types BEFORE exiting to capture
    /// variables that should be quantified.
    pub fn exit_scope(&mut self) {
        self.unify.exit_scope();
        if let Some(parent) = self.env.parent() {
            self.env = parent;
        }
    }

    // ========================================
    // Type Variable Creation
    // ========================================

    /// Create a fresh unbound type variable.
    #[inline]
    pub fn fresh_var(&mut self) -> Idx {
        self.unify.fresh_var()
    }

    /// Create a fresh named type variable (for better error messages).
    #[inline]
    pub fn fresh_named_var(&mut self, name: Name) -> Idx {
        self.unify.fresh_named_var(name)
    }

    // ========================================
    // Resolution & Unification
    // ========================================

    /// Resolve a type by following links.
    #[inline]
    pub fn resolve(&mut self, ty: Idx) -> Idx {
        self.unify.resolve(ty)
    }

    /// Unify two types.
    #[inline]
    pub fn unify_types(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        self.unify.unify(a, b)
    }

    // ========================================
    // Generalization & Instantiation
    // ========================================

    /// Generalize a type at the current scope.
    ///
    /// Returns a type scheme if any variables were generalized,
    /// or the original type if it's monomorphic.
    #[inline]
    pub fn generalize(&mut self, ty: Idx) -> Idx {
        self.unify.generalize(ty)
    }

    /// Instantiate a type scheme with fresh variables.
    ///
    /// Returns the type unchanged if it's not a scheme.
    #[inline]
    pub fn instantiate(&mut self, scheme: Idx) -> Idx {
        self.unify.instantiate(scheme)
    }

    // ========================================
    // Expression Type Storage
    // ========================================

    /// Store the inferred type for an expression.
    pub fn store_type(&mut self, expr: ExprIndex, ty: Idx) {
        self.expr_types.insert(expr, ty);
    }

    /// Get the inferred type for an expression.
    pub fn get_type(&self, expr: ExprIndex) -> Option<Idx> {
        self.expr_types.get(&expr).copied()
    }

    /// Get all expression types.
    pub fn expr_types(&self) -> &FxHashMap<ExprIndex, Idx> {
        &self.expr_types
    }

    /// Take expression types, leaving an empty map.
    pub fn take_expr_types(&mut self) -> FxHashMap<ExprIndex, Idx> {
        std::mem::take(&mut self.expr_types)
    }

    // ========================================
    // Context Management
    // ========================================

    /// Push a context onto the stack (for nested error tracking).
    pub fn push_context(&mut self, ctx: ContextKind) {
        self.context_stack.push(ctx);
    }

    /// Pop a context from the stack.
    pub fn pop_context(&mut self) -> Option<ContextKind> {
        self.context_stack.pop()
    }

    /// Get the current context (top of stack).
    pub fn current_context(&self) -> Option<&ContextKind> {
        self.context_stack.last()
    }

    /// Execute a closure with a temporary context pushed.
    ///
    /// The context is automatically popped when the closure returns.
    pub fn with_context<T, F>(&mut self, ctx: ContextKind, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.push_context(ctx);
        let result = f(self);
        self.pop_context();
        result
    }

    // ========================================
    // Error Management
    // ========================================

    /// Check if any errors have been accumulated.
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get accumulated errors.
    #[inline]
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.errors
    }

    /// Take accumulated errors, leaving an empty vector.
    pub fn take_errors(&mut self) -> Vec<TypeCheckError> {
        std::mem::take(&mut self.errors)
    }

    /// Push a type check error.
    pub fn push_error(&mut self, error: TypeCheckError) {
        self.errors.push(error);
    }

    // ========================================
    // Bidirectional Type Checking
    // ========================================

    /// Check a type against an expected type.
    ///
    /// This is the "check" direction of bidirectional type checking:
    /// given an expected type, verify that the inferred type matches.
    ///
    /// On unification failure, converts the error to a rich `TypeCheckError`
    /// with context and suggestions.
    #[expect(
        clippy::result_large_err,
        reason = "TypeCheckError is intentionally large for rich error context with suggestions"
    )]
    pub fn check_type(
        &mut self,
        inferred: Idx,
        expected: &Expected,
        span: ori_ir::Span,
    ) -> Result<(), TypeCheckError> {
        match self.unify.unify(inferred, expected.ty) {
            Ok(()) => Ok(()),
            Err(ref unify_err) => {
                let error = self.make_type_error(inferred, expected, span, unify_err);
                self.errors.push(error.clone());
                Err(error)
            }
        }
    }

    /// Convert a unification error to a rich type check error.
    fn make_type_error(
        &self,
        inferred: Idx,
        expected: &Expected,
        span: ori_ir::Span,
        unify_err: &UnifyError,
    ) -> TypeCheckError {
        // Resolve both types to get their final forms
        let resolved_inferred = self.unify.resolve_readonly(inferred);
        let resolved_expected = self.unify.resolve_readonly(expected.ty);

        // Identify specific problems between the types
        let problems = diff_types(self.pool(), resolved_expected, resolved_inferred);

        // Generate suggestions based on the problems
        let suggestions = self.generate_suggestions(&problems);

        // Build context from current state
        let context = ErrorContext {
            checking: self.current_context().cloned(),
            expected_because: Some(expected.origin.clone()),
            notes: self.make_context_notes(unify_err),
        };

        TypeCheckError {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: resolved_expected,
                found: resolved_inferred,
                problems,
            },
            context,
            suggestions,
        }
    }

    /// Generate suggestions based on identified problems.
    #[expect(
        clippy::unused_self,
        reason = "Will use pool for formatting when string interning is added"
    )]
    fn generate_suggestions(&self, problems: &[TypeProblem]) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        for problem in problems {
            suggestions.extend(problem.suggestions());
        }

        // Sort by priority and deduplicate
        suggestions.sort_by_key(|s| s.priority);
        suggestions.dedup_by(|a, b| a.message == b.message);

        suggestions
    }

    /// Generate context notes from a unification error.
    #[expect(
        clippy::unused_self,
        reason = "Will use pool for name resolution when string interning is added"
    )]
    fn make_context_notes(&self, unify_err: &UnifyError) -> Vec<String> {
        let mut notes = Vec::new();

        match unify_err {
            UnifyError::InfiniteType { var_id, .. } => {
                notes.push(format!(
                    "Type variable ${var_id} would create an infinite type"
                ));
            }
            UnifyError::RigidMismatch { rigid_name, .. } => {
                // Note: rigid_name is a Name which we can't resolve to string here.
                // The error formatter will need access to a string interner.
                notes.push(format!(
                    "Type parameter (id={}) is rigid and cannot be unified with a concrete type",
                    rigid_name.raw()
                ));
            }
            UnifyError::RigidRigidMismatch { rigid1, rigid2 } => {
                notes.push(format!(
                    "Type parameters (id={}) and (id={}) are different and cannot be unified",
                    rigid1.raw(),
                    rigid2.raw()
                ));
            }
            _ => {}
        }

        notes
    }

    // ========================================
    // Literal Inference Helpers
    // ========================================

    /// Infer the type of an integer literal.
    #[inline]
    pub fn infer_int(&self) -> Idx {
        Idx::INT
    }

    /// Infer the type of a float literal.
    #[inline]
    pub fn infer_float(&self) -> Idx {
        Idx::FLOAT
    }

    /// Infer the type of a boolean literal.
    #[inline]
    pub fn infer_bool(&self) -> Idx {
        Idx::BOOL
    }

    /// Infer the type of a string literal.
    #[inline]
    pub fn infer_str(&self) -> Idx {
        Idx::STR
    }

    /// Infer the type of a character literal.
    #[inline]
    pub fn infer_char(&self) -> Idx {
        Idx::CHAR
    }

    /// Infer the type of a byte literal.
    #[inline]
    pub fn infer_byte(&self) -> Idx {
        Idx::BYTE
    }

    /// Infer the type of a unit literal.
    #[inline]
    pub fn infer_unit(&self) -> Idx {
        Idx::UNIT
    }

    // ========================================
    // Collection Inference Helpers
    // ========================================

    /// Infer the type of an empty list.
    ///
    /// Returns `[?a]` where `?a` is a fresh type variable.
    pub fn infer_empty_list(&mut self) -> Idx {
        let elem = self.fresh_var();
        self.pool_mut().list(elem)
    }

    /// Infer the type of a list with a known element type.
    pub fn infer_list(&mut self, elem_ty: Idx) -> Idx {
        self.pool_mut().list(elem_ty)
    }

    /// Infer the type of an empty map.
    ///
    /// Returns `{?k: ?v}` where `?k` and `?v` are fresh type variables.
    pub fn infer_empty_map(&mut self) -> Idx {
        let key = self.fresh_var();
        let value = self.fresh_var();
        self.pool_mut().map(key, value)
    }

    /// Infer the type of a map with known key and value types.
    pub fn infer_map(&mut self, key_ty: Idx, value_ty: Idx) -> Idx {
        self.pool_mut().map(key_ty, value_ty)
    }

    /// Infer the type of a tuple.
    pub fn infer_tuple(&mut self, elem_types: &[Idx]) -> Idx {
        self.pool_mut().tuple(elem_types)
    }

    /// Infer the type of an option with known inner type.
    pub fn infer_option(&mut self, inner_ty: Idx) -> Idx {
        self.pool_mut().option(inner_ty)
    }

    /// Infer the type of a result with known ok and error types.
    pub fn infer_result(&mut self, ok_ty: Idx, err_ty: Idx) -> Idx {
        self.pool_mut().result(ok_ty, err_ty)
    }

    /// Infer the type of a function.
    pub fn infer_function(&mut self, params: &[Idx], ret: Idx) -> Idx {
        self.pool_mut().function(params, ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExpectedOrigin;

    #[test]
    fn test_literal_inference() {
        let mut pool = Pool::new();
        let engine = InferEngine::new(&mut pool);

        assert_eq!(engine.infer_int(), Idx::INT);
        assert_eq!(engine.infer_float(), Idx::FLOAT);
        assert_eq!(engine.infer_bool(), Idx::BOOL);
        assert_eq!(engine.infer_str(), Idx::STR);
        assert_eq!(engine.infer_char(), Idx::CHAR);
        assert_eq!(engine.infer_byte(), Idx::BYTE);
        assert_eq!(engine.infer_unit(), Idx::UNIT);
    }

    #[test]
    fn test_scope_management() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        // Initial state
        let initial_rank = engine.unify.current_rank();

        // Enter scope
        engine.enter_scope();
        assert!(engine.unify.current_rank() > initial_rank);

        // Exit scope
        engine.exit_scope();
        assert_eq!(engine.unify.current_rank(), initial_rank);
    }

    #[test]
    fn test_context_management() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        assert!(engine.current_context().is_none());

        engine.push_context(ContextKind::IfCondition);
        assert!(matches!(
            engine.current_context(),
            Some(ContextKind::IfCondition)
        ));

        engine.push_context(ContextKind::FunctionReturn { func_name: None });
        assert!(matches!(
            engine.current_context(),
            Some(ContextKind::FunctionReturn { .. })
        ));

        engine.pop_context();
        assert!(matches!(
            engine.current_context(),
            Some(ContextKind::IfCondition)
        ));
    }

    #[test]
    fn test_with_context() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        let result = engine.with_context(ContextKind::ListElement { index: 0 }, |eng| {
            assert!(matches!(
                eng.current_context(),
                Some(ContextKind::ListElement { index: 0 })
            ));
            42
        });

        assert_eq!(result, 42);
        assert!(engine.current_context().is_none());
    }

    #[test]
    fn test_expression_type_storage() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        engine.store_type(0, Idx::INT);
        engine.store_type(1, Idx::STR);
        engine.store_type(2, Idx::BOOL);

        assert_eq!(engine.get_type(0), Some(Idx::INT));
        assert_eq!(engine.get_type(1), Some(Idx::STR));
        assert_eq!(engine.get_type(2), Some(Idx::BOOL));
        assert_eq!(engine.get_type(99), None);
    }

    #[test]
    fn test_collection_inference() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        // Empty list has fresh variable element type
        let empty_list = engine.infer_empty_list();
        assert_eq!(engine.pool().tag(empty_list), crate::Tag::List);

        // List with known element type
        let int_list = engine.infer_list(Idx::INT);
        assert_eq!(engine.pool().tag(int_list), crate::Tag::List);

        // Tuple
        let tuple = engine.infer_tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
        assert_eq!(engine.pool().tag(tuple), crate::Tag::Tuple);
        assert_eq!(engine.pool().tuple_elems(tuple).len(), 3);
    }

    #[test]
    fn test_check_type_success() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        let expected = Expected {
            ty: Idx::INT,
            origin: ExpectedOrigin::NoExpectation,
        };

        // Should succeed: INT matches INT
        let result = engine.check_type(Idx::INT, &expected, ori_ir::Span::DUMMY);
        assert!(result.is_ok());
        assert!(!engine.has_errors());
    }

    #[test]
    fn test_check_type_with_variable() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        let var = engine.fresh_var();
        let expected = Expected {
            ty: Idx::INT,
            origin: ExpectedOrigin::NoExpectation,
        };

        // Should succeed: variable unifies with INT
        let result = engine.check_type(var, &expected, ori_ir::Span::DUMMY);
        assert!(result.is_ok());

        // Variable should now resolve to INT
        assert_eq!(engine.resolve(var), Idx::INT);
    }

    #[test]
    fn test_check_type_failure() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        let expected = Expected {
            ty: Idx::INT,
            origin: ExpectedOrigin::NoExpectation,
        };

        // Should fail: STR doesn't match INT
        let result = engine.check_type(Idx::STR, &expected, ori_ir::Span::DUMMY);
        assert!(result.is_err());
        assert!(engine.has_errors());

        let errors = engine.errors();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0].kind, TypeErrorKind::Mismatch { .. }));
    }

    #[test]
    #[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
    fn test_let_polymorphism() {
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);

        // Simulate: let id = |x| x
        engine.enter_scope();

        // Create id: a -> a with fresh variable
        let a = engine.fresh_var();
        let id_ty = engine.infer_function(&[a], a);

        // Generalize at scope exit
        let id_scheme = engine.generalize(id_ty);

        engine.exit_scope();

        // Bind in environment
        engine.env_mut().bind_scheme(
            ori_ir::Name::from_raw(1), // "id"
            id_scheme,
        );

        // Use id with int
        let id_int = engine.instantiate(
            engine
                .env()
                .lookup_scheme(ori_ir::Name::from_raw(1))
                .expect("id should be bound"),
        );
        let params_int = engine.pool().function_params(id_int);
        assert!(engine.unify_types(params_int[0], Idx::INT).is_ok());

        // Use id with str (should work due to polymorphism)
        let id_str = engine.instantiate(
            engine
                .env()
                .lookup_scheme(ori_ir::Name::from_raw(1))
                .expect("id should be bound"),
        );
        let params_str = engine.pool().function_params(id_str);
        assert!(engine.unify_types(params_str[0], Idx::STR).is_ok());

        // Verify independence
        assert_eq!(engine.resolve(params_int[0]), Idx::INT);
        assert_eq!(engine.resolve(params_str[0]), Idx::STR);
    }
}
