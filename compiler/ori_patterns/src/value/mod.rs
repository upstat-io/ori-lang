//! Runtime values for the Ori interpreter.
//!
//! # Arc Enforcement Architecture
//!
//! This module enforces that all heap allocations go through factory methods
//! on `Value`. The `Heap<T>` wrapper type has a private constructor, so
//! external code cannot create heap values directly.
//!
//! ## Correct Usage
//!
//! ```text
//! let s = Value::string("hello");        // OK
//! let list = Value::list(vec![]);        // OK
//! let opt = Value::some(Value::int(42)); // OK
//! ```
//!
//! ## Prevented (Won't Compile)
//!
//! ```text
//! let s = Value::Str(Heap::new(...));    // ERROR: Heap::new is pub(super)
//! let list = Value::List(Arc::new(...)); // ERROR: Expected Heap, got Arc
//! ```
//!
//! # Thread Safety
//!
//! All heap types use `Arc` internally for thread-safe reference counting.
//! The `FunctionValue` type uses immutable captures (no `RwLock`), eliminating
//! potential race conditions.

mod composite;
mod heap;
mod scalar_int;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

// Re-export StringLookup from ori_ir for convenience
pub use ori_ir::{Name, StringLookup};

pub use composite::{FunctionValue, MemoizedFunctionValue, RangeValue, StructLayout, StructValue};
pub use heap::Heap;
pub use scalar_int::ScalarInt;

/// Type conversion function signature.
///
/// `function_val`: type conversion functions like int(x), str(x), float(x)
/// that allow positional arguments per the spec.
pub type FunctionValFn = fn(&[Value]) -> Result<Value, String>;

/// Runtime value in the Ori interpreter.
#[derive(Clone)]
pub enum Value {
    // Primitives (inline, no heap allocation)
    /// Integer value (uses `ScalarInt` to prevent unchecked arithmetic).
    Int(ScalarInt),
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Character value.
    Char(char),
    /// Byte value.
    Byte(u8),
    /// Void (unit) value.
    Void,
    /// Duration value (in milliseconds).
    Duration(u64),
    /// Size value (in bytes).
    Size(u64),

    // Heap Types (use Heap<T> for enforced Arc usage)
    /// String value.
    Str(Heap<String>),
    /// List of values.
    List(Heap<Vec<Value>>),
    /// Map from string keys to values.
    Map(Heap<HashMap<String, Value>>),
    /// Tuple of values.
    Tuple(Heap<Vec<Value>>),

    // Algebraic Types (use Heap<T> for consistency)
    /// Option: Some(value).
    Some(Heap<Value>),
    /// Option: None.
    None,
    /// Result: Ok(value).
    Ok(Heap<Value>),
    /// Result: Err(error).
    Err(Heap<Value>),
    /// User-defined sum type variant.
    ///
    /// Stores the type name (e.g., "Status"), variant name (e.g., "Running"),
    /// and variant fields (may be empty for unit variants).
    Variant {
        type_name: Name,
        variant_name: Name,
        fields: Heap<Vec<Value>>,
    },
    /// Variant constructor for sum types with fields.
    ///
    /// When called with arguments, constructs a `Value::Variant`.
    /// Used for variants like `Done(reason: str)` where the variant
    /// has fields that need to be provided.
    VariantConstructor {
        type_name: Name,
        variant_name: Name,
        field_count: usize,
    },

    /// Newtype wrapper value.
    ///
    /// Newtypes are nominally distinct wrappers around an existing type.
    /// For example, `type UserId = str` creates a `UserId` newtype.
    Newtype { type_name: Name, inner: Heap<Value> },
    /// Newtype constructor.
    ///
    /// When called with one argument, constructs a `Value::Newtype`.
    /// Used for newtypes like `UserId("abc")`.
    NewtypeConstructor { type_name: Name },

    // Composite Types
    /// Struct instance.
    Struct(StructValue),
    /// Function value (closure).
    Function(FunctionValue),
    /// Memoized function value (closure with result caching).
    ///
    /// Used by the `recurse` pattern with `memo: true` to cache results
    /// of recursive calls, enabling efficient algorithms like memoized Fibonacci.
    MemoizedFunction(MemoizedFunctionValue),
    /// Type conversion function (`function_val`).
    /// Examples: int(x), str(x), float(x), byte(x)
    FunctionVal(FunctionValFn, &'static str),
    /// Range value.
    Range(RangeValue),

    /// Module namespace for qualified access.
    ///
    /// Created by module alias imports like `use std.net.http as http`.
    /// Enables qualified access like `http.get()`.
    ModuleNamespace(Heap<HashMap<Name, Value>>),

    // Error Recovery
    /// Error value for error recovery.
    Error(String),
}

// Factory Methods (ONLY way to construct heap values)

impl Value {
    /// Create an integer value from a raw `i64`.
    ///
    /// This is the preferred way to construct `Value::Int` — it wraps the
    /// raw integer in `ScalarInt` automatically.
    #[inline]
    pub fn int(n: i64) -> Self {
        Value::Int(ScalarInt::new(n))
    }

