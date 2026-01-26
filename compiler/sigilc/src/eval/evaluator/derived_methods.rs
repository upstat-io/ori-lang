//! Derived method evaluation for `#[derive(...)]` attributes.
//!
//! This module handles evaluation of methods generated from derive attributes:
//! - `#[derive(Eq)]` -> `eq` method
//! - `#[derive(Clone)]` -> `clone` method
//! - `#[derive(Hashable)]` -> `hash` method
//! - `#[derive(Printable)]` -> `to_string` method
//! - `#[derive(Default)]` -> `default` method

use sigil_eval::{DerivedMethodInfo, DerivedTrait, default_requires_type_context, wrong_function_args};
use super::{Evaluator, EvalResult};
use super::super::value::Value;

impl Evaluator<'_> {
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
                    if sv != ov {
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
                val.hash(&mut hasher);
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
        Err(default_requires_type_context())
    }
}
