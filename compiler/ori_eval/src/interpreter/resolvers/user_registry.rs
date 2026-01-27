//! Unified resolver for user-defined and derived methods.
//!
//! This resolver combines the functionality of what was previously separate
//! `UserMethodResolver` and `DerivedMethodResolver` into a single resolver,
//! reducing the number of resolvers in the chain.

use super::{MethodResolution, MethodResolver, Value};
use crate::{SharedMutableRegistry, UserMethodRegistry};
use ori_ir::Name;

/// Resolver for both user-defined and derived methods.
///
/// Priority 0 (highest) - these methods take precedence over all others.
///
/// Resolution order within this resolver:
/// 1. User-defined methods from impl blocks (checked first)
/// 2. Derived methods from `#[derive(...)]` (checked second)
///
/// Uses `SharedMutableRegistry` so that methods registered after the dispatcher
/// is created are still visible.
pub struct UserRegistryResolver {
    registry: SharedMutableRegistry<UserMethodRegistry>,
}

impl UserRegistryResolver {
    /// Create a new resolver with the given registry.
    pub fn new(registry: SharedMutableRegistry<UserMethodRegistry>) -> Self {
        Self { registry }
    }
}

impl MethodResolver for UserRegistryResolver {
    fn resolve(&self, _receiver: &Value, type_name: Name, method_name: Name) -> MethodResolution {
        let registry = self.registry.read();

        // Check user-defined methods first
        if let Some(user_method) = registry.lookup(type_name, method_name) {
            return MethodResolution::User(user_method.clone());
        }

        // Check derived methods second
        if let Some(derived_info) = registry.lookup_derived(type_name, method_name) {
            return MethodResolution::Derived(derived_info.clone());
        }

        MethodResolution::NotFound
    }

    fn priority(&self) -> u8 {
        0 // Highest priority
    }

    fn name(&self) -> &'static str {
        "UserRegistryResolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

    #[test]
    fn test_priority() {
        let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
        let resolver = UserRegistryResolver::new(registry);
        assert_eq!(resolver.priority(), 0);
    }

    #[test]
    fn test_not_found_for_missing_method() {
        let interner = SharedInterner::default();
        let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
        let resolver = UserRegistryResolver::new(registry);

        let int_type = interner.intern("int");
        let unknown_method = interner.intern("unknown_method");
        let result = resolver.resolve(&Value::int(42), int_type, unknown_method);
        assert!(matches!(result, MethodResolution::NotFound));
    }
}
