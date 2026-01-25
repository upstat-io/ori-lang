//! Method dispatch methods for the Evaluator.

use crate::ir::{Name, SharedArena};
use sigil_eval::{DerivedMethodInfo, DerivedTrait, UserMethod, wrong_function_args};
use super::{Evaluator, EvalResult};
use super::super::value::Value;

impl Evaluator<'_> {
    /// Evaluate a method call.
    ///
    /// Priority order:
    /// 1. User-defined methods from impl blocks
    /// 2. Derived methods from `#[derive(...)]`
    /// 3. Built-in methods in `MethodRegistry`
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

        // Second, check derived methods
        if let Some(derived_info) = self.user_method_registry.lookup_derived(&type_name, method_name) {
            let info = derived_info.clone();
            return self.eval_derived_method(receiver, &info, &args);
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

    /// Evaluate a derived method (from `#[derive(...)]`).
    ///
    /// These methods operate directly on struct field values rather than
    /// having an expression body.
    pub(super) fn eval_derived_method(
        &mut self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        match info.trait_kind {
            DerivedTrait::Eq => self.eval_derived_eq(receiver, info, args),
            DerivedTrait::Clone => self.eval_derived_clone(receiver, info),
            DerivedTrait::Hashable => self.eval_derived_hash(receiver, info),
            DerivedTrait::Printable => self.eval_derived_to_string(receiver, info),
            DerivedTrait::Default => self.eval_derived_default(info),
        }
    }

    /// Evaluate derived `eq` method for structs.
    ///
    /// Compares each field recursively.
    fn eval_derived_eq(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        // eq takes one argument: the other value to compare
        if args.len() != 1 {
            return Err(wrong_function_args(1, args.len()));
        }

        let other = &args[0];

        // Both must be structs
        let (self_struct, other_struct) = match (&receiver, other) {
            (Value::Struct(s), Value::Struct(o)) => (s, o),
            _ => return Ok(Value::Bool(false)), // Different types are not equal
        };

        // Must be the same type
        if self_struct.type_name != other_struct.type_name {
            return Ok(Value::Bool(false));
        }

        // Compare each field
        for field_name in &info.field_names {
            let self_val = self_struct.get_field(*field_name);
            let other_val = other_struct.get_field(*field_name);

            match (self_val, other_val) {
                (Some(sv), Some(ov)) => {
                    if !values_equal(sv, ov) {
                        return Ok(Value::Bool(false));
                    }
                }
                _ => return Ok(Value::Bool(false)), // Missing field
            }
        }

        Ok(Value::Bool(true))
    }

    /// Evaluate derived `clone` method for structs.
    ///
    /// Creates a deep copy of the struct.
    fn eval_derived_clone(&self, receiver: Value, _info: &DerivedMethodInfo) -> EvalResult {
        let struct_val = match receiver {
            Value::Struct(s) => s,
            _ => return Ok(receiver.clone()), // Non-structs just clone directly
        };

        // Clone the struct (Value::Struct already uses Arc for cheap cloning)
        // For a true deep clone, we'd need to recursively clone nested values,
        // but for now we rely on the structural clone behavior of Value.
        Ok(Value::Struct(struct_val.clone()))
    }

    /// Evaluate derived `hash` method for structs.
    ///
    /// Combines hashes of all fields.
    fn eval_derived_hash(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let struct_val = match &receiver {
            Value::Struct(s) => s,
            _ => {
                // For non-structs, use a simple hash
                let mut hasher = DefaultHasher::new();
                receiver.type_name().hash(&mut hasher);
                return Ok(Value::Int(hasher.finish() as i64));
            }
        };

        let mut hasher = DefaultHasher::new();

        // Hash the type name
        self.interner.lookup(struct_val.type_name).hash(&mut hasher);

        // Hash each field value
        for field_name in &info.field_names {
            if let Some(val) = struct_val.get_field(*field_name) {
                hash_value(val, &mut hasher);
            }
        }

        Ok(Value::Int(hasher.finish() as i64))
    }

    /// Evaluate derived `to_string` method for structs.
    ///
    /// Produces a string representation like "Point { x: 10, y: 20 }".
    fn eval_derived_to_string(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        let struct_val = match &receiver {
            Value::Struct(s) => s,
            _ => return Ok(Value::string(format!("{receiver}"))),
        };

        let type_name = self.interner.lookup(struct_val.type_name);
        let mut fields = Vec::new();

        for field_name in &info.field_names {
            let field_str = self.interner.lookup(*field_name);
            if let Some(val) = struct_val.get_field(*field_name) {
                fields.push(format!("{field_str}: {val}"));
            }
        }

        let result = format!("{type_name} {{ {} }}", fields.join(", "));
        Ok(Value::string(result))
    }

    /// Evaluate derived `default` method for structs.
    ///
    /// Returns the default value for the type.
    /// Note: This is currently a stub - a proper implementation would need
    /// to recursively default-construct each field.
    fn eval_derived_default(&self, _info: &DerivedMethodInfo) -> EvalResult {
        // Default is a static method that doesn't take self.
        // For now, return an error since we'd need type information
        // to construct the default struct.
        Err(sigil_eval::EvalError::new(
            "default() requires type context; use explicit construction instead",
        ))
    }
}

