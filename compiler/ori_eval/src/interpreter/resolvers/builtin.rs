//! Built-in method resolver.
//!
//! Resolves methods from the `MethodRegistry` (built-in methods on primitive types).

use super::{MethodResolution, MethodResolver, Value};
use ori_ir::Name;

/// Resolver for built-in methods from `MethodRegistry`.
///
/// Priority 2 (lowest) - built-in methods are the fallback when no other
/// resolver handles the method.
///
/// This resolver always returns `Builtin` for any method, delegating
/// the actual lookup to the `MethodRegistry`. If the method doesn't exist,
/// the registry will return an appropriate error.
#[derive(Clone)]
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
    fn resolve(&self, _receiver: &Value, _type_name: Name, _method_name: Name) -> MethodResolution {
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
    use ori_ir::SharedInterner;

    #[test]
    fn test_priority() {
        let resolver = BuiltinMethodResolver::new();
        assert_eq!(resolver.priority(), 2);
    }

    #[test]
    fn test_always_returns_builtin() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new();

        let int_type = interner.intern("int");
        let abs_method = interner.intern("abs");
        let str_type = interner.intern("str");
        let len_method = interner.intern("len");

        // Any method on any type returns Builtin
        let result = resolver.resolve(&Value::int(42), int_type, abs_method);
        assert!(matches!(result, MethodResolution::Builtin));

        let result = resolver.resolve(&Value::string("hello"), str_type, len_method);
        assert!(matches!(result, MethodResolution::Builtin));
    }
}
