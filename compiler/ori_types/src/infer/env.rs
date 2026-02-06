//! Type environment for the inference engine.
//!
//! This provides name → type scheme bindings with scope support,
//! using the new `Idx` type handles instead of `TypeId`.

#![expect(
    clippy::disallowed_types,
    reason = "Rc<TypeEnvInner> is intentional for O(1) parent chain cloning via Rc::make_mut copy-on-write semantics. This matches TypeEnv in env.rs."
)]

use ori_ir::Name;
use rustc_hash::FxHashMap;
use std::rc::Rc;

use crate::Idx;

/// Internal storage for `TypeEnv`.
#[derive(Clone, Debug)]
struct TypeEnvInner {
    /// Name → type scheme bindings.
    /// Schemes are just `Idx` (could be a Scheme tag or a monomorphic type).
    bindings: FxHashMap<Name, Idx>,

    /// Parent scope for lookup chaining.
    parent: Option<TypeEnv>,
}

/// Type environment.
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
/// let mut env = TypeEnv::new();
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
pub struct TypeEnv(Rc<TypeEnvInner>);

impl TypeEnv {
    /// Create a new empty environment.
    pub fn new() -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
            bindings: FxHashMap::default(),
            parent: None,
        }))
    }

    /// Create a child scope.
    ///
    /// This is O(1) due to Rc-based parent sharing.
    #[must_use]
    pub fn child(&self) -> Self {
        TypeEnv(Rc::new(TypeEnvInner {
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
    /// Uses Levenshtein edit distance to find names within a dynamic threshold
    /// based on the target name's length. Returns up to `max_results` similar names,
    /// sorted by edit distance (best match first).
    ///
    /// The `resolve` closure maps `Name` handles to their string representations.
    /// If the resolver returns `None` for a name, that name is skipped.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let similar = env.find_similar(misspelled, 3, |n| interner.lookup(n));
    /// // Returns up to 3 names sorted by closeness
    /// ```
    pub fn find_similar<'r>(
        &self,
        target: Name,
        max_results: usize,
        resolve: impl Fn(Name) -> Option<&'r str>,
    ) -> Vec<Name> {
        let Some(target_str) = resolve(target) else {
            return Vec::new();
        };

        if target_str.is_empty() || max_results == 0 {
            return Vec::new();
        }

        let threshold = default_threshold(target_str.len());

        // Collect (name, distance) pairs, deduplicating by name
        let mut seen = rustc_hash::FxHashSet::default();
        let mut matches: Vec<(Name, usize)> = self
            .names()
            .filter(|&name| name != target && seen.insert(name))
            .filter_map(|name| {
                let candidate = resolve(name)?;
                // Quick reject: if lengths differ too much, skip expensive computation
                let len_diff = target_str.len().abs_diff(candidate.len());
                if len_diff > threshold {
                    return None;
                }
                let distance = crate::edit_distance(target_str, candidate);
                (distance <= threshold).then_some((name, distance))
            })
            .collect();

        // Sort by distance, then by name for determinism
        matches.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

        matches
            .into_iter()
            .take(max_results)
            .map(|(name, _)| name)
            .collect()
    }

    /// Count bindings in the current scope only.
    pub fn local_count(&self) -> usize {
        self.0.bindings.len()
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

// ============================================================================
// Edit distance utilities
// ============================================================================

/// Dynamic threshold based on name length.
///
/// Shorter names need stricter matching to avoid false positives:
/// - 1-2 chars: distance ≤ 1
/// - 3-5 chars: distance ≤ 2
/// - 6+ chars: distance ≤ 3
fn default_threshold(name_len: usize) -> usize {
    match name_len {
        0 => 0,
        1..=2 => 1,
        3..=5 => 2,
        _ => 3,
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
        let env = TypeEnv::new();
        assert!(env.lookup(name(1)).is_none());
        assert!(!env.is_bound_locally(name(1)));
    }

    #[test]
    fn test_bind_and_lookup() {
        let mut env = TypeEnv::new();
        env.bind(name(1), Idx::INT);

        assert_eq!(env.lookup(name(1)), Some(Idx::INT));
        assert!(env.lookup(name(2)).is_none());
    }

    #[test]
    fn test_child_scope_shadows_parent() {
        let mut parent = TypeEnv::new();
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
        let mut parent = TypeEnv::new();
        parent.bind(name(1), Idx::INT);

        let child = parent.child();

        // Child can see parent's bindings
        assert_eq!(child.lookup(name(1)), Some(Idx::INT));
    }

    #[test]
    fn test_is_bound_locally() {
        let mut parent = TypeEnv::new();
        parent.bind(name(1), Idx::INT);

        let child = parent.child();

        // name(1) is in parent, not local to child
        assert!(!child.is_bound_locally(name(1)));
        assert!(parent.is_bound_locally(name(1)));
    }

    #[test]
    fn test_names_iterator() {
        let mut parent = TypeEnv::new();
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
        let mut env = TypeEnv::new();
        assert_eq!(env.local_count(), 0);

        env.bind(name(1), Idx::INT);
        assert_eq!(env.local_count(), 1);

        env.bind(name(2), Idx::BOOL);
        assert_eq!(env.local_count(), 2);
    }

    #[test]
    fn test_parent() {
        let parent = TypeEnv::new();
        let child = parent.child();

        assert!(parent.parent().is_none());
        assert!(child.parent().is_some());
    }

    // ====================================================================
    // Edit distance tests (uses crate::edit_distance from type_error/diff.rs)
    // ====================================================================

    use crate::edit_distance;

    #[test]
    fn test_edit_distance_identical() {
        assert_eq!(edit_distance("hello", "hello"), 0);
        assert_eq!(edit_distance("", ""), 0);
    }

    #[test]
    fn test_edit_distance_empty() {
        assert_eq!(edit_distance("hello", ""), 5);
        assert_eq!(edit_distance("", "world"), 5);
    }

    #[test]
    fn test_edit_distance_single_edit() {
        assert_eq!(edit_distance("abc", "adc"), 1); // substitution
        assert_eq!(edit_distance("abc", "abcd"), 1); // insertion
        assert_eq!(edit_distance("abcd", "abc"), 1); // deletion
    }

    #[test]
    fn test_edit_distance_typos() {
        assert_eq!(edit_distance("lenght", "length"), 2); // transposition (2 edits in Levenshtein)
        assert_eq!(edit_distance("helo", "hello"), 1); // missing char
        assert_eq!(edit_distance("mpa", "map"), 2); // transposition
    }

    #[test]
    fn test_default_threshold() {
        assert_eq!(default_threshold(0), 0);
        assert_eq!(default_threshold(1), 1);
        assert_eq!(default_threshold(2), 1);
        assert_eq!(default_threshold(3), 2);
        assert_eq!(default_threshold(5), 2);
        assert_eq!(default_threshold(6), 3);
        assert_eq!(default_threshold(10), 3);
    }

    // ====================================================================
    // find_similar tests
    // ====================================================================

    /// Create a simple resolver mapping Name(raw) -> &str.
    fn make_resolver<'a>(names: &'a [(u32, &'a str)]) -> impl Fn(Name) -> Option<&'a str> + 'a {
        move |n: Name| {
            names
                .iter()
                .find(|(id, _)| Name::from_raw(*id) == n)
                .map(|(_, s)| *s)
        }
    }

    #[test]
    fn test_find_similar_basic_typo() {
        let mut env = TypeEnv::new();
        // Bind "length", "height", "width"
        env.bind(name(1), Idx::INT); // "length"
        env.bind(name(2), Idx::INT); // "height"
        env.bind(name(3), Idx::INT); // "width"

        let resolver = make_resolver(&[(1, "length"), (2, "height"), (3, "width"), (4, "lenght")]);

        // "lenght" (typo) should find "length"
        let similar = env.find_similar(name(4), 3, &resolver);
        assert!(!similar.is_empty(), "should find at least one suggestion");
        assert_eq!(similar[0], name(1), "best match should be 'length'");
    }

    #[test]
    fn test_find_similar_no_match() {
        let mut env = TypeEnv::new();
        env.bind(name(1), Idx::INT); // "alpha"
        env.bind(name(2), Idx::INT); // "beta"

        let resolver = make_resolver(&[(1, "alpha"), (2, "beta"), (3, "xyz")]);

        let similar = env.find_similar(name(3), 3, &resolver);
        assert!(similar.is_empty(), "no similar names should be found");
    }

    #[test]
    fn test_find_similar_empty_env() {
        let env = TypeEnv::new();
        let resolver = make_resolver(&[(1, "anything")]);

        let similar = env.find_similar(name(1), 3, &resolver);
        assert!(similar.is_empty());
    }

    #[test]
    fn test_find_similar_respects_max_results() {
        let mut env = TypeEnv::new();
        env.bind(name(1), Idx::INT); // "abc"
        env.bind(name(2), Idx::INT); // "abd"
        env.bind(name(3), Idx::INT); // "abe"
        env.bind(name(4), Idx::INT); // "abf"

        let resolver = make_resolver(&[(1, "abc"), (2, "abd"), (3, "abe"), (4, "abf"), (5, "abx")]);

        let similar = env.find_similar(name(5), 2, &resolver);
        assert!(similar.len() <= 2, "should respect max_results limit");
    }

    #[test]
    fn test_find_similar_searches_parent_scopes() {
        let mut parent = TypeEnv::new();
        parent.bind(name(1), Idx::INT); // "filter" in parent

        let mut child = parent.child();
        child.bind(name(2), Idx::INT); // "map" in child

        let resolver = make_resolver(&[(1, "filter"), (2, "map"), (3, "fiter")]);

        // "fiter" should find "filter" from parent scope
        let similar = child.find_similar(name(3), 3, &resolver);
        assert!(!similar.is_empty(), "should search parent scopes");
        assert_eq!(similar[0], name(1));
    }

    #[test]
    fn test_find_similar_skips_target_name() {
        let mut env = TypeEnv::new();
        env.bind(name(1), Idx::INT); // "foo"

        let resolver = make_resolver(&[(1, "foo")]);

        // Looking up "foo" itself shouldn't suggest "foo" back
        let similar = env.find_similar(name(1), 3, &resolver);
        assert!(
            similar.is_empty(),
            "should not suggest the target name itself"
        );
    }

    #[test]
    fn test_find_similar_sorted_by_distance() {
        let mut env = TypeEnv::new();
        env.bind(name(1), Idx::INT); // "abcde" (distance 2 from "abxyz")
        env.bind(name(2), Idx::INT); // "abcyz" (distance 1 from "abxyz")

        let resolver = make_resolver(&[(1, "abcde"), (2, "abcyz"), (3, "abxyz")]);

        let similar = env.find_similar(name(3), 3, &resolver);
        if similar.len() >= 2 {
            // Closer match should come first
            assert_eq!(similar[0], name(2), "closer match should be first");
        }
    }

    #[test]
    fn test_find_similar_unresolvable_target() {
        let mut env = TypeEnv::new();
        env.bind(name(1), Idx::INT);

        // Resolver that can't resolve the target
        let resolver = |_: Name| -> Option<&str> { None };

        let similar = env.find_similar(name(99), 3, resolver);
        assert!(similar.is_empty());
    }
}
