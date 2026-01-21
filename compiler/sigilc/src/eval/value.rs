// Runtime value types and environment
//
// This module contains the core types used throughout the evaluator.
// Other eval submodules depend on this module.

use crate::ast::{Expr, FunctionDef};
use crate::core::Binding as CoreBinding;
use std::collections::HashMap;
use std::fmt;

/// Local variable binding - alias for core Binding<Value>
pub type Binding = CoreBinding<Value>;

/// Runtime values
#[derive(Debug, Clone)]
pub enum Value {
    /// Integer value
    Int(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Boolean value
    Bool(bool),
    /// Nil/null value
    Nil,
    /// List of values
    List(Vec<Value>),
    /// Map/dictionary of string keys to values
    Map(HashMap<String, Value>),
    /// Tuple of values
    Tuple(Vec<Value>),
    /// Struct with named fields
    Struct {
        name: String,
        fields: HashMap<String, Value>,
    },
    /// User-defined function (closure)
    Function {
        params: Vec<String>,
        body: Expr,
        env: HashMap<String, Value>,
    },
    /// Builtin operator function (+, *, etc.)
    BuiltinFunction(String),
    /// Ok variant of Result type
    Ok(Box<Value>),
    /// Err variant of Result type
    Err(Box<Value>),
    /// Some variant of Option type
    Some(Box<Value>),
    /// None variant of Option type
    None_,
}

/// Implement PartialEq for Value following Rust's pattern
/// Functions are compared by identity (always not equal to other functions)
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::None_, Value::None_) => true,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
            (Value::Some(a), Value::Some(b)) => a == b,
            (Value::BuiltinFunction(a), Value::BuiltinFunction(b)) => a == b,
            // Functions are never equal (like Rust closures)
            (Value::Function { .. }, Value::Function { .. }) => false,
            // Structs compare by name and fields
            (
                Value::Struct {
                    name: n1,
                    fields: f1,
                },
                Value::Struct {
                    name: n2,
                    fields: f2,
                },
            ) => n1 == n2 && f1.len() == f2.len() && f1.iter().all(|(k, v)| f2.get(k) == Some(v)),
            // Maps compare by contents
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len() && a.iter().all(|(k, v)| b.get(k) == Some(v))
            }
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
            Value::Struct { name, fields } => {
                write!(f, "{} {{ ", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, " }}")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Function { .. } => write!(f, "<function>"),
            Value::BuiltinFunction(name) => write!(f, "<builtin:{}>", name),
            Value::Ok(v) => write!(f, "Ok({})", v),
            Value::Err(v) => write!(f, "Err({})", v),
            Value::Some(v) => write!(f, "Some({})", v),
            Value::None_ => write!(f, "None"),
        }
    }
}

/// Extension methods for Binding to provide backward compatibility
pub trait BindingExt {
    /// Get the value of this binding.
    fn value(&self) -> &Value;
    /// Check if this binding is mutable.
    fn mutable(&self) -> bool;
}

impl BindingExt for Binding {
    fn value(&self) -> &Value {
        self.get()
    }
    fn mutable(&self) -> bool {
        self.is_mutable()
    }
}

/// Runtime environment
pub struct Environment {
    /// Global config values
    pub configs: HashMap<String, Value>,
    /// Global functions
    pub functions: HashMap<String, FunctionDef>,
    /// Local variables with mutability tracking
    pub locals: HashMap<String, Binding>,
    /// Current function parameter names (in order, for recursion)
    pub current_params: Vec<String>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            configs: HashMap::new(),
            functions: HashMap::new(),
            locals: HashMap::new(),
            current_params: Vec::new(),
        }
    }

    /// Get the value of a local variable
    pub fn get(&self, name: &str) -> Option<Value> {
        self.locals.get(name).map(|b| b.cloned())
    }

    /// Define a new local variable with mutability
    pub fn define(&mut self, name: String, value: Value, mutable: bool) {
        self.locals.insert(name, Binding::new(value, mutable));
    }

    /// Set (reassign) an existing mutable variable
    /// Returns an error if the variable doesn't exist or isn't mutable
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        match self.locals.get_mut(name) {
            Some(binding) if binding.is_mutable() => {
                // Use the internal set method which will always succeed here
                let _ = binding.set(value);
                Ok(())
            }
            Some(_) => Err(format!(
                "Cannot assign to immutable variable '{}'",
                name
            )),
            None => Err(format!("Variable '{}' not found", name)),
        }
    }

    /// Legacy set method for backwards compatibility (treats as mutable)
    pub fn set_mutable(&mut self, name: String, value: Value) {
        self.locals.insert(name, Binding::mutable(value));
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    pub fn define_function(&mut self, name: String, def: FunctionDef) {
        self.functions.insert(name, def);
    }

    pub fn set_config(&mut self, name: String, value: Value) {
        self.configs.insert(name, value);
    }

    /// Get a HashMap of just the values (for lambda captures)
    /// This extracts just the values, discarding mutability info
    pub fn locals_values(&self) -> HashMap<String, Value> {
        self.locals
            .iter()
            .map(|(k, b)| (k.clone(), b.cloned()))
            .collect()
    }

    /// Create locals from a HashMap of values (for restoring closure env)
    /// All restored values are treated as immutable (captures are immutable)
    pub fn locals_from_values(values: HashMap<String, Value>) -> HashMap<String, Binding> {
        values
            .into_iter()
            .map(|(k, v)| (k, Binding::immutable(v)))
            .collect()
    }
}

/// Check if a value is truthy
pub fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::Nil => false,
        Value::None_ => false,
        Value::Int(0) => false,
        Value::String(s) if s.is_empty() => false,
        Value::List(l) if l.is_empty() => false,
        _ => true,
    }
}
