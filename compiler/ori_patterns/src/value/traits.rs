//! Standard trait implementations for `Value`.
//!
//! Contains `Debug`, `Display`, `PartialEq`, `Eq`, and `Hash` implementations.

use std::fmt;

use super::conversions::format_duration;
use super::Value;

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "Int({n})"),
            Value::Float(n) => write!(f, "Float({n})"),
            Value::Bool(b) => write!(f, "Bool({b})"),
            Value::Str(s) => write!(f, "Str({:?})", &**s),
            Value::Char(c) => write!(f, "Char({c:?})"),
            Value::Byte(b) => write!(f, "Byte({b:?})"),
            Value::Void => write!(f, "Void"),
            Value::List(items) => write!(f, "List({:?})", &items[..]),
            Value::Map(map) => write!(f, "Map({:?})", &**map),
            Value::Set(items) => write!(f, "Set({:?})", items.values().collect::<Vec<_>>()),
            Value::Tuple(items) => write!(f, "Tuple({:?})", &**items),
            Value::Some(v) => write!(f, "Some({:?})", &**v),
            Value::None => write!(f, "None"),
            Value::Ok(v) => write!(f, "Ok({:?})", &**v),
            Value::Err(v) => write!(f, "Err({:?})", &**v),
            Value::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                write!(
                    f,
                    "Variant({:?}::{:?}, {:?})",
                    type_name, variant_name, &**fields
                )
            }
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                write!(
                    f,
                    "VariantConstructor({type_name:?}::{variant_name:?}, {field_count} fields)"
                )
            }
            Value::Newtype { type_name, inner } => {
                write!(f, "Newtype({type_name:?}, {:?})", &**inner)
            }
            Value::NewtypeConstructor { type_name } => {
                write!(f, "NewtypeConstructor({type_name:?})")
            }
            Value::Struct(s) => write!(f, "Struct({s:?})"),
            Value::Function(func) => write!(f, "Function({func:?})"),
            Value::MemoizedFunction(mf) => write!(f, "MemoizedFunction({mf:?})"),
            Value::FunctionVal(_, name) => write!(f, "FunctionVal({name})"),
            Value::Duration(ms) => write!(f, "Duration({ms}ms)"),
            Value::Size(bytes) => write!(f, "Size({bytes}b)"),
            Value::Ordering(ord) => write!(f, "Ordering({ord:?})"),
            Value::Range(r) => write!(f, "Range({r:?})"),
            Value::Iterator(it) => write!(f, "Iterator({it:?})"),
            Value::ModuleNamespace(ns) => write!(f, "ModuleNamespace({} items)", ns.len()),
            Value::Error(ev) => write!(f, "Error({})", ev.message()),
            Value::TypeRef { type_name } => write!(f, "TypeRef({type_name:?})"),
        }
    }
}

