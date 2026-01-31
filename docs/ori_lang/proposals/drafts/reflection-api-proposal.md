# Proposal: Reflection API

**Status:** Draft
**Author:** Ori Language Team
**Created:** 2026-01-31
**Affects:** Compiler (type system, derive macros), stdlib, spec

## Summary

This proposal introduces a reflection API for Ori that enables runtime type introspection while maintaining the language's safety guarantees. The design centers on an opt-in `Reflect` trait with static metadata tables, an `Any` type for type-erased values with safe downcasting, and a `TypeInfo` structure providing comprehensive type metadata. The primary use case is generic serialization/deserialization (JSON, TOML, etc.).

## Motivation

### Problem Statement

Currently, Ori lacks the ability to inspect types at runtime. This prevents implementation of:

1. **Generic Serialization** — Cannot write a single `to_json()` function that works on any struct
2. **Debug Formatters** — Custom debug output requires manual implementation for each type
3. **ORM/Database Mapping** — Cannot automatically map structs to database rows
4. **Configuration Loading** — Cannot populate structs from config files generically
5. **Validation Frameworks** — Cannot apply validation rules based on field names/types
6. **Testing Utilities** — Cannot implement property-based testing or fuzzing generically

### Design Goals

1. **Opt-in** — Types must explicitly derive `Reflect`; no implicit metadata generation
2. **Zero-cost abstraction** — Non-reflecting types pay nothing; reflecting types pay minimal static storage
3. **Read-only** — No mutable reflection; values cannot be modified through reflection
4. **Type-safe** — All dynamic operations return `Option` or `Result`; no unsafe downcasts
5. **ARC-compatible** — Works with Ori's reference-counted memory model
6. **Testable** — Reflection operations are pure and deterministic

### Non-Goals

1. **Method Reflection** — Calling methods dynamically (future proposal)
2. **Mutable Reflection** — Modifying values through reflection
3. **Dynamic Type Creation** — Creating new types at runtime
4. **Proxy/Interception** — AOP-style method interception
5. **Private Field Access** — Only public fields are visible to reflection

## Design

### 1. The `Reflect` Trait

```ori
// Defined in std.reflect

trait Reflect {
    // Get static type information
    @type_info (self) -> TypeInfo

    // Field access by index (for iteration)
    @field_count (self) -> int

    // Get field value as Any by index (0-based)
    @field_by_index (self, index: int) -> Option<Any>

    // Get field value as Any by name
    @field_by_name (self, name: str) -> Option<Any>
}
```

**Derivation:**

```ori
#derive(Reflect)
type Person = {
    name: str,
    age: int,
    email: Option<str>,
}
```

The derive macro generates implementations that:
- Return static `TypeInfo` (interned once per type)
- Provide O(1) field access by index
- Provide O(1) field access by name (via static hash map)

**Constraints:**
- All fields must also implement `Reflect`
- Private fields (`::`-prefixed) are excluded from reflection
- Generic types derive conditionally: `Person<T>` reflects when `T: Reflect`

### 2. The `TypeInfo` Structure

```ori
// Comprehensive type metadata

type TypeInfo = {
    name: str,           // Simple type name: "Person"
    module: str,         // Full module path: "myapp.models"
    kind: TypeKind,      // Category of type
    fields: [FieldInfo], // For structs (empty for others)
    variants: [VariantInfo], // For enums (empty for others)
    type_params: [str],  // Generic parameter names: ["T", "E"]
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
    type_name: str,      // Type as string: "Option<str>"
    index: int,          // 0-based position
    is_optional: bool,   // True if type is Option<T>
}

type VariantInfo = {
    name: str,           // Variant name: "Some", "None"
    index: int,          // Variant index (for matching)
    fields: [FieldInfo], // Payload fields (empty for unit variants)
}
```

**Static Metadata:**

TypeInfo is generated at compile time and stored in static tables. Each reflecting type has exactly one TypeInfo instance, referenced by all values of that type.

```ori
// Accessing type info
let person = Person { name: "Alice", age: 30, email: None }
let info = person.type_info()

assert_eq(actual: info.name, expected: "Person")
assert_eq(actual: info.kind, expected: Struct)
assert_eq(actual: len(collection: info.fields), expected: 3)
assert_eq(actual: info.fields[0].name, expected: "name")
```

