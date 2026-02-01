//! Salsa-compatible evaluation output types.
//!
//! These types are designed for use in Salsa queries, requiring
//! Clone + Eq + `PartialEq` + Hash + Debug traits.

use super::value::Value;
use crate::ir::StringInterner;
use std::hash::{Hash, Hasher};

/// Salsa-compatible representation of an evaluated value.
///
/// Unlike `Value`, this type has Clone + Eq + Hash for use in Salsa queries.
/// Complex values (functions, structs) are represented as strings.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug)]
pub enum EvalOutput {
    /// Integer value.
    Int(i64),
    /// Floating-point value (stored as bits for Hash).
    Float(u64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    Str(String),
    /// Character value.
    Char(char),
    /// Byte value.
    Byte(u8),
    /// Void (unit) value.
    Void,
    /// List of values.
    List(Vec<EvalOutput>),
    /// Tuple of values.
    Tuple(Vec<EvalOutput>),
    /// Option: Some(value).
    Some(Box<EvalOutput>),
    /// Option: None.
    None,
    /// Result: Ok(value).
    Ok(Box<EvalOutput>),
    /// Result: Err(error).
    Err(Box<EvalOutput>),
    /// Duration in nanoseconds.
    Duration(i64),
    /// Size in bytes.
    Size(u64),
    /// Ordering value (Less=0, Equal=1, Greater=2).
    Ordering(i8),
    /// Range value.
    Range {
        start: i64,
        end: i64,
        inclusive: bool,
    },
    /// Function (not directly representable, stored as description).
    Function(String),
    /// Struct (stored as description).
    Struct(String),
    /// User-defined variant.
    Variant {
        type_name: String,
        variant_name: String,
        fields: Vec<EvalOutput>,
    },
    /// Map (stored as key-value pairs).
    Map(Vec<(String, EvalOutput)>),
    /// Error during evaluation.
    Error(String),
}

impl EvalOutput {
    /// Convert a runtime Value to a Salsa-compatible `EvalOutput`.
    pub fn from_value(value: &Value, interner: &StringInterner) -> Self {
        match value {
            Value::Int(n) => EvalOutput::Int(n.raw()),
            Value::Float(f) => EvalOutput::Float(f.to_bits()),
            Value::Bool(b) => EvalOutput::Bool(*b),
            Value::Str(s) => EvalOutput::Str(s.to_string()),
            Value::Char(c) => EvalOutput::Char(*c),
            Value::Byte(b) => EvalOutput::Byte(*b),
            Value::Void => EvalOutput::Void,
            Value::List(items) => EvalOutput::List(
                items
                    .iter()
                    .map(|v| Self::from_value(v, interner))
                    .collect(),
            ),
            Value::Tuple(items) => EvalOutput::Tuple(
                items
                    .iter()
                    .map(|v| Self::from_value(v, interner))
                    .collect(),
            ),
            Value::Some(v) => EvalOutput::Some(Box::new(Self::from_value(v, interner))),
            Value::None => EvalOutput::None,
            Value::Ok(v) => EvalOutput::Ok(Box::new(Self::from_value(v, interner))),
            Value::Err(v) => EvalOutput::Err(Box::new(Self::from_value(v, interner))),
            Value::Duration(ms) => EvalOutput::Duration(*ms),
            Value::Size(bytes) => EvalOutput::Size(*bytes),
            Value::Ordering(ord) => EvalOutput::Ordering(ord.to_tag()),
            Value::Range(r) => EvalOutput::Range {
                start: r.start,
                end: r.end,
                inclusive: r.inclusive,
            },
            Value::Function(f) => {
                EvalOutput::Function(format!("<function with {} params>", f.params.len()))
            }
            Value::MemoizedFunction(mf) => EvalOutput::Function(format!(
                "<memoized function with {} params>",
                mf.func.params.len()
            )),
            Value::FunctionVal(_, name) => EvalOutput::Function(format!("<{name}>")),
            Value::Struct(s) => {
                EvalOutput::Struct(format!("<struct {}>", interner.lookup(s.name())))
            }
            Value::Variant {
                type_name,
                variant_name,
                fields,
            } => EvalOutput::Variant {
                type_name: interner.lookup(*type_name).to_string(),
                variant_name: interner.lookup(*variant_name).to_string(),
                fields: fields
                    .iter()
                    .map(|v| Self::from_value(v, interner))
                    .collect(),
            },
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                let type_str = interner.lookup(*type_name);
                let variant_str = interner.lookup(*variant_name);
                EvalOutput::Function(format!(
                    "<{type_str}::{variant_str} constructor ({field_count} fields)>"
                ))
            }
            Value::Newtype { type_name, inner } => {
                // Display newtype by showing the wrapped value
                let type_str = interner.lookup(*type_name);
                let inner_output = Self::from_value(inner, interner);
                EvalOutput::Struct(format!("{type_str}({inner_output:?})"))
            }
            Value::NewtypeConstructor { type_name } => {
                let type_str = interner.lookup(*type_name);
                EvalOutput::Function(format!("<{type_str} constructor>"))
            }
            Value::Map(map) => {
                let entries: Vec<_> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::from_value(v, interner)))
                    .collect();
                EvalOutput::Map(entries)
            }
            Value::ModuleNamespace(ns) => {
                EvalOutput::Function(format!("<module namespace with {} items>", ns.len()))
            }
            Value::Error(msg) => EvalOutput::Error(msg.clone()),
            Value::TypeRef { type_name } => {
                let type_str = interner.lookup(*type_name);
                EvalOutput::Function(format!("<type {type_str}>"))
            }
        }
    }

    /// Get a display string for this output.
    pub fn display(&self) -> String {
        match self {
            EvalOutput::Int(n) => n.to_string(),
            EvalOutput::Float(bits) => f64::from_bits(*bits).to_string(),
            EvalOutput::Bool(b) => b.to_string(),
            EvalOutput::Str(s) => format!("\"{s}\""),
            EvalOutput::Char(c) => format!("'{c}'"),
            EvalOutput::Byte(b) => format!("0x{b:02x}"),
            EvalOutput::Void => "void".to_string(),
            EvalOutput::List(items) => {
                let inner: Vec<_> = items.iter().map(EvalOutput::display).collect();
                format!("[{}]", inner.join(", "))
            }
            EvalOutput::Tuple(items) => {
                let inner: Vec<_> = items.iter().map(EvalOutput::display).collect();
                format!("({})", inner.join(", "))
            }
            EvalOutput::Some(v) => format!("Some({})", v.display()),
            EvalOutput::None => "None".to_string(),
            EvalOutput::Ok(v) => format!("Ok({})", v.display()),
            EvalOutput::Err(v) => format!("Err({})", v.display()),
            EvalOutput::Duration(ms) => format!("{ms}ms"),
            EvalOutput::Size(bytes) => format!("{bytes}b"),
            EvalOutput::Ordering(tag) => match tag {
                0 => "Less".to_string(),
                1 => "Equal".to_string(),
                2 => "Greater".to_string(),
                _ => format!("Ordering({tag})"),
            },
            EvalOutput::Range {
                start,
                end,
                inclusive,
            } => {
                if *inclusive {
                    format!("{start}..={end}")
                } else {
                    format!("{start}..{end}")
                }
            }
            EvalOutput::Function(desc) | EvalOutput::Struct(desc) => desc.clone(),
            EvalOutput::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                if fields.is_empty() {
                    format!("{type_name}::{variant_name}")
                } else {
                    let inner: Vec<_> = fields.iter().map(EvalOutput::display).collect();
                    format!("{type_name}::{variant_name}({})", inner.join(", "))
                }
            }
            EvalOutput::Map(entries) => {
                let inner: Vec<_> = entries
                    .iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v.display()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            EvalOutput::Error(msg) => format!("<error: {msg}>"),
        }
    }
}