impl fmt::Display for Value {
    #[expect(clippy::too_many_lines, reason = "exhaustive Value Display formatting")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "\"{}\"", &**s),
            Value::Char(c) => write!(f, "'{c}'"),
            Value::Byte(b) => write!(f, "0x{b:02x}"),
            Value::Void => write!(f, "void"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Map(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{k}\": {v}")?;
                }
                write!(f, "}}")
            }
            Value::Set(items) => {
                write!(f, "Set {{")?;
                for (i, v) in items.values().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "}}")
            }
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Value::Some(v) => write!(f, "Some({})", &**v),
            Value::None => write!(f, "None"),
            Value::Ok(v) => write!(f, "Ok({})", &**v),
            Value::Err(e) => write!(f, "Err({})", &**e),
            Value::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                if fields.is_empty() {
                    write!(f, "<variant {type_name:?}::{variant_name:?}>")
                } else {
                    write!(f, "<variant {type_name:?}::{variant_name:?}(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{field}")?;
                    }
                    write!(f, ")>")
                }
            }
            Value::VariantConstructor {
                type_name,
                variant_name,
                ..
            } => {
                write!(f, "<variant_constructor {type_name:?}::{variant_name:?}>")
            }
            Value::Newtype { type_name, inner } => {
                write!(f, "<newtype {type_name:?}({})>", &**inner)
            }
            Value::NewtypeConstructor { type_name } => {
                write!(f, "<newtype_constructor {type_name:?}>")
            }
            Value::Struct(s) => write!(f, "<struct {:?}>", s.type_name),
            Value::Function(_) | Value::MemoizedFunction(_) => write!(f, "<function>"),
            Value::FunctionVal(_, name) => write!(f, "<function_val {name}>"),
            Value::Duration(ns) => write!(f, "{}", format_duration(*ns)),
            Value::Size(bytes) => {
                if *bytes >= 1024 * 1024 {
                    write!(f, "{}mb", bytes / (1024 * 1024))
                } else if *bytes >= 1024 {
                    write!(f, "{}kb", bytes / 1024)
                } else {
                    write!(f, "{bytes}b")
                }
            }
            Value::Ordering(ord) => write!(f, "{}", ord.name()),
            Value::Range(r) => {
                if let Some(end) = r.end {
                    let op = if r.inclusive { "..=" } else { ".." };
                    write!(f, "{}{op}{end}", r.start)?;
                } else {
                    write!(f, "{}..", r.start)?;
                }
                if r.step != 1 {
                    write!(f, " by {}", r.step)?;
                }
                Ok(())
            }
            Value::Iterator(it) => write!(f, "<iterator {it:?}>"),
            Value::ModuleNamespace(_) => write!(f, "<module>"),
            Value::Error(ev) => write!(f, "<error: {}>", ev.message()),
            Value::TypeRef { type_name } => write!(f, "<type {type_name:?}>"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Byte(a), Value::Byte(b)) => a == b,
            (Value::Void, Value::Void) | (Value::None, Value::None) => true,
            (Value::Some(a), Value::Some(b))
            | (Value::Ok(a), Value::Ok(b))
            | (Value::Err(a), Value::Err(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Duration(a), Value::Duration(b)) => a == b,
            (Value::Size(a), Value::Size(b)) => a == b,
            (Value::Ordering(a), Value::Ordering(b)) => a == b,
            (Value::FunctionVal(_, name_a), Value::FunctionVal(_, name_b)) => name_a == name_b,
            // Functions are equal by canonical body identity
            (Value::Function(a), Value::Function(b)) => a.can_body == b.can_body,
            (Value::MemoizedFunction(a), Value::MemoizedFunction(b)) => {
                a.func.can_body == b.func.can_body
            }
            (Value::Struct(a), Value::Struct(b)) => {
                a.type_name == b.type_name
                    && a.fields
                        .iter()
                        .zip(b.fields.iter())
                        .all(|(av, bv)| av == bv)
            }
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
            ) => t1 == t2 && v1 == v2 && f1 == f2,
            (
                Value::VariantConstructor {
                    type_name: t1,
                    variant_name: v1,
                    field_count: c1,
                },
                Value::VariantConstructor {
                    type_name: t2,
                    variant_name: v2,
                    field_count: c2,
                },
            ) => t1 == t2 && v1 == v2 && c1 == c2,
            (
                Value::Newtype {
                    type_name: t1,
                    inner: i1,
                },
                Value::Newtype {
                    type_name: t2,
                    inner: i2,
                },
            ) => t1 == t2 && i1 == i2,
            (
                Value::NewtypeConstructor { type_name: t1 },
                Value::NewtypeConstructor { type_name: t2 },
            ) => t1 == t2,
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|bv| v == bv))
            }
            (Value::Set(a), Value::Set(b)) => {
                a.len() == b.len() && a.keys().all(|k| b.contains_key(k))
            }
            _ => false,
        }
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use discriminant tags to distinguish variants
        std::mem::discriminant(self).hash(state);

        match self {
            Value::Int(n) => n.hash(state),
            Value::Float(f) => f.to_bits().hash(state),
            Value::Bool(b) => b.hash(state),
            Value::Str(s) => s.hash(state),
            Value::Char(c) => c.hash(state),
            Value::Byte(b) => b.hash(state),
            Value::Void | Value::None => {}
            Value::Duration(d) => d.hash(state),
            Value::Size(s) => s.hash(state),
            Value::Ordering(ord) => ord.hash(state),
            Value::Some(v) | Value::Ok(v) | Value::Err(v) => v.hash(state),
            Value::List(items) => {
                for item in items.iter() {
                    item.hash(state);
                }
            }
            Value::Tuple(items) => {
                for item in items.iter() {
                    item.hash(state);
                }
            }
            Value::Map(m) => {
                // Hash length for consistency
                m.len().hash(state);
                // BTreeMap iterates in sorted key order, so no sorting needed
                for (k, v) in m.iter() {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::Set(s) => {
                s.len().hash(state);
                // BTreeMap iterates in sorted key order, so no sorting needed
                for k in s.keys() {
                    k.hash(state);
                }
            }
            Value::Struct(s) => {
                s.type_name.hash(state);
                for v in s.fields.iter() {
                    v.hash(state);
                }
            }
            Value::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                type_name.hash(state);
                variant_name.hash(state);
                for v in fields.iter() {
                    v.hash(state);
                }
            }
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                type_name.hash(state);
                variant_name.hash(state);
                field_count.hash(state);
            }
            Value::Newtype { type_name, inner } => {
                type_name.hash(state);
                inner.hash(state);
            }
            Value::NewtypeConstructor { type_name } => {
                type_name.hash(state);
            }
            Value::Function(f) => {
                // Hash by function identity (canonical body ID)
                f.can_body.hash(state);
            }
            Value::MemoizedFunction(mf) => {
                // Hash by underlying function identity
                mf.func.can_body.hash(state);
            }
            Value::FunctionVal(_, name) => name.hash(state),
            Value::Range(r) => {
                r.start.hash(state);
                r.end.hash(state);
                r.step.hash(state);
                r.inclusive.hash(state);
            }
            Value::Iterator(it) => it.hash(state),
            Value::ModuleNamespace(ns) => {
                // Hash by namespace size (discriminant already hashed)
                ns.len().hash(state);
            }
            Value::Error(ev) => ev.hash(state),
            Value::TypeRef { type_name } => type_name.hash(state),
        }
    }
}
