//! Type environment for the Types V2 inference engine.
//!
//! This provides name → type scheme bindings with scope support,
//! using the new `Idx` type handles instead of `TypeId`.

#![expect(
    clippy::disallowed_types,
    reason = "Rc<TypeEnvV2Inner> is intentional for O(1) parent chain cloning via Rc::make_mut copy-on-write semantics. This matches TypeEnv in env.rs."
)]

use ori_ir::Name;
use rustc_hash::FxHashMap;
use std::rc::Rc;

use crate::Idx;

/// Internal storage for `TypeEnvV2`.
#[derive(Clone, Debug)]
struct TypeEnvV2Inner {
    /// Name → type scheme bindings.
    /// Schemes are just `Idx` (could be a Scheme tag or a monomorphic type).
    bindings: FxHashMap<Name, Idx>,

    /// Parent scope for lookup chaining.
    parent: Option<TypeEnvV2>,
}

/// Type environment for Types V2.
///
/// Maps names to type schemes (polymorphic types) using `Idx` handles.
///
/// # Performance
///
/// Uses `Rc` for O(1) parent chain cloning. Creating a child scope
/// doesn't clone the entire parent chain.
///
/// # Usage
///
/// ```ignore
/// let mut env = TypeEnvV2::new();
///
/// // Bind a monomorphic type
/// env.bind(name, Idx::INT);
///
/// // Create child scope for let binding
/// let mut child = env.child();
/// child.bind(local_name, local_ty);
///
/// // Lookup searches parent chain
/// assert_eq!(child.lookup(name), Some(Idx::INT));
/// ```
#[derive(Clone, Debug)]
pub struct TypeEnvV2(Rc<TypeEnvV2Inner>);

impl TypeEnvV2 {
    /// Create a new empty environment.
    pub fn new() -> Self {
        TypeEnvV2(Rc::new(TypeEnvV2Inner {
            bindings: FxHashMap::default(),
            parent: None,
        }))
    }

    /// Create a child scope.
    ///
    /// This is O(1) due to Rc-based parent sharing.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnvV2(Rc::new(TypeEnvV2Inner {
            bindings: FxHashMap::default(),
            parent: Some(self.clone()),
        }))
    }

    /// Get the parent scope, if any.
    pub fn parent(&self) -> Option<Self> {
        self.0.parent.clone()
    }

    /// Bind a name to a type (or type scheme) in the current scope.
    ///
    /// For monomorphic types, pass the type directly.
    /// For polymorphic types, pass a Scheme `Idx`.
    pub fn bind(&mut self, name: Name, ty: Idx) {
        Rc::make_mut(&mut self.0).bindings.insert(name, ty);
    }

    /// Bind a name to a type scheme (alias for `bind`).
    ///
    /// Use this when you know you're binding a polymorphic scheme
    /// for code clarity.
    #[inline]
    pub fn bind_scheme(&mut self, name: Name, scheme: Idx) {
        self.bind(name, scheme);
    }

    /// Look up a name, searching parent scopes.
    ///
    /// Returns the type scheme `Idx` if found.
    /// Use `InferEngine::instantiate()` to get a concrete type.
    pub fn lookup(&self, name: Name) -> Option<Idx> {
        self.0
            .bindings
            .get(&name)
            .copied()
            .or_else(|| self.0.parent.as_ref().and_then(|p| p.lookup(name)))
    }

    /// Look up a name, returning the type scheme.
    ///
    /// Alias for `lookup` - use whichever is clearer in context.
    #[inline]
    pub fn lookup_scheme(&self, name: Name) -> Option<Idx> {
        self.lookup(name)
    }

    /// Check if a name is bound in the current scope only.
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.0.bindings.contains_key(&name)
    }

    /// Iterate over all bound names in this environment.
    ///
    /// Includes names from parent scopes. Names may be duplicated
    /// if shadowed.
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        NamesIterator {
            current: Some(self),
            current_iter: None,
        }
    }

    /// Find names similar to the given name (for typo suggestions).
    ///
    /// Returns names with edit distance ≤ 2 from the target.
    pub fn find_similar(&self, target: Name) -> Vec<Name> {
        // We need a string interner to compute edit distance
        // For now, just return empty - will be filled in when integrated
        // with the string interner
        let _ = target;
        Vec::new()
    }

    /// Count bindings in the current scope only.
    pub fn local_count(&self) -> usize {
        self.0.bindings.len()
    }
}

impl Default for TypeEnvV2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over all bound names in a `TypeEnvV2`.
struct NamesIterator<'a> {
    current: Option<&'a TypeEnvV2>,
    current_iter: Option<std::collections::hash_map::Keys<'a, Name, Idx>>,
}

impl Iterator for NamesIterator<'_> {
    type Item = Name;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref mut iter) = self.current_iter {
                if let Some(&name) = iter.next() {
                    return Some(name);
                }
            }

            let env = self.current.take()?;
            self.current_iter = Some(env.0.bindings.keys());
            self.current = env.0.parent.as_ref();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(n: u32) -> Name {
        Name::from_raw(n)
    }

    #[test]
    fn test_new_env_is_empty() {
        let env = TypeEnvV2::new();
        assert!(env.lookup(name(1)).is_none());
        assert!(!env.is_bound_locally(name(1)));
    }

    #[test]
    fn test_bind_and_lookup() {
        let mut env = TypeEnvV2::new();
        env.bind(name(1), Idx::INT);

        assert_eq!(env.lookup(name(1)), Some(Idx::INT));
        assert!(env.lookup(name(2)).is_none());
    }

    #[test]
    fn test_child_scope_shadows_parent() {
        let mut parent = TypeEnvV2::new();
        parent.bind(name(1), Idx::INT);

        let mut child = parent.child();
        child.bind(name(1), Idx::BOOL);

        // Child sees shadowed value
        assert_eq!(child.lookup(name(1)), Some(Idx::BOOL));
        // Parent still has original
        assert_eq!(parent.lookup(name(1)), Some(Idx::INT));
    }

    #[test]
    fn test_child_sees_parent_bindings() {
        let mut parent = TypeEnvV2::new();
        parent.bind(name(1), Idx::INT);

        let child = parent.child();

        // Child can see parent's bindings
        assert_eq!(child.lookup(name(1)), Some(Idx::INT));
    }

    #[test]
    fn test_is_bound_locally() {
        let mut parent = TypeEnvV2::new();
        parent.bind(name(1), Idx::INT);

        let child = parent.child();

        // name(1) is in parent, not local to child
        assert!(!child.is_bound_locally(name(1)));
        assert!(parent.is_bound_locally(name(1)));
    }

    #[test]
    fn test_names_iterator() {
        let mut parent = TypeEnvV2::new();
        parent.bind(name(1), Idx::INT);
        parent.bind(name(2), Idx::BOOL);

        let mut child = parent.child();
        child.bind(name(3), Idx::STR);

        let names: Vec<Name> = child.names().collect();

        assert!(names.contains(&name(1)));
        assert!(names.contains(&name(2)));
        assert!(names.contains(&name(3)));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_local_count() {
        let mut env = TypeEnvV2::new();
        assert_eq!(env.local_count(), 0);

        env.bind(name(1), Idx::INT);
        assert_eq!(env.local_count(), 1);

        env.bind(name(2), Idx::BOOL);
        assert_eq!(env.local_count(), 2);
    }

    #[test]
    fn test_parent() {
        let parent = TypeEnvV2::new();
        let child = parent.child();

        assert!(parent.parent().is_none());
        assert!(child.parent().is_some());
    }
}
