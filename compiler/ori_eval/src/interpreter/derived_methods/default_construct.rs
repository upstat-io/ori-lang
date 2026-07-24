//! `DefaultConstruct` derive strategy — build a struct with default field values.

use ori_ir::{DerivedMethodInfo, Name, TypeId};
use rustc_hash::FxHashMap;

use super::super::Interpreter;
use crate::derives::DefaultFieldType;
use crate::{EvalResult, StructValue, Value};

impl Interpreter<'_> {
    /// Construct a struct with all fields set to their type's default value.
    ///
    /// Called as a static method: `Point.default()` returns `Point { x: 0, y: 0 }`.
    /// Field types are looked up from the `DefaultFieldTypeRegistry` rather than
    /// from `DerivedMethodInfo` — this keeps evaluator-specific data out of the IR.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent strategy-driven dispatch signature"
    )]
    pub(super) fn eval_default_construct(
        &mut self,
        receiver: Value,
        info: &DerivedMethodInfo,
    ) -> EvalResult {
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
