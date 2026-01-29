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
├── mod.rs        # Value enum and factory methods (~569 lines)
├── heap.rs       # Heap<T> wrapper for Arc enforcement (~147 lines)
└── composite.rs  # FunctionValue, StructValue, RangeValue
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
    Int(ScalarInt),  // Uses ScalarInt to prevent unchecked arithmetic
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

    // User-defined Sum Types (enums)
    Variant {
        type_name: Name,
        variant_name: Name,
        fields: Heap<Vec<Value>>,  // May be empty for unit variants
    },
    VariantConstructor {
        type_name: Name,
        variant_name: Name,
        field_count: usize,
    },

    // Newtypes (nominally distinct type wrappers)
    Newtype {
        type_name: Name,
        inner: Heap<Value>,
    },
    NewtypeConstructor { type_name: Name },

    // Composite Types
    Struct(StructValue),
    Function(FunctionValue),
    MemoizedFunction(MemoizedFunctionValue),  // For recurse with memo: true
    FunctionVal(FunctionValFn, &'static str),  // Type conversions: int(), str()
    Range(RangeValue),

    // Module System
    ModuleNamespace(Heap<HashMap<Name, Value>>),  // For module aliases

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
| `Value::int(n)` | `Value::Int` | Create integer from `i64` (wraps in ScalarInt) |
| `Value::string(s)` | `Value::Str` | Create string from `impl Into<String>` |
| `Value::list(vec)` | `Value::List` | Create list from `Vec<Value>` |
| `Value::map(map)` | `Value::Map` | Create map from `HashMap<String, Value>` |
| `Value::tuple(vec)` | `Value::Tuple` | Create tuple from `Vec<Value>` |
| `Value::some(v)` | `Value::Some` | Wrap value in Some |
| `Value::ok(v)` | `Value::Ok` | Wrap value in Ok |
| `Value::err(v)` | `Value::Err` | Wrap value in Err |
| `Value::variant(type_name, variant_name, fields)` | `Value::Variant` | Create sum type variant |
| `Value::variant_constructor(type_name, variant_name, field_count)` | `Value::VariantConstructor` | Create variant constructor |
| `Value::newtype(type_name, inner)` | `Value::Newtype` | Create newtype wrapper |
| `Value::newtype_constructor(type_name)` | `Value::NewtypeConstructor` | Create newtype constructor |
| `Value::module_namespace(members)` | `Value::ModuleNamespace` | Create module namespace for qualified access |

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

## User-Defined Sum Types (Variants)

Sum types (enums) are represented with two Value variants:

### Variant Values

```rust
Value::Variant {
    type_name: Name,      // The enum type (e.g., "Status")
    variant_name: Name,   // The variant (e.g., "Running" or "Done")
    fields: Heap<Vec<Value>>,  // Variant fields (empty for unit variants)
}
```

Examples:
- Unit variant: `Running` → `Variant { type_name: "Status", variant_name: "Running", fields: [] }`
- Single field: `Some(42)` → `Some(Heap::new(Value::Int(42)))` (built-in Option uses dedicated variant)
- Multi-field: `Click(x: 10, y: 20)` → `Variant { fields: [Int(10), Int(20)] }`

### Variant Constructors

For variants with fields, a constructor value is registered in the environment:

```rust
Value::VariantConstructor {
    type_name: Name,
    variant_name: Name,
    field_count: usize,
}
```

When called, the constructor creates a `Value::Variant` with the provided arguments:

```rust
// In evaluator
Value::VariantConstructor { type_name, variant_name, field_count } => {
    if args.len() != field_count {
        return Err(wrong_arg_count(...));
    }
    Ok(Value::variant(type_name, variant_name, args.to_vec()))
}
```

## Newtypes

Newtypes are nominally distinct wrappers around existing types.

### Newtype Values

```rust
Value::Newtype {
    type_name: Name,      // The newtype name (e.g., "UserId")
    inner: Heap<Value>,   // The wrapped value
}
```

### Newtype Constructors

```rust
Value::NewtypeConstructor { type_name: Name }
```

When called with one argument, creates a `Value::Newtype`:

```rust
// In evaluator
Value::NewtypeConstructor { type_name } => {
    if args.len() != 1 {
        return Err(wrong_arg_count(...));
    }
    Ok(Value::newtype(type_name, args[0].clone()))
}
```

### Newtype Methods

Newtypes support a single built-in method:

- `unwrap()` — Returns the inner value

```rust
// In method dispatch
fn dispatch_newtype_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Newtype { inner, .. } = receiver else { unreachable!() };
    match method {
        "unwrap" => {
            if !args.is_empty() {
                return Err(wrong_arg_count("unwrap", 0, args.len()));
            }
            Ok((*inner).clone())
        }
        _ => Err(no_such_method(method, "newtype")),
    }
}
```

## Module Namespace Values

Module namespaces represent imported modules accessed via alias. They enable qualified access like `http.get(...)`.

```rust
Value::ModuleNamespace(Heap<HashMap<Name, Value>>)
```

Created when processing module alias imports:

```ori
use std.net.http as http
```

Field access on a module namespace looks up the member by name:

```rust
fn eval_field_access(receiver: Value, field: Name) -> EvalResult {
    match receiver {
        Value::ModuleNamespace(ns) => {
            ns.get(&field)
                .cloned()
                .ok_or_else(|| no_member_in_module(field))
        }
        // ... other cases
    }
}
```

Factory method:

```rust
impl Value {
    pub fn module_namespace(members: HashMap<Name, Value>) -> Self {
        Value::ModuleNamespace(Heap::new(members))
    }
}
```

## Function Values

```rust
pub struct FunctionValue {
    /// Parameter names.
    pub params: Vec<Name>,

    /// Body expression.
    pub body: ExprId,

    /// Captured environment (frozen at creation).
    /// Uses Arc for efficient sharing between multiple functions.
    captures: Arc<HashMap<Name, Value>>,

    /// Arena for expression resolution.
    /// Required for thread safety - the body ExprId must be resolved
    /// against this arena, not whatever arena happens to be in scope.
    arena: SharedArena,

    /// Required capabilities (from `uses` clause).
    capabilities: Vec<Name>,
}
```

### FunctionValue Constructors

```rust
impl FunctionValue {
    /// Create a function with owned captures (clones the HashMap).
    pub fn new(params, body, captures: HashMap<Name, Value>, arena) -> Self;

    /// Create a function with capabilities.
    pub fn with_capabilities(params, body, captures, arena, capabilities) -> Self;

    /// Create a function with shared captures (avoids cloning).
    /// Use when multiple functions should share the same captures
    /// (e.g., module functions for mutual recursion).
    pub fn with_shared_captures(params, body, captures: Arc<HashMap<Name, Value>>, arena, capabilities) -> Self;
}
```

The `with_shared_captures` constructor is used during module loading to avoid cloning the captures HashMap for each function in a module.

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

The `FunctionValue` type uses immutable captures wrapped in `Arc` (no `RwLock`), eliminating potential race conditions. Multiple functions can efficiently share captures via `with_shared_captures()` without cloning.
