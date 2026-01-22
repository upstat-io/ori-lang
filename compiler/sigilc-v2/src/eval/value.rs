//! Runtime values for the Sigil interpreter.

use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use crate::intern::Name;
use crate::syntax::ExprId;

/// Built-in function signature.
pub type BuiltinFn = fn(&[Value]) -> Result<Value, String>;

/// Runtime value in the Sigil interpreter.
#[derive(Clone)]
pub enum Value {
    /// Integer value.
    Int(i64),
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    Str(Rc<String>),
    /// Character value.
    Char(char),
    /// Byte value.
    Byte(u8),
    /// Void (unit) value.
    Void,
    /// List of values.
    List(Rc<Vec<Value>>),
    /// Map from string keys to values.
    Map(Rc<HashMap<String, Value>>),
    /// Tuple of values.
    Tuple(Rc<Vec<Value>>),
    /// Option: Some(value).
    Some(Box<Value>),
    /// Option: None.
    None,
    /// Result: Ok(value).
    Ok(Box<Value>),
    /// Result: Err(error).
    Err(Box<Value>),
    /// Struct instance.
    Struct(StructValue),
    /// Function value (closure).
    Function(FunctionValue),
    /// Built-in function.
    Builtin(BuiltinFn, &'static str),
    /// Duration value (in milliseconds).
    Duration(u64),
    /// Size value (in bytes).
    Size(u64),
    /// Range value.
    Range(RangeValue),
    /// Error value for error recovery.
    Error(String),
}

/// Struct instance with efficient field access.
#[derive(Clone, Debug)]
pub struct StructValue {
    /// Type name of the struct.
    pub type_name: Name,
    /// Field values in layout order.
    pub fields: Rc<Vec<Value>>,
    /// Layout for O(1) field access.
    pub layout: Rc<StructLayout>,
}

/// Layout information for O(1) struct field access.
#[derive(Clone, Debug)]
pub struct StructLayout {
    /// Map from field name to index.
    field_indices: HashMap<Name, usize>,
}

impl StructLayout {
    /// Create a new struct layout from field names.
    pub fn new(field_names: &[Name]) -> Self {
        let field_indices = field_names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, i))
            .collect();
        StructLayout { field_indices }
    }

    /// Get the index of a field by name.
    pub fn get_index(&self, field: Name) -> Option<usize> {
        self.field_indices.get(&field).copied()
    }

    /// Get the number of fields.
    pub fn len(&self) -> usize {
        self.field_indices.len()
    }

    /// Check if the layout has no fields.
    pub fn is_empty(&self) -> bool {
        self.field_indices.is_empty()
    }
}

impl StructValue {
    /// Create a new struct value from a name and field values.
    pub fn new(name: Name, field_values: HashMap<Name, Value>) -> Self {
        let field_names: Vec<Name> = field_values.keys().cloned().collect();
        let layout = Rc::new(StructLayout::new(&field_names));
        let mut fields = vec![Value::Void; field_names.len()];
        for (name, value) in field_values {
            if let Some(idx) = layout.get_index(name) {
                fields[idx] = value;
            }
        }
        StructValue {
            type_name: name,
            fields: Rc::new(fields),
            layout,
        }
    }

    /// Alias for type_name field access.
    pub fn name(&self) -> Name {
        self.type_name
    }

    /// Get a field value by name with O(1) lookup.
    pub fn get_field(&self, field: Name) -> Option<&Value> {
        let index = self.layout.get_index(field)?;
        self.fields.get(index)
    }
}

/// Function value (closure).
#[derive(Clone)]
pub struct FunctionValue {
    /// Parameter names.
    pub params: Vec<Name>,
    /// Body expression.
    pub body: ExprId,
    /// Captured environment (for closures).
    pub captures: Rc<RefCell<HashMap<Name, Value>>>,
}

impl fmt::Debug for FunctionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionValue")
            .field("params", &self.params)
            .field("body", &self.body)
            .finish()
    }
}

/// Range value.
#[derive(Clone, Debug)]
pub struct RangeValue {
    /// Start of range (inclusive).
    pub start: i64,
    /// End of range.
    pub end: i64,
    /// Whether end is inclusive.
    pub inclusive: bool,
}

impl RangeValue {
    /// Create an exclusive range.
    pub fn exclusive(start: i64, end: i64) -> Self {
        RangeValue { start, end, inclusive: false }
    }

    /// Create an inclusive range.
    pub fn inclusive(start: i64, end: i64) -> Self {
        RangeValue { start, end, inclusive: true }
    }

    /// Iterate over the range values.
    pub fn iter(&self) -> impl Iterator<Item = i64> {
        let end = if self.inclusive { self.end + 1 } else { self.end };
        self.start..end
    }

    /// Get the length of the range.
    pub fn len(&self) -> usize {
        let end = if self.inclusive { self.end + 1 } else { self.end };
        (end - self.start).max(0) as usize
    }

