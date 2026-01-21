// Pass manager for Sigil compiler
// Handles pass registration, ordering, and execution

use super::{
    ConstantFoldingPass, DeadCodePass, Pass, PassContext, PassError, PatternLoweringPass,
};
use crate::ir::{dump_tir, TModule};
use std::collections::HashSet;
use std::time::Instant;

/// Manages and runs compiler passes
pub struct PassManager {
    pub(crate) passes: Vec<Box<dyn Pass>>,
    disabled: HashSet<String>,
}

impl Default for PassManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PassManager {
    pub fn new() -> Self {
        PassManager {
            passes: Vec::new(),
            disabled: HashSet::new(),
        }
    }

    /// Create a pass manager with the default pipeline
    pub fn default_pipeline() -> Self {
        let mut pm = PassManager::new();

        // Add passes in order
        pm.add(ConstantFoldingPass); // Optional: fold constants
        pm.add(DeadCodePass); // Optional: mark unreachable code
        pm.add(PatternLoweringPass); // Required: patterns → loops

        pm
    }

    /// Create a minimal pass manager with only required passes
    pub fn minimal() -> Self {
        let mut pm = PassManager::new();
        pm.add(PatternLoweringPass); // Required: patterns → loops
        pm
    }

    /// Add a pass to the manager
    pub fn add<P: Pass + 'static>(&mut self, pass: P) {
        self.passes.push(Box::new(pass));
    }

    /// Disable a pass by name
    pub fn disable(&mut self, name: &str) {
        self.disabled.insert(name.to_string());
    }

    /// Enable a previously disabled pass
    pub fn enable(&mut self, name: &str) {
        self.disabled.remove(name);
    }

    /// Check if a pass is enabled
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.contains(name)
    }

    /// Run all passes on the module
    pub fn run(&self, ir: &mut TModule, ctx: &mut PassContext) -> Result<(), PassError> {
        // Check pass dependencies
        self.verify_dependencies()?;

        for pass in &self.passes {
            let name = pass.name();

            // Skip disabled passes (unless required)
            if self.disabled.contains(name) {
                if pass.required() {
                    return Err(PassError::new(
                        name,
                        "Cannot disable required pass",
                    ));
                }
                if ctx.debug.verbose {
                    eprintln!("[pass] Skipping disabled pass: {}", name);
                }
                continue;
            }

            if ctx.debug.verbose {
                eprintln!("[pass] Running: {}", name);
            }

            let start = Instant::now();
            let mut result = pass.run(ir, ctx)?;
            result.stats.duration = start.elapsed();

            if ctx.debug.print_timing {
                eprintln!(
                    "[pass] {} completed in {:?} (changed: {}, items: {})",
                    name,
                    result.stats.duration,
                    result.changed,
                    result.stats.items_transformed
                );
            }

            if ctx.debug.dump_after_each && result.changed {
                eprintln!("=== After {} ===", name);
                eprintln!("{}", dump_tir(ir));
                eprintln!("================");
            }

            // Invalidate call graph if the pass made changes
            if result.changed {
                ctx.invalidate_call_graph();
            }
        }

        Ok(())
    }

    /// Verify that all pass dependencies are satisfied
    fn verify_dependencies(&self) -> Result<(), PassError> {
        let pass_names: HashSet<_> = self.passes.iter().map(|p| p.name()).collect();
        let mut seen = HashSet::new();

        for pass in &self.passes {
            for req in pass.requires() {
                // Check that required pass exists
                if !pass_names.contains(req) {
                    return Err(PassError::new(
                        pass.name(),
                        format!("Required pass '{}' not found in pipeline", req),
                    ));
                }
                // Check that required pass comes before this one
                if !seen.contains(req) {
                    return Err(PassError::new(
                        pass.name(),
                        format!("Required pass '{}' must run before '{}'", req, pass.name()),
                    ));
                }
            }
            seen.insert(pass.name());
        }

        Ok(())
    }

    /// Get the number of passes
    pub fn len(&self) -> usize {
        self.passes.len()
    }

    /// Check if the manager has no passes
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Get pass names
    pub fn pass_names(&self) -> Vec<&'static str> {
        self.passes.iter().map(|p| p.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_manager_new() {
        let pm = PassManager::new();
        assert!(pm.is_empty());
    }

    #[test]
    fn test_pass_manager_default_pipeline() {
        let pm = PassManager::default_pipeline();
        assert!(!pm.is_empty());
        assert!(pm.pass_names().contains(&"pattern_lowering"));
    }

    #[test]
    fn test_pass_manager_minimal() {
        let pm = PassManager::minimal();
        assert_eq!(pm.len(), 1);
        assert!(pm.pass_names().contains(&"pattern_lowering"));
    }

    #[test]
    fn test_pass_manager_disable() {
        let mut pm = PassManager::default_pipeline();
        assert!(pm.is_enabled("constant_folding"));
        pm.disable("constant_folding");
        assert!(!pm.is_enabled("constant_folding"));
        pm.enable("constant_folding");
        assert!(pm.is_enabled("constant_folding"));
    }
}
