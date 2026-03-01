//! Value inspection, conversion, and display methods.

use std::borrow::Cow;

use super::Value;
pub use ori_ir::StringLookup;

// Value inspection and conversion methods

impl Value {
    /// Check if this value is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => !n.is_zero(),
            Value::Str(s) => !s.is_empty(),
            Value::List(items) => !items.is_empty(),
            Value::Set(items) => !items.is_empty(),
            Value::None | Value::Err(_) | Value::Void => false,
            _ => true,
        }
    }

    /// Try to convert to an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(n.raw()),
            _ => None,
        }
    }

    /// Try to convert to a float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n) => {
                let raw = n.raw();
                // Use i32 for lossless f64 conversion when possible
                if let Ok(i32_val) = i32::try_from(raw) {
                    Some(f64::from(i32_val))
                } else {
                    // For larger values, use string parsing to avoid cast warning
                    Some(format!("{raw}").parse().unwrap_or(f64::NAN))
                }
            }
            _ => None,
        }
    }

    /// Try to convert to a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to convert to a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Try to convert to a char.
    pub fn as_char(&self) -> Option<char> {
        match self {
            Value::Char(c) => Some(*c),
            _ => None,
        }
    }

    /// Try to convert to a list.
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(list) => Some(list),
            _ => None,
        }
    }

    /// Get the type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::Str(_) => "str",
            Value::Char(_) => "char",
            Value::Byte(_) => "byte",
            Value::Void => "void",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Set(_) => "Set",
            Value::Tuple(_) => "tuple",
            Value::Some(_) | Value::None => "Option",
            Value::Ok(_) | Value::Err(_) => "Result",
            Value::Variant { .. } => "variant",
            Value::VariantConstructor { .. } => "variant_constructor",
            Value::Newtype { .. } => "newtype",
            Value::NewtypeConstructor { .. } => "newtype_constructor",
            Value::Struct(_) => "struct",
            Value::Function(_) | Value::MemoizedFunction(_) => "function",
            Value::FunctionVal(_, _) => "function_val",
            Value::Duration(_) => "Duration",
            Value::Size(_) => "Size",
            Value::Ordering(_) => "Ordering",
            Value::Range(_) => "Range",
            Value::Iterator(_) => "Iterator",
            Value::ModuleNamespace(_) => "module",
            Value::Error(..) => "error",
            Value::TypeRef { .. } => "type",
        }
    }

    /// Get the concrete type name, resolving struct names via the interner.
    ///
    /// For struct values, this returns the actual struct name (e.g., "Point").
    /// For variant values, this returns the enum type name (e.g., "Status").
    /// For Range values, returns "range" (lowercase) for method dispatch consistency.
    /// For all other types, delegates to `type_name()`.
    ///
    /// This method unifies the type name logic that was previously duplicated
    /// between `Value::type_name()` and `Evaluator::get_value_type_name()`.
    pub fn type_name_with_interner<I: StringLookup>(&self, interner: &I) -> Cow<'static, str> {
        match self {
            Value::Struct(s) => Cow::Owned(interner.lookup(s.type_name).to_string()),
            Value::Variant { type_name, .. } | Value::Newtype { type_name, .. } => {
                Cow::Owned(interner.lookup(*type_name).to_string())
            }
            // Range uses lowercase for method dispatch (distinct from type_name()'s "Range")
            Value::Range(_) => Cow::Borrowed("range"),
            _ => Cow::Borrowed(self.type_name()),
        }
    }

    /// Display value for user output (without type wrapper).
    pub fn display_value(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Str(s) => s.to_string(),
            Value::Char(c) => c.to_string(),
            Value::Byte(b) => format!("0x{b:02x}"),
            Value::Void => "void".to_string(),
            Value::List(items) => {
                let inner: Vec<_> = items.iter().map(Value::display_value).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Map(map) => {
                let inner: Vec<_> = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.display_value()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Set(items) => {
                let inner: Vec<_> = items.values().map(Value::display_value).collect();
                format!("Set {{{}}}", inner.join(", "))
            }
            Value::Tuple(items) => {
                let inner: Vec<_> = items.iter().map(Value::display_value).collect();
                format!("({})", inner.join(", "))
            }
            Value::Some(v) => format!("Some({})", v.display_value()),
            Value::None => "None".to_string(),
            Value::Ok(v) => format!("Ok({})", v.display_value()),
            Value::Err(v) => format!("Err({})", v.display_value()),
            Value::Variant { fields, .. } => {
                if fields.is_empty() {
                    "<variant>".to_string()
                } else {
                    let inner: Vec<_> = fields.iter().map(Value::display_value).collect();
                    format!("<variant>({})", inner.join(", "))
                }
            }
            Value::VariantConstructor { .. } => "<variant_constructor>".to_string(),
            Value::Newtype { inner, .. } => inner.display_value(),
            Value::NewtypeConstructor { .. } => "<newtype_constructor>".to_string(),
            Value::Struct(s) => format!("{s:?}"),
            Value::Function(_) | Value::MemoizedFunction(_) => "<function>".to_string(),
            Value::FunctionVal(_, name) => format!("<function_val {name}>"),
            Value::Duration(ns) => format_duration(*ns),
            Value::Size(bytes) => format!("{bytes}b"),
            Value::Ordering(ord) => ord.name().to_string(),
            Value::Range(r) => format!("{}", Value::Range(r.clone())),
            Value::Iterator(it) => format!("<iterator {it:?}>"),
            Value::ModuleNamespace(_) => "<module>".to_string(),
            Value::Error(ev) => format!("Error({})", ev.message()),
            Value::TypeRef { .. } => "<type>".to_string(),
        }
    }

    /// Convert a value to a map key string with type prefix for uniqueness.
    ///
    /// This ensures different types don't collide (e.g., int `1` vs string `"1"`).
    /// Only hashable types are valid as map keys.
    pub fn to_map_key(&self) -> Result<String, &'static str> {
        match self {
            Value::Int(n) => Ok(format!("i:{n}")),
            Value::Float(f) => Ok(format!("f:{f}")),
            Value::Bool(b) => Ok(format!("b:{b}")),
            Value::Str(s) => Ok(format!("s:{s}")),
            Value::Char(c) => Ok(format!("c:{c}")),
            Value::Byte(b) => Ok(format!("y:{b}")),
            Value::Duration(ns) => Ok(format!("d:{ns}")),
            Value::Size(bytes) => Ok(format!("z:{bytes}")),
            Value::Ordering(ord) => Ok(format!("o:{}", ord.to_tag())),
            Value::None => Ok("n:".to_string()),
            Value::Some(v) => {
                let inner = v.to_map_key()?;
                Ok(format!("S:{inner}"))
            }
            Value::Ok(v) => {
                let inner = v.to_map_key()?;
                Ok(format!("O:{inner}"))
            }
            Value::Err(v) => {
                let inner = v.to_map_key()?;
                Ok(format!("E:{inner}"))
            }
            Value::Tuple(items) => {
                let mut key = String::from("t:");
                for item in items.iter() {
                    key.push_str(&item.to_map_key()?);
                    key.push(';');
                }
                Ok(key)
            }
            // Non-hashable types cannot be map keys
            Value::Void
            | Value::List(_)
            | Value::Map(_)
            | Value::Set(_)
            | Value::Variant { .. }
            | Value::VariantConstructor { .. }
            | Value::Newtype { .. }
            | Value::NewtypeConstructor { .. }
            | Value::Struct(_)
            | Value::Function(_)
            | Value::MemoizedFunction(_)
            | Value::FunctionVal(_, _)
            | Value::Range(_)
            | Value::Iterator(_)
            | Value::ModuleNamespace(_)
            | Value::Error(_)
            | Value::TypeRef { .. } => Err("value is not hashable and cannot be a map key"),
        }
    }

    /// Check structural equality with another value.
    pub fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Byte(a), Value::Byte(b)) => a == b,
            (Value::Void, Value::Void) | (Value::None, Value::None) => true,
            (Value::Some(a), Value::Some(b))
            | (Value::Ok(a), Value::Ok(b))
            | (Value::Err(a), Value::Err(b)) => a.equals(b),
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Set(a), Value::Set(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|bv| v.equals(bv)))
            }
            (Value::Duration(a), Value::Duration(b)) => a == b,
            (Value::Size(a), Value::Size(b)) => a == b,
            (Value::Ordering(a), Value::Ordering(b)) => a == b,
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
            ) => {
                t1 == t2
                    && v1 == v2
                    && f1.len() == f2.len()
                    && f1.iter().zip(f2.iter()).all(|(x, y)| x.equals(y))
            }
            (
                Value::Newtype {
                    type_name: t1,
                    inner: i1,
                },
                Value::Newtype {
                    type_name: t2,
                    inner: i2,
                },
            ) => t1 == t2 && i1.equals(i2),
            _ => false,
        }
    }
}

