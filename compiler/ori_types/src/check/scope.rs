//! Scope and context management for `ModuleChecker`.
//!
//! Handles scope context (current function, impl self type, capabilities),
//! environment management (freezing/child envs), error accumulation,
//! expression type storage, inference engine creation, and closure-based
//! save/restore context scoping (NOT RAII — no `Drop`, not panic-safe).

use ori_ir::{Name, Span};
use rustc_hash::FxHashSet;

use super::ModuleChecker;
use crate::{Idx, InferEngine, TypeCheckError, TypeCheckWarning, TypeEnv};

impl ModuleChecker<'_> {
    // Scope Context

    /// Get the current function type (for `recurse`).
    #[inline]
    pub fn current_function(&self) -> Option<Idx> {
        self.current_function
    }

    /// Get the current impl self type.
    #[inline]
    pub fn current_impl_self(&self) -> Option<Idx> {
        self.current_impl_self
    }

    /// The current impl's associated-type projection context: the in-scope
    /// `assoc_name -> Idx` bindings plus the impl `trait_idx`. `None` outside an
    /// impl method-signature resolution. Read by the `ParsedType::AssociatedType`
    /// resolver arm to project `Self.Item` for the impl currently being resolved.
    #[inline]
    pub(crate) fn current_impl_assoc(
        &self,
    ) -> Option<&(rustc_hash::FxHashMap<Name, Idx>, Option<Idx>)> {
        self.current_impl_assoc.as_ref()
    }

    /// Run `f` with the impl's associated-type projection context installed,
    /// restoring the prior context on return. Mirrors `with_impl_scope`:
    /// not panic-safe (an unwinding panic inside `f` does not restore), matching
    /// the closure-based save/restore discipline.
    pub(crate) fn with_impl_assoc_scope<T, F>(
        &mut self,
        bindings: rustc_hash::FxHashMap<Name, Idx>,
        trait_idx: Option<Idx>,
        f: F,
    ) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved = self.current_impl_assoc.replace((bindings, trait_idx));
        let result = f(self);
        self.current_impl_assoc = saved;
        result
    }

    /// Check if a capability is available (declared or provided).
    pub fn has_capability(&self, cap: Name) -> bool {
        self.current_capabilities.contains(&cap) || self.provided_capabilities.contains(&cap)
    }

    /// Get the type of a constant.
    pub fn const_type(&self, name: Name) -> Option<Idx> {
        self.const_types.get(&name).copied()
    }

    /// Register a constant type.
    pub fn register_const_type(&mut self, name: Name, ty: Idx) {
        self.const_types.insert(name, ty);
    }

    // Environment Management

    /// Freeze the current environment as the base.
    ///
    /// Called after signature collection to preserve function bindings.
    /// Function body checking creates child environments from this base.
    pub fn freeze_base_env(&mut self, env: TypeEnv) {
        self.base_env = Some(env);
    }

    /// Get a child of the frozen base environment.
    ///
    /// Returns `None` if the base hasn't been frozen yet.
    pub fn child_of_base(&self) -> Option<TypeEnv> {
        self.base_env.as_ref().map(TypeEnv::child)
    }

    /// Get the frozen base environment.
    pub fn base_env(&self) -> Option<&TypeEnv> {
        self.base_env.as_ref()
    }

    // Error Management

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

    /// Push a type check error.
    pub fn push_error(&mut self, error: TypeCheckError) {
        self.errors.push(error);
    }

    /// Push a type check warning.
    pub fn push_warning(&mut self, warning: TypeCheckWarning) {
        self.warnings.push(warning);
    }

    /// Report an undefined identifier error.
    pub fn error_undefined(&mut self, name: Name, span: Span) {
        self.errors
            .push(TypeCheckError::undefined_identifier(name, span));
    }

    // Expression Types

    /// Store the inferred type for an expression.
    ///
    /// Expression indices are assumed to be sequential starting from 0.
    /// If the index exceeds current capacity, the vector is extended.
    pub fn store_expr_type(&mut self, expr_index: usize, ty: Idx) {
        if expr_index >= self.expr_types.len() {
            self.expr_types.resize(expr_index + 1, Idx::ERROR);
        }
        self.expr_types[expr_index] = ty;
    }

    /// Get the inferred type for an expression.
    pub fn get_expr_type(&self, expr_index: usize) -> Option<Idx> {
        self.expr_types.get(expr_index).copied()
    }

    // Inference Engine Creation

    /// Create an inference engine for checking a scope.
    ///
    /// The engine borrows the pool mutably and starts with a fresh environment.
    /// Propagates capability state so the engine can validate call-site capabilities.
    pub fn create_engine(&mut self) -> InferEngine<'_> {
        let interner = self.interner;
        let well_known = &self.well_known;
        // Split borrow: pool (mut) + traits, signatures, types, consts (shared)
        let traits = &self.traits;
        let sigs = &self.signatures;
        let aliases = &self.module_aliases;
        let types = &self.types;
        let consts = &self.const_types;
        let impl_self = self.current_impl_self;
        let current_caps = self.current_capabilities.clone();
        let provided_caps = self.provided_capabilities.clone();
        let builtin_exts = self.builtin_extensions.clone();
        let mut engine = InferEngine::new(&mut self.pool);
        engine.set_interner(interner);
        engine.set_well_known(well_known);
        engine.set_trait_registry(traits);
        engine.set_signatures(sigs);
        engine.set_module_aliases(aliases);
        engine.set_type_registry(types);
        engine.set_const_types(consts);
        engine.set_builtin_extensions(builtin_exts);
        engine.set_capabilities(current_caps, provided_caps);
        if let Some(self_ty) = impl_self {
            engine.set_impl_self_type(self_ty);
        }
        engine
    }

    /// Create an inference engine with a specific environment.
    ///
    /// Use this when you need to start with pre-bound variables
    /// (e.g., function parameters).
    /// Propagates capability state so the engine can validate call-site capabilities.
    pub fn create_engine_with_env(&mut self, env: TypeEnv) -> InferEngine<'_> {
        let interner = self.interner;
        let well_known = &self.well_known;
        // Split borrow: pool (mut) + traits, signatures, types, consts (shared)
        let traits = &self.traits;
        let sigs = &self.signatures;
        let aliases = &self.module_aliases;
        let types = &self.types;
        let consts = &self.const_types;
        let impl_self = self.current_impl_self;
        let current_caps = self.current_capabilities.clone();
        let provided_caps = self.provided_capabilities.clone();
        // Snapshot the module-level name set (prelude imports + same-module
        // function signatures + trait-registered names) before handing off
        // to the engine. `base_env.names()` walks the parent chain, so this
        // captures imports via `import_env` as well. Used by
        // `InferEngine::collect_lexical_outer` to distinguish names the
        // user bound lexically inside a function body from names that
        // merely came from the module surface — critical for the Value
        // Restriction's capture check in `should_generalize`.
        let module_scope_snapshot = self
            .base_env
            .as_ref()
            .map(|base| base.names().collect::<rustc_hash::FxHashSet<_>>());
        let builtin_exts = self.builtin_extensions.clone();
        let mut engine = InferEngine::with_env(&mut self.pool, env);
        engine.set_interner(interner);
        engine.set_well_known(well_known);
        engine.set_trait_registry(traits);
        engine.set_signatures(sigs);
        engine.set_module_aliases(aliases);
        engine.set_type_registry(types);
        engine.set_const_types(consts);
        engine.set_builtin_extensions(builtin_exts);
        engine.set_capabilities(current_caps, provided_caps);
        if let Some(snapshot) = module_scope_snapshot {
            engine.set_module_scope_snapshot(snapshot);
        }
        if let Some(self_ty) = impl_self {
            engine.set_impl_self_type(self_ty);
        }
        engine
    }

    // Context Management (closure-based save/restore; not RAII)

    /// Execute a closure with a function scope.
    ///
    /// Sets up `current_function` and `current_capabilities` for the duration.
    pub fn with_function_scope<T, F>(
        &mut self,
        fn_type: Idx,
        capabilities: FxHashSet<Name>,
        f: F,
    ) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved_fn = self.current_function.replace(fn_type);
        let saved_caps = std::mem::replace(&mut self.current_capabilities, capabilities);

        let result = f(self);

        self.current_function = saved_fn;
        self.current_capabilities = saved_caps;

        result
    }

    /// Execute a closure with an impl scope.
    ///
    /// Sets up `current_impl_self` for the duration.
    pub fn with_impl_scope<T, F>(&mut self, self_ty: Idx, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved = self.current_impl_self.replace(self_ty);
        let result = f(self);
        self.current_impl_self = saved;
        result
    }

    /// Execute a closure with additional provided capabilities.
    ///
    /// Used for `with...in` expressions.
    pub fn with_provided_capabilities<T, F>(&mut self, caps: FxHashSet<Name>, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let saved = std::mem::take(&mut self.provided_capabilities);
        self.provided_capabilities = caps;
        let result = f(self);
        self.provided_capabilities = saved;
        result
    }
}