### 3. The `Any` Type

```ori
// Type-erased container with safe downcasting

type Any = {
    // Private: holds erased value and type info
    ::value: ErasedValue,
    ::type_info: TypeInfo,
}

impl Any {
    // Create an Any from a reflecting value
    @new<T: Reflect> (value: T) -> Any

    // Get the type name
    @type_name (self) -> str

    // Get full type info
    @type_info (self) -> TypeInfo

    // Check if this Any holds a value of type T
    @is<T: Reflect> (self) -> bool

    // Attempt to downcast to concrete type T
    @downcast<T: Reflect> (self) -> Option<T>

    // Downcast or panic with message
    @unwrap<T: Reflect> (self) -> T

    // Downcast or return default
    @unwrap_or<T: Reflect> (self, default: T) -> T
}
```

**Usage:**

```ori
let value: Any = Any.new(value: 42)

assert(condition: value.is<int>())
assert_eq(actual: value.type_name(), expected: "int")

// Safe downcast
match value.downcast<int>() {
    Some(n) -> print(msg: `Got integer: {n}`),
    None -> print(msg: "Not an integer"),
}

// Or with unwrap
let n = value.unwrap<int>()  // Panics if not int
```

**Primitive Implementations:**

All primitives (`int`, `float`, `str`, `bool`, `char`, `byte`) implement `Reflect` with their respective TypeInfo.

### 4. Standard Library Reflect Implementations

The following types implement `Reflect`:

**Primitives:**
- `int`, `float`, `str`, `bool`, `char`, `byte`, `void`

**Collections:**
- `[T]` where `T: Reflect`
- `{K: V}` where `K: Reflect, V: Reflect`
- `Set<T>` where `T: Reflect`

**Option and Result:**
- `Option<T>` where `T: Reflect`
- `Result<T, E>` where `T: Reflect, E: Reflect`

**Tuples:**
- `()`, `(T)`, `(T, U)`, etc. where all elements implement `Reflect`

**Special Types:**
- `Duration`, `Size`

### 5. Derived Reflect Implementation

For a struct:

```ori
#derive(Reflect)
type Point = {
    x: int,
    y: int,
}
```

The compiler generates:

```ori
impl Reflect for Point {
    @type_info (self) -> TypeInfo = $POINT_TYPE_INFO

    @field_count (self) -> int = 2

    @field_by_index (self, index: int) -> Option<Any> = match index {
        0 -> Some(Any.new(value: self.x)),
        1 -> Some(Any.new(value: self.y)),
        _ -> None,
    }

    @field_by_name (self, name: str) -> Option<Any> = match name {
        "x" -> Some(Any.new(value: self.x)),
        "y" -> Some(Any.new(value: self.y)),
        _ -> None,
    }
}
```

Where `$POINT_TYPE_INFO` is a compile-time constant:

```ori
let $POINT_TYPE_INFO = TypeInfo {
    name: "Point",
    module: "myapp.geometry",
    kind: Struct,
    fields: [
        FieldInfo { name: "x", type_name: "int", index: 0, is_optional: false },
        FieldInfo { name: "y", type_name: "int", index: 1, is_optional: false },
    ],
    variants: [],
    type_params: [],
}
```

For an enum:

```ori
#derive(Reflect)
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Point
```

The compiler generates similar code with variant matching:

```ori
impl Reflect for Shape {
    @type_info (self) -> TypeInfo = $SHAPE_TYPE_INFO

    @field_count (self) -> int = match self {
        Circle(_) -> 1,
        Rectangle(_, _) -> 2,
        Point -> 0,
    }

    @field_by_index (self, index: int) -> Option<Any> = match self {
        Circle(radius) if index == 0 -> Some(Any.new(value: radius)),
        Rectangle(width, _) if index == 0 -> Some(Any.new(value: width)),
        Rectangle(_, height) if index == 1 -> Some(Any.new(value: height)),
        _ -> None,
    }

    @field_by_name (self, name: str) -> Option<Any> = match self {
        Circle(radius) if name == "radius" -> Some(Any.new(value: radius)),
        Rectangle(width, _) if name == "width" -> Some(Any.new(value: width)),
        Rectangle(_, height) if name == "height" -> Some(Any.new(value: height)),
        _ -> None,
    }
}
```

