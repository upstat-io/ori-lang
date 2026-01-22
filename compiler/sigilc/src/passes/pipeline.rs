// Pass Pipeline Builder for Sigil compiler
//
// Provides a fluent API for constructing pass pipelines.
// Handles dependency resolution and topological sorting.

use super::{Pass, PassError, PassManager};
use std::collections::{HashMap, HashSet, VecDeque};

/// Builder for constructing pass pipelines.
///
/// # Example
/// ```ignore
/// let pipeline = PassPipeline::new()
///     .add("constant_folding")
///     .add("dead_code")
///     .add("pattern_lowering")
///     .build()?;
/// ```
pub struct PassPipeline {
    /// Passes to include in the pipeline
    passes: Vec<String>,
    /// Explicitly disabled passes
    disabled: HashSet<String>,
    /// Custom passes (not from registry)
    custom: Vec<Box<dyn Pass>>,
    /// Whether to auto-add dependencies
    resolve_deps: bool,
}

impl Default for PassPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl PassPipeline {
    /// Create a new empty pipeline builder.
    pub fn new() -> Self {
        PassPipeline {
            passes: Vec::new(),
            disabled: HashSet::new(),
            custom: Vec::new(),
            resolve_deps: true,
        }
    }

    /// Create a pipeline with all default passes.
    pub fn default_passes() -> Self {
        PassPipeline::new()
            .add("constant_folding")
            .add("dead_code")
            .add("pattern_lowering")
    }

    /// Create a minimal pipeline with only required passes.
    pub fn minimal() -> Self {
        PassPipeline::new().add("pattern_lowering")
    }

    /// Add a pass by name (from registry).
    pub fn add(mut self, name: &str) -> Self {
        self.passes.push(name.to_string());
        self
    }

    /// Add multiple passes by name.
    pub fn add_all(mut self, names: &[&str]) -> Self {
        for name in names {
            self.passes.push(name.to_string());
        }
        self
    }

    /// Disable a pass by name.
    pub fn disable(mut self, name: &str) -> Self {
        self.disabled.insert(name.to_string());
        self
    }

    /// Enable a previously disabled pass.
    pub fn enable(mut self, name: &str) -> Self {
        self.disabled.remove(name);
        self
    }

    /// Add a custom pass instance.
    pub fn add_custom<P: Pass + 'static>(mut self, pass: P) -> Self {
        self.custom.push(Box::new(pass));
        self
    }

    /// Set whether to automatically resolve dependencies.
    pub fn resolve_dependencies(mut self, resolve: bool) -> Self {
        self.resolve_deps = resolve;
        self
    }

    /// Build the pass manager from this pipeline.
    pub fn build(self) -> Result<PassManager, PassError> {
        let mut manager = PassManager::new();

        // Add passes from registry
        if let Ok(registry) = super::registry::registry().read() {
            // Collect all passes we need
            let mut needed: Vec<String> = self.passes.clone();

            // Resolve dependencies if requested
            if self.resolve_deps {
                needed = self.resolve_pass_dependencies(&needed, &registry)?;
            }

            // Add passes in order
            for name in &needed {
                if let Some(pass) = registry.get(name) {
                    manager.add_boxed(pass);
                } else {
                    return Err(PassError::new(
                        "pipeline",
                        format!("Unknown pass: {}", name),
                    ));
                }
            }
        }

        // Add custom passes
        for pass in self.custom {
            manager.add_boxed(pass);
        }

        // Apply disabled list
        for name in &self.disabled {
            manager.disable(name);
        }

        Ok(manager)
    }

    /// Resolve pass dependencies and return topologically sorted list.
    fn resolve_pass_dependencies(
        &self,
        passes: &[String],
        registry: &super::registry::PassRegistry,
    ) -> Result<Vec<String>, PassError> {
        // Build dependency graph
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_needed: HashSet<String> = passes.iter().cloned().collect();

        // Expand dependencies
        let mut queue: VecDeque<String> = passes.iter().cloned().collect();
        while let Some(name) = queue.pop_front() {
            if deps.contains_key(&name) {
                continue;
            }

            if let Some(info) = registry.info(&name) {
                let pass_deps: Vec<String> = info.requires.iter().map(|s| s.to_string()).collect();

                for dep in &pass_deps {
                    if !all_needed.contains(dep) {
                        all_needed.insert(dep.clone());
                        queue.push_back(dep.clone());
                    }
                }

                deps.insert(name, pass_deps);
            } else {
                return Err(PassError::new(
                    "pipeline",
                    format!("Unknown pass: {}", name),
                ));
            }
        }

        // Topological sort
        topological_sort(&all_needed, &deps)
    }
}

