# Value System

The Value enum represents runtime values in the Sigil evaluator.

## Location

```
compiler/sigilc/src/eval/value/mod.rs (~566 lines)
```

## Value Enum

```rust
#[derive(Clone, Debug)]
pub enum Value {
    // Primitives
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Void,

    // Heap-allocated (shared via Arc)
    String(Arc<String>),
    List(Arc<Vec<Value>>),
    Map(Arc<HashMap<Value, Value>>),
    Set(Arc<HashSet<Value>>),

    // Structs
    Struct {
        name: Name,
        fields: Arc<HashMap<Name, Value>>,
    },

    // Enums / variants
    Variant {
        enum_name: Name,
        variant: Name,
        data: Option<Arc<Value>>,
    },

    // Option and Result
    Option(Option<Arc<Value>>),
    Result(Result<Arc<Value>, Arc<Value>>),

    // Functions
    Function(FunctionValue),
    Builtin(BuiltinFn),

    // Special
    Duration(Duration),
    Size(Size),
    Range(RangeValue),
}
```

## Primitives

Primitives are stored inline (no heap allocation):

```rust
Value::Int(42)
Value::Float(3.14)
Value::Bool(true)
Value::Char('λ')
Value::Void
```

## Heap Values

Large or shared values use `Arc`:

```rust
// String
Value::String(Arc::new("hello".to_string()))

// List
Value::List(Arc::new(vec![
    Value::Int(1),
    Value::Int(2),
    Value::Int(3),
]))

// Map
Value::Map(Arc::new(HashMap::from([
    (Value::String(Arc::new("key".into())), Value::Int(42)),
])))
```

Benefits of Arc:
- Cheap cloning (just increment refcount)
- Safe sharing in closures
- Automatic cleanup when unused

## Function Values

```rust
#[derive(Clone, Debug)]
pub struct FunctionValue {
    /// Parameter names
    pub params: Vec<Name>,

    /// Body expression
    pub body: ExprId,

    /// Captured environment (for closures)
    pub captured_env: Scope,

    /// Optional function name (for recursion)
    pub name: Option<Name>,
}
```

Creating a function value:

```rust
// Lambda: x -> x + 1
Value::Function(FunctionValue {
    params: vec![x_name],
    body: add_expr_id,
    captured_env: current_scope.clone(),
    name: None,
})

// Named function: @double (x: int) -> int = x * 2
Value::Function(FunctionValue {
    params: vec![x_name],
    body: mul_expr_id,
    captured_env: Scope::empty(),
    name: Some(double_name),
})
```

## Struct Values

```rust
// Point { x: 10, y: 20 }
Value::Struct {
    name: point_name,
    fields: Arc::new(HashMap::from([
        (x_name, Value::Int(10)),
        (y_name, Value::Int(20)),
    ])),
}
```

Field access:

```rust
impl Value {
    pub fn get_field(&self, field: Name) -> Result<&Value, EvalError> {
        match self {
            Value::Struct { fields, .. } => {
                fields.get(&field).ok_or(EvalError::NoSuchField(field))
            }
            _ => Err(EvalError::NotAStruct),
        }
    }
}
```

## Enum Values

```rust
// Some(42)
Value::Variant {
    enum_name: option_name,
    variant: some_name,
    data: Some(Arc::new(Value::Int(42))),
}

// None
Value::Variant {
    enum_name: option_name,
    variant: none_name,
    data: None,
}

// Ok("success")
Value::Result(Ok(Arc::new(Value::String(Arc::new("success".into())))))

// Err("failed")
Value::Result(Err(Arc::new(Value::String(Arc::new("failed".into())))))
```

## Type Conversions

```rust
impl Value {
    pub fn as_int(&self) -> Result<i64, EvalError> {
        match self {
            Value::Int(n) => Ok(*n),
            _ => Err(EvalError::TypeMismatch {
                expected: "int",
                found: self.type_name(),
            }),
        }
    }

    pub fn as_bool(&self) -> Result<bool, EvalError> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err(EvalError::TypeMismatch {
                expected: "bool",
                found: self.type_name(),
            }),
        }
    }

    pub fn as_list(&self) -> Result<&[Value], EvalError> {
        match self {
            Value::List(items) => Ok(items.as_slice()),
            _ => Err(EvalError::TypeMismatch {
                expected: "list",
                found: self.type_name(),
            }),
        }
    }

    pub fn as_function(&self) -> Result<&FunctionValue, EvalError> {
        match self {
            Value::Function(f) => Ok(f),
            _ => Err(EvalError::NotCallable),
        }
    }
}
```

## Equality

```rust
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Void, Value::Void) => true,
            _ => false,
        }
    }
}

impl Eq for Value {}
```

Note: Float comparison uses bitwise equality (not IEEE semantics).

## Hashing

For use in maps/sets:

```rust
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Int(n) => n.hash(state),
            Value::Bool(b) => b.hash(state),
            Value::String(s) => s.hash(state),
            Value::List(items) => items.hash(state),
            // Float uses bits for deterministic hashing
            Value::Float(f) => f.to_bits().hash(state),
            _ => {}
        }
    }
}
```

## Display

```rust
impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Void => write!(f, "void"),
            Value::Function(_) => write!(f, "<function>"),
            // ...
        }
    }
}
```

## Memory Safety

The `Heap<T>` wrapper ensures safe allocation:

```rust
pub struct Heap<T>(Arc<T>);

impl<T> Heap<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }
}
```

This prevents accidentally creating bare `Arc`s and ensures consistent memory management.