    /// Create a string value.
    ///
    /// # Example
    ///
    /// ```text
    /// let s = Value::string("hello");
    /// let s2 = Value::string(format!("value: {}", x));
    /// ```
    #[inline]
    pub fn string(s: impl Into<String>) -> Self {
        Value::Str(Heap::new(s.into()))
    }

    /// Create a list value.
    ///
    /// # Example
    ///
    /// ```text
    /// let empty = Value::list(vec![]);
    /// let nums = Value::list(vec![Value::int(1), Value::int(2)]);
    /// ```
    #[inline]
    pub fn list(items: Vec<Value>) -> Self {
        Value::List(Heap::new(items))
    }

    /// Create a map value with String keys.
    ///
    /// # Example
    ///
    /// ```text
    /// let empty = Value::map(HashMap::new());
    /// ```
    #[inline]
    pub fn map(entries: HashMap<String, Value>) -> Self {
        Value::Map(Heap::new(entries))
    }

    /// Create a tuple value.
    ///
    /// # Example
    ///
    /// ```text
    /// let pair = Value::tuple(vec![Value::int(1), Value::Bool(true)]);
    /// ```
    #[inline]
    pub fn tuple(items: Vec<Value>) -> Self {
        Value::Tuple(Heap::new(items))
    }

    /// Create a Some value.
    ///
    /// # Example
    ///
    /// ```text
    /// let some = Value::some(Value::int(42));
    /// ```
    #[inline]
    pub fn some(v: Value) -> Self {
        Value::Some(Heap::new(v))
    }

    /// Create an Ok value.
    ///
    /// # Example
    ///
    /// ```text
    /// let ok = Value::ok(Value::int(42));
    /// let ok_void = Value::ok(Value::Void);
    /// ```
    #[inline]
    pub fn ok(v: Value) -> Self {
        Value::Ok(Heap::new(v))
    }

    /// Create an Err value.
    ///
    /// # Example
    ///
    /// ```text
    /// let err = Value::err(Value::string("something went wrong"));
    /// ```
    #[inline]
    pub fn err(v: Value) -> Self {
        Value::Err(Heap::new(v))
    }

    /// Create a user-defined variant value.
    ///
    /// # Example
    ///
    /// ```text
    /// // Unit variant: Status::Running
    /// let running = Value::variant(status_name, running_name, vec![]);
    ///
    /// // Variant with fields: Result::Success(value: 42)
    /// let success = Value::variant(result_name, success_name, vec![Value::int(42)]);
    /// ```
    #[inline]
    pub fn variant(type_name: Name, variant_name: Name, fields: Vec<Value>) -> Self {
        Value::Variant {
            type_name,
            variant_name,
            fields: Heap::new(fields),
        }
    }

    /// Create a variant constructor for sum types with fields.
    ///
    /// # Example
    ///
    /// ```text
    /// // Constructor for Done(reason: str) variant
    /// let done_ctor = Value::variant_constructor(status_name, done_name, 1);
    /// ```
    #[inline]
    pub fn variant_constructor(type_name: Name, variant_name: Name, field_count: usize) -> Self {
        Value::VariantConstructor {
            type_name,
            variant_name,
            field_count,
        }
    }

    /// Create a newtype value.
    ///
    /// # Example
    ///
    /// ```text
    /// // Create a UserId newtype wrapping a string
    /// let user_id = Value::newtype(user_id_name, Value::string("user-123"));
    /// ```
    #[inline]
    pub fn newtype(type_name: Name, inner: Value) -> Self {
        Value::Newtype {
            type_name,
            inner: Heap::new(inner),
        }
    }

    /// Create a newtype constructor.
    ///
    /// # Example
    ///
    /// ```text
    /// // Constructor for UserId newtype
    /// let user_id_ctor = Value::newtype_constructor(user_id_name);
    /// ```
    #[inline]
    pub fn newtype_constructor(type_name: Name) -> Self {
        Value::NewtypeConstructor { type_name }
    }

