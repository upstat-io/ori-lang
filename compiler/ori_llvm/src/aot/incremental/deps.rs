//! Dependency Tracking for Incremental Compilation
//!
//! Tracks import relationships between source files to determine
//! what needs recompilation when a file changes.

use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use super::hash::ContentHash;

/// A node in the dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// Path to the source file.
    pub path: PathBuf,
    /// Content hash of the file.
    pub hash: ContentHash,
    /// Files this file imports (direct dependencies).
    pub imports: Vec<PathBuf>,
}

/// A dependency graph tracking import relationships.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// Map from file path to its dependency node.
    nodes: FxHashMap<PathBuf, DependencyNode>,
    /// Reverse dependency map: file -> files that import it.
    dependents: FxHashMap<PathBuf, FxHashSet<PathBuf>>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file with its imports to the graph.
    pub fn add_file(&mut self, path: PathBuf, hash: ContentHash, imports: Vec<PathBuf>) {
        // Update reverse dependency map
        for import in &imports {
            self.dependents
                .entry(import.clone())
                .or_default()
                .insert(path.clone());
        }

        // Remove old imports from reverse map if updating
        // Use HashSet for O(1) lookup instead of O(n) Vec::contains
        if let Some(old_node) = self.nodes.get(&path) {
            let imports_set: FxHashSet<&PathBuf> = imports.iter().collect();
            for old_import in &old_node.imports {
                if !imports_set.contains(old_import) {
                    if let Some(deps) = self.dependents.get_mut(old_import) {
                        deps.remove(&path);
                    }
                }
            }
        }

        self.nodes.insert(
            path.clone(),
            DependencyNode {
                path,
                hash,
                imports,
            },
        );
    }

    /// Remove a file from the graph.
    pub fn remove_file(&mut self, path: &Path) {
        if let Some(node) = self.nodes.remove(path) {
            // Remove from reverse map
            for import in &node.imports {
                if let Some(deps) = self.dependents.get_mut(import) {
                    deps.remove(path);
                }
            }
        }
        // Also remove as a dependent
        self.dependents.remove(path);
    }

    /// Get direct dependencies of a file.
    #[must_use]
    pub fn get_imports(&self, path: &Path) -> Option<&[PathBuf]> {
        self.nodes.get(path).map(|n| n.imports.as_slice())
    }

    /// Get files that directly depend on the given file.
    #[must_use]
    pub fn get_dependents(&self, path: &Path) -> Option<&FxHashSet<PathBuf>> {
        self.dependents.get(path)
    }

    /// Get the hash of a file.
    #[must_use]
    pub fn get_hash(&self, path: &Path) -> Option<ContentHash> {
        self.nodes.get(path).map(|n| n.hash)
    }

    /// Check if a file is in the graph.
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.nodes.contains_key(path)
    }

    /// Get all files in the graph.
    pub fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.nodes.keys()
    }

    /// Get the number of files in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Compute the transitive closure of dependencies for a file.
    ///
    /// Returns all files that this file depends on, directly or indirectly.
    #[must_use]
    pub fn transitive_dependencies(&self, path: &Path) -> FxHashSet<PathBuf> {
        // Use references internally to avoid cloning during traversal
        let mut visited: FxHashSet<&PathBuf> = FxHashSet::default();
        let mut queue: VecDeque<&PathBuf> = VecDeque::new();

        if let Some(node) = self.nodes.get(path) {
            for import in &node.imports {
                queue.push_back(import);
            }
        }

        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                if let Some(node) = self.nodes.get(current) {
                    for import in &node.imports {
                        if !visited.contains(import) {
                            queue.push_back(import);
                        }
                    }
                }
            }
        }

        // Clone only at the end when building the result
        visited.into_iter().cloned().collect()
    }

    /// Compute the transitive closure of dependents for a file.
    ///
    /// Returns all files that depend on this file, directly or indirectly.
    #[must_use]
    pub fn transitive_dependents(&self, path: &Path) -> FxHashSet<PathBuf> {
        // Use references internally to avoid cloning during traversal
        let mut visited: FxHashSet<&PathBuf> = FxHashSet::default();
        let mut queue: VecDeque<&PathBuf> = VecDeque::new();

        if let Some(deps) = self.dependents.get(path) {
            for dep in deps {
                queue.push_back(dep);
            }
        }

        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                if let Some(deps) = self.dependents.get(current) {
                    for dep in deps {
                        if !visited.contains(dep) {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        // Clone only at the end when building the result
        visited.into_iter().cloned().collect()
    }

    /// Compute a topological ordering for compilation.
    ///
    /// Files are ordered so that dependencies come before dependents.
    /// Returns None if there's a cycle.
    ///
    /// Note: This produces a deterministic ordering by sorting paths with equal
    /// in-degree. This ensures consistent compilation order across runs.
    pub fn topological_order(&self) -> Option<Vec<PathBuf>> {
        // Count how many dependencies each node has (in-degree in dependency graph)
        let mut in_degree: FxHashMap<&PathBuf, usize> = FxHashMap::default();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // Initialize: count dependencies for each node
        for (path, node) in &self.nodes {
            // Filter to only count imports that are in the graph
            let dep_count = node
                .imports
                .iter()
                .filter(|i| self.nodes.contains_key(*i))
                .count();
            in_degree.insert(path, dep_count);
        }

        // Find nodes with no dependencies (can compile first)
        // Sort paths for deterministic ordering when multiple nodes have zero in-degree
        let mut zero_degree: Vec<&PathBuf> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(path, _)| *path)
            .collect();
        zero_degree.sort();
        for path in zero_degree {
            queue.push_back(path);
        }

        // Process nodes in order
        while let Some(path) = queue.pop_front() {
            result.push(path.clone());

            // When we complete this file, files that depend on it can decrement their count
            if let Some(deps) = self.dependents.get(path) {
                // Collect newly ready deps and sort for determinism
                let mut newly_ready: Vec<&PathBuf> = Vec::new();
                for dep in deps {
                    if let Some(degree) = in_degree.get_mut(dep) {
                        *degree -= 1;
                        if *degree == 0 {
                            newly_ready.push(dep);
                        }
                    }
                }
                // Sort for deterministic ordering
                newly_ready.sort();
                for dep in newly_ready {
                    queue.push_back(dep);
                }
            }
        }

        // Check for cycles
        if result.len() == self.nodes.len() {
            Some(result)
        } else {
            None // Cycle detected
        }
    }

    /// Find files that need recompilation when the given files change.
    ///
    /// Returns all changed files plus all their transitive dependents.
    #[must_use]
    pub fn files_to_recompile(&self, changed: &[PathBuf]) -> FxHashSet<PathBuf> {
        let mut result = FxHashSet::default();

        for path in changed {
            result.insert(path.clone());
            result.extend(self.transitive_dependents(path));
        }

        result
    }
}

/// Tracks dependencies for incremental compilation.
pub struct DependencyTracker {
    /// The dependency graph.
    graph: DependencyGraph,
    /// Cache directory for storing dependency info.
    cache_dir: PathBuf,
}

impl DependencyTracker {
    /// Create a new dependency tracker.
    #[must_use]
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            graph: DependencyGraph::new(),
            cache_dir,
        }
    }

    /// Get a reference to the dependency graph.
    #[must_use]
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    /// Get a mutable reference to the dependency graph.
    pub fn graph_mut(&mut self) -> &mut DependencyGraph {
        &mut self.graph
    }

    /// Register a file with its imports.
    pub fn register(&mut self, path: PathBuf, hash: ContentHash, imports: Vec<PathBuf>) {
        self.graph.add_file(path, hash, imports);
    }

    /// Get the cache directory.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Compute files that need recompilation.
    #[must_use]
    pub fn needs_recompilation(&self, changed: &[PathBuf]) -> FxHashSet<PathBuf> {
        self.graph.files_to_recompile(changed)
    }

    /// Get a topological ordering for compilation.
    pub fn compilation_order(&self) -> Option<Vec<PathBuf>> {
        self.graph.topological_order()
    }
}

/// Error during dependency tracking.
#[derive(Debug, Clone)]
pub enum DependencyError {
    /// Circular dependency detected.
    CyclicDependency { cycle: Vec<PathBuf> },
    /// Failed to read dependency file.
    IoError { path: PathBuf, message: String },
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CyclicDependency { cycle } => {
                write!(f, "circular dependency detected: ")?;
                for (i, path) in cycle.iter().enumerate() {
                    if i > 0 {
                        write!(f, " -> ")?;
                    }
                    write!(f, "{}", path.display())?;
                }
                Ok(())
            }
            Self::IoError { path, message } => {
                write!(f, "failed to read '{}': {}", path.display(), message)
            }
        }
    }
}

impl std::error::Error for DependencyError {}

#[cfg(test)]
mod tests;
