// Trait implementations for Environment
//
// Makes Environment implement the context traits for evaluation phase.
// Note: Environment doesn't implement TypeLookup since evaluation
// doesn't need type definitions - types are already checked.

use crate::ast::FunctionDef;
use crate::context::traits::*;
use crate::eval::{Environment, Value};

impl FunctionLookup<FunctionDef> for Environment {
    fn lookup_function(&self, name: &str) -> Option<&FunctionDef> {
        Environment::get_function(self, name)
    }
}

impl ConfigLookup<Value> for Environment {
    fn lookup_config(&self, name: &str) -> Option<&Value> {
        self.configs.get(name)
    }
}

/// Runtime variable scope for evaluation.
///
/// Note: Environment stores Value directly, but we expose the full Binding
/// through a different interface since VariableScope expects the binding type.
impl VariableScope for Environment {
    type Binding = Value;

    fn define_variable(&mut self, name: String, value: Value, mutable: bool) {
        self.define(name, value, mutable);
    }

    fn lookup_variable(&self, name: &str) -> Option<&Value> {
        self.locals.get(name).map(|b| b.get())
    }

    fn is_variable_mutable(&self, name: &str) -> Option<bool> {
        self.locals.get(name).map(|b| b.is_mutable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_function_lookup() {
        let env = Environment::new();

        // Empty environment has no functions
        assert!(FunctionLookup::<FunctionDef>::lookup_function(&env, "foo").is_none());
    }

    #[test]
    fn test_environment_variable_scope() {
        let mut env = Environment::new();

        // Define immutable variable
        env.define_variable("x".to_string(), Value::Int(42), false);
        assert_eq!(env.lookup_variable("x"), Some(&Value::Int(42)));
        assert_eq!(env.is_variable_mutable("x"), Some(false));

        // Define mutable variable
        env.define_variable("y".to_string(), Value::Int(10), true);
        assert_eq!(env.is_variable_mutable("y"), Some(true));
    }

    #[test]
    fn test_environment_config_lookup() {
        let mut env = Environment::new();

        env.set_config("timeout".to_string(), Value::Int(5000));
        assert_eq!(
            ConfigLookup::<Value>::lookup_config(&env, "timeout"),
            Some(&Value::Int(5000))
        );
    }
}
