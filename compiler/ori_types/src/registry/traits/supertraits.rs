//! Super-trait DAG traversal and default-conflict analysis for `TraitRegistry`.
//!
//! Transitive super-trait collection, method gathering with closest-override
//! priority, and detection of methods whose defaults conflict across unrelated
//! super-trait paths. Read-only lookup of single traits/impls lives in the
//! sibling `lookup` module.

use std::collections::VecDeque;

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::Idx;

use super::{TraitMethodDef, TraitRegistry};

impl TraitRegistry {
    // Super-trait Queries

    /// Collect all super-traits transitively (DAG walk with cycle protection).
    ///
    /// Returns a de-duplicated list of all ancestor trait indices, in
    /// breadth-first order. Gracefully handles missing traits (returns empty
    /// for unknown indices, allowing registration-order independence).
    pub fn all_super_traits(&self, trait_idx: Idx) -> Vec<Idx> {
        let mut visited = FxHashSet::default();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // Seed with direct super-traits
        if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            for &st in &entry.super_traits {
                if visited.insert(st) {
                    queue.push_back(st);
                }
            }
        }

        while let Some(idx) = queue.pop_front() {
            result.push(idx);
            if let Some(entry) = self.get_trait_by_idx(idx) {
                for &st in &entry.super_traits {
                    if visited.insert(st) {
                        queue.push_back(st);
                    }
                }
            }
        }

        result
    }

    /// Gather methods from a trait and all its ancestors, deduplicating by name.
    ///
    /// Closest override wins: methods defined on the trait itself take priority
    /// over those inherited from super-traits. Returns `(method_name,
    /// owning_trait_idx, &TraitMethodDef)` tuples.
    pub fn collected_methods(&self, trait_idx: Idx) -> Vec<(Name, Idx, &TraitMethodDef)> {
        let mut seen = FxHashSet::default();
        let mut result = Vec::new();

        // Direct methods first (highest priority)
        if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            for (name, method) in &entry.methods {
                if seen.insert(*name) {
                    result.push((*name, trait_idx, method));
                }
            }
        }

        // Walk ancestors in BFS order; skip already-seen names
        for ancestor_idx in self.all_super_traits(trait_idx) {
            if let Some(entry) = self.get_trait_by_idx(ancestor_idx) {
                for (name, method) in &entry.methods {
                    if seen.insert(*name) {
                        result.push((*name, ancestor_idx, method));
                    }
                }
            }
        }

        result
    }

    /// Find methods with conflicting defaults from different super-trait paths.
    ///
    /// Returns `(method_name, [conflicting_trait_indices])` for each method
    /// that has default implementations from multiple unrelated super-trait
    /// paths. Only includes methods where the conflict isn't resolved by the
    /// trait itself (i.e., the trait's own methods are excluded from conflict
    /// detection since they are the resolution).
    pub fn find_conflicting_defaults(&self, trait_idx: Idx) -> Vec<(Name, Vec<Idx>)> {
        // Track which methods we've seen defaults for, and from which traits
        let mut defaults_by_method: FxHashMap<Name, Vec<Idx>> = FxHashMap::default();

        // Collect defaults from all direct super-traits and their ancestors.
        // Only look at direct super-traits' collected_methods — each super-trait
        // provides its resolved set (closest override wins within its branch).
        //
        // Registration discipline: trait_idx MUST already be registered before
        // conflict-default analysis runs (Registration group pass 0c precedes
        // any consumer per CK-1). Missing entry = missing-registration bug,
        // NOT "no super-traits" — silently swallowing as empty Vec hid the
        // bug class. debug_assert in debug builds; production fallback returns
        // empty conflict set (consumer sees zero conflicts vs panicking).
        let direct_supers = if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            entry.super_traits.clone()
        } else {
            debug_assert!(
                false,
                "find_conflicting_defaults called with unregistered trait_idx={trait_idx:?} — \
                 Registration group pass 0c must precede this query (CK-1)"
            );
            return Vec::new();
        };

        for &super_idx in &direct_supers {
            for (name, _owner, method) in self.collected_methods(super_idx) {
                if method.has_default {
                    defaults_by_method.entry(name).or_default().push(super_idx);
                }
            }
        }

        // Methods defined directly on this trait are NOT conflicting — they
        // serve as the resolution. Also, if this trait declares a method
        // (default or not), it overrides any inherited version.
        if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            for name in entry.methods.keys() {
                defaults_by_method.remove(name);
            }
        }

        // Conflicting: >1 distinct super-trait provides a default for the same method
        defaults_by_method
            .into_iter()
            .filter(|(_, providers)| providers.len() > 1)
            .collect()
    }
}