impl PartialEq for EvalOutput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // i64 types (Int, Duration in nanoseconds)
            (EvalOutput::Int(a), EvalOutput::Int(b))
            | (EvalOutput::Duration(a), EvalOutput::Duration(b)) => a == b,
            (EvalOutput::Bool(a), EvalOutput::Bool(b)) => a == b,
            (EvalOutput::Byte(a), EvalOutput::Byte(b)) => a == b,
            // i8 types (Ordering tag)
            (EvalOutput::Ordering(a), EvalOutput::Ordering(b)) => a == b,
            (EvalOutput::Char(a), EvalOutput::Char(b)) => a == b,
            // u64 types (Float stored as bits, Size in bytes)
            (EvalOutput::Float(a), EvalOutput::Float(b))
            | (EvalOutput::Size(a), EvalOutput::Size(b)) => a == b,
            // String types can be merged
            (EvalOutput::Str(a), EvalOutput::Str(b))
            | (EvalOutput::Function(a), EvalOutput::Function(b))
            | (EvalOutput::Struct(a), EvalOutput::Struct(b))
            | (EvalOutput::Error(a), EvalOutput::Error(b)) => a == b,
            // Vec<EvalOutput> types can be merged
            (EvalOutput::List(a), EvalOutput::List(b))
            | (EvalOutput::Tuple(a), EvalOutput::Tuple(b)) => a == b,
            // Box<EvalOutput> types can be merged
            (EvalOutput::Some(a), EvalOutput::Some(b))
            | (EvalOutput::Ok(a), EvalOutput::Ok(b))
            | (EvalOutput::Err(a), EvalOutput::Err(b)) => a == b,
            (EvalOutput::Map(a), EvalOutput::Map(b)) => a == b,
            // Unit types
            (EvalOutput::Void, EvalOutput::Void) | (EvalOutput::None, EvalOutput::None) => true,
            // Range with multiple fields
            (
                EvalOutput::Range {
                    start: s1,
                    end: e1,
                    inclusive: i1,
                },
                EvalOutput::Range {
                    start: s2,
                    end: e2,
                    inclusive: i2,
                },
            ) => s1 == s2 && e1 == e2 && i1 == i2,
            // Variant with type, variant name, and fields
            (
                EvalOutput::Variant {
                    type_name: t1,
                    variant_name: v1,
                    fields: f1,
                },
                EvalOutput::Variant {
                    type_name: t2,
                    variant_name: v2,
                    fields: f2,
                },
            ) => t1 == t2 && v1 == v2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for EvalOutput {}