### 6. Reflection for Generic Types

Generic types derive `Reflect` conditionally:

```ori
#derive(Reflect)
type Container<T> = {
    items: [T],
    count: int,
}

// Reflects when T: Reflect
impl<T: Reflect> Reflect for Container<T> {
    @type_info (self) -> TypeInfo = run(
        let base = $CONTAINER_TYPE_INFO,
        TypeInfo {
            ...base,
            type_params: [T.type_info().name],
        },
    )

    // ... other methods
}
```

### 7. Type Comparison and Identity

```ori
// Types can be compared by TypeInfo
@types_equal<A: Reflect, B: Reflect> () -> bool = run(
    let a_info = A.type_info(),
    let b_info = B.type_info(),
    a_info.name == b_info.name && a_info.module == b_info.module,
)

// Get type identity from a value
@type_id_of<T: Reflect> (value: T) -> int = run(
    let info = value.type_info(),
    hash_combine(seed: info.name.hash(), value: info.module.hash()),
)
```

### 8. Iteration Over Fields

```ori
// Helper for iterating all fields of a reflecting value
extend<T: Reflect> T {
    @fields (self) -> impl Iterator<Item = (str, Any)> = run(
        let count = self.field_count(),
        let info = self.type_info(),
        (0..count).iter()
            .filter_map(transform: i -> run(
                let field_info = info.fields[i],
                self.field_by_index(index: i)
                    .map(transform: v -> (field_info.name, v)),
            )),
    )
}
```

**Usage:**

```ori
let person = Person { name: "Alice", age: 30, email: Some("alice@example.com") }

for (name, value) in person.fields() do
    print(msg: `{name}: {value.type_name()}`)

// Output:
// name: str
// age: int
// email: Option<str>
```

## Examples

### Example 1: Generic JSON Serialization

```ori
use std.json { Json, JsonValue }
use std.reflect { Reflect, TypeKind }

// Generic to_json for any reflecting type
@to_json_generic<T: Reflect> (value: T) -> JsonValue = run(
    let info = value.type_info(),
    match info.kind {
        Primitive -> to_json_primitive(value:),
        Struct -> to_json_struct(value:),
        Enum -> to_json_enum(value:),
        List -> to_json_list(value:),
        Map -> to_json_map(value:),
        _ -> JsonValue.Null,
    },
)

@to_json_struct<T: Reflect> (value: T) -> JsonValue = run(
    let pairs = for (name, field_value) in value.fields()
        yield (name, to_json_any(value: field_value)),
    JsonValue.Object(pairs.collect()),
)

@to_json_any (value: Any) -> JsonValue = match value.type_info().kind {
    Primitive -> match value.type_name() {
        "int" -> JsonValue.Number(value.unwrap<int>() as float),
        "float" -> JsonValue.Number(value.unwrap<float>()),
        "str" -> JsonValue.String(value.unwrap<str>()),
        "bool" -> JsonValue.Bool(value.unwrap<bool>()),
        _ -> JsonValue.Null,
    },
    Struct -> to_json_struct(value: value.unwrap<_>()),
    List -> to_json_list(value: value.unwrap<_>()),
    _ -> JsonValue.Null,
}
```

### Example 2: Generic Debug Printer

```ori
use std.reflect { Reflect, TypeKind }

@debug_print<T: Reflect> (value: T, indent: int = 0) -> str = run(
    let info = value.type_info(),
    let prefix = " ".repeat(count: indent * 2),
    match info.kind {
        Struct -> run(
            let fields_str = for (name, field) in value.fields()
                yield `{prefix}  {name}: {debug_print(value: field, indent: indent + 1)}`,
            `{info.name} {{\n{fields_str.join(separator: ",\n")}\n{prefix}}}`,
        ),
        Enum -> run(
            let variant = get_variant_name(value:),
            if value.field_count() == 0 then variant
            else run(
                let fields_str = for (_, field) in value.fields()
                    yield debug_print(value: field, indent: indent + 1),
                `{variant}({fields_str.join(separator: ", ")})`,
            ),
        ),
        _ -> value.to_str(),
    },
)
```

