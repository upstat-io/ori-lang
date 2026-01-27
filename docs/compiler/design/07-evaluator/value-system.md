---
title: "Value System"
description: "Ori Compiler Design — Value System"
order: 704
section: "Evaluator"
---

# Value System

The Value enum represents runtime values in the Ori evaluator.

## Location

```
compiler/ori_patterns/src/value/
├── mod.rs          # Value enum and factory methods (~569 lines)
├── heap.rs         # Heap<T> wrapper for Arc enforcement (~147 lines)
└── composite.rs    # FunctionValue, StructValue, RangeValue
```

## Heap<T> Wrapper

The `Heap<T>` type enforces controlled heap allocations. External code cannot construct heap values directly because the constructor is `pub(super)`.

```rust
/// A heap-allocated value wrapper using Arc internally.
#[repr(transparent)]
pub struct Heap<T: ?Sized>(Arc<T>);

impl<T> Heap<T> {
    /// Private constructor - only visible within the value module.
    pub(super) fn new(value: T) -> Self {
        Heap(Arc::new(value))
    }
}
```

This design ensures:
- All heap allocations go through `Value` factory methods
- Consistent memory management across the codebase
- Zero-cost abstraction (`#[repr(transparent)]`)

## Value Enum

```rust
pub enum Value {
    // Primitives (inline, no heap allocation)
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Byte(u8),
    Void,
    Duration(u64),  // milliseconds
    Size(u64),      // bytes

    // Heap Types (use Heap<T> for enforced Arc usage)
    Str(Heap<String>),
    List(Heap<Vec<Value>>),
    Map(Heap<HashMap<String, Value>>),
    Tuple(Heap<Vec<Value>>),

    // Algebraic Types
    Some(Heap<Value>),
    None,
    Ok(Heap<Value>),
    Err(Heap<Value>),

    // Composite Types
    Struct(StructValue),
    Function(FunctionValue),
    FunctionVal(FunctionValFn, &'static str),  // Type conversions: int(), str()
    Range(RangeValue),

    // Error Recovery
    Error(String),
}
```

## Factory Methods (Required for Heap Values)

External code must use factory methods to create heap-allocated values:

```rust
// Correct - use factory methods
let s = Value::string("hello");
let list = Value::list(vec![Value::Int(1), Value::Int(2)]);
let opt = Value::some(Value::Int(42));
let ok = Value::ok(Value::Int(42));
let err = Value::err(Value::string("failed"));
let map = Value::map(HashMap::new());
let tuple = Value::tuple(vec![Value::Int(1), Value::Bool(true)]);

// Prevented at compile time
let s = Value::Str(Heap::new(...));  // ERROR: Heap::new is pub(super)
```

Available factory methods:

| Method | Returns | Description |
|--------|---------|-------------|
| `Value::string(s)` | `Value::Str` | Create string from `impl Into<String>` |
| `Value::list(vec)` | `Value::List` | Create list from `Vec<Value>` |
| `Value::map(map)` | `Value::Map` | Create map from `HashMap<String, Value>` |
| `Value::tuple(vec)` | `Value::Tuple` | Create tuple from `Vec<Value>` |
| `Value::some(v)` | `Value::Some` | Wrap value in Some |
| `Value::ok(v)` | `Value::Ok` | Wrap value in Ok |
| `Value::err(v)` | `Value::Err` | Wrap value in Err |

## Primitives

Primitives are stored inline (no heap allocation):

```rust
Value::Int(42)
Value::Float(3.14)
Value::Bool(true)
Value::Char('λ')
Value::Byte(0xFF)
Value::Void
Value::Duration(5000)  // 5 seconds
Value::Size(1024)      // 1kb
```

## Function Values

```rust
pub struct FunctionValue {
    /// Parameter names
    pub params: Vec<Name>,

    /// Body expression
    pub body: ExprId,

    /// Captured environment (for closures)
    pub captured: Option<Heap<HashMap<Name, Value>>>,

    /// Optional function name (for recursion)
    pub name: Option<Name>,
}
```

## Struct Values

```rust
pub struct StructValue {
    /// Type name
    pub type_name: Name,

    /// Field values by name
    pub fields: Heap<HashMap<Name, Value>>,

    /// Field layout for ordering
    pub layout: StructLayout,
}
```

