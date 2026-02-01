//! Dependency Tracking for Incremental Compilation
//!
//! Tracks import relationships between source files to determine
//! what needs recompilation when a file changes.

use std::collections::{HashMap, HashSet, VecDeque};
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
    nodes: HashMap<PathBuf, DependencyNode>,
    /// Reverse dependency map: file -> files that import it.
    dependents: HashMap<PathBuf, HashSet<PathBuf>>,
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
        if let Some(old_node) = self.nodes.get(&path) {
            for old_import in &old_node.imports {
                if !imports.contains(old_import) {
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
    pub fn get_dependents(&self, path: &Path) -> Option<&HashSet<PathBuf>> {
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
    pub fn transitive_dependencies(&self, path: &Path) -> HashSet<PathBuf> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(node) = self.nodes.get(path) {
            for import in &node.imports {
                queue.push_back(import.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            if visited.insert(current.clone()) {
                if let Some(node) = self.nodes.get(&current) {
                    for import in &node.imports {
                        if !visited.contains(import) {
                            queue.push_back(import.clone());
                        }
                    }
                }
            }
        }

        visited
    }

    /// Compute the transitive closure of dependents for a file.
    ///
    /// Returns all files that depend on this file, directly or indirectly.
    #[must_use]
    pub fn transitive_dependents(&self, path: &Path) -> HashSet<PathBuf> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(deps) = self.dependents.get(path) {
            for dep in deps {
                queue.push_back(dep.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            if visited.insert(current.clone()) {
                if let Some(deps) = self.dependents.get(&current) {
                    for dep in deps {
                        if !visited.contains(dep) {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        visited
    }

    /// Compute a topological ordering for compilation.
    ///
    /// Files are ordered so that dependencies come before dependents.
    /// Returns None if there's a cycle.
    pub fn topological_order(&self) -> Option<Vec<PathBuf>> {
        // Count how many dependencies each node has (in-degree in dependency graph)
        let mut in_degree: HashMap<&PathBuf, usize> = HashMap::new();
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
        for (path, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(*path);
            }
        }

        // Process nodes in order
        while let Some(path) = queue.pop_front() {
            result.push(path.clone());

            // When we complete this file, files that depend on it can decrement their count
            if let Some(deps) = self.dependents.get(path) {
                for dep in deps {
                    if let Some(degree) = in_degree.get_mut(dep) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dep);
                        }
                    }
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
    pub fn files_to_recompile(&self, changed: &[PathBuf]) -> HashSet<PathBuf> {
        let mut result = HashSet::new();

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
    pub fn needs_recompilation(&self, changed: &[PathBuf]) -> HashSet<PathBuf> {
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
mod tests {
    use super::*;

    fn h(n: u64) -> ContentHash {
        ContentHash::new(n)
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn test_add_and_get_imports() {
        let mut graph = DependencyGraph::new();

        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori"), p("utils.ori")]);

        let imports = graph.get_imports(Path::new("main.ori")).unwrap();
        assert_eq!(imports.len(), 2);
        assert!(imports.contains(&p("lib.ori")));
        assert!(imports.contains(&p("utils.ori")));
    }

    #[test]
    fn test_get_dependents() {
        let mut graph = DependencyGraph::new();

        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("tests.ori"), h(2), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(3), vec![]);

        let dependents = graph.get_dependents(Path::new("lib.ori")).unwrap();
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&p("main.ori")));
        assert!(dependents.contains(&p("tests.ori")));
    }

    #[test]
    fn test_transitive_dependencies() {
        let mut graph = DependencyGraph::new();

        // main -> lib -> utils -> core
        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
        graph.add_file(p("utils.ori"), h(3), vec![p("core.ori")]);
        graph.add_file(p("core.ori"), h(4), vec![]);

        let deps = graph.transitive_dependencies(Path::new("main.ori"));
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&p("lib.ori")));
        assert!(deps.contains(&p("utils.ori")));
        assert!(deps.contains(&p("core.ori")));
    }

    #[test]
    fn test_transitive_dependents() {
        let mut graph = DependencyGraph::new();

        // main -> lib -> utils
        // test -> lib
        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("test.ori"), h(2), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(3), vec![p("utils.ori")]);
        graph.add_file(p("utils.ori"), h(4), vec![]);

        // Changes to utils should trigger recompilation of lib, main, test
        let deps = graph.transitive_dependents(Path::new("utils.ori"));
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&p("lib.ori")));
        assert!(deps.contains(&p("main.ori")));
        assert!(deps.contains(&p("test.ori")));
    }

    #[test]
    fn test_topological_order() {
        let mut graph = DependencyGraph::new();

        // main -> lib -> utils
        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
        graph.add_file(p("utils.ori"), h(3), vec![]);

        let order = graph.topological_order().unwrap();
        assert_eq!(order.len(), 3);

        // utils must come before lib, lib must come before main
        let utils_pos = order.iter().position(|x| x == &p("utils.ori")).unwrap();
        let lib_pos = order.iter().position(|x| x == &p("lib.ori")).unwrap();
        let main_pos = order.iter().position(|x| x == &p("main.ori")).unwrap();

        assert!(utils_pos < lib_pos);
        assert!(lib_pos < main_pos);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph::new();

        // a -> b -> c -> a (cycle)
        graph.add_file(p("a.ori"), h(1), vec![p("b.ori")]);
        graph.add_file(p("b.ori"), h(2), vec![p("c.ori")]);
        graph.add_file(p("c.ori"), h(3), vec![p("a.ori")]);

        assert!(graph.topological_order().is_none());
    }

    #[test]
    fn test_files_to_recompile() {
        let mut graph = DependencyGraph::new();

        // main -> lib -> utils
        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(2), vec![p("utils.ori")]);
        graph.add_file(p("utils.ori"), h(3), vec![]);

        // Changing utils should require recompiling utils, lib, main
        let to_recompile = graph.files_to_recompile(&[p("utils.ori")]);
        assert_eq!(to_recompile.len(), 3);
        assert!(to_recompile.contains(&p("utils.ori")));
        assert!(to_recompile.contains(&p("lib.ori")));
        assert!(to_recompile.contains(&p("main.ori")));
    }

    #[test]
    fn test_remove_file() {
        let mut graph = DependencyGraph::new();

        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(2), vec![]);

        graph.remove_file(Path::new("main.ori"));

        assert!(!graph.contains(Path::new("main.ori")));
        assert!(graph
            .get_dependents(Path::new("lib.ori"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_update_imports() {
        let mut graph = DependencyGraph::new();

        // Initially main imports lib
        graph.add_file(p("main.ori"), h(1), vec![p("lib.ori")]);
        graph.add_file(p("lib.ori"), h(2), vec![]);
        graph.add_file(p("utils.ori"), h(3), vec![]);

        assert!(graph
            .get_dependents(Path::new("lib.ori"))
            .unwrap()
            .contains(&p("main.ori")));

        // Update: main now imports utils instead
        graph.add_file(p("main.ori"), h(1), vec![p("utils.ori")]);

        // lib should no longer have main as dependent
        assert!(!graph
            .get_dependents(Path::new("lib.ori"))
            .unwrap()
            .contains(&p("main.ori")));
        // utils should now have main as dependent
        assert!(graph
            .get_dependents(Path::new("utils.ori"))
            .unwrap()
            .contains(&p("main.ori")));
    }

    #[test]
    fn test_dependency_tracker() {
        let tracker = DependencyTracker::new(PathBuf::from("/tmp/cache"));

        assert_eq!(tracker.cache_dir(), Path::new("/tmp/cache"));
        assert!(tracker.graph().is_empty());
    }

    #[test]
    fn test_dependency_error_display() {
        let err = DependencyError::CyclicDependency {
            cycle: vec![p("a.ori"), p("b.ori"), p("a.ori")],
        };
        let msg = err.to_string();
        assert!(msg.contains("circular dependency"));
        assert!(msg.contains("a.ori"));
        assert!(msg.contains("b.ori"));

        let err = DependencyError::IoError {
            path: p("/test.ori"),
            message: "not found".to_string(),
        };
        assert!(err.to_string().contains("/test.ori"));
    }

    #[test]
    fn test_graph_len_and_empty() {
        let mut graph = DependencyGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);

        graph.add_file(p("a.ori"), h(1), vec![]);
        assert!(!graph.is_empty());
        assert_eq!(graph.len(), 1);
    }
}
