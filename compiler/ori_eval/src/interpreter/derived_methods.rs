//! Derived method evaluation for `#[derive(...)]` attributes.
//!
//! This module handles evaluation of methods generated from derive attributes:
//! - `#[derive(Eq)]` -> `eq` method
//! - `#[derive(Clone)]` -> `clone` method
//! - `#[derive(Hashable)]` -> `hash` method
//! - `#[derive(Printable)]` -> `to_str` method
//! - `#[derive(Debug)]` -> `debug` method
//! - `#[derive(Default)]` -> `default` method
//! - `#[derive(Comparable)]` -> `compare` method

use crate::derives::DefaultFieldType;
use crate::errors::wrong_function_args;
use crate::{EvalResult, StructValue, Value};
use ori_ir::{DerivedMethodInfo, DerivedTrait, Name, TypeId};
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
            DerivedTrait::Printable => self.eval_derived_to_str(receiver, info),
            DerivedTrait::Debug => self.eval_derived_debug(receiver, info),
            DerivedTrait::Default => self.eval_derived_default(receiver, info),
            DerivedTrait::Comparable => self.eval_derived_compare(receiver, info, args),
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

        match (&receiver, other) {
            // Struct equality: compare named fields
            (Value::Struct(self_struct), Value::Struct(other_struct)) => {
                if self_struct.type_name != other_struct.type_name {
                    return Ok(Value::Bool(false));
                }
                for field_name in &info.field_names {
                    let self_val = self_struct.get_field(*field_name);
                    let other_val = other_struct.get_field(*field_name);
                    match (self_val, other_val) {
                        (Some(sv), Some(ov)) if sv == ov => {}
                        _ => return Ok(Value::Bool(false)),
                    }
                }
                Ok(Value::Bool(true))
            }
            // Variant equality: same type + same variant + equal payloads
            (
                Value::Variant {
                    type_name: t1,
                    variant_name: v1,
                    fields: f1,
                },
                Value::Variant {
                    type_name: t2,
                    variant_name: v2,
                    fields: f2,
                },
            ) => Ok(Value::Bool(t1 == t2 && v1 == v2 && f1 == f2)),
            // Mismatched value kinds are not equal
            _ => Ok(Value::Bool(false)),
        }
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

        match &receiver {
            Value::Struct(struct_val) => {
                let mut hasher = DefaultHasher::new();
                self.interner.lookup(struct_val.type_name).hash(&mut hasher);
                for field_name in &info.field_names {
                    if let Some(val) = struct_val.get_field(*field_name) {
                        val.hash(&mut hasher);
                    }
                }
                Ok(Value::int(hasher.finish() as i64))
            }
            Value::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                let mut hasher = DefaultHasher::new();
                // Hash type + variant name for discriminant
                self.interner.lookup(*type_name).hash(&mut hasher);
                self.interner.lookup(*variant_name).hash(&mut hasher);
                // Hash each payload field
                for field in fields.as_ref() {
                    field.hash(&mut hasher);
                }
                Ok(Value::int(hasher.finish() as i64))
            }
            _ => {
                let mut hasher = DefaultHasher::new();
                receiver.type_name().hash(&mut hasher);
                Ok(Value::int(hasher.finish() as i64))
            }
        }
    }

    /// Evaluate derived `to_str` method for structs.
    ///
    /// Produces human-readable format like `Point(10, 20)` per spec §7
    /// (type name + field values in parens, no field names).
    ///
    /// Uses [`format_value_printable`] for each field to ensure:
    /// - Strings are unquoted (human-readable)
    /// - Nested structs are recursively formatted as `TypeName(vals...)`
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent derived method dispatch interface"
    )]
    fn eval_derived_to_str(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        match &receiver {
            Value::Struct(struct_val) => {
                let type_name = self.interner.lookup(struct_val.type_name);
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "capacity estimation, overflow is safe"
                )]
                let capacity = type_name.len() + 2 + info.field_names.len() * 12;
                let mut result = String::with_capacity(capacity);
                result.push_str(type_name);
                result.push('(');
                let mut first = true;
                for field_name in &info.field_names {
                    if let Some(val) = struct_val.get_field(*field_name) {
                        if !first {
                            result.push_str(", ");
                        }
                        first = false;
                        result.push_str(&self.format_value_printable(val));
                    }
                }
                result.push(')');
                Ok(Value::string(result))
            }
            Value::Variant {
                variant_name,
                fields,
                ..
            } => {
                let vname = self.interner.lookup(*variant_name);
                if fields.is_empty() {
                    Ok(Value::string(vname.to_string()))
                } else {
                    let mut result = String::from(vname);
                    result.push('(');
                    for (i, val) in fields.iter().enumerate() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&self.format_value_printable(val));
                    }
                    result.push(')');
                    Ok(Value::string(result))
                }
            }
            _ => Ok(Value::string(format!("{receiver}"))),
        }
    }

    /// Format a value in Printable style (human-readable, no quotes on strings).
    ///
    /// Unlike `Value::Display` which wraps strings in quotes and shows raw
    /// struct debug info, this produces the human-readable Printable format:
    /// - Strings: content directly (no quotes)
    /// - Chars: character directly (no quotes)
    /// - Structs: `TypeName(val1, val2)` via recursive lookup
    /// - Other values: standard Display format
    fn format_value_printable(&self, val: &Value) -> String {
        match val {
            Value::Str(s) => (**s).to_string(),
            Value::Char(c) => c.to_string(),
            Value::Struct(sv) => {
                let to_str_name = self.interner.intern("to_str");
                let derived_info = self
                    .user_method_registry
                    .read()
                    .lookup_derived(sv.type_name, to_str_name)
                    .cloned();

                let type_name = self.interner.lookup(sv.type_name);
                let mut result = String::from(type_name);
                result.push('(');

                if let Some(ref info) = derived_info {
                    let mut first = true;
                    for field_name in &info.field_names {
                        if let Some(fv) = sv.get_field(*field_name) {
                            if !first {
                                result.push_str(", ");
                            }
                            first = false;
                            result.push_str(&self.format_value_printable(fv));
                        }
                    }
                }

                result.push(')');
                result
            }
            Value::Variant {
                variant_name,
                fields,
                ..
            } => {
                let vname = self.interner.lookup(*variant_name);
                if fields.is_empty() {
                    vname.to_string()
                } else {
                    let mut result = String::from(vname);
                    result.push('(');
                    for (i, fv) in fields.iter().enumerate() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&self.format_value_printable(fv));
                    }
                    result.push(')');
                    result
                }
            }
            _ => format!("{val}"),
        }
    }

    /// Evaluate derived `debug` method for structs.
    ///
    /// Produces a developer-facing string like `Point { x: 10, y: 20 }` where
    /// nested values use debug formatting (strings are quoted/escaped, etc.).
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent derived method dispatch interface"
    )]
    fn eval_derived_debug(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        use crate::methods::helpers::debug_value;
        use std::fmt::Write;

        match &receiver {
            Value::Struct(struct_val) => {
                let type_name = self.interner.lookup(struct_val.type_name);
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
                        let _ = write!(result, "{field_str}: {}", debug_value(val));
                    }
                }
                result.push_str(" }");
                Ok(Value::string(result))
            }
            Value::Variant {
                variant_name,
                fields,
                ..
            } => {
                let vname = self.interner.lookup(*variant_name);
                if fields.is_empty() {
                    Ok(Value::string(vname.to_string()))
                } else {
                    let mut result = String::from(vname);
                    result.push('(');
                    for (i, val) in fields.iter().enumerate() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&debug_value(val));
                    }
                    result.push(')');
                    Ok(Value::string(result))
                }
            }
            _ => Ok(Value::string(debug_value(&receiver))),
        }
    }

    /// Evaluate derived `default` method for structs.
    ///
    /// Constructs a struct with all fields set to their type's default value.
    /// Called as a static method: `Point.default()` returns `Point { x: 0, y: 0 }`.
    ///
    /// Field types are looked up from the `DefaultFieldTypeRegistry` rather than
    /// from `DerivedMethodInfo` — this keeps evaluator-specific data out of the IR.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    fn eval_derived_default(&mut self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        let Value::TypeRef { type_name } = receiver else {
            return Err(crate::errors::no_such_method("default", "non-type").into());
        };

        let default_name = self.interner.intern("default");

        // Look up field types from the evaluator-local registry
        let field_types = self
            .default_field_types
            .read()
            .lookup(type_name, default_name)
            .map(Vec::from);

        let Some(field_types) = field_types else {
            // No field types registered — fall back to empty struct
            return Ok(Value::Struct(StructValue::new(
                type_name,
                FxHashMap::default(),
            )));
        };

        let mut fields = FxHashMap::default();
        for (name, field_type) in info.field_names.iter().zip(field_types.iter()) {
            let value = self.default_value_for_field(type_name, field_type)?;
            fields.insert(*name, value);
        }

        Ok(Value::Struct(StructValue::new(type_name, fields)))
    }

    /// Evaluate derived `compare` method for structs.
    ///
    /// Lexicographic field comparison: compares fields in declaration order,
    /// short-circuiting on the first non-equal field. Uses `compare_values()`
    /// for recursive comparison of field values.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent derived method dispatch signature"
    )]
    fn eval_derived_compare(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        use crate::methods::compare::{compare_values, ordering_to_value};

        if args.len() != 1 {
            return Err(wrong_function_args(1, args.len()).into());
        }

        let other = &args[0];

        match (&receiver, other) {
            // Struct comparison: lexicographic by field declaration order
            (Value::Struct(self_struct), Value::Struct(other_struct)) => {
                if self_struct.type_name != other_struct.type_name {
                    return Err(
                        crate::errors::no_such_method("compare", "different struct types").into(),
                    );
                }
                for field_name in &info.field_names {
                    let self_val = self_struct.get_field(*field_name);
                    let other_val = other_struct.get_field(*field_name);
                    match (self_val, other_val) {
                        (Some(sv), Some(ov)) => {
                            let ord = compare_values(sv, ov, self.interner)?;
                            if ord != std::cmp::Ordering::Equal {
                                return Ok(ordering_to_value(ord));
                            }
                        }
                        _ => {
                            return Err(crate::errors::no_such_method(
                                "compare",
                                "struct with missing field",
                            )
                            .into());
                        }
                    }
                }
                Ok(ordering_to_value(std::cmp::Ordering::Equal))
            }
            // Variant comparison: by declaration order, then by payload
            (
                Value::Variant {
                    variant_name: v1,
                    fields: f1,
                    ..
                },
                Value::Variant {
                    variant_name: v2,
                    fields: f2,
                    ..
                },
            ) => {
                // Find positions in declaration order
                let pos1 = info.variant_names.iter().position(|n| n == v1);
                let pos2 = info.variant_names.iter().position(|n| n == v2);
                match (pos1, pos2) {
                    (Some(i1), Some(i2)) => {
                        let ord = i1.cmp(&i2);
                        if ord != std::cmp::Ordering::Equal {
                            return Ok(ordering_to_value(ord));
                        }
                        // Same variant — compare payloads lexicographically
                        for (sv, ov) in f1.iter().zip(f2.iter()) {
                            let ord = compare_values(sv, ov, self.interner)?;
                            if ord != std::cmp::Ordering::Equal {
                                return Ok(ordering_to_value(ord));
                            }
                        }
                        Ok(ordering_to_value(std::cmp::Ordering::Equal))
                    }
                    _ => Err(
                        crate::errors::no_such_method("compare", "variant not found in type")
                            .into(),
                    ),
                }
            }
            _ => Err(crate::errors::no_such_method("compare", "incomparable values").into()),
        }
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
