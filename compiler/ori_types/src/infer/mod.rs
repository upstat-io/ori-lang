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

mod body_finalize;
mod context;
mod env;
mod expr;
mod scope;
mod state;
mod type_builders;

pub use env::TypeEnv;
pub(crate) use expr::OP_TRAIT_MAP;
pub use expr::{check_expr, infer_expr, resolve_parsed_type};

use ori_ir::{ExprId, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    check::WellKnownNames, ContextKind, FunctionSig, Idx, MonoInstanceId, PatternKey,
    PatternResolution, Pool, Tag, TraitRegistry, TypeCheckError, TypeCheckWarning, TypeRegistry,
    UnifyEngine, UnifyError,
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

    /// Accumulated type check warnings.
    warnings: Vec<TypeCheckWarning>,

    /// String interner for resolving names in error messages.
    interner: Option<&'pool StringInterner>,

    /// Pre-interned well-known type names for O(1) annotation resolution.
    well_known: Option<&'pool WellKnownNames>,

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

    /// Monomorphization instances discovered during inference.
    ///
    /// Populated by `record_mono_instance()` when a generic function is called
    /// with concrete type arguments. Extracted via `take_mono_instances()`.
    mono_instances: Vec<crate::MonoInstance>,

    /// Pre-dedup `(call_expr_id, MonoInstanceId)` pairs accumulated during
    /// inference. Each [`MonoInstanceId`] is a position into `mono_instances`
    /// at insertion time (this body's local index space); the body-pass
    /// absorbs both vectors together via
    /// [`crate::check::ModuleChecker::accumulate_mono_session`] which offsets
    /// the local indices into module-wide positions, and
    /// [`crate::check::ModuleChecker::finish_with_pool`] then remaps them
    /// once more across dedup + sort before storing in
    /// [`crate::TypedModule::mono_dispatch_map`].
    ///
    /// Populated by `record_mono_with_dispatch()` from the eager call-site
    /// path (`infer::expr::calls::monomorphization::maybe_record_mono_instance`).
    /// The deferred-resolution path (`check::exports::resolve_deferred_mono_calls`)
    /// is `bug-tracker/plans/BUG-01-002/section-05-implementation.md` §C.2
    /// sub-step 1b-deferred and does NOT populate this map; the dispatch map
    /// will be silent for transitive generic-calling-generic call sites until
    /// that sub-step lands.
    mono_dispatch_pre_dedup: Vec<(ExprId, MonoInstanceId)>,

    /// Deferred monomorphization calls (generic calling generic).
    ///
    /// Populated by `record_deferred_mono_call()` when a generic function calls
    /// another generic with type variables still unresolved. Extracted via
    /// `take_deferred_mono_calls()` and resolved in `finish_with_pool()`.
    deferred_mono_calls: Vec<crate::DeferredMonoCall>,

    /// Name of the function currently being type-checked.
    ///
    /// Used by `maybe_record_mono_instance()` to identify the caller when
    /// recording deferred mono calls.
    current_function: Option<Name>,

    /// Snapshot of name bindings that were in place at engine-creation time
    /// (typically `base_env.names()` — the module-level set of imported
    /// prelude functions, imported module bindings, and same-module function
    /// signatures from the Signatures-group pass).
    ///
    /// Used by [`InferEngine::collect_lexical_outer`] to distinguish names
    /// bound lexically within the current function body (params, local
    /// `let`s, `for`/`match` bindings) from names that are "visible but
    /// globally scoped". Required for `should_generalize`'s Value Restriction
    /// capture check to treat `let f = x -> len(xs: x)` (prelude-qualified
    /// body) as non-capturing, while still treating
    /// `let outer = 1; let f = x -> outer` (lexically-captured body) as
    /// capturing.
    ///
    /// `None` means no snapshot was provided — in that case every env name
    /// is conservatively treated as lexical (the pre-snapshot behavior).
    module_scope_snapshot: Option<FxHashSet<Name>>,

    /// Var ids that were generalized by `generalize()` during body
    /// inference. Tracked so the end-of-body normalization pass
    /// ([`InferEngine::normalize_body_generalized_to_bound_var`]) can rewrite
    /// matching `Tag::Var` leaves in `expr_types` / `FunctionSig` positions
    /// to `Tag::BoundVar` per `types.md §SC-1`.
    ///
    /// Drained per-body: between bodies, the set must be empty. Accumulates
    /// via [`InferEngine::record_generalized_vars`] from the `generalize()`
    /// call path (inner let-polymorphism); top-level polymorphic function
    /// signatures' scheme var ids are passed directly to the normalization
    /// method by the body-pass caller.
    pending_generalized_vars: Vec<u32>,
}

impl<'pool> InferEngine<'pool> {
    /// Create a new inference engine.
    pub fn new(pool: &'pool mut Pool) -> Self {
        Self::build(pool, TypeEnv::new())
    }

    /// Create a new inference engine with an existing environment.
    ///
    /// Use this when you need to share type bindings across inference sessions.
    pub fn with_env(pool: &'pool mut Pool, env: TypeEnv) -> Self {
        Self::build(pool, env)
    }