    /// Create a module namespace for qualified access.
    ///
    /// # Example
    ///
    /// ```text
    /// // Create a namespace for `use std.net.http as http`
    /// let ns = Value::module_namespace(members);
    /// ```
    #[inline]
    pub fn module_namespace(members: HashMap<Name, Value>) -> Self {
        Value::ModuleNamespace(Heap::new(members))
    }
}

// Value Methods

impl Value {
    /// Check if this value is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => !n.is_zero(),
            Value::Str(s) => !s.is_empty(),
            Value::List(items) => !items.is_empty(),
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
            Value::Range(_) => "Range",
            Value::ModuleNamespace(_) => "module",
            Value::Error(_) => "error",
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
            Value::Duration(ms) => format!("{ms}ms"),
            Value::Size(bytes) => format!("{bytes}b"),
            Value::Range(r) => format!("{r:?}"),
            Value::ModuleNamespace(_) => "<module>".to_string(),
            Value::Error(msg) => format!("Error({msg})"),
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
            (Value::List(a), Value::List(b)) | (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Duration(a), Value::Duration(b)) | (Value::Size(a), Value::Size(b)) => a == b,
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

// Trait Implementations

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
            Value::List(items) => write!(f, "List({:?})", &**items),
            Value::Map(map) => write!(f, "Map({:?})", &**map),
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
            Value::Range(r) => write!(f, "Range({r:?})"),
            Value::ModuleNamespace(ns) => write!(f, "ModuleNamespace({} items)", ns.len()),
            Value::Error(msg) => write!(f, "Error({msg})"),
        }
    }
}

impl fmt::Display for Value {
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
            Value::Duration(ms) => {
                if *ms >= 1000 {
                    write!(f, "{}s", ms / 1000)
                } else {
                    write!(f, "{ms}ms")
                }
            }
            Value::Size(bytes) => {
                if *bytes >= 1024 * 1024 {
                    write!(f, "{}mb", bytes / (1024 * 1024))
                } else if *bytes >= 1024 {
                    write!(f, "{}kb", bytes / 1024)
                } else {
                    write!(f, "{bytes}b")
                }
            }
            Value::Range(r) => {
                if r.inclusive {
                    write!(f, "{}..={}", r.start, r.end)
                } else {
                    write!(f, "{}..{}", r.start, r.end)
                }
            }
            Value::ModuleNamespace(_) => write!(f, "<module>"),
            Value::Error(msg) => write!(f, "<error: {msg}>"),
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
            (Value::List(a), Value::List(b)) | (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Duration(a), Value::Duration(b)) | (Value::Size(a), Value::Size(b)) => a == b,
            (Value::FunctionVal(_, name_a), Value::FunctionVal(_, name_b)) => name_a == name_b,
            // Functions are equal by body identity
            (Value::Function(a), Value::Function(b)) => a.body == b.body,
            (Value::MemoizedFunction(a), Value::MemoizedFunction(b)) => a.func.body == b.func.body,
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
            Value::Some(v) | Value::Ok(v) | Value::Err(v) => v.hash(state),
            Value::List(items) | Value::Tuple(items) => {
                for item in items.iter() {
                    item.hash(state);
                }
            }
            Value::Map(m) => {
                // Hash length for consistency
                m.len().hash(state);
                // Note: Map iteration order may vary, so we sort keys for determinism
                let mut keys: Vec<_> = m.keys().collect();
                keys.sort();
                for k in keys {
                    k.hash(state);
                    m.get(k).hash(state);
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
                // Hash by function identity (body expression ID)
                f.body.hash(state);
            }
            Value::MemoizedFunction(mf) => {
                // Hash by underlying function identity
                mf.func.body.hash(state);
            }
            Value::FunctionVal(_, name) => name.hash(state),
            Value::Range(r) => {
                r.start.hash(state);
                r.end.hash(state);
                r.inclusive.hash(state);
            }
            Value::ModuleNamespace(ns) => {
                // Hash by namespace size (discriminant already hashed)
                ns.len().hash(state);
            }
            Value::Error(msg) => msg.hash(state),
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
        assert!(Value::int(1).is_truthy());
        assert!(!Value::int(0).is_truthy());
        assert!(!Value::None.is_truthy());
    }