/// Check if two values are equal (structural equality).
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Char(x), Value::Char(y)) => x == y,
        (Value::Byte(x), Value::Byte(y)) => x == y,
        (Value::Void, Value::Void) => true,
        (Value::None, Value::None) => true,
        (Value::Some(x), Value::Some(y)) => values_equal(x, y),
        (Value::Ok(x), Value::Ok(y)) => values_equal(x, y),
        (Value::Err(x), Value::Err(y)) => values_equal(x, y),
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Tuple(x), Value::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Struct(x), Value::Struct(y)) => {
            if x.type_name != y.type_name {
                return false;
            }
            // Compare field values by iterating the underlying Vec
            // Note: Both structs should have the same layout if they have the same type_name
            x.fields.iter().zip(y.fields.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            if x.len() != y.len() {
                return false;
            }
            for (k, v) in x.iter() {
                if let Some(other_v) = y.get(k) {
                    if !values_equal(v, other_v) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }
        _ => false, // Different types
    }
}

/// Hash a value into a hasher.
fn hash_value<H: std::hash::Hasher>(value: &Value, hasher: &mut H) {
    use std::hash::Hash;

    match value {
        Value::Int(n) => n.hash(hasher),
        Value::Float(f) => f.to_bits().hash(hasher),
        Value::Bool(b) => b.hash(hasher),
        Value::Str(s) => s.hash(hasher),
        Value::Char(c) => c.hash(hasher),
        Value::Byte(b) => b.hash(hasher),
        Value::Void => 0u8.hash(hasher),
        Value::None => 1u8.hash(hasher),
        Value::Some(v) => {
            2u8.hash(hasher);
            hash_value(v, hasher);
        }
        Value::Ok(v) => {
            3u8.hash(hasher);
            hash_value(v, hasher);
        }
        Value::Err(v) => {
            4u8.hash(hasher);
            hash_value(v, hasher);
        }
        Value::List(items) => {
            5u8.hash(hasher);
            for item in items.iter() {
                hash_value(item, hasher);
            }
        }
        Value::Tuple(items) => {
            6u8.hash(hasher);
            for item in items.iter() {
                hash_value(item, hasher);
            }
        }
        Value::Struct(s) => {
            7u8.hash(hasher);
            // Hash all field values
            for v in s.fields.iter() {
                hash_value(v, hasher);
            }
        }
        Value::Map(m) => {
            8u8.hash(hasher);
            for (k, v) in m.iter() {
                k.hash(hasher);
                hash_value(v, hasher);
            }
        }
        _ => {
            // For other types, just hash the type name
            255u8.hash(hasher);
        }
    }
}