    /// SSOT constructor: builds an `InferEngine` with all-default inner state
    /// and the supplied type environment. Both [`Self::new`] and
    /// [`Self::with_env`] delegate here so adding a new `InferEngine` field
    /// requires exactly one edit.
    fn build(pool: &'pool mut Pool, env: TypeEnv) -> Self {
        Self {
            unify: UnifyEngine::new(pool),
            env,
            expr_types: FxHashMap::default(),
            context_stack: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            interner: None,
            well_known: None,
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
            mono_instances: Vec::new(),
            mono_dispatch_pre_dedup: Vec::new(),
            deferred_mono_calls: Vec::new(),
            current_function: None,
            module_scope_snapshot: None,
            pending_generalized_vars: Vec::new(),
        }
    }

    /// Set the string interner for resolving names in error messages.
    pub fn set_interner(&mut self, interner: &'pool StringInterner) {
        self.interner = Some(interner);
    }

    /// Record the set of names that are already in scope at engine-creation
    /// time (i.e., names present in `base_env` before any function-body
    /// `let`/`for`/`match` binder has executed). Used by
    /// [`Self::collect_lexical_outer`] to filter `env().names()` down to
    /// the lexically-introduced subset.
    pub(crate) fn set_module_scope_snapshot(&mut self, names: FxHashSet<Name>) {
        self.module_scope_snapshot = Some(names);
    }

    /// Collect the lexically-bound outer-scope names visible from the
    /// current inference frame, excluding module-level references (prelude
    /// free functions, imports, same-module function signatures).
    ///
    /// Used by `should_generalize` (Value Restriction per `typeck.md §GN-3`)
    /// to ensure a lambda body that references a prelude free function
    /// (e.g., `len(collection: xs)`) is NOT mis-classified as capturing.
    /// The classifier receives only names the user lexically bound inside
    /// the enclosing function — params, prior `let` bindings, loop/match
    /// pattern names — which is the semantically correct domain for
    /// Value Restriction.
    ///
    /// When no snapshot was recorded (engine constructed without going
    /// through `create_engine_with_env` — e.g., unit tests), every visible
    /// name is conservatively returned; this preserves the pre-snapshot
    /// behavior of test harnesses that build lambdas directly.
    pub fn collect_lexical_outer(&self) -> FxHashSet<Name> {
        let all: FxHashSet<Name> = self.env.names().collect();
        match &self.module_scope_snapshot {
            Some(base) => all.difference(base).copied().collect(),
            None => all,
        }
    }

    /// Set the well-known names cache for O(1) type annotation resolution.
    pub(crate) fn set_well_known(&mut self, wk: &'pool WellKnownNames) {
        self.well_known = Some(wk);
    }

    /// Get the well-known names cache.
    ///
    /// Returns with the `'pool` lifetime so the result can be used while
    /// mutably borrowing the engine for pool operations.
    pub(crate) fn well_known(&self) -> Option<&'pool WellKnownNames> {
        self.well_known
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
    ///
    /// The returned `&str` has the interner's lifetime (`'pool`), not the engine
    /// borrow lifetime. This allows holding the result while mutably borrowing
    /// the engine for other operations.
    pub fn lookup_name(&self, name: Name) -> Option<&'pool str> {
        self.interner.map(|i| i.lookup(name))
    }

    /// Intern a string into a `Name`, if the interner is available.
    pub fn intern_name(&self, s: &str) -> Option<Name> {
        self.interner.map(|i| i.intern(s))
    }

    // Pool Access

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

    // Environment Access

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

    // Type Variable Creation

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

    // Resolution & Unification

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

    // Generalization & Instantiation

    /// Generalize a type at the current scope.
    ///
    /// Returns a type scheme if any variables were generalized,
    /// or the original type if it's monomorphic.
    ///
    /// §08.3b.1 — Records every newly-generalized var id in
    /// `pending_generalized_vars` so the end-of-body normalization pass
    /// ([`InferEngine::normalize_body_generalized_to_bound_var`]) can rewrite
    /// matching `Tag::Var` leaves in `expr_types` / `FunctionSig` positions
    /// to `Tag::BoundVar` per `types.md §SC-1`.
    pub fn generalize(&mut self, ty: Idx) -> Idx {
        let scheme = self.unify.generalize(ty);
        // If generalize returned a scheme, extract its bound var ids and
        // record them for the end-of-body normalization pass. If no
        // generalization happened (monomorphic), the returned idx is not
        // a scheme — skip recording.
        if self.unify.pool().tag(scheme) == Tag::Scheme {
            let vars = self.unify.pool().scheme_vars(scheme).to_vec();
            self.pending_generalized_vars.extend(vars);
        }
        scheme
    }

    /// Instantiate a type scheme with fresh variables.
    ///
    /// Returns the type unchanged if it's not a scheme.
    #[inline]
    pub fn instantiate(&mut self, scheme: Idx) -> Idx {
        self.unify.instantiate(scheme)
    }

    /// Instantiate a scheme and expose the `scheme_var_id → fresh_var_idx` map.
    ///
    /// Used by the impl-method call path to enforce method-level inline bounds
    /// (`<T: Bound>`): the substitution map lets the bound checker find the
    /// post-instantiation Var Idx for each method-level binder by its
    /// registration-time `var_id` (recorded in
    /// `ImplMethodDef.scheme_var_ids`).
    #[inline]
    pub fn instantiate_with_subst(
        &mut self,
        scheme: Idx,
    ) -> (Idx, rustc_hash::FxHashMap<u32, Idx>) {
        self.unify.instantiate_with_subst(scheme)
    }
}

#[cfg(test)]
mod tests;
