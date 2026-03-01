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
mod conversions;
mod error_value;
mod heap;
pub(crate) mod iterator;
mod list_data;
mod scalar_int;
mod traits;

use std::borrow::Cow;
use std::collections::BTreeMap;

// Re-export StringLookup from ori_ir for convenience
pub use ori_ir::{Name, StringLookup};

pub use composite::{FunctionValue, MemoizedFunctionValue, RangeValue, StructLayout, StructValue};
pub use error_value::{ErrorValue, TraceEntryData};
pub use heap::Heap;
pub use iterator::IteratorValue;
pub use list_data::ListData;
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
    ///
    /// Uses `ListData` for zero-copy slicing: `slice`/`take`/`skip` share
    /// the backing Arc buffer instead of copying elements.
    List(ListData),
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
    /// Error value with optional trace storage for Traceable trait.
    Error(Heap<ErrorValue>),

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
        Value::List(ListData::owned(items))
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

    /// Create an error value with no trace entries.
    ///
    /// This is the standard way to create error values. The trace will be
    /// populated as the error propagates through `?` operator sites.
    #[inline]
    pub fn error(msg: impl Into<String>) -> Self {
        Value::Error(Heap::new(ErrorValue::new(msg)))
    }

    /// Create an error value from an existing `ErrorValue`.
    ///
    /// Used when constructing errors with pre-existing trace data
    /// (e.g., after `with_entry()` or `push_trace()`).
    #[inline]
    pub fn error_from(ev: ErrorValue) -> Self {
        Value::Error(Heap::new(ev))
    }

    /// Extract the error value, if this is a `Value::Error`.
    #[inline]
    pub fn as_error(&self) -> Option<&ErrorValue> {
        match self {
            Value::Error(ev) => Some(ev),
            _ => None,
        }
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

#[cfg(test)]
mod tests;
