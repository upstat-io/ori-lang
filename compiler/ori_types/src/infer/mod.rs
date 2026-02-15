//! Type inference engine.
//!
//! This module provides the main orchestrator for Hindley-Milner type inference,
//! connecting the Pool, `UnifyEngine`, and error system into a unified inference API.
//!
//! # Architecture
//!
//! `InferEngine` wraps `UnifyEngine` and adds:
//! - Expression type storage (`expr_types`)
//! - Type environment management (`TypeEnv`)
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
//! The engine uses:
//! - `Idx` as the canonical type handle (not `Type` or `TypeId`)
//! - `UnifyEngine` for O(α(n)) unification
//! - `Pool` for O(1) type equality
//! - Rich error context for helpful diagnostic messages

mod env;
mod expr;

pub use env::TypeEnv;
pub use expr::{check_expr, infer_expr, resolve_parsed_type};

use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use ori_diagnostic::Suggestion;

use crate::{
    diff_types, ContextKind, ErrorContext, Expected, FunctionSig, Idx, PatternKey,
    PatternResolution, Pool, TraitRegistry, TypeCheckError, TypeErrorKind, TypeProblem,
    TypeRegistry, UnifyEngine, UnifyError,
};

/// Expression ID type (mirrors `ori_ir::ExprId`).
///
/// Using a simple usize to avoid dependency on `ori_ir` for the core types module.
/// Maps to `ori_ir::ExprId` when integrating with the module checker.
pub type ExprIndex = usize;

/// The type inference engine.
///
/// Orchestrates Hindley-Milner type inference:
/// - `Pool` for type storage and interning
/// - `UnifyEngine` for unification with path compression
/// - `TypeEnv` for name bindings
/// - Error accumulation for comprehensive diagnostics
///
/// # Component Structure
///
/// ```text
/// InferEngine
/// ├── UnifyEngine (unification, resolution, generalization)
/// │   └── Pool (type storage, interning, flags)
/// ├── TypeEnv (name → type scheme bindings)
/// ├── expr_types (expression → inferred type)
/// ├── context_stack (error context tracking)
/// └── errors (accumulated type errors)
/// ```
pub struct InferEngine<'pool> {
    /// The unification engine (owns mutable pool access).
    unify: UnifyEngine<'pool>,

    /// Type environment for name bindings.
    env: TypeEnv,

    /// Inferred types for expressions (expr index → type).
    expr_types: FxHashMap<ExprIndex, Idx>,

    /// Context stack for error reporting.
    context_stack: Vec<ContextKind>,

    /// Accumulated type check errors.
    errors: Vec<TypeCheckError>,

    /// String interner for resolving names in error messages.
    interner: Option<&'pool StringInterner>,

    /// Trait registry for where-clause validation at call sites.
    trait_registry: Option<&'pool TraitRegistry>,

    /// Function signatures for where-clause lookup.
    signatures: Option<&'pool FxHashMap<Name, FunctionSig>>,

    /// Type registry for struct/enum/newtype lookup during inference.
    type_registry: Option<&'pool TypeRegistry>,

    /// Current function type for `self` references (recursive calls in patterns).
    self_type: Option<Idx>,

    /// Current impl's `Self` type (for `Self` in type annotations within impl blocks).
    impl_self_type: Option<Idx>,

    /// Stack of expected break value types for nested loops.
    /// Each `loop()` pushes a fresh type variable; `break expr` unifies with it.
    loop_break_types: Vec<Idx>,

    /// Capabilities declared by the current function (`uses` clause).
    current_capabilities: FxHashSet<Name>,

    /// Capabilities provided in scope (`with...in`).
    provided_capabilities: FxHashSet<Name>,

    /// Pattern resolutions accumulated during match checking.
    ///
    /// Records `Binding` patterns that were resolved to unit variants.
    /// Extracted via `take_pattern_resolutions()` after checking.
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    /// Module-level constant types for `$name` reference resolution.
    const_types: Option<&'pool FxHashMap<Name, Idx>>,
}

