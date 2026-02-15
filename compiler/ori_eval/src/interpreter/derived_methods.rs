//! Derived method evaluation for `#[derive(...)]` attributes.
//!
//! This module handles evaluation of methods generated from derive attributes:
//! - `#[derive(Eq)]` -> `eq` method
//! - `#[derive(Clone)]` -> `clone` method
//! - `#[derive(Hashable)]` -> `hash` method
//! - `#[derive(Printable)]` -> `to_string` method
//! - `#[derive(Default)]` -> `default` method

use crate::errors::wrong_function_args;
use crate::{EvalResult, StructValue, Value};
use ori_ir::{DefaultFieldType, DerivedMethodInfo, DerivedTrait, Name, TypeId};
use rustc_hash::FxHashMap;

use super::Interpreter;

impl Interpreter<'_> {
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
            DerivedTrait::Default => self.eval_derived_default(receiver, info),
        }
    }

    /// Evaluate derived `eq` method for structs.
    ///
    /// Compares each field recursively.
    #[expect(
        clippy::unused_self,
        reason = "Method on Interpreter for organizational consistency with other derived methods"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    fn eval_derived_eq(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        // eq takes one argument: the other value to compare
        if args.len() != 1 {
            return Err(wrong_function_args(1, args.len()).into());
        }

        let other = &args[0];

        // Both must be structs
        let (Value::Struct(self_struct), Value::Struct(other_struct)) = (&receiver, other) else {
            return Ok(Value::Bool(false)); // Different types are not equal
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
    #[expect(
        clippy::unused_self,
        reason = "Method on Interpreter for organizational consistency with other derived methods"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent derived method dispatch interface"
    )]
    fn eval_derived_clone(&self, receiver: Value, _info: &DerivedMethodInfo) -> EvalResult {
        let Value::Struct(struct_val) = receiver else {
            return Ok(receiver.clone()); // Non-structs just clone directly
        };

        // Clone the struct (Value::Struct already uses Arc for cheap cloning)
        // For a true deep clone, we'd need to recursively clone nested values,
        // but for now we rely on the structural clone behavior of Value.
        Ok(Value::Struct(struct_val.clone()))
    }

    /// Evaluate derived `hash` method for structs.
    ///
    /// Combines hashes of all fields.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent derived method dispatch interface"
    )]
    #[expect(
        clippy::cast_possible_wrap,
        reason = "Hash values are opaque identifiers; wrapping is acceptable"
    )]
    fn eval_derived_hash(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let Value::Struct(struct_val) = &receiver else {
            // For non-structs, use a simple hash
            let mut hasher = DefaultHasher::new();
            receiver.type_name().hash(&mut hasher);
            return Ok(Value::int(hasher.finish() as i64));
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

        Ok(Value::int(hasher.finish() as i64))
    }

    /// Evaluate derived `to_string` method for structs.
    ///
    /// Produces a string representation like "Point { x: 10, y: 20 }".
    ///
    /// # Performance
    /// Uses a single String builder with `write!()` instead of Vec + format! + join
    /// to minimize allocations on this hot path.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent derived method dispatch interface"
    )]
    fn eval_derived_to_string(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        use std::fmt::Write;

        let Value::Struct(struct_val) = &receiver else {
            return Ok(Value::string(format!("{receiver}")));
        };

        let type_name = self.interner.lookup(struct_val.type_name);
        // Pre-allocate capacity: type_name + " { " + estimated field content + " }"
        // Overflow is impossible for reasonable struct sizes, and even if it wrapped,
        // String::with_capacity handles it safely by allocating less.
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "capacity estimation, overflow is safe"
        )]
        let capacity = type_name.len() + 4 + info.field_names.len() * 20;
        let mut result = String::with_capacity(capacity);

        result.push_str(type_name);
        result.push_str(" { ");

        let mut first = true;
        for field_name in &info.field_names {
            let field_str = self.interner.lookup(*field_name);
            if let Some(val) = struct_val.get_field(*field_name) {
                if !first {
                    result.push_str(", ");
                }
                first = false;
                // write! returns fmt::Result but we're writing to String which is infallible
                let _ = write!(result, "{field_str}: {val}");
            }
        }

        result.push_str(" }");
        Ok(Value::string(result))
    }

    /// Evaluate derived `default` method for structs.
    ///
    /// Constructs a struct with all fields set to their type's default value.
    /// Called as a static method: `Point.default()` returns `Point { x: 0, y: 0 }`.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    fn eval_derived_default(&mut self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        let Value::TypeRef { type_name } = receiver else {
            return Err(crate::errors::no_such_method("default", "non-type").into());
        };

        let mut fields = FxHashMap::default();
        for (name, field_type) in info.field_names.iter().zip(info.field_types.iter()) {
            let value = self.default_value_for_field(type_name, field_type)?;
            fields.insert(*name, value);
        }

        Ok(Value::Struct(StructValue::new(type_name, fields)))
    }

    /// Produce the default value for a single field based on its type.
    fn default_value_for_field(
        &mut self,
        _parent_type: Name,
        field_type: &DefaultFieldType,
    ) -> EvalResult {
        match field_type {
            DefaultFieldType::Primitive(id) => Ok(primitive_default(*id)),
            DefaultFieldType::Named(name) => {
                let name_str = self.interner.lookup(*name);
                if name_str == "Option" {
                    return Ok(Value::None);
                }
                // Recursively call Type.default() for named types
                let type_ref = Value::TypeRef { type_name: *name };
                let default_name = self.interner.intern("default");
                self.eval_method_call(type_ref, default_name, vec![])
            }
        }
    }
}

/// Return the default `Value` for a primitive `TypeId`.
fn primitive_default(id: TypeId) -> Value {
    match id {
        TypeId::INT => Value::int(0),
        TypeId::FLOAT => Value::Float(0.0),
        TypeId::BOOL => Value::Bool(false),
        TypeId::STR => Value::string(String::new()),
        TypeId::CHAR => Value::Char('\0'),
        TypeId::BYTE => Value::Byte(0),
        TypeId::DURATION => Value::Duration(0),
        TypeId::SIZE => Value::Size(0),
        _ => Value::Void,
    }
}