### Example 3: Struct Validation

```ori
use std.reflect { Reflect, TypeKind }

type ValidationError = {
    field: str,
    message: str,
}

@validate_not_empty<T: Reflect> (value: T) -> [ValidationError] = run(
    let info = value.type_info(),
    for field_info in info.fields if field_info.type_name == "str"
        let field_value = value.field_by_name(name: field_info.name)
        if field_value.is_some() && is_empty_str(value: field_value.unwrap())
            yield ValidationError {
                field: field_info.name,
                message: "cannot be empty",
            },
)

@is_empty_str (value: Any) -> bool = match value.downcast<str>() {
    Some(s) -> is_empty(collection: s),
    None -> false,
}
```

### Example 4: Generic Clone via Reflection

```ori
// Deep clone any reflecting type
@deep_clone<T: Reflect + Clone> (value: T) -> T = value.clone()

// Reflection-based clone for types that can't derive Clone
@reflect_clone<T: Reflect> (value: T) -> T = run(
    let info = value.type_info(),
    match info.kind {
        Struct -> reflect_clone_struct(value:),
        Enum -> reflect_clone_enum(value:),
        _ -> value,  // Primitives are Copy
    },
)
```

### Example 5: Configuration Loading

```ori
use std.reflect { Reflect, TypeInfo }
use std.toml { TomlValue }

@load_config<T: Reflect + Default> (toml: TomlValue) -> Result<T, str> = run(
    let default = T.default(),
    let info = default.type_info(),
    populate_from_toml(target: default, source: toml, info:),
)

@populate_from_toml<T: Reflect> (target: T, source: TomlValue, info: TypeInfo) -> Result<T, str> =
    // Build new value from TOML using reflection
    for field_info in info.fields do
        let toml_key = field_info.name
        match source.get(key: toml_key) {
            Some(toml_value) -> set_field_from_toml(target:, field: field_info, value: toml_value)?,
            None if field_info.is_optional -> continue,
            None -> Err(`missing required field: {toml_key}`),
        }
    Ok(target)
```

## Integration with Ori Design Pillars

### Mandatory Verification

All reflection operations are testable:

```ori
@test_person_type_info tests @type_info () -> void = run(
    let person = Person { name: "Test", age: 0, email: None },
    let info = person.type_info(),
    assert_eq(actual: info.name, expected: "Person"),
    assert_eq(actual: info.fields[0].name, expected: "name"),
)
```

### Explicit Effects

Reflection operations are pure—they don't require capabilities. The `Reflect` trait is a compile-time marker that enables runtime introspection without side effects.

### ARC-Safe

- `Any` owns its value (reference counted)
- No shared mutable references through reflection
- Field access returns copies wrapped in `Any`
- No raw pointers or unsafe memory access

### Opt-in Design

Types must explicitly derive `Reflect`:

```ori
// This type does NOT reflect (no metadata generated)
type Secret = {
    password: str,
}

// This type DOES reflect
#derive(Reflect)
type PublicInfo = {
    username: str,
}
```

This ensures:
1. No hidden code size cost
2. Sensitive types can avoid reflection
3. Explicit contract about type capabilities

## Error Handling

### Derive Errors

```
error[E0450]: cannot derive `Reflect` for `Container`
  --> src/types.ori:10:1
   |
10 | #derive(Reflect)
   | ^^^^^^^^^^^^^^^^
   |
   = note: field `secret` has type `Password` which does not implement `Reflect`
   = help: either derive `Reflect` for `Password` or remove it from `Container`
```

### Downcast Errors

```ori
let value: Any = Any.new(value: "hello")
let result = value.downcast<int>()  // Returns None

// unwrap panics with clear message
let n = value.unwrap<int>()
// panic: type mismatch: expected `int`, found `str`
```

### Field Access Errors

```ori
let person = Person { name: "Alice", age: 30, email: None }

// Invalid field name returns None
let bad = person.field_by_name(name: "nonexistent")  // None

// Invalid index returns None
let also_bad = person.field_by_index(index: 100)  // None
```

## Performance Considerations

### Static Metadata