impl<'pool> InferEngine<'pool> {
    /// Create a new inference engine.
    pub fn new(pool: &'pool mut Pool) -> Self {
        Self {
            unify: UnifyEngine::new(pool),
            env: TypeEnv::new(),
            expr_types: FxHashMap::default(),
            context_stack: Vec::new(),
            errors: Vec::new(),
            interner: None,
            trait_registry: None,
            signatures: None,
            type_registry: None,
            self_type: None,
            impl_self_type: None,
            loop_break_types: Vec::new(),
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            pattern_resolutions: Vec::new(),
            const_types: None,
        }
    }

    /// Create a new inference engine with an existing environment.
    ///
    /// Use this when you need to share type bindings across inference sessions.
    pub fn with_env(pool: &'pool mut Pool, env: TypeEnv) -> Self {
        Self {
            unify: UnifyEngine::new(pool),
            env,
            expr_types: FxHashMap::default(),
            context_stack: Vec::new(),
            errors: Vec::new(),
            interner: None,
            trait_registry: None,
            signatures: None,
            type_registry: None,
            self_type: None,
            impl_self_type: None,
            loop_break_types: Vec::new(),
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            pattern_resolutions: Vec::new(),
            const_types: None,
        }
    }

    /// Set the string interner for resolving names in error messages.
    pub fn set_interner(&mut self, interner: &'pool StringInterner) {
        self.interner = Some(interner);
    }

    /// Set the trait registry for where-clause validation.
    pub fn set_trait_registry(&mut self, registry: &'pool TraitRegistry) {
        self.trait_registry = Some(registry);
    }

    /// Set function signatures for where-clause lookup.
    pub fn set_signatures(&mut self, sigs: &'pool FxHashMap<Name, FunctionSig>) {
        self.signatures = Some(sigs);
    }

    /// Set the type registry for struct/enum/newtype lookup.
    pub fn set_type_registry(&mut self, registry: &'pool TypeRegistry) {
        self.type_registry = Some(registry);
    }

    /// Set module-level constant types for `$name` reference resolution.
    pub fn set_const_types(&mut self, consts: &'pool FxHashMap<Name, Idx>) {
        self.const_types = Some(consts);
    }

    /// Look up a constant's type by name.
    pub fn const_type(&self, name: Name) -> Option<Idx> {
        self.const_types.and_then(|m| m.get(&name).copied())
    }

    /// Set the current function type for `self` references.
    pub fn set_self_type(&mut self, ty: Idx) {
        self.self_type = Some(ty);
    }

    /// Get the current function type for `self` references.
    pub fn self_type(&self) -> Option<Idx> {
        self.self_type
    }

    /// Set the current impl's `Self` type for type annotation resolution.
    pub fn set_impl_self_type(&mut self, ty: Idx) {
        self.impl_self_type = Some(ty);
    }

    /// Get the current impl's `Self` type.
    pub fn impl_self_type(&self) -> Option<Idx> {
        self.impl_self_type
    }

    /// Push a loop break type variable onto the stack.
    /// Called when entering a `loop()` expression.
    pub fn push_loop_break_type(&mut self, ty: Idx) {
        self.loop_break_types.push(ty);
    }

    /// Pop the loop break type variable.
    /// Called when exiting a `loop()` expression.
    pub fn pop_loop_break_type(&mut self) -> Option<Idx> {
        self.loop_break_types.pop()
    }

    /// Get the current loop's break type variable (innermost loop).
    pub fn current_loop_break_type(&self) -> Option<Idx> {
        self.loop_break_types.last().copied()
    }

    // ========================================
    // Capability Management
    // ========================================

    /// Set capabilities for the current function scope.
    ///
    /// `current` contains capabilities declared via `uses` on the function.
    /// `provided` contains capabilities introduced via `with...in`.
    pub fn set_capabilities(&mut self, current: FxHashSet<Name>, provided: FxHashSet<Name>) {
        self.current_capabilities = current;
        self.provided_capabilities = provided;
    }

