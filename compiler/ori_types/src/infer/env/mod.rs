//! Type environment for the inference engine.
//!
//! This provides name → type scheme bindings with scope support,
//! using the new `Idx` type handles instead of `TypeId`.

#![expect(
    clippy::disallowed_types,
    reason = "Rc<TypeEnvInner> is intentional for O(1) parent chain cloning via Rc::make_mut copy-on-write semantics. This matches TypeEnv in env.rs."
)]

use ori_ir::{Mutability, Name};
use rustc_hash::FxHashMap;
use std::rc::Rc;

use crate::Idx;

/// A single binding entry in the type environment.
///
/// Combines the type scheme and optional mutability info into one struct,
/// eliminating the need for parallel `bindings`/`mutability` maps.
#[derive(Copy, Clone, Debug)]
struct Binding {
    /// The type (or type scheme) for this name.
    ty: Idx,
    /// Mutability from `let` bindings. `None` for prelude/param bindings
    /// that don't carry explicit mutability.
    mutable: Option<Mutability>,
}

/// Internal storage for `TypeEnv`.
#[derive(Clone, Debug)]
struct TypeEnvInner {
    /// Name → binding (type + mutability) map.
    bindings: FxHashMap<Name, Binding>,

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
        Rc::make_mut(&mut self.0)
            .bindings
            .insert(name, Binding { ty, mutable: None });
    }

    /// Bind a name to a type and record its mutability.
    ///
    /// `Mutability::Mutable` = `let x` (can be reassigned).
    /// `Mutability::Immutable` = `let $x` (immutable binding).
    pub fn bind_with_mutability(&mut self, name: Name, ty: Idx, mutable: Mutability) {
        Rc::make_mut(&mut self.0).bindings.insert(
            name,
            Binding {
                ty,
                mutable: Some(mutable),
            },
        );
    }

    /// Check if a binding is mutable, searching parent scopes.
    ///
    /// Returns `Some(true)` for mutable, `Some(false)` for immutable,
    /// `None` if the name has no recorded mutability (e.g., function params,
    /// prelude bindings).
    pub fn is_mutable(&self, name: Name) -> Option<bool> {
        self.0
            .bindings
            .get(&name)
            .and_then(|b| b.mutable)
            .map(Mutability::is_mutable)
            .or_else(|| self.0.parent.as_ref().and_then(|p| p.is_mutable(name)))
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
            .map(|b| b.ty)
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
    current_iter: Option<std::collections::hash_map::Keys<'a, Name, Binding>>,
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
mod tests;
