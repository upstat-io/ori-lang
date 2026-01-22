//! Module dependency graph for parallel type checking.
//!
//! This module builds a dependency graph from parsed modules and provides
//! topological ordering for level-based parallel type checking.

use std::collections::{HashMap, HashSet, VecDeque};
use rayon::prelude::*;

use super::ParsedFile;

/// A node in the dependency graph representing a module.
#[derive(Clone, Debug)]
pub struct ModuleNode {
    /// Module name.
    pub name: String,
    /// Index in the original module list.
    pub index: usize,
    /// Modules this module depends on (imports).
    pub dependencies: HashSet<String>,
    /// Modules that depend on this module.
    pub dependents: HashSet<String>,
    /// Dependency level (0 = no dependencies).
    pub level: usize,
}

impl ModuleNode {
    /// Create a new module node.
    pub fn new(name: String, index: usize) -> Self {
        ModuleNode {
            name,
            index,
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            level: 0,
        }
    }

    /// Check if this module has no dependencies.
    pub fn is_leaf(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Get the in-degree (number of dependencies).
    pub fn in_degree(&self) -> usize {
        self.dependencies.len()
    }

    /// Get the out-degree (number of dependents).
    pub fn out_degree(&self) -> usize {
        self.dependents.len()
    }
}

/// A level of modules that can be processed in parallel.
#[derive(Clone, Debug)]
pub struct DependencyLevel {
    /// Level number (0 = no dependencies).
    pub level: usize,
    /// Module names at this level.
    pub modules: Vec<String>,
}

impl DependencyLevel {
    /// Create a new dependency level.
    pub fn new(level: usize) -> Self {
        DependencyLevel {
            level,
            modules: Vec::new(),
        }
    }

    /// Check if the level is empty.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Get the number of modules at this level.
    pub fn len(&self) -> usize {
        self.modules.len()
    }
}

/// Dependency graph for modules.
#[derive(Clone, Debug)]
pub struct DependencyGraph {
    /// Map from module name to node.
    nodes: HashMap<String, ModuleNode>,
    /// Levels for parallel processing.
    levels: Vec<DependencyLevel>,
    /// Whether the graph has cycles.
    has_cycles: bool,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn new() -> Self {
        DependencyGraph {
            nodes: HashMap::new(),
            levels: Vec::new(),
            has_cycles: false,
        }
    }

    /// Build a dependency graph from parsed modules.
    pub fn from_modules(modules: &[ParsedFile]) -> Self {
        let mut graph = DependencyGraph::new();

        // Add all modules as nodes
        for (index, module) in modules.iter().enumerate() {
            let mut node = ModuleNode::new(module.module_name.clone(), index);

            // Add dependencies from imports
            for import in &module.imports {
                // Extract module name from import path
                let dep_name = Self::import_to_module_name(import);
                node.dependencies.insert(dep_name);
            }

            graph.nodes.insert(module.module_name.clone(), node);
        }

        // Build reverse dependencies (dependents)
        let node_names: Vec<_> = graph.nodes.keys().cloned().collect();
        for name in &node_names {
            let deps: Vec<_> = graph.nodes[name].dependencies.iter().cloned().collect();
            for dep in deps {
                if let Some(dep_node) = graph.nodes.get_mut(&dep) {
                    dep_node.dependents.insert(name.clone());
                }
            }
        }

        // Compute levels using topological sort
        graph.compute_levels();

        graph
    }

    /// Convert an import path to a module name.
    fn import_to_module_name(import: &str) -> String {
        // Handle both relative imports ('./math') and absolute ('std.math')
        if import.starts_with("./") || import.starts_with("../") {
            // Relative import: extract last component
            import
                .rsplit('/')
                .next()
                .unwrap_or(import)
                .to_string()
        } else {
            // Absolute import: use last component
            import
                .rsplit('.')
                .next()
                .unwrap_or(import)
                .to_string()
        }
    }

    /// Compute dependency levels using Kahn's algorithm.
    fn compute_levels(&mut self) {
        let mut in_degrees: HashMap<String, usize> = self
            .nodes
            .iter()
            .map(|(name, node)| {
                // Only count dependencies that exist in the graph
                let valid_deps = node
                    .dependencies
                    .iter()
                    .filter(|d| self.nodes.contains_key(*d))
                    .count();
                (name.clone(), valid_deps)
            })
            .collect();

        let mut queue: VecDeque<String> = in_degrees
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut current_level = 0;
        let mut processed = 0;

        while !queue.is_empty() {
            let mut level = DependencyLevel::new(current_level);

            // Process all modules at current level
            let level_size = queue.len();
            for _ in 0..level_size {
                if let Some(name) = queue.pop_front() {
                    // Set the level for this node
                    if let Some(node) = self.nodes.get_mut(&name) {
                        node.level = current_level;
                    }

                    level.modules.push(name.clone());
                    processed += 1;

                    // Decrease in-degree for dependents
                    if let Some(node) = self.nodes.get(&name) {
                        for dependent in &node.dependents {
                            if let Some(deg) = in_degrees.get_mut(dependent) {
                                *deg = deg.saturating_sub(1);
                                if *deg == 0 {
                                    queue.push_back(dependent.clone());
                                }
                            }
                        }
                    }
                }
            }

            if !level.is_empty() {
                self.levels.push(level);
            }
            current_level += 1;
        }

        // Check for cycles
        self.has_cycles = processed < self.nodes.len();
    }

    /// Get all nodes in the graph.
    pub fn nodes(&self) -> &HashMap<String, ModuleNode> {
        &self.nodes
    }

