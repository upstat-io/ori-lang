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
pub(crate) mod iterator;
mod scalar_int;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

// Re-export StringLookup from ori_ir for convenience
pub use ori_ir::{Name, StringLookup};

pub use composite::{FunctionValue, MemoizedFunctionValue, RangeValue, StructLayout, StructValue};
pub use heap::Heap;
pub use iterator::IteratorValue;
pub use scalar_int::ScalarInt;

/// Ordering value representing comparison results.
///
/// This is a first-class representation of the `Ordering` type, avoiding
/// the overhead of `Value::Variant` for this frequently-used type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum OrderingValue {
    /// Left operand is less than right.
    Less,
    /// Operands are equal.
    Equal,
    /// Left operand is greater than right.
    Greater,
}

impl OrderingValue {
    /// Create from the raw i8 tag value.
    ///
    /// Uses the same convention as `ori_ir::builtin_constants::ordering`:
    /// - 0 = Less
    /// - 1 = Equal
    /// - 2 = Greater
    #[must_use]
    pub const fn from_tag(tag: i8) -> Option<Self> {
        match tag {
            0 => Some(Self::Less),
            1 => Some(Self::Equal),
            2 => Some(Self::Greater),
            _ => None,
        }
    }

    /// Get the raw i8 tag value.
    #[must_use]
    pub const fn to_tag(self) -> i8 {
        match self {
            Self::Less => 0,
            Self::Equal => 1,
            Self::Greater => 2,
        }
    }

    /// Get the display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Less => "Less",
            Self::Equal => "Equal",
            Self::Greater => "Greater",
        }
    }
}

