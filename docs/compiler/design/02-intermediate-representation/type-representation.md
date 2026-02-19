---
title: "Type Representation"
description: "Ori Compiler Design — Type Representation"
order: 204
section: "Intermediate Representation"
---

# Type Representation

This document describes how types are represented in the Ori compiler. Note that `ori_ir` only contains `TypeId` (a flat `u32` index with pre-interned primitive constants). All of the types described below (`Type` enum, `StructType`, `EnumType`, `TypeRegistry`, etc.) live in the `ori_types` crate.

## Type Enum

Types are represented as an enum in `ori_types`:

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Type {
    // Primitive types
    Int,
    Float,
    Bool,
    String,
    Char,
    Byte,
    Void,
    Never,

    // Compound types
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Set(Box<Type>),
    Tuple(Vec<Type>),

    // Optional/Result
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),

    // Functions
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
        capabilities: Vec<Capability>,
    },

    // User-defined
    Named(Name),
    Struct(StructType),
    Enum(EnumType),

    // Type variables (for inference)
    TypeVar(TypeVarId),

    // Generic instantiation
    Generic {
        base: Box<Type>,
        args: Vec<Type>,
    },

    // Special
    Duration,
    Size,
    Range(Box<Type>),
    Channel(Box<Type>),
    Ordering,
}
```

## Primitive Types

Primitive types are simple variants with no data:

```rust
Type::Int    // Signed integer (canonical: 64-bit, range: [-2⁶³, 2⁶³-1])
Type::Float  // IEEE 754 double-precision (canonical: 64-bit)
Type::Bool   // Boolean
Type::String // UTF-8 string
Type::Char   // Unicode scalar value
Type::Byte   // Unsigned integer (range: [0, 255])
Type::Void   // Unit type
Type::Never  // Bottom type (for panic, etc.)
```

## Compound Types

Compound types contain other types:

```rust
// List of integers
Type::List(Box::new(Type::Int))

// Map from string to int
Type::Map(Box::new(Type::String), Box::new(Type::Int))

// Tuple of (int, bool)
Type::Tuple(vec![Type::Int, Type::Bool])

// Option<String>
Type::Option(Box::new(Type::String))

// Result<int, Error>
Type::Result(Box::new(Type::Int), Box::new(Type::Named(error_name)))
```

## Function Types

Function types include parameter types, return type, and capabilities:

```rust
// (int, int) -> int
Type::Function {
    params: vec![Type::Int, Type::Int],
    ret: Box::new(Type::Int),
    capabilities: vec![],
}

// (str) -> Result<str, Error> uses Http
Type::Function {
    params: vec![Type::String],
    ret: Box::new(Type::Result(
        Box::new(Type::String),
        Box::new(Type::Named(error_name)),
    )),
    capabilities: vec![Capability::Http],
}
```

## User-Defined Types

All user-defined type representations (`StructType`, `EnumType`, `TypeRegistry`) are in `ori_types`.

### Named Types

Simple reference to a user-defined type:

```rust
// type Point = { x: int, y: int }
Type::Named(point_name)  // References "Point" in TypeRegistry (ori_types)
```

### Struct Types

Inline struct definition:

```rust
Type::Struct(StructType {
    name: point_name,
    fields: vec![
        Field { name: x_name, ty: Type::Int },
        Field { name: y_name, ty: Type::Int },
    ],
})
```

### Enum Types

Sum types with variants:

```rust
Type::Enum(EnumType {
    name: option_name,
    variants: vec![
        Variant::Unit(some_name),
        Variant::Tuple(none_name, vec![Type::TypeVar(t)]),
    ],
})
```

## Type Variables

Type variables are used during type inference:

```rust
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct TypeVarId(pub u32);

// Fresh type variable
Type::TypeVar(TypeVarId(0))
```

Type variables are resolved through unification:

```rust
// Initially: TypeVar(0)
// After inference: Int
```

## Generic Types

Generic type instantiation:

```rust
// List<int> (generic list with int argument)
Type::Generic {
    base: Box::new(Type::Named(list_name)),
    args: vec![Type::Int],
}

// Map<str, User>
Type::Generic {
    base: Box::new(Type::Named(map_name)),
    args: vec![Type::String, Type::Named(user_name)],
}
```

## Type Comparison

Types implement `Eq` for structural comparison:

```rust
let t1 = Type::List(Box::new(Type::Int));
let t2 = Type::List(Box::new(Type::Int));
let t3 = Type::List(Box::new(Type::Float));

assert_eq!(t1, t2);  // Same structure
assert_ne!(t1, t3);  // Different element type
```

## Type Display

Types can be formatted for error messages:

```rust
impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::List(elem) => write!(f, "[{}]", elem),
            Type::Option(inner) => write!(f, "Option<{}>", inner),
            Type::Function { params, ret, .. } => {
                write!(f, "({}) -> {}", params.join(", "), ret)
            }
            // ...
        }
    }
}
```

## Type Registry

User-defined types are stored in `TypeRegistry` (in `ori_types`, not `ori_ir`):

```rust
pub struct TypeRegistry {
    types: HashMap<Name, TypeDef>,
}

pub enum TypeDef {
    Struct(StructDef),
    Enum(EnumDef),
    Alias(Type),
}
```

Looking up a named type:

```rust
// Type::Named(point_name)
let def = registry.get(point_name)?;
```

## Salsa Compatibility

The Type enum derives all required traits:

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Type { ... }
```

This allows types to be:
- Stored in Salsa query results
- Compared for early cutoff
- Hashed for memoization

## Boxing Strategy

Recursive types use `Box<Type>` to avoid infinite size:

```rust
// Would be infinite size without Box:
enum Type {
    List(Type),  // Error: infinite size
}

// Fixed with Box:
enum Type {
    List(Box<Type>),  // OK: Box has fixed size
}
```

The boxing overhead is acceptable because:
- Types are not allocated frequently (once per expression)
- Deep nesting is rare in practice
- Comparison uses structural equality anyway

## Special Types

### Duration and Size

Built-in types for literals:

```rust
Type::Duration  // 100ms, 5s, 2h
Type::Size      // 4kb, 10mb, 2gb
```

### Range

Range type for iterators:

```rust
Type::Range(Box::new(Type::Int))  // Range<int>
```

### Channel

Channel type for concurrency:

```rust
Type::Channel(Box::new(Type::String))  // Channel<str>
```

### Ordering

Comparison result type:

```rust
Type::Ordering  // Less | Equal | Greater
```
