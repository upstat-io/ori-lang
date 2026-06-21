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
pub(crate) use expr::register_concrete_applied_resolutions;
pub(crate) use expr::{NestedPathStep, RefutableReason};
mod scope;
pub(crate) use scope::{LoopContext, LoopForm};
mod accessors;
mod state;
mod type_builders;

pub use env::TypeEnv;
pub use expr::{check_expr, infer_expr, resolve_parsed_type};
pub(crate) use expr::{tag_to_type_tag, OP_TRAIT_MAP};

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
/// |-- UnifyEngine (unification, resolution, generalization)
/// |   `-- Pool (type storage, interning, flags)
/// |-- TypeEnv (name -> type scheme bindings)
/// |-- expr_types (expression -> inferred type)
/// |-- context_stack (error context tracking)
/// `-- errors (accumulated type errors)
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

    /// Builtin-type extension methods (`extend str { @m }`, `extend [T] { @m }`)
    /// keyed by builtin `TypeTag`. Populated from `module.extends` at body-check
    /// entry. Consulted by `emit_unknown_method` so an `extend`-provided method on
    /// a builtin receiver is not false-rejected as unknown (TR-9 dispatch stays
    /// target-only; the evaluator resolves the actual call).
    builtin_extensions: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>>,

    /// Current function type for `self` references (recursive calls in patterns).
    self_type: Option<Idx>,

    /// Current impl's `Self` type (for `Self` in type annotations within impl blocks).
    impl_self_type: Option<Idx>,

    /// Stack of enclosing-loop contexts for nested loops.
    /// Each loop form pushes a [`LoopContext`] recording the expected break
    /// value type plus whether `break value` / `continue value` are permitted
    /// (per spec: `loop` and `for...yield` carry values; `while` / `for...do`
    /// do not). `break` / `continue` consult the innermost entry.
    loop_contexts: Vec<scope::LoopContext>,

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

    /// Method-level const-generic parameter types for `$name` reference
    /// resolution inside an impl/def-impl method body. Populated at body-check
    /// entry so that bodies can use `$N` in type-arg
    /// positions (e.g., `to_fixed<$N>()`). Owned (not borrowed) because the
    /// scope is per-method, not module-level. Consulted by `const_type` AFTER
    /// the module-level lookup misses.
    method_const_types: FxHashMap<Name, Idx>,

    /// Method-level `RigidVar` trait-bound assumptions for body-internal trait
    /// dispatch. Populated at impl/def-impl body-check
    /// entry from inline `<T: Bound>` and trailing `where T: Bound` forms so
    /// that body-internal trait queries on a method-level `RigidVar` (e.g.,
    /// `prefix.to_str()` in `@to_string<T: Printable> (prefix: T) -> str`)
    /// succeed when the trait is in the binder's declared bounds.
    ///
    /// Keyed by the `RigidVar`'s `var_id` (extractable via `pool.data(idx)`);
    /// value is the `Vec` of trait `Name`s assumed for that binder. Owned (not
    /// borrowed) because the scope is per-method, not module-level.
    /// Consulted by `rigid_var_satisfies_bound`.
    method_rigid_bounds: FxHashMap<u32, Vec<Name>>,

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
    /// is the sub-step 1b-deferred resolution and does NOT populate this map;
    /// the dispatch map
    /// will be silent for transitive generic-calling-generic call sites until
    /// that sub-step lands.
    mono_dispatch_pre_dedup: Vec<(ExprId, MonoInstanceId)>,

    /// Type-directed desugar plans for `ExprKind::AssignTarget` chains.
    ///
    /// Populated by `record_assign_desugar()` during `infer_assign_target`
    /// (one entry per index/field assignment). Each entry pairs the
    /// `AssignTarget` node's AST `ExprId` with its resolved per-level types.
    /// Extracted via `take_assign_desugars()` and forwarded to
    /// [`crate::TypedModule::assign_desugar_map`]. `ori_canon` consumes the
    /// map to synthesize the pure-reassignment form so AIMS never sees a
    /// `CanExpr::Assign { target: Index/Field }`. Keys are module-wide AST
    /// `ExprId`s; no body-local re-anchoring needed.
    assign_desugars: Vec<(ExprId, crate::AssignDesugar)>,

    /// Deferred monomorphization calls (generic calling generic).
    ///
    /// Populated by `record_deferred_mono_call()` when a generic function calls
    /// another generic with type variables still unresolved. Extracted via
    /// `take_deferred_mono_calls()` and resolved in `finish_with_pool()`.
    deferred_mono_calls: Vec<crate::DeferredMonoCall>,

    /// Set to `true` at the end of body inference, immediately before the
    /// end-of-body defaulting pre-pass runs. `default_unbound_vars_in_scope`
    /// and `default_unbound_vars_from_empty_literals` debug-assert this flag
    /// so a pass-order inversion (defaulting called before body inference
    /// finished) panics in debug builds instead of silently defaulting vars
    /// that bidirectional propagation should have pinned.
    body_inference_complete: bool,

    /// Composed `UserBurdenSpec` entries discovered during monomorphization.
    ///
    /// Populated by
    /// `infer::expr::calls::monomorphization::maybe_record_mono_instance`
    /// when a generic builtin (Option<T>, Result<T, E>, [T], {K: V}, Set<T>)
    /// reaches its first-instantiation form with fully-resolved type args.
    /// Each entry is `(monomorphized_idx, composed_spec)`; consumed downstream
    /// by the body-pass extractor (`take_composed_burdens`) and flushed into
    /// `TypeRegistry::burden` once the mutable-registry surface lands.
    ///
    /// Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding
    /// the lattice-driven analysis. Composition at type-instantiation time
    /// (this accumulator) prevents Phase 5 from emitting indirect dispatch on
    /// each burden walk (proposal §Generic Burden Composition rationale).
    composed_burdens: Vec<(Idx, crate::registry::burden::UserBurdenSpec)>,

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
    /// to `Tag::BoundVar` per.
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
            builtin_extensions: FxHashMap::default(),
            self_type: None,
            impl_self_type: None,
            loop_contexts: Vec::new(),
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            pattern_resolutions: Vec::new(),
            const_types: None,
            method_const_types: FxHashMap::default(),
            method_rigid_bounds: FxHashMap::default(),
            mono_instances: Vec::new(),
            mono_dispatch_pre_dedup: Vec::new(),
            assign_desugars: Vec::new(),
            deferred_mono_calls: Vec::new(),
            composed_burdens: Vec::new(),
            current_function: None,
            module_scope_snapshot: None,
            pending_generalized_vars: Vec::new(),
            body_inference_complete: false,
        }
    }

    /// Compose + register the `UserBurdenSpec` for every generic-builtin type
    /// that appears in the body's `expr_types`, keyed by the EXACT `Idx` the
    /// typed IR (and thus ARC) carries.
    ///
    /// Runs at end-of-body, before the burden sweep in `take_composed_burdens`.
    /// A late-resolved generic-builtin instantiation (inline `Ok([..])` typed
    /// `Result<[int], Var(k)→int>`) is stored as a `HAS_VAR` Idx; the pool-scan
    /// sweep `collect_candidate_indices` excludes `HAS_VAR` Idx, so its burden
    /// is never composed and the RL-2 scope-exit dec is dropped (Spec: Annex E
    /// §AIMS). `compose_for_idx` composes from the Idx's structure (the heap-
    /// carrying `Ok` variant yields a non-empty burden regardless of the still-
    /// linked `Err` slot) and records it under the exact Idx ARC will query —
    /// without mutating the IR types. The composed-burden accumulator dedups
    /// against the pool-scan sweep, so running both is idempotent.
    pub fn compose_body_type_burdens<K>(&mut self, expr_types: &rustc_hash::FxHashMap<K, Idx>) {
        let idxs: Vec<Idx> = expr_types.values().copied().collect();
        for idx in idxs {
            crate::infer::expr::compose_for_idx(self, idx);
        }
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
    /// Records every newly-generalized var id in
    /// `pending_generalized_vars` so the end-of-body normalization pass
    /// ([`InferEngine::normalize_body_generalized_to_bound_var`]) can rewrite
    /// matching `Tag::Var` leaves in `expr_types` / `FunctionSig` positions
    /// to `Tag::BoundVar` (SC-1 scheme bound-var layout).
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