impl Hash for EvalOutput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            EvalOutput::Int(n) => n.hash(state),
            EvalOutput::Bool(b) => b.hash(state),
            EvalOutput::Char(c) => c.hash(state),
            EvalOutput::Byte(b) => b.hash(state),
            EvalOutput::Ordering(tag) => tag.hash(state),
            // u64 types
            EvalOutput::Float(bits) | EvalOutput::Size(bits) => {
                bits.hash(state);
            }
            // i64 types
            EvalOutput::Duration(ns) => ns.hash(state),
            // String types
            EvalOutput::Str(s)
            | EvalOutput::Function(s)
            | EvalOutput::Struct(s)
            | EvalOutput::Error(s) => s.hash(state),
            // Vec<EvalOutput> types
            EvalOutput::List(items) | EvalOutput::Tuple(items) => items.hash(state),
            // Box<EvalOutput> types
            EvalOutput::Some(v) | EvalOutput::Ok(v) | EvalOutput::Err(v) => v.hash(state),
            EvalOutput::Map(entries) => entries.hash(state),
            // Unit types
            EvalOutput::Void | EvalOutput::None => {}
            EvalOutput::Range {
                start,
                end,
                inclusive,
            } => {
                start.hash(state);
                end.hash(state);
                inclusive.hash(state);
            }
            EvalOutput::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                type_name.hash(state);
                variant_name.hash(state);
                fields.hash(state);
            }
        }
    }
}

/// Result of evaluating a module.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleEvalResult {
    /// The result value (if evaluation succeeded).
    pub result: Option<EvalOutput>,
    /// Error message (if evaluation failed).
    pub error: Option<String>,
    /// Captured stdout output (if any).
    pub stdout: String,
}

impl ModuleEvalResult {
    /// Create a successful result.
    pub fn success(result: EvalOutput) -> Self {
        ModuleEvalResult {
            result: Some(result),
            error: None,
            stdout: String::new(),
        }
    }

    /// Create an error result.
    pub fn failure(error: String) -> Self {
        ModuleEvalResult {
            result: None,
            error: Some(error),
            stdout: String::new(),
        }
    }

    /// Check if evaluation succeeded.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Check if evaluation failed.
    pub fn is_failure(&self) -> bool {
        self.error.is_some()
    }
}

impl Default for ModuleEvalResult {
    fn default() -> Self {
        ModuleEvalResult {
            result: Some(EvalOutput::Void),
            error: None,
            stdout: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SharedInterner;

    #[test]
    fn test_eval_output_from_value() {
        let interner = SharedInterner::default();

        assert_eq!(
            EvalOutput::from_value(&Value::int(42), &interner),
            EvalOutput::Int(42)
        );
        assert_eq!(
            EvalOutput::from_value(&Value::Bool(true), &interner),
            EvalOutput::Bool(true)
        );
        assert_eq!(
            EvalOutput::from_value(&Value::Void, &interner),
            EvalOutput::Void
        );
        assert_eq!(
            EvalOutput::from_value(&Value::None, &interner),
            EvalOutput::None
        );
    }

    #[test]
    fn test_eval_output_display() {
        assert_eq!(EvalOutput::Int(42).display(), "42");
        assert_eq!(EvalOutput::Bool(true).display(), "true");
        assert_eq!(EvalOutput::Void.display(), "void");
        assert_eq!(EvalOutput::None.display(), "None");
        assert_eq!(
            EvalOutput::Some(Box::new(EvalOutput::Int(1))).display(),
            "Some(1)"
        );
        assert_eq!(
            EvalOutput::Ok(Box::new(EvalOutput::Int(1))).display(),
            "Ok(1)"
        );
        assert_eq!(
            EvalOutput::List(vec![EvalOutput::Int(1), EvalOutput::Int(2)]).display(),
            "[1, 2]"
        );
        assert_eq!(
            EvalOutput::Tuple(vec![EvalOutput::Int(1), EvalOutput::Bool(true)]).display(),
            "(1, true)"
        );
    }

    #[test]
    fn test_eval_output_equality() {
        assert_eq!(EvalOutput::Int(42), EvalOutput::Int(42));
        assert_ne!(EvalOutput::Int(42), EvalOutput::Int(43));
        assert_ne!(EvalOutput::Int(42), EvalOutput::Bool(true));

        assert_eq!(
            EvalOutput::List(vec![EvalOutput::Int(1)]),
            EvalOutput::List(vec![EvalOutput::Int(1)])
        );
    }

    #[test]
    fn test_module_eval_result() {
        let success = ModuleEvalResult::success(EvalOutput::Int(42));
        assert!(success.is_success());
        assert!(!success.is_failure());

        let failure = ModuleEvalResult::failure("test error".to_string());
        assert!(!failure.is_success());
        assert!(failure.is_failure());
    }
}