/// Topological sort of passes based on dependencies.
fn topological_sort(
    passes: &HashSet<String>,
    deps: &HashMap<String, Vec<String>>,
) -> Result<Vec<String>, PassError> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut in_progress = HashSet::new();

    fn visit(
        name: &str,
        deps: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        in_progress: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) -> Result<(), PassError> {
        if visited.contains(name) {
            return Ok(());
        }
        if in_progress.contains(name) {
            return Err(PassError::new(
                "pipeline",
                format!("Circular dependency detected involving pass: {}", name),
            ));
        }

        in_progress.insert(name.to_string());

        if let Some(pass_deps) = deps.get(name) {
            for dep in pass_deps {
                visit(dep, deps, visited, in_progress, result)?;
            }
        }

        in_progress.remove(name);
        visited.insert(name.to_string());
        result.push(name.to_string());

        Ok(())
    }

    for name in passes {
        visit(name, deps, &mut visited, &mut in_progress, &mut result)?;
    }

    Ok(result)
}

// Extend PassManager to support boxed passes
impl PassManager {
    /// Add a boxed pass to the manager.
    pub fn add_boxed(&mut self, pass: Box<dyn Pass>) {
        self.passes.push(pass);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_builder_empty() {
        let pipeline = PassPipeline::new();
        let manager = pipeline.build().unwrap();
        assert!(manager.is_empty());
    }

    #[test]
    fn test_pipeline_builder_add() {
        let pipeline = PassPipeline::new()
            .add("constant_folding")
            .add("pattern_lowering");

        let manager = pipeline.build().unwrap();
        assert!(manager.pass_names().contains(&"constant_folding"));
        assert!(manager.pass_names().contains(&"pattern_lowering"));
    }

    #[test]
    fn test_pipeline_builder_default() {
        let pipeline = PassPipeline::default_passes();
        let manager = pipeline.build().unwrap();

        assert!(manager.pass_names().contains(&"constant_folding"));
        assert!(manager.pass_names().contains(&"dead_code"));
        assert!(manager.pass_names().contains(&"pattern_lowering"));
    }

    #[test]
    fn test_pipeline_builder_minimal() {
        let pipeline = PassPipeline::minimal();
        let manager = pipeline.build().unwrap();

        assert_eq!(manager.len(), 1);
        assert!(manager.pass_names().contains(&"pattern_lowering"));
    }

    #[test]
    fn test_pipeline_builder_disable() {
        let pipeline = PassPipeline::default_passes().disable("constant_folding");

        let manager = pipeline.build().unwrap();
        assert!(!manager.is_enabled("constant_folding"));
    }

    #[test]
    fn test_topological_sort_no_deps() {
        let passes: HashSet<String> = vec!["a".to_string(), "b".to_string(), "c".to_string()]
            .into_iter()
            .collect();
        let deps: HashMap<String, Vec<String>> = vec![
            ("a".to_string(), vec![]),
            ("b".to_string(), vec![]),
            ("c".to_string(), vec![]),
        ]
        .into_iter()
        .collect();

        let result = topological_sort(&passes, &deps).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_topological_sort_with_deps() {
        let passes: HashSet<String> = vec!["a".to_string(), "b".to_string(), "c".to_string()]
            .into_iter()
            .collect();
        let deps: HashMap<String, Vec<String>> = vec![
            ("a".to_string(), vec![]),
            ("b".to_string(), vec!["a".to_string()]),
            ("c".to_string(), vec!["b".to_string()]),
        ]
        .into_iter()
        .collect();

        let result = topological_sort(&passes, &deps).unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_topological_sort_cycle_detection() {
        let passes: HashSet<String> = vec!["a".to_string(), "b".to_string()].into_iter().collect();
        let deps: HashMap<String, Vec<String>> = vec![
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["a".to_string()]),
        ]
        .into_iter()
        .collect();

        let result = topological_sort(&passes, &deps);
        assert!(result.is_err());
    }
}