All `TypeInfo` is generated at compile time and stored in read-only static memory:

```
// Conceptual layout (actual implementation is Rust)
static PERSON_TYPE_INFO: TypeInfo = TypeInfo { ... };
```

- No per-instance overhead
- No runtime metadata construction
- O(1) access to type information

### Field Access

- By index: O(1) via match dispatch
- By name: O(1) via static hash map (generated at compile time)

### Any Boxing

Creating `Any` requires:
- One allocation for the erased value (reference counted)
- One pointer copy for TypeInfo reference

This is comparable to `Box<dyn Trait>` in Rust.

### Code Size

Each reflecting type adds:
- One static `TypeInfo` (tens to hundreds of bytes)
- Match-based field accessors (linear in field count)
- No vtables or dynamic dispatch beyond `Any`

### Opt-Out for Performance

Performance-critical code can avoid reflection:

```ori
// No Reflect derive = zero reflection cost
type HotPath = {
    data: [float],
    count: int,
}
```

## Dependencies

This proposal depends on:

1. **Phase 03 (Traits)** — Trait definitions and implementations
2. **Phase 05 (Type Declarations)** — Struct and enum syntax
3. **Phase 07 (Derive Macros)** — `#derive()` infrastructure
4. **Phase 11 (Generics)** — Generic type support

## Future Extensions

The following are explicitly deferred to future proposals:

### Method Reflection

```ori
// Future: reflect on methods
trait MethodReflect: Reflect {
    @method_count (self) -> int
    @method_by_name (self, name: str) -> Option<MethodInfo>
    @call (self, method: str, args: [Any]) -> Result<Any, str>
}
```

### Mutable Reflection

```ori
// Future: modify values through reflection
trait ReflectMut: Reflect {
    @set_field_by_name (self, name: str, value: Any) -> Result<void, str>
}
```

### Dynamic Type Creation

```ori
// Future: create types at runtime
@create_struct (name: str, fields: [(str, TypeInfo)]) -> TypeInfo
```

## Spec Changes Required

### New Spec Section: 27-reflection.md

Add comprehensive section covering:
- Reflect trait definition
- TypeInfo structure
- Any type
- Derive semantics
- Standard implementations

### Update: 08-declarations.md

Add `Reflect` to the list of derivable traits.

### Update: Prelude

Add to prelude:
- `Reflect` trait
- `TypeInfo`, `TypeKind`, `FieldInfo`, `VariantInfo` types
- `Any` type

### Update: std.reflect Module

New module with:
- Reflect trait and related types
- Helper functions for reflection operations
- Extension methods for reflecting types

## Summary Table

| Feature | Design Decision |
|---------|-----------------|
| Opt-in | Types must derive `Reflect` explicitly |
| Metadata Storage | Static, compile-time generated |
| Field Access | O(1) by index and name |
| Type Safety | All operations return `Option` or `Result` |
| Any Type | Reference-counted, type-erased container |
| Mutable Reflection | Not supported (deferred) |
| Method Reflection | Not supported (deferred) |
| Private Fields | Not visible to reflection |
| Generic Types | Conditional derivation when bounds satisfied |
| Primitives | All implement `Reflect` by default |
| Collections | Implement when element types reflect |
| Performance | Zero cost for non-reflecting types |
| Code Size | Static metadata per reflecting type |

## Open Questions

1. **Trait Object Reflection** — Should `dyn Trait` implement `Reflect`? This requires storing TypeInfo in the vtable.

2. **Function Reflection** — Should function types have TypeInfo for parameter/return types?

3. **Const Generics** — How should `[T, max N]` reflect its capacity?

4. **Variant Access** — Should enums provide `@current_variant (self) -> VariantInfo`?

5. **Derive Attributes** — Should reflection support attributes like `#reflect(skip)` or `#reflect(rename: "json_name")`?

## Conclusion

This proposal provides a minimal but complete reflection API for Ori that enables important use cases like generic serialization while maintaining the language's safety guarantees. The opt-in design ensures zero cost for types that don't need reflection, while the static metadata approach keeps runtime overhead minimal.

The focus on read-only data reflection provides a solid foundation that can be extended in future proposals to include method reflection and mutable operations as needed.
