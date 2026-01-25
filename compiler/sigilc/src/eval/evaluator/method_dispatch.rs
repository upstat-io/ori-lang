//! Method dispatch methods for the Evaluator.

use crate::ir::{Name, SharedArena};
use super::super::errors::wrong_function_args;
use super::{Evaluator, EvalResult};
use super::super::value::Value;
use super::super::user_methods::UserMethod;

impl Evaluator<'_> {
    /// Evaluate a method call.
    ///
    /// First checks user-defined methods from impl blocks, then falls back
    /// to built-in methods in the `MethodRegistry`.
    pub(super) fn eval_method_call(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult {
        let method_name = self.interner.lookup(method);

        // Get the concrete type name for lookup
        let type_name = self.get_value_type_name(&receiver);

        // First, check user-defined methods
        if let Some(user_method) = self.user_method_registry.lookup(&type_name, method_name) {
            // Clone the method to release the borrow on user_method_registry
            let method = user_method.clone();
            return self.eval_user_method(receiver, &method, &args);
        }

        // Fall back to built-in methods
        self.method_registry.dispatch(receiver, method_name, args)
    }

    /// Get the concrete type name for a value (for method lookup).
    ///
    /// For struct values, returns the actual struct name.
    /// For other values, returns the basic type name.
    pub(super) fn get_value_type_name(&self, value: &Value) -> String {
        match value {
            Value::Struct(s) => self.interner.lookup(s.type_name).to_string(),
            Value::List(_) => "list".to_string(),
            Value::Str(_) => "str".to_string(),
            Value::Int(_) => "int".to_string(),
            Value::Float(_) => "float".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Char(_) => "char".to_string(),
            Value::Byte(_) => "byte".to_string(),
            Value::Map(_) => "map".to_string(),
            Value::Tuple(_) => "tuple".to_string(),
            Value::Some(_) | Value::None => "Option".to_string(),
            Value::Ok(_) | Value::Err(_) => "Result".to_string(),
            Value::Range(_) => "range".to_string(),
            _ => value.type_name().to_string(),
        }
    }

    /// Evaluate a user-defined method from an impl block.
    pub(super) fn eval_user_method(&mut self, receiver: Value, method: &UserMethod, args: &[Value]) -> EvalResult {
        // Method params include 'self' as first parameter
        if method.params.len() != args.len() + 1 {
            return Err(wrong_function_args(method.params.len() - 1, args.len()));
        }

        // Create new environment with captures
        let mut call_env = self.env.child();
        call_env.push_scope();

        // Bind captured variables
        for (name, value) in &method.captures {
            call_env.define(*name, value.clone(), false);
        }

        // Bind 'self' to receiver (first parameter)
        if let Some(&self_param) = method.params.first() {
            call_env.define(self_param, receiver, false);
        }

        // Bind remaining parameters
        for (param, arg) in method.params.iter().skip(1).zip(args.iter()) {
            call_env.define(*param, arg.clone(), false);
        }

        // Evaluate method body
        let result = if let Some(ref func_arena) = method.arena {
            // Method from an imported module - use its arena
            let imported_arena = SharedArena::new((**func_arena).clone());
            let mut call_evaluator = Evaluator::with_imported_arena(
                self.interner, func_arena, call_env, imported_arena, self.user_method_registry.clone()
            );
            let result = call_evaluator.eval(method.body);
            call_evaluator.env.pop_scope();
            result
        } else {
            // Local method - use our arena
            let mut call_evaluator = Evaluator::with_env(self.interner, self.arena, call_env, self.user_method_registry.clone());
            let result = call_evaluator.eval(method.body);
            call_evaluator.env.pop_scope();
            result
        };

        result
    }
}
