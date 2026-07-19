//! Sparse side-table — canonical home for rare per-key IR metadata stored
//! out-of-band from a dense arena / typed module.
//!
//! A handful of IR concerns attach metadata to only a few of the many
//! expressions in a module (call-site turbofish type-args, monomorphization
//! dispatch, assign-desugar plans, pattern resolutions). Storing that metadata
//! inline on every node would bloat the hot path, so each concern keeps a
//! sparse `(key, value)` table keyed by the owning `ExprId` / `PatternKey`.
//!
//! All such tables share one storage + lookup contract here so there is a
//! single linear-vs-binary-search policy and a single Salsa `Eq + Hash` motive
//! (every field type derives `Clone + Eq + PartialEq + Hash + Debug` per the
//! `ori_ir` design philosophy).

use std::fmt;

/// Sparse `(key, value)` side-table keyed by a `Copy + Ord` key, kept sorted so
/// every lookup is `O(log n)` binary search.
///
/// The sorted invariant is maintained at the type boundary: `insert` performs a
/// sorted insert (overwriting on a duplicate key), `from_unsorted` sorts once on
/// construction. Callers never sort or finalize — `get` always sees a sorted
/// table.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SparseSideTable<K, V> {
    /// Entries sorted ascending by key.
    entries: Vec<(K, V)>,
}

impl<K, V> Default for SparseSideTable<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<K: Copy + Ord, V: Clone> SparseSideTable<K, V> {
    /// Create an empty table.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a table from an unsorted `(key, value)` vec, sorting once.
    /// Preferred over repeated `insert` when the entries are bulk-collected.
    pub fn from_unsorted(mut entries: Vec<(K, V)>) -> Self {
        entries.sort_by_key(|(k, _)| *k);
        Self { entries }
    }

    /// Insert a `(key, value)` entry, preserving the sorted invariant.
    /// Overwrites the value on a duplicate key.
    pub fn insert(&mut self, key: K, value: V) {
        match self.entries.binary_search_by_key(&key, |(k, _)| *k) {
            Ok(idx) => self.entries[idx].1 = value,
            Err(idx) => self.entries.insert(idx, (key, value)),
        }
    }

    /// Look up the value for `key` via `O(log n)` binary search.
    #[inline]
    pub fn get(&self, key: K) -> Option<&V> {
        self.entries
            .binary_search_by_key(&key, |(k, _)| *k)
            .ok()
            .map(|idx| &self.entries[idx].1)
    }

    /// Whether `key` has an entry.
    #[inline]
    pub fn contains_key(&self, key: K) -> bool {
        self.entries.binary_search_by_key(&key, |(k, _)| *k).is_ok()
    }

    /// Remove all entries.
    #[inline]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Whether the table is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Iterate the sorted `(key, value)` entries.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, (K, V)> {
        self.entries.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a SparseSideTable<K, V> {
    type Item = &'a (K, V);
    type IntoIter = std::slice::Iter<'a, (K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for SparseSideTable<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

#[cfg(test)]
mod tests;
