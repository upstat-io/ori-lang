//! Function-Level Dependency Tracking
//!
//! Tracks call relationships between functions to enable precise
//! recompilation decisions:
//! - **Body-only change**: The function itself is recompiled but callers are not
//! - **Signature change**: The function AND all its callers must be recompiled
//!
//! # Design
//!
//! Inspired by Lean 4's dependency tracking in `src/Lean/Compiler/LCNF/`
//! and Rust's query-based incremental compilation.

use rustc_hash::{FxHashMap, FxHashSet};

use super::hash::ContentHash;

/// Dependency information for a single function.
#[derive(Debug, Clone)]
pub struct FunctionDeps {
    /// Function name.
    pub name: String,
    /// Functions this function calls (direct dependencies).
    pub callees: Vec<String>,
    /// Hash of the function's signature (for caller invalidation).
    pub signature_hash: ContentHash,
    /// Hash of the function's full content (body + sig + callees + globals).
    pub content_hash: ContentHash,
}

/// A dependency graph tracking call relationships between functions.
///
/// Provides signature-aware invalidation: when a function's signature
/// changes, all callers are marked for recompilation.
#[derive(Debug, Default)]
pub struct FunctionDependencyGraph {
    /// Forward index: function name → its dependency info.
    functions: FxHashMap<String, FunctionDeps>,
    /// Reverse index: function name → set of callers.
    callers: FxHashMap<String, FxHashSet<String>>,
}

impl FunctionDependencyGraph {
    /// Create a new empty dependency graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a function and its dependencies to the graph.
    pub fn add_function(&mut self, deps: FunctionDeps) {
        // Build reverse index: for each callee, record this function as a caller
        for callee in &deps.callees {
            self.callers
                .entry(callee.clone())
                .or_default()
                .insert(deps.name.clone());
        }

        self.functions.insert(deps.name.clone(), deps);
    }

    /// Get the dependency info for a function.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&FunctionDeps> {
        self.functions.get(name)
    }

    /// Get all direct callers of a function.
    #[must_use]
    pub fn callers_of(&self, name: &str) -> Option<&FxHashSet<String>> {
        self.callers.get(name)
    }

    /// Determine which functions need recompilation given a set of changed functions.
    ///
    /// For each changed function:
    /// - If only the body changed (`signature_hash` unchanged): only that function
    /// - If the signature changed: that function + all transitive callers
    ///
    /// `old_sigs` maps function name → previous signature hash. Functions not
    /// in `old_sigs` are treated as new (signature changed).
    pub fn functions_to_recompile(
        &self,
        changed: &[String],
        old_sigs: &FxHashMap<String, ContentHash>,
    ) -> FxHashSet<String> {
        let mut to_recompile = FxHashSet::default();

        for name in changed {
            to_recompile.insert(name.clone());

            // Check if the signature changed
            let sig_changed = match (self.functions.get(name), old_sigs.get(name)) {
                (Some(current), Some(old_sig)) => current.signature_hash != *old_sig,
                // New function or missing old data — treat as signature change
                _ => true,
            };

            if sig_changed {
                // Signature changed: propagate to all transitive callers
                self.collect_transitive_callers(name, &mut to_recompile);
            }
        }

        to_recompile
    }

    /// Collect all transitive callers of a function via BFS.
    fn collect_transitive_callers(&self, name: &str, result: &mut FxHashSet<String>) {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(name.to_string());

        while let Some(current) = queue.pop_front() {
            if let Some(callers) = self.callers.get(&current) {
                for caller in callers {
                    if result.insert(caller.clone()) {
                        // Newly added — check its callers too
                        queue.push_back(caller.clone());
                    }
                }
            }
        }
    }

    /// Get the number of tracked functions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Check if the graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

#[cfg(test)]
mod tests;
