//! User-defined method registry for impl block methods.
//!
//! This module stores methods defined in `impl` blocks so they can be
//! dispatched at runtime. Built-in methods (in `methods.rs`) take precedence,
//! but user-defined methods can extend types with new functionality.

use std::collections::HashMap;
use sigil_ir::{ExprId, Name, SharedArena};
use sigil_patterns::Value;

/// A user-defined method from an impl block.
#[derive(Clone, Debug)]
pub struct UserMethod {
    /// Parameter names (first is always `self`).
    pub params: Vec<Name>,
    /// Method body expression.
    pub body: ExprId,
    /// Arena for evaluating the body (Some for imported methods).
    pub arena: Option<SharedArena>,
    /// Captured variables from the defining scope.
    pub captures: HashMap<Name, Value>,
}

impl UserMethod {
    /// Create a new user method.
    pub fn new(params: Vec<Name>, body: ExprId) -> Self {
        UserMethod {
            params,
            body,
            arena: None,
            captures: HashMap::new(),
        }
    }

    /// Create a user method from an imported module.
    pub fn from_import(params: Vec<Name>, body: ExprId, arena: SharedArena) -> Self {
        UserMethod {
            params,
            body,
            arena: Some(arena),
            captures: HashMap::new(),
        }
    }

    /// Create a user method with captures.
    pub fn with_captures(params: Vec<Name>, body: ExprId, captures: HashMap<Name, Value>) -> Self {
        UserMethod {
            params,
            body,
            arena: None,
            captures,
        }
    }
}

/// Registry for user-defined methods from impl blocks.
///
/// Methods are keyed by (`type_name`, `method_name`) pairs.
/// Type names are strings like "Point", "int", "[int]", etc.
#[derive(Clone, Debug, Default)]
pub struct UserMethodRegistry {
    /// Map from (`type_name`, `method_name`) to method definition.
    methods: HashMap<(String, String), UserMethod>,
}

impl UserMethodRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        UserMethodRegistry {
            methods: HashMap::new(),
        }
    }

    /// Register a user-defined method.
    ///
    /// # Arguments
    /// * `type_name` - The type this method is defined on (e.g., "Point", "int")
    /// * `method_name` - The method name (e.g., "distance", "double")
    /// * `method` - The method definition
    pub fn register(&mut self, type_name: String, method_name: String, method: UserMethod) {
        self.methods.insert((type_name, method_name), method);
    }

    /// Look up a user-defined method.
    ///
    /// Returns None if no method is registered for this type/method combination.
    pub fn lookup(&self, type_name: &str, method_name: &str) -> Option<&UserMethod> {
        self.methods
            .get(&(type_name.to_string(), method_name.to_string()))
    }

    /// Check if a method exists for the given type.
    pub fn has_method(&self, type_name: &str, method_name: &str) -> bool {
        self.methods
            .contains_key(&(type_name.to_string(), method_name.to_string()))
    }

    /// Get all registered methods (for debugging).
    pub fn all_methods(&self) -> impl Iterator<Item = (&(String, String), &UserMethod)> {
        self.methods.iter()
    }

    /// Merge another registry into this one.
    pub fn merge(&mut self, other: UserMethodRegistry) {
        self.methods.extend(other.methods);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_ir::{ExprId, SharedInterner};

    fn dummy_expr_id() -> ExprId {
        ExprId::new(0)
    }

    fn dummy_name() -> Name {
        let interner = SharedInterner::default();
        interner.intern("dummy")
    }

    #[test]
    fn test_register_and_lookup() {
        let mut registry = UserMethodRegistry::new();
        let method = UserMethod::new(vec![dummy_name()], dummy_expr_id());

        registry.register("Point".to_string(), "distance".to_string(), method);

        assert!(registry.has_method("Point", "distance"));
        assert!(!registry.has_method("Point", "other"));
        assert!(!registry.has_method("Other", "distance"));

        let found = registry.lookup("Point", "distance");
        assert!(found.is_some());
    }

    #[test]
    fn test_empty_registry() {
        let registry = UserMethodRegistry::new();
        assert!(!registry.has_method("Point", "distance"));
        assert!(registry.lookup("Point", "distance").is_none());
    }
}
