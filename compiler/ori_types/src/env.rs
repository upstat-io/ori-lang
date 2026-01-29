//! Type environment for name resolution and scoping.

#![expect(
    clippy::disallowed_types,
    reason = "Rc<TypeEnvInner> is intentional for O(1) parent chain cloning via Rc::make_mut copy-on-write semantics. This differs from LocalScope<T> which uses Rc<RefCell<T>> for interior mutability."
)]

use ori_ir::{Name, TypeId};
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::rc::Rc;

use crate::context::InferenceContext;
use crate::core::{Type, TypeScheme, TypeSchemeId};
use crate::data::TypeVar;
use crate::type_interner::{SharedTypeInterner, TypeInterner};

/// Internal storage for `TypeEnv`, wrapped in Rc for cheap cloning.
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
        self.0.bindings.get(&name).or_else(|| {
            self.0
                .parent
                .as_ref()
                .and_then(|p| p.lookup_scheme_id(name))
        })
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

    /// Iterate over all bound names in this environment (current + parent scopes).
    ///
    /// Names from inner scopes may shadow names from outer scopes.
    /// This iterator yields all names, including duplicates from parent scopes.
    /// Use this for "did you mean?" suggestions.
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        NamesIterator {
            current: Some(self),
            current_iter: None,
        }
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
                // scheme.vars is typically small (<5 type parameters), so linear scan is acceptable
                if !scheme.vars.contains(&v) {
                    vars.insert(v);
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

/// Iterator over all bound names in a `TypeEnv`.
struct NamesIterator<'a> {
    current: Option<&'a TypeEnv>,
    current_iter: Option<std::collections::hash_map::Keys<'a, Name, TypeSchemeId>>,
}

impl Iterator for NamesIterator<'_> {
    type Item = Name;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Try to get the next item from the current iterator
            if let Some(ref mut iter) = self.current_iter {
                if let Some(&name) = iter.next() {
                    return Some(name);
                }
            }

            // Move to the next scope
            let env = self.current.take()?;
            self.current_iter = Some(env.0.bindings.keys());
            self.current = env.0.parent.as_ref();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

    #[test]
    fn test_new_env_is_empty() {
        let env = TypeEnv::new();
        let interner = SharedInterner::default();
        let name = interner.intern("x");

        assert!(env.lookup(name).is_none());
        assert!(!env.is_bound_locally(name));
    }

    #[test]
    fn test_bind_and_lookup() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let mut env = TypeEnv::new();
        env.bind(x, Type::Int);

        assert_eq!(env.lookup(x), Some(Type::Int));
        assert!(env.lookup(y).is_none());
    }

    #[test]
    fn test_child_scope_shadows_parent() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut parent = TypeEnv::new();
        parent.bind(x, Type::Int);

        let mut child = parent.child();
        child.bind(x, Type::Bool);

        // Child sees shadowed value
        assert_eq!(child.lookup(x), Some(Type::Bool));
        // Parent still has original value
        assert_eq!(parent.lookup(x), Some(Type::Int));
    }

    #[test]
    fn test_is_bound_locally() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let mut parent = TypeEnv::new();
        parent.bind(x, Type::Int);

        let child = parent.child();

        // x is in parent, not local to child
        assert!(!child.is_bound_locally(x));
        assert!(parent.is_bound_locally(x));
        // y is nowhere
        assert!(!child.is_bound_locally(y));
    }

    #[test]
    fn test_names_iterator() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");
        let z = interner.intern("z");

        let mut parent = TypeEnv::new();
        parent.bind(x, Type::Int);
        parent.bind(y, Type::Bool);

        let mut child = parent.child();
        child.bind(z, Type::Str);

        let names: Vec<Name> = child.names().collect();

        // Should contain all three names (z from child, x and y from parent)
        assert!(names.contains(&x));
        assert!(names.contains(&y));
        assert!(names.contains(&z));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_free_vars_collection() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");

        let mut env = TypeEnv::new();
        let ctx = InferenceContext::new();

        // Bind a monomorphic type (no free vars)
        env.bind(x, Type::Int);

        let free = env.free_vars(&ctx);
        assert!(free.is_empty());
    }
}
