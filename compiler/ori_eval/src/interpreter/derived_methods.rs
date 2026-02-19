//! Derived method evaluation for `@derive(...)` attributes.
//!
//! Uses [`DeriveStrategy`](ori_ir::derives::strategy::DeriveStrategy) from `ori_ir`
//! to drive field iteration and result combination. Each strategy variant
//! (`ForEachField`, `FormatFields`, etc.) has a corresponding handler that
//! interprets the strategy using `Value` operations.

use crate::derives::DefaultFieldType;
use crate::errors::wrong_function_args;
use crate::{EvalResult, StructValue, Value};
use ori_ir::{CombineOp, FieldOp, FormatOpen, StructBody};
use ori_ir::{DerivedMethodInfo, Name, TypeId};
use rustc_hash::FxHashMap;

use super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a derived method using its [`DeriveStrategy`].
    ///
    /// Dispatches on the strategy's `struct_body` to select the appropriate
    /// evaluation handler: field comparison, string formatting, cloning, or
    /// default construction.
    pub(super) fn eval_derived_method(
        &mut self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        let strategy = info.trait_kind.strategy();
        match strategy.struct_body {
            StructBody::ForEachField { field_op, combine } => {
                self.eval_for_each_field(receiver, info, args, field_op, combine)
            }
            StructBody::FormatFields {
                open,
                separator,
                suffix,
                include_names,
            } => self.eval_format_fields(receiver, info, open, separator, suffix, include_names),
            StructBody::CloneFields => Ok(receiver),
            StructBody::DefaultConstruct => self.eval_default_construct(receiver, info),
        }
    }

    // ── ForEachField strategy ───────────────────────────────────────────

    /// Apply a per-field operation and combine results.
    ///
    /// Routes to struct or variant handling based on the receiver's shape.
    /// Binary operations (Eq, Compare) require one argument; unary (Hash)
    /// requires none.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent strategy-driven dispatch signature"
    )]
    fn eval_for_each_field(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
        field_op: FieldOp,
        combine: CombineOp,
    ) -> EvalResult {
        let other = if matches!(field_op, FieldOp::Equals | FieldOp::Compare) {
            if args.len() != 1 {
                return Err(wrong_function_args(1, args.len()).into());
            }
            Some(&args[0])
        } else {
            None
        };

        match (&receiver, other) {
            (Value::Struct(self_s), Some(Value::Struct(other_s))) => {
                self.for_each_struct(self_s, Some(other_s), info, field_op, combine)
            }
            (Value::Struct(self_s), None) => {
                self.for_each_struct(self_s, None, info, field_op, combine)
            }
            (
                Value::Variant {
                    type_name: t1,
                    variant_name: v1,
                    fields: f1,
                },
                Some(Value::Variant {
                    type_name: t2,
                    variant_name: v2,
                    fields: f2,
                }),
            ) => self.for_each_variant_binary(*t1, *v1, f1, *t2, *v2, f2, info, combine),
            (
                Value::Variant {
                    variant_name,
                    fields,
                    ..
                },
                None,
            ) => self.for_each_variant_unary(*variant_name, fields, combine),
            _ => match combine {
                CombineOp::AllTrue => Ok(Value::Bool(false)),
                CombineOp::HashCombine => {
                    use crate::methods::compare::FNV_OFFSET_BASIS;
                    Ok(Value::int(FNV_OFFSET_BASIS.cast_signed()))
                }
                CombineOp::Lexicographic => Err(crate::errors::no_such_method(
                    info.trait_kind.method_name(),
                    "incompatible values",
                )
                .into()),
            },
        }
    }

    /// `ForEachField` on named struct fields.
    fn for_each_struct(
        &self,
        self_s: &StructValue,
        other_s: Option<&StructValue>,
        info: &DerivedMethodInfo,
        field_op: FieldOp,
        combine: CombineOp,
    ) -> EvalResult {
        match (field_op, combine) {
            (FieldOp::Equals, CombineOp::AllTrue) => {
                let Some(other) = other_s else {
                    debug_assert!(false, "Equals requires other");
                    return Ok(Value::Bool(false));
                };
                if self_s.type_name != other.type_name {
                    return Ok(Value::Bool(false));
                }
                for field_name in &info.field_names {
                    match (self_s.get_field(*field_name), other.get_field(*field_name)) {
                        (Some(sv), Some(ov)) if sv == ov => {}
                        _ => return Ok(Value::Bool(false)),
                    }
                }
                Ok(Value::Bool(true))
            }
            (FieldOp::Compare, CombineOp::Lexicographic) => {
                use crate::methods::compare::{compare_values, ordering_to_value};
                let Some(other) = other_s else {
                    debug_assert!(false, "Compare requires other");
                    return Err(
                        crate::errors::no_such_method("compare", "missing other argument").into(),
                    );
                };
                if self_s.type_name != other.type_name {
                    return Err(
                        crate::errors::no_such_method("compare", "different struct types").into(),
                    );
                }
                for field_name in &info.field_names {
                    match (self_s.get_field(*field_name), other.get_field(*field_name)) {
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
            (FieldOp::Hash, CombineOp::HashCombine) => {
                use crate::methods::compare::{hash_value, FNV_OFFSET_BASIS, FNV_PRIME};
                let mut hash = FNV_OFFSET_BASIS;
                for field_name in &info.field_names {
                    if let Some(val) = self_s.get_field(*field_name) {
                        let field_hash = hash_value(val, self.interner)?.cast_unsigned();
                        hash ^= field_hash;
                        hash = hash.wrapping_mul(FNV_PRIME);
                    }
                }
                Ok(Value::int(hash.cast_signed()))
            }
            _ => unreachable!(
                "unsupported FieldOp+CombineOp: {:?}+{:?}",
                field_op, combine
            ),
        }
    }

    /// `ForEachField` on variant payloads — binary case (Eq, Compare).
    #[expect(
        clippy::too_many_arguments,
        reason = "variant fields from destructured match arms"
    )]
    fn for_each_variant_binary(
        &self,
        t1: Name,
        v1: Name,
        f1: &[Value],
        t2: Name,
        v2: Name,
        f2: &[Value],
        info: &DerivedMethodInfo,
        combine: CombineOp,
    ) -> EvalResult {
        match combine {
            CombineOp::AllTrue => Ok(Value::Bool(t1 == t2 && v1 == v2 && f1 == f2)),
            CombineOp::Lexicographic => {
                use crate::methods::compare::{compare_values, ordering_to_value};
                let pos1 = info.variant_names.iter().position(|n| *n == v1);
                let pos2 = info.variant_names.iter().position(|n| *n == v2);
                match (pos1, pos2) {
                    (Some(i1), Some(i2)) => {
                        let ord = i1.cmp(&i2);
                        if ord != std::cmp::Ordering::Equal {
                            return Ok(ordering_to_value(ord));
                        }
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
            CombineOp::HashCombine => unreachable!("Hash is unary, not binary"),
        }
    }

    /// `ForEachField` on variant payloads — unary case (Hash).
    fn for_each_variant_unary(
        &self,
        variant_name: Name,
        fields: &[Value],
        combine: CombineOp,
    ) -> EvalResult {
        match combine {
            CombineOp::HashCombine => {
                use crate::methods::compare::{
                    fnv1a_hash, hash_value, FNV_OFFSET_BASIS, FNV_PRIME,
                };
                let mut hash = FNV_OFFSET_BASIS;
                let variant_str = self.interner.lookup(variant_name);
                let discriminant = fnv1a_hash(variant_str.as_bytes()).cast_unsigned();
                hash ^= discriminant;
                hash = hash.wrapping_mul(FNV_PRIME);
                for field in fields {
                    let field_hash = hash_value(field, self.interner)?.cast_unsigned();
                    hash ^= field_hash;
                    hash = hash.wrapping_mul(FNV_PRIME);
                }
                Ok(Value::int(hash.cast_signed()))
            }
            _ => unreachable!("only HashCombine uses unary variant handling"),
        }
    }

    // ── FormatFields strategy ───────────────────────────────────────────

    /// Format struct/variant fields into a string representation.
    ///
    /// Uses `format_value_printable` (Printable) or `debug_value` (Debug)
    /// based on `include_names`.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent strategy-driven dispatch signature"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent strategy-driven interface"
    )]
    fn eval_format_fields(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        open: FormatOpen,
        separator: &str,
        suffix: &str,
        include_names: bool,
    ) -> EvalResult {
        let fmt = |val: &Value| -> String {
            if include_names {
                crate::methods::helpers::debug_value(val)
            } else {
                self.format_value_printable(val)
            }
        };

        match &receiver {
            Value::Struct(struct_val) => {
                let type_name = self.interner.lookup(struct_val.type_name);
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "capacity estimation, overflow is safe"
                )]
                let capacity = type_name.len() + 4 + info.field_names.len() * 12;
                let mut result = String::with_capacity(capacity);

                result.push_str(type_name);
                match open {
                    FormatOpen::TypeNameParen => result.push('('),
                    FormatOpen::TypeNameBrace => result.push_str(" { "),
                }

                let mut first = true;
                for field_name in &info.field_names {
                    if let Some(val) = struct_val.get_field(*field_name) {
                        if !first {
                            result.push_str(separator);
                        }
                        first = false;
                        if include_names {
                            let name_str = self.interner.lookup(*field_name);
                            result.push_str(name_str);
                            result.push_str(": ");
                        }
                        result.push_str(&fmt(val));
                    }
                }

                result.push_str(suffix);
                Ok(Value::string(result))
            }
            Value::Variant {
                variant_name,
                fields,
                ..
            } => {
                let vname = self.interner.lookup(*variant_name);
                if fields.is_empty() {
                    return Ok(Value::string(vname.to_string()));
                }
                let mut result = String::from(vname);
                result.push('(');
                for (i, val) in fields.iter().enumerate() {
                    if i > 0 {
                        result.push_str(separator);
                    }
                    result.push_str(&fmt(val));
                }
                result.push(')');
                Ok(Value::string(result))
            }
            _ => Ok(Value::string(fmt(&receiver))),
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

    // ── DefaultConstruct strategy ───────────────────────────────────────

    /// Construct a struct with all fields set to their type's default value.
    ///
    /// Called as a static method: `Point.default()` returns `Point { x: 0, y: 0 }`.
    /// Field types are looked up from the `DefaultFieldTypeRegistry` rather than
    /// from `DerivedMethodInfo` — this keeps evaluator-specific data out of the IR.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent strategy-driven dispatch signature"
    )]
    fn eval_default_construct(&mut self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        let Value::TypeRef { type_name } = receiver else {
            return Err(crate::errors::no_such_method("default", "non-type").into());
        };

        let default_name = self.interner.intern("default");

        let field_types = self
            .default_field_types
            .read()
            .lookup(type_name, default_name)
            .map(Vec::from);

        let Some(field_types) = field_types else {
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
