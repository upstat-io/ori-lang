//! `InferEngine` state accessors — registry/context setters, getters, and
//! method-level rigid-bound + const helpers.

use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use super::InferEngine;
use crate::{check::WellKnownNames, FunctionSig, Idx, Pool, Tag, TraitRegistry, TypeRegistry};

impl<'pool> InferEngine<'pool> {
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
    /// Used by `should_generalize`
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

    /// Install the builtin-type extension-method index (from `module.extends`).
    pub fn set_builtin_extensions(
        &mut self,
        ext: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>>,
    ) {
        self.builtin_extensions = ext;
    }

    /// True iff an `extend <builtin> { @method }` provides `method` on the builtin
    /// `type_tag` receiver. Lets `emit_unknown_method` avoid false-rejecting an
    /// extension-provided method (the evaluator owns the actual dispatch).
    pub fn builtin_extension_provides(
        &self,
        type_tag: ori_registry::TypeTag,
        method: Name,
    ) -> bool {
        self.builtin_extensions
            .get(&type_tag)
            .is_some_and(|methods| methods.contains(&method))
    }

    /// Set module-level constant types for `$name` reference resolution.
    pub fn set_const_types(&mut self, consts: &'pool FxHashMap<Name, Idx>) {
        self.const_types = Some(consts);
    }

    /// Mark body inference complete. Called at the end of body inference in
    /// each of the four body-check call paths (`check_function`, `check_test`,
    /// `check_impl_method`, `check_def_impl_method`) immediately before the
    /// end-of-body defaulting pre-pass runs. Defaulting helpers
    /// (`default_unbound_vars_from_empty_literals`, `default_unbound_vars_in_scope`)
    /// debug-assert this flag so a pass-order inversion panics in debug builds.
    pub fn mark_body_inference_complete(&mut self) {
        self.body_inference_complete = true;
    }

    /// Look up a constant's type by name.
    ///
    /// Checks module-level constants first, then method-level const generics
    /// (Phase B-Residual-2 (a)). Method-level const generics shadow module-
    /// level constants of the same name only inside their owning method body.
    pub fn const_type(&self, name: Name) -> Option<Idx> {
        if let Some(ty) = self.const_types.and_then(|m| m.get(&name).copied()) {
            return Some(ty);
        }
        self.method_const_types.get(&name).copied()
    }

    /// Bind a method-level const generic in scope for the body being checked.
    ///
    /// Called by `check_impl_method` / `check_def_impl_method` at body-check
    /// entry, mirroring the `param_env.bind` for identifier-position lookups.
    /// This binding makes `$name` references inside the body's type-arg
    /// positions (e.g., `to_fixed<$N>()`) resolve correctly.
    pub fn bind_method_const(&mut self, name: Name, ty: Idx) {
        self.method_const_types.insert(name, ty);
    }

    /// Register a trait-bound assumption on a method-level `RigidVar`.
    ///
    /// Called by `check_impl_method` / `check_def_impl_method` at body-check
    /// entry for each declared bound on a method-level type generic. Multiple
    /// calls for the same `RigidVar` accumulate (`<T: Eq + Clone>` registers
    /// both `Eq` and `Clone` independently). Body-internal trait dispatch
    /// queries (e.g., string-interpolation `Printable` check, map-key
    /// `Hashable` check) consult these assumptions via
    /// `rigid_var_satisfies_bound`.
    pub fn bind_method_rigid_bound(&mut self, rigid_idx: Idx, trait_name: Name) {
        let var_id = self.pool().data(rigid_idx);
        self.method_rigid_bounds
            .entry(var_id)
            .or_default()
            .push(trait_name);
    }

    /// Check whether a method-level `RigidVar` satisfies a trait via its
    /// declared bounds.
    ///
    /// Returns `false` for any non-`RigidVar` `Idx` (callers handle non-rigid
    /// types through the standard `type_satisfies_trait` /
    /// `TraitRegistry::has_impl` paths). Returns `true` iff `ty` is a
    /// `RigidVar` AND the declared bounds set for that `RigidVar` contains
    /// `trait_name`.
    ///
    /// Supertrait transitivity is NOT applied here — a binder declared
    /// `T: Hashable` does not implicitly satisfy `Eq` via Hashable's
    /// supertrait chain at this query layer. Callers requiring supertrait
    /// transitivity should expand the bound list at registration time.
    pub fn rigid_var_satisfies_bound(&self, ty: Idx, trait_name: Name) -> bool {
        if self.pool().tag(ty) != Tag::RigidVar {
            return false;
        }
        let var_id = self.pool().data(ty);
        self.method_rigid_bounds
            .get(&var_id)
            .is_some_and(|bounds| bounds.contains(&trait_name))
    }

    /// Get the full list of trait-bound names declared on a generic
    /// type-parameter variable.
    ///
    /// Accepts both `Tag::RigidVar` (impl/def-impl method-level binders
    /// created via `pool.rigid_var`) and `Tag::Var` (top-level function
    /// type-param binders created via `pool.fresh_named_var` per
    /// `check/signatures/mod.rs`). Returns `None` for non-variable
    /// types or for variables with no registered bounds. Consumed by
    /// bound-chain method dispatch.
    pub fn rigid_var_bounds(&self, ty: Idx) -> Option<&[Name]> {
        let tag = self.pool().tag(ty);
        if tag != Tag::RigidVar && tag != Tag::Var {
            return None;
        }
        let var_id = self.pool().data(ty);
        self.method_rigid_bounds.get(&var_id).map(Vec::as_slice)
    }

    /// Register a rigid-var trait bound directly by `var_id`.
    ///
    /// Variant of `bind_method_rigid_bound` that takes the `var_id` directly
    /// rather than extracting it from a `Tag::RigidVar` `Idx`. Used by
    /// `check_function` to register bounds from `FunctionSig.scheme_var_ids`
    /// and `type_param_bounds` (parallel arrays) without needing to look up
    /// the rigid `Idx` for each var. Top-level function bounds come from
    /// the signature, not from per-method rebinding like impl methods.
    pub fn bind_rigid_bound_by_var_id(&mut self, var_id: u32, trait_name: Name) {
        self.method_rigid_bounds
            .entry(var_id)
            .or_default()
            .push(trait_name);
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
}
