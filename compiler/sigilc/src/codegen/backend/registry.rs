// Backend Registry for Sigil compiler
//
// Provides global registration and discovery of code generation backends.

use super::traits::Backend;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

/// Factory function for creating backends.
type BackendFactory = Box<dyn Fn() -> Box<dyn Backend> + Send + Sync>;

/// Entry in the backend registry.
struct BackendEntry {
    name: &'static str,
    description: &'static str,
    factory: BackendFactory,
}

/// Global backend registry.
pub struct BackendRegistry {
    backends: HashMap<&'static str, BackendEntry>,
    default: Option<&'static str>,
}

impl BackendRegistry {
    fn new() -> Self {
        BackendRegistry {
            backends: HashMap::new(),
            default: None,
        }
    }

    /// Register a backend.
    pub fn register<B, F>(&mut self, factory: F)
    where
        B: Backend + Default + 'static,
        F: Fn() -> B + Send + Sync + 'static,
    {
        let sample = factory();
        let name = sample.name();
        let description = sample.description();

        self.backends.insert(
            name,
            BackendEntry {
                name,
                description,
                factory: Box::new(move || Box::new(factory())),
            },
        );
    }

    /// Register a backend with explicit metadata.
    pub fn register_with_meta(
        &mut self,
        name: &'static str,
        description: &'static str,
        factory: impl Fn() -> Box<dyn Backend> + Send + Sync + 'static,
    ) {
        self.backends.insert(
            name,
            BackendEntry {
                name,
                description,
                factory: Box::new(factory),
            },
        );
    }

    /// Set the default backend.
    pub fn set_default(&mut self, name: &'static str) {
        if self.backends.contains_key(name) {
            self.default = Some(name);
        }
    }

    /// Get the default backend name.
    pub fn default_name(&self) -> Option<&'static str> {
        self.default
    }

    /// Get a backend by name.
    pub fn get(&self, name: &str) -> Option<Box<dyn Backend>> {
        self.backends.get(name).map(|entry| (entry.factory)())
    }

    /// Get the default backend.
    pub fn get_default(&self) -> Option<Box<dyn Backend>> {
        self.default.and_then(|name| self.get(name))
    }

    /// Check if a backend is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.backends.contains_key(name)
    }

    /// Get all registered backend names.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.backends.keys().copied()
    }

    /// Get information about all backends.
    pub fn all_info(&self) -> Vec<BackendInfo> {
        self.backends
            .values()
            .map(|entry| BackendInfo {
                name: entry.name,
                description: entry.description,
                is_default: self.default == Some(entry.name),
            })
            .collect()
    }
}

/// Information about a registered backend.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub is_default: bool,
}

/// Global registry instance.
static REGISTRY: LazyLock<RwLock<BackendRegistry>> = LazyLock::new(|| {
    let mut registry = BackendRegistry::new();

    // Register the C backend
    registry.register_with_meta("c", "C code generation backend", || {
        Box::new(super::c::CBackend::new())
    });

    // Set C as default
    registry.set_default("c");

    RwLock::new(registry)
});

/// Get a reference to the global registry.
pub fn registry() -> &'static RwLock<BackendRegistry> {
    &REGISTRY
}

/// Get a backend by name.
pub fn get_backend(name: &str) -> Option<Box<dyn Backend>> {
    REGISTRY.read().ok()?.get(name)
}

/// Get the default backend.
pub fn get_default_backend() -> Option<Box<dyn Backend>> {
    REGISTRY.read().ok()?.get_default()
}

/// Check if a backend is registered.
pub fn has_backend(name: &str) -> bool {
    REGISTRY
        .read()
        .map(|reg| reg.contains(name))
        .unwrap_or(false)
}

/// Get all registered backend names.
pub fn backend_names() -> Vec<&'static str> {
    REGISTRY
        .read()
        .map(|reg| reg.names().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_c_backend() {
        assert!(has_backend("c"));
    }

    #[test]
    fn test_registry_get_c_backend() {
        let backend = get_backend("c");
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name(), "c");
    }

    #[test]
    fn test_registry_default_is_c() {
        if let Ok(reg) = registry().read() {
            assert_eq!(reg.default_name(), Some("c"));
        }
    }

    #[test]
    fn test_registry_get_default() {
        let backend = get_default_backend();
        assert!(backend.is_some());
        assert_eq!(backend.unwrap().name(), "c");
    }

    #[test]
    fn test_backend_names() {
        let names = backend_names();
        assert!(names.contains(&"c"));
    }
}