## Range Values

```rust
pub struct RangeValue {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
}

impl RangeValue {
    pub fn exclusive(start: i64, end: i64) -> Self;
    pub fn inclusive(start: i64, end: i64) -> Self;
    pub fn contains(&self, value: i64) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = i64>;
}
```

## Type Conversions

```rust
impl Value {
    pub fn as_int(&self) -> Option<i64>;
    pub fn as_float(&self) -> Option<f64>;
    pub fn as_bool(&self) -> Option<bool>;
    pub fn as_str(&self) -> Option<&str>;
    pub fn as_list(&self) -> Option<&[Value]>;
    pub fn type_name(&self) -> &'static str;
    pub fn is_truthy(&self) -> bool;
}
```

## Type Name Resolution

Two methods exist for getting type names:

### `type_name()` - Static Type Names

Returns a static string for the value's type. For structs, returns `"struct"`
(cannot resolve the actual type name without an interner).

```rust
pub fn type_name(&self) -> &'static str {
    match self {
        Value::Int(_) => "int",
        Value::Struct(_) => "struct",  // Cannot resolve actual name
        // ...
    }
}
```

### `type_name_with_interner()` - Full Type Names

Returns the actual type name, using the interner to resolve struct type names:

```rust
pub fn type_name_with_interner<I: StringLookup>(&self, interner: &I) -> Cow<'static, str> {
    match self {
        Value::Struct(s) => Cow::Owned(interner.lookup(s.type_name).to_string()),
        Value::Range(_) => Cow::Borrowed("range"),
        _ => Cow::Borrowed(self.type_name()),
    }
}
```

The `StringLookup` trait is defined in `ori_ir::interner` and implemented for
`StringInterner`. This avoids circular dependencies between crates.

**Usage in method dispatch:**

```rust
// In Evaluator::get_value_type_name()
pub(super) fn get_value_type_name(&self, value: &Value) -> String {
    value.type_name_with_interner(self.interner).into_owned()
}
```

Truthiness rules:
- `Bool(false)`, `Int(0)`, empty string, empty list, `None`, `Err`, `Void` → falsy
- Everything else → truthy

## Equality and Hashing

Values implement `Eq` and `Hash` for use in collections:

```rust
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Struct(a), Value::Struct(b)) => {
                a.type_name == b.type_name
                    && a.fields.iter().zip(b.fields.iter()).all(|(av, bv)| av == bv)
            }
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, v)| b.get(k).is_some_and(|bv| v == bv))
            }
            // ... other cases
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Int(n) => n.hash(state),
            Value::Float(f) => f.to_bits().hash(state),
            Value::Map(m) => {
                m.len().hash(state);
                // Sort keys for deterministic hashing
                let mut keys: Vec<_> = m.keys().collect();
                keys.sort();
                for k in keys {
                    k.hash(state);
                    m.get(k).hash(state);
                }
            }
            // ... other cases
        }
    }
}
```

This enables Values to be used in `HashSet<Value>` and as keys in `HashMap<Value, _>`:

```rust
let mut set: HashSet<Value> = HashSet::new();
set.insert(Value::Int(1));
set.insert(Value::Int(2));
set.insert(Value::Int(1)); // Duplicate, not added
assert_eq!(set.len(), 2);
```

Note: Float comparison uses direct `==` (may differ from IEEE semantics for NaN). Float hashing uses `to_bits()` for consistency.

## Display

```rust
impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // Int: "42"
        // Float: "3.14"
        // Str: "\"hello\""
        // List: "[1, 2, 3]"
        // Map: "{\"key\": 42}"
        // Tuple: "(1, true)"
        // Some: "Some(42)"
        // None: "None"
        // Ok: "Ok(42)"
        // Err: "Err(\"message\")"
        // Duration: "5s" or "100ms"
        // Size: "1kb" or "1mb" or "1024b"
        // Function: "<function>"
    }
}
```

## Thread Safety

All heap types use `Arc` internally (wrapped by `Heap<T>`), providing:
- Safe sharing across threads
- Cheap cloning (reference count increment)
- Automatic cleanup when unused

The `FunctionValue` type uses immutable captures (no `RwLock`), eliminating potential race conditions.
