//! Built-in method resolver.
//!
//! Resolves methods from the MethodRegistry (built-in methods on primitive types).

use super::{MethodResolution, MethodResolver, Value};

/// Resolver for built-in methods from MethodRegistry.
///
/// Priority 2 (lowest) - built-in methods are the fallback when no other
/// resolver handles the method.
///
/// This resolver always returns `Builtin` for any method, delegating
/// the actual lookup to the MethodRegistry. If the method doesn't exist,
/// the registry will return an appropriate error.
pub struct BuiltinMethodResolver;

impl BuiltinMethodResolver {
    /// Create a new built-in method resolver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for BuiltinMethodResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl MethodResolver for BuiltinMethodResolver {
    fn resolve(&self, _receiver: &Value, _type_name: &str, _method_name: &str) -> MethodResolution {
        // Always return Builtin - the actual lookup happens in the registry
        // This acts as a catch-all that delegates to the MethodRegistry
        MethodResolution::Builtin
    }

    fn priority(&self) -> u8 {
        2 // Lowest priority - fallback
    }

    fn name(&self) -> &'static str {
        "BuiltinMethodResolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority() {
        let resolver = BuiltinMethodResolver::new();
        assert_eq!(resolver.priority(), 2);
    }

    #[test]
    fn test_always_returns_builtin() {
        let resolver = BuiltinMethodResolver::new();

        // Any method on any type returns Builtin
        let result = resolver.resolve(&Value::Int(42), "int", "abs");
        assert!(matches!(result, MethodResolution::Builtin));

        let result = resolver.resolve(&Value::string("hello"), "str", "len");
        assert!(matches!(result, MethodResolution::Builtin));
    }
}
