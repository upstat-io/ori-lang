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
mod tests {
    use super::*;
    use crate::aot::incremental::hash::hash_string;

    fn deps(name: &str, callees: &[&str], sig: &str) -> FunctionDeps {
        FunctionDeps {
            name: name.to_string(),
            callees: callees.iter().map(|s| (*s).to_string()).collect(),
            signature_hash: hash_string(sig),
            content_hash: hash_string(&format!("{name}_content")),
        }
    }

    #[test]
    fn body_only_change_skips_callers() {
        let mut graph = FunctionDependencyGraph::new();

        // main calls helper; helper calls utils
        graph.add_function(deps("main", &["helper"], "main_sig"));
        graph.add_function(deps("helper", &["utils"], "helper_sig"));
        graph.add_function(deps("utils", &[], "utils_sig"));

        // Change helper's body but NOT its signature
        let old_sigs: FxHashMap<String, ContentHash> = [
            ("main".to_string(), hash_string("main_sig")),
            ("helper".to_string(), hash_string("helper_sig")), // Same sig!
            ("utils".to_string(), hash_string("utils_sig")),
        ]
        .into_iter()
        .collect();

        let recompile = graph.functions_to_recompile(&["helper".to_string()], &old_sigs);

        // Only helper should be recompiled (body-only change)
        assert!(recompile.contains("helper"));
        assert!(
            !recompile.contains("main"),
            "main should not be recompiled for body-only change"
        );
        assert!(!recompile.contains("utils"));
    }

    #[test]
    fn signature_change_propagates_to_callers() {
        let mut graph = FunctionDependencyGraph::new();

        // main calls helper; helper calls utils
        graph.add_function(deps("main", &["helper"], "main_sig"));
        graph.add_function(deps("helper", &["utils"], "helper_sig_v2")); // Changed sig
        graph.add_function(deps("utils", &[], "utils_sig"));

        let old_sigs: FxHashMap<String, ContentHash> = [
            ("main".to_string(), hash_string("main_sig")),
            ("helper".to_string(), hash_string("helper_sig_v1")), // Different from current
            ("utils".to_string(), hash_string("utils_sig")),
        ]
        .into_iter()
        .collect();

        let recompile = graph.functions_to_recompile(&["helper".to_string()], &old_sigs);

        // helper AND main should be recompiled (signature change propagates)
        assert!(recompile.contains("helper"));
        assert!(
            recompile.contains("main"),
            "main should be recompiled when helper's signature changes"
        );
        assert!(!recompile.contains("utils"));
    }

    #[test]
    fn transitive_signature_propagation() {
        let mut graph = FunctionDependencyGraph::new();

        // a calls b, b calls c, c calls d
        graph.add_function(deps("a", &["b"], "a_sig"));
        graph.add_function(deps("b", &["c"], "b_sig"));
        graph.add_function(deps("c", &["d"], "c_sig_changed"));
        graph.add_function(deps("d", &[], "d_sig"));

        let old_sigs: FxHashMap<String, ContentHash> = [
            ("a".to_string(), hash_string("a_sig")),
            ("b".to_string(), hash_string("b_sig")),
            ("c".to_string(), hash_string("c_sig_original")), // Different
            ("d".to_string(), hash_string("d_sig")),
        ]
        .into_iter()
        .collect();

        let recompile = graph.functions_to_recompile(&["c".to_string()], &old_sigs);

        // c's signature changed, so c, b (caller), and a (transitive caller) recompile
        assert!(recompile.contains("c"));
        assert!(recompile.contains("b"));
        assert!(recompile.contains("a"));
        assert!(!recompile.contains("d")); // d is a callee, not a caller
    }

    #[test]
    fn new_function_treated_as_signature_change() {
        let mut graph = FunctionDependencyGraph::new();

        graph.add_function(deps("main", &["new_fn"], "main_sig"));
        graph.add_function(deps("new_fn", &[], "new_fn_sig"));

        // new_fn not in old_sigs — treated as new (signature change)
        let old_sigs: FxHashMap<String, ContentHash> =
            [("main".to_string(), hash_string("main_sig"))]
                .into_iter()
                .collect();

        let recompile = graph.functions_to_recompile(&["new_fn".to_string()], &old_sigs);

        assert!(recompile.contains("new_fn"));
        assert!(
            recompile.contains("main"),
            "caller should recompile for new function"
        );
    }

    #[test]
    fn empty_changed_set() {
        let mut graph = FunctionDependencyGraph::new();
        graph.add_function(deps("main", &[], "main_sig"));

        let old_sigs = FxHashMap::default();
        let recompile = graph.functions_to_recompile(&[], &old_sigs);

        assert!(recompile.is_empty());
    }

    #[test]
    fn graph_size_queries() {
        let mut graph = FunctionDependencyGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);

        graph.add_function(deps("a", &[], "sig"));
        assert!(!graph.is_empty());
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn callers_of_query() {
        let mut graph = FunctionDependencyGraph::new();
        graph.add_function(deps("a", &["c"], "a_sig"));
        graph.add_function(deps("b", &["c"], "b_sig"));
        graph.add_function(deps("c", &[], "c_sig"));

        let callers = graph.callers_of("c");
        assert!(callers.is_some());
        let callers = callers.unwrap_or_else(|| panic!("callers should exist"));
        assert!(callers.contains("a"));
        assert!(callers.contains("b"));
        assert_eq!(callers.len(), 2);
    }
}