/// Format a duration value (in nanoseconds) for display.
/// Uses the largest whole unit that doesn't lose precision.
pub(super) fn format_duration(ns: i64) -> String {
    // Unit constants (nanoseconds per unit)
    const HOUR_NS: u64 = 60 * 60 * 1_000_000_000;
    const MIN_NS: u64 = 60 * 1_000_000_000;
    const SEC_NS: u64 = 1_000_000_000;
    const MS_NS: u64 = 1_000_000;
    const US_NS: u64 = 1_000;

    let abs_ns = ns.unsigned_abs();
    let sign = if ns < 0 { "-" } else { "" };

    if abs_ns >= HOUR_NS && abs_ns.is_multiple_of(HOUR_NS) {
        let val = abs_ns / HOUR_NS;
        format!("{sign}{val}h")
    } else if abs_ns >= MIN_NS && abs_ns.is_multiple_of(MIN_NS) {
        let val = abs_ns / MIN_NS;
        format!("{sign}{val}m")
    } else if abs_ns >= SEC_NS && abs_ns.is_multiple_of(SEC_NS) {
        let val = abs_ns / SEC_NS;
        format!("{sign}{val}s")
    } else if abs_ns >= MS_NS && abs_ns.is_multiple_of(MS_NS) {
        let val = abs_ns / MS_NS;
        format!("{sign}{val}ms")
    } else if abs_ns >= US_NS && abs_ns.is_multiple_of(US_NS) {
        let val = abs_ns / US_NS;
        format!("{sign}{val}us")
    } else {
        format!("{sign}{abs_ns}ns")
    }
}
