//! Type environment for name resolution and scoping.

use ori_ir::{Name, TypeId};
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::rc::Rc;

use crate::context::InferenceContext;
use crate::core::{Type, TypeScheme, TypeSchemeId};
use crate::data::TypeVar;
use crate::type_interner::{SharedTypeInterner, TypeInterner};

/// Internal storage for TypeEnv, wrapped in Rc for cheap cloning.
#[derive(Clone, Debug)]
struct TypeEnvInner {
    /// Variable bindings: name -> type scheme (stored as `TypeSchemeId` for efficiency)
    bindings: FxHashMap<Name, TypeSchemeId>,
    /// Parent scope (for nested scopes) - cheap Rc clone when creating child scopes
    parent: Option<TypeEnv>,
    /// Type interner for converting between Type and `TypeId`
    interner: SharedTypeInterner,
}

/// Type environment for name resolution and scoping.
///
/// Supports both monomorphic types and polymorphic type schemes.
/// Internally uses `TypeSchemeId` for O(1) type equality comparisons.
///
/// # Performance
/// Uses `Rc<TypeEnvInner>` internally for O(1) parent chain cloning.
/// Creating a child scope no longer clones the entire parent chain.
#[derive(Clone, Debug)]
pub struct TypeEnv(Rc<TypeEnvInner>);

impl TypeEnv {
    /// Create a new empty environment with a new type interner.
    pub fn new() -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
            interner: SharedTypeInterner::new(),
        }))
    }

    /// Create a new empty environment with a shared type interner.
    ///
    /// Use this when you want to share the interner with other compiler phases.
    pub fn with_interner(interner: SharedTypeInterner) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
            interner,
        }))
    }

    /// Get a reference to the type interner.
    pub fn interner(&self) -> &TypeInterner {
        &self.0.interner
    }

    /// Get the shared type interner handle.
    pub fn shared_interner(&self) -> SharedTypeInterner {
        self.0.interner.clone()
    }

    /// Create a child scope.
    ///
    /// This is O(1) due to Rc-based parent sharing - no recursive cloning.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: Some(self.clone()), // Cheap Rc clone
            interner: self.0.interner.clone(),
        }))
    }

    /// Bind a name to a monomorphic type in the current scope.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers often construct Type inline; changing to &Type would add .clone() noise"
    )]
    pub fn bind(&mut self, name: Name, ty: Type) {
        let inner = Rc::make_mut(&mut self.0);
        let ty_id = ty.to_type_id(&inner.interner);
        inner.bindings.insert(name, TypeSchemeId::mono(ty_id));
    }

    /// Bind a name to a monomorphic `TypeId` in the current scope.
    pub fn bind_id(&mut self, name: Name, ty: TypeId) {
        let inner = Rc::make_mut(&mut self.0);
        inner.bindings.insert(name, TypeSchemeId::mono(ty));
    }

    /// Bind a name to a polymorphic type scheme in the current scope.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers often construct TypeScheme inline; changing to &TypeScheme would add .clone() noise"
    )]
    pub fn bind_scheme(&mut self, name: Name, scheme: TypeScheme) {
        let inner = Rc::make_mut(&mut self.0);
        let scheme_id = scheme.to_scheme_id(&inner.interner);
        inner.bindings.insert(name, scheme_id);
    }

    /// Bind a name to a polymorphic `TypeSchemeId` in the current scope.
    pub fn bind_scheme_id(&mut self, name: Name, scheme: TypeSchemeId) {
        let inner = Rc::make_mut(&mut self.0);
        inner.bindings.insert(name, scheme);
    }

    /// Look up a name, searching parent scopes.
    /// Returns the type scheme (use instantiate to get a concrete type).
    ///
    /// Note: This converts from internal `TypeSchemeId`. For high-performance
    /// code, use `lookup_scheme_id` instead.
    pub fn lookup_scheme(&self, name: Name) -> Option<TypeScheme> {
        self.lookup_scheme_id(name)
            .map(|s| s.to_scheme(&self.0.interner))
    }

    /// Look up a name, searching parent scopes.
    /// Returns the `TypeSchemeId` (internal representation).
    pub fn lookup_scheme_id(&self, name: Name) -> Option<&TypeSchemeId> {
        self.0
            .bindings
            .get(&name)
            .or_else(|| self.0.parent.as_ref().and_then(|p| p.lookup_scheme_id(name)))
    }

    /// Look up a name and return just the type (for monomorphic lookups).
    /// For polymorphic types, returns the uninstantiated type.
    ///
    /// Note: This converts from internal `TypeId`. For high-performance
    /// code, use `lookup_id` instead.
    pub fn lookup(&self, name: Name) -> Option<Type> {
        self.lookup_id(name).map(|id| self.0.interner.to_type(id))
    }

    /// Look up a name and return just the `TypeId` (for monomorphic lookups).
    /// For polymorphic types, returns the uninstantiated type.
    pub fn lookup_id(&self, name: Name) -> Option<TypeId> {
        self.lookup_scheme_id(name).map(|s| s.ty)
    }

    /// Check if a name is bound in the current scope only.
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.0.bindings.contains_key(&name)
    }

    /// Collect all free type variables in the environment.
    ///
    /// This is used during generalization to avoid quantifying over
    /// variables that are free in the environment.
    pub fn free_vars(&self, ctx: &InferenceContext) -> Vec<TypeVar> {
        let mut vars = HashSet::new();
        self.collect_env_free_vars(ctx, &mut vars);
        vars.into_iter().collect()
    }

    fn collect_env_free_vars(&self, ctx: &InferenceContext, vars: &mut HashSet<TypeVar>) {
        for scheme in self.0.bindings.values() {
            // Only collect free vars that are NOT quantified in the scheme
            let scheme_free = ctx.free_vars_id(scheme.ty);
            for v in scheme_free {
                if !scheme.vars.contains(&v) {
                    vars.insert(v); // O(1) instead of O(n)
                }
            }
        }
        if let Some(parent) = &self.0.parent {
            parent.collect_env_free_vars(ctx, vars);
        }
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}
