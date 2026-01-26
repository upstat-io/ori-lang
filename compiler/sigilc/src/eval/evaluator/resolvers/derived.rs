//! Derived method resolver.
//!
//! Resolves methods generated from `#[derive(...)]` attributes.

use crate::context::SharedMutableRegistry;
use sigil_eval::UserMethodRegistry;
use super::{MethodResolution, MethodResolver, Value};

/// Resolver for derived methods from `#[derive(...)]` attributes.
///
/// Priority 1 - derived methods are checked after user-defined methods
/// but before collection methods.
///
/// Uses `SharedMutableRegistry` so that methods registered after the dispatcher
/// is created are still visible.
pub struct DerivedMethodResolver {
    registry: SharedMutableRegistry<UserMethodRegistry>,
}

impl DerivedMethodResolver {
    /// Create a new resolver with the given registry.
    pub fn new(registry: SharedMutableRegistry<UserMethodRegistry>) -> Self {
        Self { registry }
    }
}

impl MethodResolver for DerivedMethodResolver {
    fn resolve(&self, _receiver: &Value, type_name: &str, method_name: &str) -> MethodResolution {
        if let Some(derived_info) = self.registry.read().lookup_derived(type_name, method_name) {
            MethodResolution::Derived(derived_info.clone())
        } else {
            MethodResolution::NotFound
        }
    }

    fn priority(&self) -> u8 {
        1
    }

    fn name(&self) -> &'static str {
        "DerivedMethodResolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority() {
        let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
        let resolver = DerivedMethodResolver::new(registry);
        assert_eq!(resolver.priority(), 1);
    }

    #[test]
    fn test_not_found_for_missing_method() {
        let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
        let resolver = DerivedMethodResolver::new(registry);

        let result = resolver.resolve(&Value::Int(42), "int", "unknown_method");
        assert!(matches!(result, MethodResolution::NotFound));
    }
}
