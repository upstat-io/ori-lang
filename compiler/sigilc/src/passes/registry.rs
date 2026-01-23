// Pass Registry for Sigil compiler
//
// Provides global registration and discovery of passes.
// This enables plugins and extensions to register custom passes.

use super::Pass;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

/// Factory function for creating passes
type PassFactory = Box<dyn Fn() -> Box<dyn Pass> + Send + Sync>;

/// Entry in the pass registry
struct PassEntry {
    /// Name of the pass
    name: &'static str,
    /// Description of the pass
    description: &'static str,
    /// Factory function to create instances
    factory: PassFactory,
    /// Whether this pass is required (cannot be disabled)
    required: bool,
    /// Pass dependencies
    requires: Vec<&'static str>,
}

/// Global pass registry
pub struct PassRegistry {
    passes: HashMap<&'static str, PassEntry>,
}

impl PassRegistry {
    fn new() -> Self {
        PassRegistry {
            passes: HashMap::new(),
        }
    }

    /// Register a pass with the registry.
    pub fn register<P, F>(&mut self, factory: F)
    where
        P: Pass + Default + 'static,
        F: Fn() -> P + Send + Sync + 'static,
    {
        let sample = factory();
        let name = sample.name();
        let required = sample.required();
        let requires = sample.requires().to_vec();

        self.passes.insert(
            name,
            PassEntry {
                name,
                description: "", // Could add description method to Pass trait
                factory: Box::new(move || Box::new(factory())),
                required,
                requires,
            },
        );
    }

    /// Register a pass with explicit metadata.
    pub fn register_with_meta(
        &mut self,
        name: &'static str,
        description: &'static str,
        required: bool,
        requires: Vec<&'static str>,
        factory: impl Fn() -> Box<dyn Pass> + Send + Sync + 'static,
    ) {
        self.passes.insert(
            name,
            PassEntry {
                name,
                description,
                factory: Box::new(factory),
                required,
                requires,
            },
        );
    }

    /// Get a pass by name.
    pub fn get(&self, name: &str) -> Option<Box<dyn Pass>> {
        self.passes.get(name).map(|entry| (entry.factory)())
    }

    /// Check if a pass is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.passes.contains_key(name)
    }

    /// Get all registered pass names.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.passes.keys().copied()
    }

    /// Get information about a pass.
    pub fn info(&self, name: &str) -> Option<PassInfo> {
        self.passes.get(name).map(|entry| PassInfo {
            name: entry.name,
            description: entry.description,
            required: entry.required,
            requires: entry.requires.clone(),
        })
    }

    /// Get all pass info.
    pub fn all_info(&self) -> Vec<PassInfo> {
        self.passes
            .values()
            .map(|entry| PassInfo {
                name: entry.name,
                description: entry.description,
                required: entry.required,
                requires: entry.requires.clone(),
            })
            .collect()
    }
}

/// Information about a registered pass.
#[derive(Debug, Clone)]
pub struct PassInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
    pub requires: Vec<&'static str>,
}

/// Global registry instance
static REGISTRY: LazyLock<RwLock<PassRegistry>> = LazyLock::new(|| {
    let mut registry = PassRegistry::new();

    // Register built-in passes
    registry.register_with_meta(
        "constant_folding",
        "Fold constant expressions at compile time",
        false,
        vec![],
        || Box::new(super::ConstantFoldingPass),
    );

    registry.register_with_meta(
        "dead_code",
        "Detect and mark unreachable code",
        false,
        vec![],
        || Box::new(super::DeadCodePass),
    );

    registry.register_with_meta(
        "pattern_lowering",
        "Lower patterns to loops and conditionals",
        true,
        vec![],
        || Box::new(super::PatternLoweringPass),
    );

    registry.register_with_meta(
        "arc_insertion",
        "Analyze and prepare ARC memory management operations",
        true,
        vec!["pattern_lowering"],
        || Box::new(super::ArcInsertionPass::new()),
    );

    RwLock::new(registry)
});

/// Get a reference to the global registry.
pub fn registry() -> &'static RwLock<PassRegistry> {
    &REGISTRY
}

/// Register a pass with the global registry.
pub fn register_pass<P, F>(factory: F)
where
    P: Pass + Default + 'static,
    F: Fn() -> P + Send + Sync + 'static,
{
    if let Ok(mut reg) = REGISTRY.write() {
        reg.register::<P, F>(factory);
    }
}

/// Get a pass from the global registry.
pub fn get_pass(name: &str) -> Option<Box<dyn Pass>> {
    REGISTRY.read().ok()?.get(name)
}

/// Check if a pass is registered.
pub fn has_pass(name: &str) -> bool {
    REGISTRY
        .read()
        .map(|reg| reg.contains(name))
        .unwrap_or(false)
}

/// Get all registered pass names.
pub fn pass_names() -> Vec<&'static str> {
    REGISTRY
        .read()
        .map(|reg| reg.names().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_builtin_passes() {
        let names = pass_names();
        assert!(names.contains(&"constant_folding"));
        assert!(names.contains(&"dead_code"));
        assert!(names.contains(&"pattern_lowering"));
    }

    #[test]
    fn test_registry_get_pass() {
        let pass = get_pass("constant_folding");
        assert!(pass.is_some());
        assert_eq!(pass.unwrap().name(), "constant_folding");
    }

    #[test]
    fn test_registry_has_pass() {
        assert!(has_pass("pattern_lowering"));
        assert!(!has_pass("nonexistent_pass"));
    }

    #[test]
    fn test_registry_pass_info() {
        if let Ok(reg) = registry().read() {
            let info = reg.info("pattern_lowering").unwrap();
            assert_eq!(info.name, "pattern_lowering");
            assert!(info.required);
        }
    }
}
