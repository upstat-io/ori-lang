---
title: "Reflection"
description: "Ori Language Specification — Reflection"
order: 27
section: "Reflection"
---

# Reflection

Reflection enables runtime type introspection for types that opt in via the `Reflect` trait.

## Overview

Ori provides read-only reflection with these key components:

| Component | Purpose |
|-----------|---------|
| `Reflect` trait | Opt-in type introspection |
| `TypeInfo` | Static type metadata |
| `Unknown` | Type-erased container with safe downcasting |

Reflection is opt-in. Types must explicitly derive `Reflect` to enable runtime introspection.

## Reflect Trait

```ori
trait Reflect {
    @type_info (self) -> TypeInfo
    @field_count (self) -> int
    @field_by_index (self, index: int) -> Option<Unknown>
    @field_by_name (self, name: str) -> Option<Unknown>
    @current_variant (self) -> Option<VariantInfo>
}
```

### Derivation

```ori
#derive(Reflect)
type Person = {
    name: str,
    age: int,
    email: Option<str>,
}
```

**Constraints:**

- All fields must implement `Reflect`
- Private fields (`::`-prefixed) are excluded from reflection
- Generic types derive conditionally: `Container<T>` reflects when `T: Reflect`

## TypeInfo Structure

```ori
type TypeInfo = {
    name: str,              // Simple type name: "Person"
    module: str,            // Full module path: "myapp.models"
    kind: TypeKind,         // Category of type
    fields: [FieldInfo],    // For structs (empty for others)
    variants: [VariantInfo], // For enums (empty for others)
    type_params: [str],     // Generic parameter names: ["T", "E"]
}

type TypeKind =
    | Struct
    | Enum
    | Primitive
    | List
    | Map
    | Tuple
    | Function
    | Trait

type FieldInfo = {
    name: str,
    type_name: str,         // Type as string: "Option<str>"
    index: int,             // 0-based position
    is_optional: bool,      // True if type is Option<T>
}

type VariantInfo = {
    name: str,              // Variant name: "Some", "None"
    index: int,             // Variant index
    fields: [FieldInfo],    // Payload fields (empty for unit variants)
}
```

TypeInfo is generated at compile time and stored in static tables. Each reflecting type has exactly one TypeInfo instance.

### Accessing Type Information

```ori
let person = Person { name: "Alice", age: 30, email: None }
let info = person.type_info()

assert_eq(actual: info.name, expected: "Person")
assert_eq(actual: info.kind, expected: Struct)
assert_eq(actual: len(collection: info.fields), expected: 3)
```

## Unknown Type

`Unknown` is a type-erased container with safe downcasting:

```ori
impl Unknown {
    @new<T: Reflect> (value: T) -> Unknown
    @type_name (self) -> str
    @type_info (self) -> TypeInfo
    @is<T: Reflect> (self) -> bool
    @downcast<T: Reflect> (self) -> Option<T>
    @unwrap<T: Reflect> (self) -> T
    @unwrap_or<T: Reflect> (self, default: T) -> T
}
```

### Usage

```ori
let value: Unknown = Unknown.new(value: 42)

assert(condition: value.is<int>())
assert_eq(actual: value.type_name(), expected: "int")

match value.downcast<int>() {
    Some(n) -> print(msg: `Got integer: {n}`),
    None -> print(msg: "Not an integer"),
}
```

Operations on `Unknown` values require explicit downcasting. Methods and fields cannot be accessed directly on `Unknown`.

## Standard Implementations

### Primitives

All primitives implement `Reflect`:
- `int`, `float`, `str`, `bool`, `char`, `byte`, `void`
- `Duration`, `Size`

### Collections

Collections implement `Reflect` when their element types implement `Reflect`:

| Type | Constraint |
|------|------------|
| `[T]` | `T: Reflect` |
| `{K: V}` | `K: Reflect, V: Reflect` |
| `Set<T>` | `T: Reflect` |
| `Option<T>` | `T: Reflect` |
| `Result<T, E>` | `T: Reflect, E: Reflect` |
| `(A, B, ...)` | All elements: `Reflect` |

## Derived Implementation

For a struct:

```ori
#derive(Reflect)
type Point = { x: int, y: int }
```

The compiler generates:

```ori
impl Reflect for Point {
    @type_info (self) -> TypeInfo = $POINT_TYPE_INFO

    @field_count (self) -> int = 2

    @field_by_index (self, index: int) -> Option<Unknown> = match index {
        0 -> Some(Unknown.new(value: self.x)),
        1 -> Some(Unknown.new(value: self.y)),
        _ -> None,
    }

    @field_by_name (self, name: str) -> Option<Unknown> = match name {
        "x" -> Some(Unknown.new(value: self.x)),
        "y" -> Some(Unknown.new(value: self.y)),
        _ -> None,
    }

    @current_variant (self) -> Option<VariantInfo> = None
}
```

### Enum Reflection

For sum types, `current_variant` returns the active variant:

```ori
#derive(Reflect)
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Point

impl Reflect for Shape {
    @current_variant (self) -> Option<VariantInfo> = Some(match self {
        Circle(_) -> VariantInfo { name: "Circle", index: 0, fields: [...] },
        Rectangle(_, _) -> VariantInfo { name: "Rectangle", index: 1, fields: [...] },
        Point -> VariantInfo { name: "Point", index: 2, fields: [] },
    })
    // ...
}
```

## Generic Reflection

Generic types derive `Reflect` conditionally:

```ori
#derive(Reflect)
type Container<T> = { items: [T], count: int }

// Reflects when T: Reflect
impl<T: Reflect> Reflect for Container<T> {
    @type_info (self) -> TypeInfo = TypeInfo {
        ...base_info,
        type_params: [T.type_info().name],
    }
    // ...
}
```

## Field Iteration

```ori
extend<T: Reflect> T {
    @fields (self) -> impl Iterator where Item == (str, Unknown)
}
```

Usage:

```ori
let person = Person { name: "Alice", age: 30, email: None }

for (name, value) in person.fields() do
    print(msg: `{name}: {value.type_name()}`)
```

## Constraints

### Read-Only

Reflection is read-only. Values cannot be modified through reflection.

### Public Fields Only

Private fields (`::`-prefixed) are not visible to reflection.

### No Method Reflection

Method reflection is not supported. Only field access is available.

### Object Safety

The `Reflect` trait methods return concrete types, making them individually object-safe. However, `Reflect` is not practically usable as a trait object because derivation requires the concrete type at compile time.

## Performance

### Static Metadata

- TypeInfo generated at compile time
- No per-instance overhead
- O(1) access to type information

### Field Access

- By index: O(1) via match dispatch
- By name: O(1) via static hash map

### Unknown Boxing

Creating `Unknown` requires one allocation for the erased value (reference counted).

### Opt-Out

Types that do not derive `Reflect` have zero reflection cost.

## Error Codes

| Code | Description |
|------|-------------|
| E0450 | Cannot derive `Reflect`: field type does not implement `Reflect` |
| E0451 | Type mismatch in downcast |
