//! Expression evaluation helpers.
//!
//! This module provides helper functions for expression evaluation including
//! literals, operators, indexing, and field access. Used by the Interpreter.
//!
//! # Specification
//!
//! - Eval rules: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/0.1-alpha/spec/09-expressions.md`

use ori_ir::{Name, StringInterner};

use crate::errors::{
    cannot_access_field, cannot_get_length, cannot_index, collection_too_large,
    index_out_of_bounds, invalid_tuple_field, no_field_on_struct, no_member_in_module,
    tuple_index_out_of_bounds, undefined_variable,
};
use crate::{ControlAction, Environment, EvalError, EvalResult, Value};

/// Evaluate an identifier lookup.
///
/// Looks up the name in the current environment. Returns `Err` with
/// `undefined_variable` if not found. Type reference resolution is
/// handled by `CanExpr::TypeRef` (emitted during canonicalization),
/// not here.
pub fn eval_ident(name: Name, env: &Environment, interner: &StringInterner) -> EvalResult {
    env.lookup(name)
        .ok_or_else(|| undefined_variable(interner.lookup(name)).into())
}

/// Get the length of a collection for `HashLength` resolution.
pub fn get_collection_length(value: &Value) -> Result<i64, EvalError> {
    let len = match value {
        Value::List(items) | Value::Tuple(items) => items.len(),
        Value::Str(s) => s.chars().count(),
        Value::Map(map) => map.len(),
        _ => return Err(cannot_get_length(value.type_name())),
    };
    i64::try_from(len).map_err(|_| collection_too_large())
}

/// Convert a signed index to unsigned, handling negative indices from the end.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "index arithmetic is bounds-checked"
)]
fn resolve_index(i: i64, len: usize) -> Option<usize> {
    if i >= 0 {
        let idx = usize::try_from(i).ok()?;
        if idx < len {
            Some(idx)
        } else {
            None
        }
    } else {
        // Negative index: count from end
        // -i is positive since i < 0, safe to convert
        let positive = usize::try_from(-i).ok()?;
        if positive <= len {
            Some(len - positive)
        } else {
            None
        }
    }
}

/// Evaluate index access.
pub fn eval_index(value: Value, index: Value) -> EvalResult {
    match (value, index) {
        (Value::List(items), Value::Int(i)) => {
            let raw = i.raw();
            let idx = resolve_index(raw, items.len())
                .ok_or_else(|| ControlAction::from(index_out_of_bounds(raw)))?;
            items
                .get(idx)
                .cloned()
                .ok_or_else(|| ControlAction::from(index_out_of_bounds(raw)))
        }
        (Value::Str(s), Value::Int(i)) => {
            // String indexing returns a single-codepoint str (not char)
            let raw = i.raw();
            let char_count = s.chars().count();
            let idx = resolve_index(raw, char_count)
                .ok_or_else(|| ControlAction::from(index_out_of_bounds(raw)))?;
            s.chars()
                .nth(idx)
                .map(|c| Value::string(c.to_string()))
                .ok_or_else(|| ControlAction::from(index_out_of_bounds(raw)))
        }
        (Value::Map(map), key) => {
            // Map indexing returns Option<V>: Some(value) if found, None if not
            // Convert the key to a map key string (type-prefixed for uniqueness)
            match key.to_map_key() {
                Ok(key_str) => Ok(map.get(&key_str).cloned().map_or(Value::None, Value::some)),
                Err(_) => Err(cannot_index("map", key.type_name()).into()),
            }
        }
        (value, index) => Err(cannot_index(value.type_name(), index.type_name()).into()),
    }
}

/// Evaluate field access.
pub fn eval_field_access(value: Value, field: Name, interner: &StringInterner) -> EvalResult {
    match value {
        Value::Struct(s) => s.get_field(field).cloned().ok_or_else(|| {
            let field_name = interner.lookup(field);
            no_field_on_struct(field_name).into()
        }),
        Value::Tuple(items) => {
            // Tuple field access like t.0, t.1
            let field_name = interner.lookup(field);
            if let Ok(idx) = field_name.parse::<usize>() {
                items
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| tuple_index_out_of_bounds(idx).into())
            } else {
                Err(invalid_tuple_field(field_name).into())
            }
        }
        Value::ModuleNamespace(ns) => {
            // Qualified access: module.member
            ns.get(&field).cloned().ok_or_else(|| {
                let member_name = interner.lookup(field);
                no_member_in_module(member_name).into()
            })
        }
        Value::Error(ref ev) => {
            // Error fields: message (spec §6 defines Error as { message: str, ... })
            let field_name = interner.lookup(field);
            match field_name {
                "message" => Ok(Value::string(ev.message())),
                _ => Err(cannot_access_field(value.type_name()).into()),
            }
        }
        value => Err(cannot_access_field(value.type_name()).into()),
    }
}