/// Type conversion function signature.
///
/// `function_val`: type conversion functions like int(x), str(x), float(x)
/// that allow positional arguments per the spec.
///
/// Uses `EvalError` instead of `String` so that conversion errors preserve
/// structured error information (kind, span, notes) across the boundary.
pub type FunctionValFn = fn(&[Value]) -> Result<Value, crate::EvalError>;

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
    /// Duration value (in nanoseconds, can be negative).
    Duration(i64),
    /// Size value (in bytes, always non-negative).
    Size(u64),
    /// Ordering value (Less, Equal, Greater).
    Ordering(OrderingValue),

    // Heap Types (use Heap<T> for enforced Arc usage)
    /// String value.
    ///
    /// Uses `Cow<'static, str>` to allow zero-copy for interned string literals
    /// (`Cow::Borrowed`) while still supporting runtime-created strings (`Cow::Owned`).
    Str(Heap<Cow<'static, str>>),
    /// List of values.
    List(Heap<Vec<Value>>),
    /// Map from string keys to values.
    ///
    /// Uses `BTreeMap` for deterministic iteration order, which enables
    /// efficient hashing without needing to sort keys.
    Map(Heap<BTreeMap<String, Value>>),
    /// Set of unique values.
    ///
    /// Uses `BTreeMap<String, Value>` keyed by `to_map_key()` for deterministic
    /// iteration order and O(log n) membership testing. The String key is the
    /// type-prefixed key from `to_map_key()`, the Value is the actual element.
    Set(Heap<BTreeMap<String, Value>>),
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
    /// Iterator value (functional — each `next()` returns a new iterator).
    Iterator(IteratorValue),

    /// Module namespace for qualified access.
    ///
    /// Created by module alias imports like `use std.net.http as http`.
    /// Enables qualified access like `http.get()`.
    ///
    /// Uses `BTreeMap` for deterministic iteration order (required for Salsa
    /// query results). The O(log n) lookup cost is acceptable since namespace
    /// lookups are not in hot paths.
    ModuleNamespace(Heap<BTreeMap<Name, Value>>),

    // Error Recovery
    /// Error value for error recovery.
    Error(String),

    /// Reference to a type for associated function dispatch.
    ///
    /// Used when a type name (like `Duration` or `Size`) is used as a receiver
    /// for associated function calls (e.g., `Duration.from_seconds(s: 10)`).
    TypeRef { type_name: Name },
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

    /// Create a string value from an owned string.
    ///
    /// This allocates for runtime-created strings. For string literals from
    /// source code, use `string_static` with the interner's `lookup_static`.
    ///
    /// # Example
    ///
    /// ```text
    /// let s = Value::string("hello");
    /// let s2 = Value::string(format!("value: {}", x));
    /// ```
    #[inline]
    pub fn string(s: impl Into<String>) -> Self {
        Value::Str(Heap::new(Cow::Owned(s.into())))
    }

    /// Create a string value from a static string reference (zero-copy).
    ///
    /// Use this for interned string literals to avoid allocation:
    /// ```text
    /// let s = Value::string_static(interner.lookup_static(name));
    /// ```
    #[inline]
    pub fn string_static(s: &'static str) -> Self {
        Value::Str(Heap::new(Cow::Borrowed(s)))
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

    /// Create a map value with String keys from a `BTreeMap`.
    ///
    /// Uses `BTreeMap` for deterministic iteration order.
    ///
    /// # Example
    ///
    /// ```text
    /// let empty = Value::map(BTreeMap::new());
    /// ```
    #[inline]
    pub fn map(entries: BTreeMap<String, Value>) -> Self {
        Value::Map(Heap::new(entries))
    }

    /// Create a map value from a `HashMap` by converting to `BTreeMap`.
    ///
    /// This preserves backwards compatibility while ensuring deterministic iteration.
    #[inline]
    pub fn map_from_hashmap(entries: std::collections::HashMap<String, Value>) -> Self {
        Value::Map(Heap::new(entries.into_iter().collect()))
    }

    /// Create a set value from a keyed `BTreeMap`.
    ///
    /// The map keys are `to_map_key()` strings for O(log n) deduplication.
    /// The map values are the actual `Value` elements.
    #[inline]
    pub fn set(items: BTreeMap<String, Value>) -> Self {
        Value::Set(Heap::new(items))
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

    /// Create an `Ordering::Less` value.
    #[inline]
    pub const fn ordering_less() -> Self {
        Value::Ordering(OrderingValue::Less)
    }

    /// Create an `Ordering::Equal` value.
    #[inline]
    pub const fn ordering_equal() -> Self {
        Value::Ordering(OrderingValue::Equal)
    }

    /// Create an `Ordering::Greater` value.
    #[inline]
    pub const fn ordering_greater() -> Self {
        Value::Ordering(OrderingValue::Greater)
    }

    /// Create an Ordering value from a comparison result.
    ///
    /// Returns Less if `cmp < 0`, Equal if `cmp == 0`, Greater if `cmp > 0`.
    #[inline]
    pub const fn ordering_from_cmp(cmp: std::cmp::Ordering) -> Self {
        match cmp {
            std::cmp::Ordering::Less => Value::Ordering(OrderingValue::Less),
            std::cmp::Ordering::Equal => Value::Ordering(OrderingValue::Equal),
            std::cmp::Ordering::Greater => Value::Ordering(OrderingValue::Greater),
        }
    }

    /// Create an iterator value from an `IteratorValue` state.
    #[inline]
    pub fn iterator(state: IteratorValue) -> Self {
        Value::Iterator(state)
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
    pub fn module_namespace(members: BTreeMap<Name, Value>) -> Self {
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
            Value::Error(_) => "error",
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
            Value::Range(r) => format!("{r:?}"),
            Value::Iterator(it) => format!("<iterator {it:?}>"),
            Value::ModuleNamespace(_) => "<module>".to_string(),
            Value::Error(msg) => format!("Error({msg})"),
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
            (Value::List(a), Value::List(b)) | (Value::Tuple(a), Value::Tuple(b)) => {
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
            Value::Error(msg) => write!(f, "Error({msg})"),
            Value::TypeRef { type_name } => write!(f, "TypeRef({type_name:?})"),
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
                if r.inclusive {
                    write!(f, "{}..={}", r.start, r.end)
                } else {
                    write!(f, "{}..{}", r.start, r.end)
                }
            }
            Value::Iterator(it) => write!(f, "<iterator {it:?}>"),
            Value::ModuleNamespace(_) => write!(f, "<module>"),
            Value::Error(msg) => write!(f, "<error: {msg}>"),
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
            (Value::List(a), Value::List(b)) | (Value::Tuple(a), Value::Tuple(b)) => a == b,
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
            Value::List(items) | Value::Tuple(items) => {
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
                r.inclusive.hash(state);
            }
            Value::Iterator(it) => it.hash(state),
            Value::ModuleNamespace(ns) => {
                // Hash by namespace size (discriminant already hashed)
                ns.len().hash(state);
            }
            Value::Error(msg) => msg.hash(state),
            Value::TypeRef { type_name } => type_name.hash(state),
        }
    }
}

/// Format a duration value (in nanoseconds) for display.
/// Uses the largest whole unit that doesn't lose precision.
fn format_duration(ns: i64) -> String {
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

#[cfg(test)]
mod tests;