    /// Get a specific node by name.
    pub fn get_node(&self, name: &str) -> Option<&ModuleNode> {
        self.nodes.get(name)
    }

    /// Get the dependency levels.
    pub fn levels(&self) -> &[DependencyLevel] {
        &self.levels
    }

    /// Get the number of levels.
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Check if the graph has cycles.
    pub fn has_cycles(&self) -> bool {
        self.has_cycles
    }

    /// Get modules at a specific level.
    pub fn modules_at_level(&self, level: usize) -> Option<&[String]> {
        self.levels.get(level).map(|l| l.modules.as_slice())
    }

    /// Get the critical path length (maximum level).
    pub fn critical_path(&self) -> usize {
        self.levels.len()
    }

    /// Get the maximum parallelism (max modules at any level).
    pub fn max_parallelism(&self) -> usize {
        self.levels.iter().map(|l| l.len()).max().unwrap_or(0)
    }

    /// Iterate over levels in order.
    pub fn iter_levels(&self) -> impl Iterator<Item = &DependencyLevel> {
        self.levels.iter()
    }

    /// Get modules in topological order.
    pub fn topological_order(&self) -> Vec<&str> {
        self.levels
            .iter()
            .flat_map(|level| level.modules.iter().map(|s| s.as_str()))
            .collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::syntax::ExprArena;

    fn make_parsed_file(name: &str, imports: Vec<&str>) -> ParsedFile {
        ParsedFile {
            path: PathBuf::from(format!("{}.si", name)),
            items: Vec::new(),
            arena: ExprArena::new(),
            errors: Vec::new(),
            success: true,
            module_name: name.to_string(),
            imports: imports.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_empty_graph() {
        let graph = DependencyGraph::new();
        assert!(graph.nodes().is_empty());
        assert_eq!(graph.level_count(), 0);
        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_single_module() {
        let modules = vec![make_parsed_file("main", vec![])];
        let graph = DependencyGraph::from_modules(&modules);

        assert_eq!(graph.nodes().len(), 1);
        assert_eq!(graph.level_count(), 1);
        assert!(!graph.has_cycles());
        assert_eq!(graph.modules_at_level(0), Some(&["main".to_string()][..]));
    }

    #[test]
    fn test_linear_dependencies() {
        // a -> b -> c (c depends on b, b depends on a)
        let modules = vec![
            make_parsed_file("a", vec![]),
            make_parsed_file("b", vec!["./a"]),
            make_parsed_file("c", vec!["./b"]),
        ];
        let graph = DependencyGraph::from_modules(&modules);

        assert_eq!(graph.level_count(), 3);
        assert!(!graph.has_cycles());
        assert_eq!(graph.critical_path(), 3);
        assert_eq!(graph.max_parallelism(), 1);

        // Check levels
        assert!(graph.modules_at_level(0).unwrap().contains(&"a".to_string()));
        assert!(graph.modules_at_level(1).unwrap().contains(&"b".to_string()));
        assert!(graph.modules_at_level(2).unwrap().contains(&"c".to_string()));
    }

    #[test]
    fn test_parallel_modules() {
        // a, b, c all have no dependencies -> level 0
        // d depends on a, b, c -> level 1
        let modules = vec![
            make_parsed_file("a", vec![]),
            make_parsed_file("b", vec![]),
            make_parsed_file("c", vec![]),
            make_parsed_file("d", vec!["./a", "./b", "./c"]),
        ];
        let graph = DependencyGraph::from_modules(&modules);

        assert_eq!(graph.level_count(), 2);
        assert!(!graph.has_cycles());
        assert_eq!(graph.max_parallelism(), 3);

        let level0 = graph.modules_at_level(0).unwrap();
        assert_eq!(level0.len(), 3);
    }

    #[test]
    fn test_diamond_dependency() {
        //     a
        //    / \
        //   b   c
        //    \ /
        //     d
        let modules = vec![
            make_parsed_file("a", vec![]),
            make_parsed_file("b", vec!["./a"]),
            make_parsed_file("c", vec!["./a"]),
            make_parsed_file("d", vec!["./b", "./c"]),
        ];
        let graph = DependencyGraph::from_modules(&modules);

        assert_eq!(graph.level_count(), 3);
        assert!(!graph.has_cycles());

        // a at level 0, b and c at level 1, d at level 2
        assert!(graph.modules_at_level(0).unwrap().contains(&"a".to_string()));
        let level1 = graph.modules_at_level(1).unwrap();
        assert!(level1.contains(&"b".to_string()));
        assert!(level1.contains(&"c".to_string()));
        assert!(graph.modules_at_level(2).unwrap().contains(&"d".to_string()));
    }

    #[test]
    fn test_import_to_module_name() {
        assert_eq!(DependencyGraph::import_to_module_name("./math"), "math");
        assert_eq!(DependencyGraph::import_to_module_name("../utils"), "utils");
        assert_eq!(DependencyGraph::import_to_module_name("std.math"), "math");
        assert_eq!(DependencyGraph::import_to_module_name("std.collections.list"), "list");
    }

    #[test]
    fn test_topological_order() {
        let modules = vec![
            make_parsed_file("a", vec![]),
            make_parsed_file("b", vec!["./a"]),
            make_parsed_file("c", vec!["./b"]),
        ];
        let graph = DependencyGraph::from_modules(&modules);

        let order = graph.topological_order();
        assert_eq!(order.len(), 3);

        // a must come before b, b must come before c
        let a_pos = order.iter().position(|&x| x == "a").unwrap();
        let b_pos = order.iter().position(|&x| x == "b").unwrap();
        let c_pos = order.iter().position(|&x| x == "c").unwrap();
        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }
}