    /// Check if a capability is available (declared or provided).
    pub fn has_capability(&self, cap: Name) -> bool {
        self.current_capabilities.contains(&cap) || self.provided_capabilities.contains(&cap)
    }

    /// Get all available capabilities (declared + provided).
    pub fn available_capabilities(&self) -> Vec<Name> {
        self.current_capabilities
            .union(&self.provided_capabilities)
            .copied()
            .collect()
    }

    /// Add a provided capability (for `with...in` scoping).
    pub fn add_provided_capability(&mut self, cap: Name) {
        self.provided_capabilities.insert(cap);
    }

    /// Remove a provided capability.
    pub fn remove_provided_capability(&mut self, cap: Name) {
        self.provided_capabilities.remove(&cap);
    }

    /// Execute a closure with a temporarily provided capability.
    ///
    /// The capability is added before executing `f` and removed after.
    /// This implements the scoped semantics of `with...in`.
    pub fn with_provided_capability<T, F>(&mut self, cap: Name, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let was_present = self.provided_capabilities.insert(cap);
        let result = f(self);
        if !was_present {
            self.provided_capabilities.remove(&cap);
        }
        result
    }

    /// Get the trait registry (if set).
    pub fn trait_registry(&self) -> Option<&TraitRegistry> {
        self.trait_registry
    }

    /// Get the type registry (if set).
    pub fn type_registry(&self) -> Option<&TypeRegistry> {
        self.type_registry
    }

    /// Look up a function signature by name.
    pub fn get_signature(&self, name: Name) -> Option<&FunctionSig> {
        self.signatures.and_then(|s| s.get(&name))
    }

    /// Resolve a `Name` to its string representation, if the interner is available.
    pub fn lookup_name(&self, name: Name) -> Option<&str> {
        self.interner.map(|i| i.lookup(name))
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

    /// Get read-only access to the unification engine.
    #[inline]
    pub fn unify_ref(&self) -> &UnifyEngine<'pool> {
        &self.unify
    }

    // ========================================
    // Environment Access
    // ========================================

    /// Get the type environment.
    #[inline]
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Get mutable access to the type environment.
    #[inline]
    pub fn env_mut(&mut self) -> &mut TypeEnv {
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

    /// Enter a rank scope only (for let-polymorphism).
    ///
    /// This only increases the unification rank, without creating
    /// a child environment scope. Use this within blocks where
    /// bindings should remain visible to subsequent statements.
    #[inline]
    pub fn enter_rank_scope(&mut self) {
        self.unify.enter_scope();
    }

    /// Exit a rank scope only.
    ///
    /// Call `generalize()` on relevant types BEFORE exiting.
    #[inline]
    pub fn exit_rank_scope(&mut self) {
        self.unify.exit_scope();
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
        tracing::debug!(kind = ?error.kind, "type error recorded");
        self.errors.push(error);
    }

    /// Get the current error count (for detecting new errors after a section).
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    // ========================================
    // Pattern Resolution
    // ========================================

    /// Record that a `Binding` pattern was resolved to a unit variant.
    pub fn record_pattern_resolution(&mut self, key: PatternKey, res: PatternResolution) {
        self.pattern_resolutions.push((key, res));
    }

    /// Take pattern resolutions, leaving an empty vector.
    pub fn take_pattern_resolutions(&mut self) -> Vec<(PatternKey, PatternResolution)> {
        std::mem::take(&mut self.pattern_resolutions)
    }

    /// Rewrite `UnknownIdent` errors matching `name` (added since `errors_before`)
    /// into `ClosureSelfCapture` errors.
    ///
    /// This detects patterns like `let f = () -> f` where a closure body
    /// references its own binding name.
    pub fn rewrite_self_capture_errors(&mut self, binding_name: Name, errors_before: usize) {
        for error in &mut self.errors[errors_before..] {
            if let TypeErrorKind::UnknownIdent { name, .. } = &error.kind {
                if *name == binding_name {
                    *error = TypeCheckError::closure_self_capture(error.span);
                }
            }
        }
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
mod tests;
