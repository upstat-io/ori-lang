//! Type environment for name resolution and scoping.

use ori_ir::{Name, TypeId};
use std::collections::HashMap;

use crate::core::{Type, TypeScheme, TypeSchemeId};
use crate::data::TypeVar;
use crate::context::InferenceContext;
use crate::type_interner::{SharedTypeInterner, TypeInterner};

/// Type environment for name resolution and scoping.
///
/// Supports both monomorphic types and polymorphic type schemes.
/// Internally uses `TypeSchemeId` for O(1) type equality comparisons.
#[derive(Clone, Debug)]
pub struct TypeEnv {
    /// Variable bindings: name -> type scheme (stored as `TypeSchemeId` for efficiency)
    bindings: HashMap<Name, TypeSchemeId>,
    /// Parent scope (for nested scopes)
    parent: Option<Box<TypeEnv>>,
    /// Type interner for converting between Type and `TypeId`
    interner: SharedTypeInterner,
}

impl TypeEnv {
    /// Create a new empty environment with a new type interner.
    pub fn new() -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: None,
            interner: SharedTypeInterner::new(),
        }
    }

    /// Create a new empty environment with a shared type interner.
    ///
    /// Use this when you want to share the interner with other compiler phases.
    pub fn with_interner(interner: SharedTypeInterner) -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: None,
            interner,
        }
    }

    /// Get a reference to the type interner.
    pub fn interner(&self) -> &TypeInterner {
        &self.interner
    }

    /// Get the shared type interner handle.
    pub fn shared_interner(&self) -> SharedTypeInterner {
        self.interner.clone()
    }

    /// Create a child scope.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: Some(Box::new(self.clone())),
            interner: self.interner.clone(),
        }
    }

    /// Bind a name to a monomorphic type in the current scope.
    #[expect(clippy::needless_pass_by_value, reason = "callers often construct Type inline; changing to &Type would add .clone() noise")]
    pub fn bind(&mut self, name: Name, ty: Type) {
        let ty_id = ty.to_type_id(&self.interner);
        self.bindings.insert(name, TypeSchemeId::mono(ty_id));
    }

    /// Bind a name to a monomorphic `TypeId` in the current scope.
    pub fn bind_id(&mut self, name: Name, ty: TypeId) {
        self.bindings.insert(name, TypeSchemeId::mono(ty));
    }

    /// Bind a name to a polymorphic type scheme in the current scope.
    #[expect(clippy::needless_pass_by_value, reason = "callers often construct TypeScheme inline; changing to &TypeScheme would add .clone() noise")]
    pub fn bind_scheme(&mut self, name: Name, scheme: TypeScheme) {
        let scheme_id = scheme.to_scheme_id(&self.interner);
        self.bindings.insert(name, scheme_id);
    }

    /// Bind a name to a polymorphic `TypeSchemeId` in the current scope.
    pub fn bind_scheme_id(&mut self, name: Name, scheme: TypeSchemeId) {
        self.bindings.insert(name, scheme);
    }

    /// Look up a name, searching parent scopes.
    /// Returns the type scheme (use instantiate to get a concrete type).
    ///
    /// Note: This converts from internal `TypeSchemeId`. For high-performance
    /// code, use `lookup_scheme_id` instead.
    pub fn lookup_scheme(&self, name: Name) -> Option<TypeScheme> {
        self.lookup_scheme_id(name).map(|s| s.to_scheme(&self.interner))
    }

    /// Look up a name, searching parent scopes.
    /// Returns the `TypeSchemeId` (internal representation).
    pub fn lookup_scheme_id(&self, name: Name) -> Option<&TypeSchemeId> {
        self.bindings.get(&name).or_else(|| {
            self.parent.as_ref().and_then(|p| p.lookup_scheme_id(name))
        })
    }

    /// Look up a name and return just the type (for monomorphic lookups).
    /// For polymorphic types, returns the uninstantiated type.
    ///
    /// Note: This converts from internal `TypeId`. For high-performance
    /// code, use `lookup_id` instead.
    pub fn lookup(&self, name: Name) -> Option<Type> {
        self.lookup_id(name).map(|id| self.interner.to_type(id))
    }

    /// Look up a name and return just the `TypeId` (for monomorphic lookups).
    /// For polymorphic types, returns the uninstantiated type.
    pub fn lookup_id(&self, name: Name) -> Option<TypeId> {
        self.lookup_scheme_id(name).map(|s| s.ty)
    }

    /// Check if a name is bound in the current scope only.
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.bindings.contains_key(&name)
    }

    /// Collect all free type variables in the environment.
    ///
    /// This is used during generalization to avoid quantifying over
    /// variables that are free in the environment.
    pub fn free_vars(&self, ctx: &InferenceContext) -> Vec<TypeVar> {
        let mut vars = Vec::new();
        self.collect_env_free_vars(ctx, &mut vars);
        vars
    }

    fn collect_env_free_vars(&self, ctx: &InferenceContext, vars: &mut Vec<TypeVar>) {
        for scheme in self.bindings.values() {
            // Only collect free vars that are NOT quantified in the scheme
            let scheme_free = ctx.free_vars_id(scheme.ty);
            for v in scheme_free {
                if !scheme.vars.contains(&v) && !vars.contains(&v) {
                    vars.push(v);
                }
            }
        }
        if let Some(parent) = &self.parent {
            parent.collect_env_free_vars(ctx, vars);
        }
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}