    #[test]
    fn test_value_display() {
        assert_eq!(format!("{}", Value::int(42)), "42");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::string("hello")), "\"hello\"");
    }

    #[test]
    fn test_factory_methods() {
        // Test that factory methods work
        let s = Value::string("hello");
        assert_eq!(s.as_str(), Some("hello"));

        let list = Value::list(vec![Value::int(1), Value::int(2)]);
        assert_eq!(list.as_list().map(<[Value]>::len), Some(2));

        let opt = Value::some(Value::int(42));
        match opt {
            Value::Some(v) => assert_eq!(*v, Value::int(42)),
            _ => panic!("expected Some"),
        }

        let ok = Value::ok(Value::int(42));
        match ok {
            Value::Ok(v) => assert_eq!(*v, Value::int(42)),
            _ => panic!("expected Ok"),
        }

        let err = Value::err(Value::string("error"));
        match err {
            Value::Err(v) => assert_eq!(v.as_str(), Some("error")),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn test_value_equality() {
        assert!(Value::int(42).equals(&Value::int(42)));
        assert!(!Value::int(42).equals(&Value::int(43)));
        assert!(Value::None.equals(&Value::None));

        let s1 = Value::string("hello");
        let s2 = Value::string("hello");
        assert!(s1.equals(&s2));
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
    fn test_range_contains() {
        let range = RangeValue::exclusive(0, 5);
        assert!(range.contains(0));
        assert!(range.contains(4));
        assert!(!range.contains(5));

        let range = RangeValue::inclusive(0, 5);
        assert!(range.contains(5));
    }

    #[test]
    fn test_function_value() {
        use ori_ir::{ExprArena, ExprId, SharedArena};
        use std::collections::HashMap;
        let arena = SharedArena::new(ExprArena::new());
        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), arena);
        assert!(func.params.is_empty());
        assert!(!func.has_captures());
    }

    #[test]
    fn test_value_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_value(v: &Value) -> u64 {
            let mut hasher = DefaultHasher::new();
            v.hash(&mut hasher);
            hasher.finish()
        }

        // Equal values must have equal hashes
        assert_eq!(hash_value(&Value::int(42)), hash_value(&Value::int(42)));
        assert_eq!(
            hash_value(&Value::Bool(true)),
            hash_value(&Value::Bool(true))
        );
        assert_eq!(hash_value(&Value::Void), hash_value(&Value::Void));
        assert_eq!(hash_value(&Value::None), hash_value(&Value::None));

        // Equal strings
        let s1 = Value::string("hello");
        let s2 = Value::string("hello");
        assert_eq!(hash_value(&s1), hash_value(&s2));

        // Equal lists
        let l1 = Value::list(vec![Value::int(1), Value::int(2)]);
        let l2 = Value::list(vec![Value::int(1), Value::int(2)]);
        assert_eq!(hash_value(&l1), hash_value(&l2));

        // Equal Option values
        let o1 = Value::some(Value::int(42));
        let o2 = Value::some(Value::int(42));
        assert_eq!(hash_value(&o1), hash_value(&o2));
    }

    #[test]
    #[expect(
        clippy::mutable_key_type,
        reason = "Value hash is based on immutable content"
    )]
    fn test_value_in_hashset() {
        use std::collections::HashSet;

        let mut set: HashSet<Value> = HashSet::new();
        set.insert(Value::int(1));
        set.insert(Value::int(2));
        set.insert(Value::int(1)); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&Value::int(1)));
        assert!(set.contains(&Value::int(2)));
        assert!(!set.contains(&Value::int(3)));
    }

    #[test]
    #[expect(
        clippy::mutable_key_type,
        reason = "Value hash is based on immutable content"
    )]
    fn test_value_as_hashmap_key() {
        use std::collections::HashMap;

        let mut map: HashMap<Value, &str> = HashMap::new();
        map.insert(Value::string("key1"), "value1");
        map.insert(Value::int(42), "value2");

        assert_eq!(map.get(&Value::string("key1")), Some(&"value1"));
        assert_eq!(map.get(&Value::int(42)), Some(&"value2"));
        assert_eq!(map.get(&Value::string("unknown")), None);
    }

    #[test]
    fn test_value_different_types_different_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_value(v: &Value) -> u64 {
            let mut hasher = DefaultHasher::new();
            v.hash(&mut hasher);
            hasher.finish()
        }

        // Different value types should (likely) have different hashes
        // This isn't guaranteed but is generally true for well-designed hash functions
        let int_hash = hash_value(&Value::int(1));
        let bool_hash = hash_value(&Value::Bool(true));
        let str_hash = hash_value(&Value::string("1"));

        // At least some should differ (collision is possible but unlikely)
        let all_same = int_hash == bool_hash && bool_hash == str_hash;
        assert!(
            !all_same,
            "Different types should generally have different hashes"
        );
    }
}