    /// Check if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a value is contained in the range.
    pub fn contains(&self, value: i64) -> bool {
        if self.inclusive {
            value >= self.start && value <= self.end
        } else {
            value >= self.start && value < self.end
        }
    }
}

impl Value {
    /// Check if this value is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Str(s) => !s.is_empty(),
            Value::List(items) => !items.is_empty(),
            Value::None => false,
            Value::Err(_) => false,
            Value::Void => false,
            _ => true,
        }
    }

    /// Try to convert to an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Try to convert to a float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n) => Some(*n as f64),
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

    /// Try to convert to a list.
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
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
            Value::Tuple(_) => "tuple",
            Value::Some(_) => "Option",
            Value::None => "Option",
            Value::Ok(_) => "Result",
            Value::Err(_) => "Result",
            Value::Struct(_) => "struct",
            Value::Function(_) => "function",
            Value::Builtin(_, _) => "builtin",
            Value::Duration(_) => "Duration",
            Value::Size(_) => "Size",
            Value::Range(_) => "Range",
            Value::Error(_) => "error",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "Int({})", n),
            Value::Float(n) => write!(f, "Float({})", n),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Str(s) => write!(f, "Str({:?})", s),
            Value::Char(c) => write!(f, "Char({:?})", c),
            Value::Byte(b) => write!(f, "Byte({:?})", b),
            Value::Void => write!(f, "Void"),
            Value::List(items) => write!(f, "List({:?})", items),
            Value::Map(map) => write!(f, "Map({:?})", map),
            Value::Tuple(items) => write!(f, "Tuple({:?})", items),
            Value::Some(v) => write!(f, "Some({:?})", v),
            Value::None => write!(f, "None"),
            Value::Ok(v) => write!(f, "Ok({:?})", v),
            Value::Err(v) => write!(f, "Err({:?})", v),
            Value::Struct(s) => write!(f, "Struct({:?})", s),
            Value::Function(func) => write!(f, "Function({:?})", func),
            Value::Builtin(_, name) => write!(f, "Builtin({})", name),
            Value::Duration(ms) => write!(f, "Duration({}ms)", ms),
            Value::Size(bytes) => write!(f, "Size({}b)", bytes),
            Value::Range(r) => write!(f, "Range({:?})", r),
            Value::Error(msg) => write!(f, "Error({})", msg),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "\"{}\"", s),
            Value::Char(c) => write!(f, "'{}'", c),
            Value::Byte(b) => write!(f, "0x{:02x}", b),
            Value::Void => write!(f, "void"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Map(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
            Value::Some(v) => write!(f, "Some({})", v),
            Value::None => write!(f, "None"),
            Value::Ok(v) => write!(f, "Ok({})", v),
            Value::Err(e) => write!(f, "Err({})", e),
            Value::Struct(s) => write!(f, "<struct {:?}>", s.type_name),
            Value::Function(_) => write!(f, "<function>"),
            Value::Builtin(_, name) => write!(f, "<builtin {}>", name),
            Value::Duration(ms) => {
                if *ms >= 1000 {
                    write!(f, "{}s", ms / 1000)
                } else {
                    write!(f, "{}ms", ms)
                }
            }
            Value::Size(bytes) => {
                if *bytes >= 1024 * 1024 {
                    write!(f, "{}mb", bytes / (1024 * 1024))
                } else if *bytes >= 1024 {
                    write!(f, "{}kb", bytes / 1024)
                } else {
                    write!(f, "{}b", bytes)
                }
            }
            Value::Range(r) => {
                if r.inclusive {
                    write!(f, "{}..={}", r.start, r.end)
                } else {
                    write!(f, "{}..{}", r.start, r.end)
                }
            }
            Value::Error(msg) => write!(f, "<error: {}>", msg),
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
            (Value::Void, Value::Void) => true,
            (Value::None, Value::None) => true,
            (Value::Some(a), Value::Some(b)) => a == b,
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Builtin(_, name_a), Value::Builtin(_, name_b)) => name_a == name_b,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_truthy() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(!Value::None.is_truthy());
    }

    #[test]
    fn test_value_display() {
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Str(Rc::new("hello".to_string()))), "\"hello\"");
    }

    #[test]
    fn test_range_iter() {
        let range = RangeValue::exclusive(0, 5);
        let values: Vec<_> = range.iter().collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4]);

        let range = RangeValue::inclusive(0, 5);
        let values: Vec<_> = range.iter().collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_struct_layout() {
        use crate::intern::StringInterner;
        let interner = StringInterner::new();
        let field_a = interner.intern("a");
        let field_b = interner.intern("b");

        let layout = StructLayout::new(&[field_a, field_b]);
        assert_eq!(layout.get_index(field_a), Some(0));
        assert_eq!(layout.get_index(field_b), Some(1));
        assert_eq!(layout.len(), 2);
    }
}
