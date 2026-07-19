//! Construction and core public operations for [`InferEngine`].

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{Idx, Pool, Tag, UnifyEngine, UnifyError};

use super::{InferEngine, TypeEnv};

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
            module_aliases: None,
            type_registry: None,
            builtin_extensions: FxHashMap::default(),
            self_type: None,
            impl_self_type: None,
            loop_contexts: Vec::new(),
            try_boundaries: Vec::new(),
            current_capabilities: FxHashSet::default(),
            provided_capabilities: FxHashSet::default(),
            capability_providers: FxHashMap::default(),
            capability_call_sites: Vec::new(),
            pattern_resolutions: Vec::new(),
            const_types: None,
            const_values: None,
            const_param_types: FxHashMap::default(),
            method_rigid_bounds: FxHashMap::default(),
            mono_instances: Vec::new(),
            mono_dispatch_pre_dedup: Vec::new(),
            index_dispatch_selections: Vec::new(),
            assign_desugars: Vec::new(),
            module_alias_calls: Vec::new(),
            iter_route_desugars: Vec::new(),
            deferred_mono_calls: Vec::new(),
            composed_burdens: Vec::new(),
            deferred_mono_caller: None,
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
