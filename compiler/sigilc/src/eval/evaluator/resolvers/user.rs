//! User-defined method resolver.
//!
//! Resolves methods defined in impl blocks.

use crate::context::SharedRegistry;
use sigil_eval::UserMethodRegistry;
use super::{MethodResolution, MethodResolver, Value};

/// Resolver for user-defined methods from impl blocks.
///
/// Priority 0 (highest) - user methods take precedence over all others.
pub struct UserMethodResolver {
    registry: SharedRegistry<UserMethodRegistry>,
}

impl UserMethodResolver {
    /// Create a new resolver with the given registry.
    pub fn new(registry: SharedRegistry<UserMethodRegistry>) -> Self {
        Self { registry }
    }
}

impl MethodResolver for UserMethodResolver {
    fn resolve(&self, _receiver: &Value, type_name: &str, method_name: &str) -> MethodResolution {
        if let Some(user_method) = self.registry.lookup(type_name, method_name) {
            MethodResolution::User(user_method.clone())
        } else {
            MethodResolution::NotFound
        }
    }

    fn priority(&self) -> u8 {
        0 // Highest priority
    }

    fn name(&self) -> &'static str {
        "UserMethodResolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority() {
        let registry = SharedRegistry::new(UserMethodRegistry::new());
        let resolver = UserMethodResolver::new(registry);
        assert_eq!(resolver.priority(), 0);
    }

    #[test]
    fn test_not_found_for_missing_method() {
        let registry = SharedRegistry::new(UserMethodRegistry::new());
        let resolver = UserMethodResolver::new(registry);

        let result = resolver.resolve(&Value::Int(42), "int", "unknown_method");
        assert!(matches!(result, MethodResolution::NotFound));
    }
}
