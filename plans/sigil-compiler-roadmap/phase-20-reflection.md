# Phase 20: Runtime Reflection

**Goal**: Enable runtime type introspection and dynamic operations

**Criticality**: Low — Serialization, debugging, metaprogramming

**Dependencies**: Phase 11 (FFI), Phases 1-3 (Type System)

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Scope | Opt-in per type | Performance, code size |
| Operations | Read-only initially | Safety, complexity |
| Type info | Minimal but useful | Balance info vs overhead |
| Integration | Via trait | Consistent with Ori |

---

## Reference Implementation

### Go

```
~/lang_repos/golang/src/reflect/type.go     # Type representation
~/lang_repos/golang/src/reflect/value.go    # Value manipulation
```

### Rust

```
# Rust has minimal reflection; std::any::TypeId, std::any::type_name
~/lang_repos/rust/library/core/src/any.rs
```

---

## 20.1 Reflect Trait

**Spec section**: `spec/27-reflection.md § Reflect Trait`

### Syntax

```ori
// Trait for reflectable types
trait Reflect {
    @type_info (self) -> TypeInfo
    @field_count (self) -> int
    @field_name (self, index: int) -> Option<str>
    @field_value (self, index: int) -> Option<dyn Any>
}

// Derived automatically
#derive(Reflect)
type Person = {
    name: str,
    age: int,
}

// Usage
let person = Person { name: "Alice", age: 30 }
let info = person.type_info()
print(`Type: {info.name}`)  // "Person"

for i in 0..person.field_count() do run(
    let name = person.field_name(index: i).unwrap()
    let value = person.field_value(index: i).unwrap()
    print(`{name}: {value.to_str()}`)
)
```

### Implementation

- [ ] **Spec**: Add `spec/27-reflection.md`
  - [ ] Reflect trait definition
  - [ ] TypeInfo type
  - [ ] Operations available

- [ ] **Stdlib**: Reflect trait
  - [ ] Define in std.reflect
  - [ ] TypeInfo struct

- [ ] **Derive**: Reflect derive macro
  - [ ] Generate field metadata
  - [ ] Generate accessors

- [ ] **Test**: `tests/spec/reflect/basic.ori`
  - [ ] Derive Reflect
  - [ ] Access type info
  - [ ] Iterate fields

---

## 20.2 TypeInfo

**Spec section**: `spec/27-reflection.md § Type Information`

### Structure

```ori
type TypeInfo = {
    name: str,           // "Person"
    module: str,         // "myapp.models"
    kind: TypeKind,      // Struct, Enum, Primitive, etc.
    fields: [FieldInfo], // For structs
    variants: [VariantInfo], // For enums
}

type TypeKind = Struct | Enum | Primitive | List | Map | Function | Trait

type FieldInfo = {
    name: str,
    type_name: str,
    offset: int,  // For FFI/unsafe access
}

type VariantInfo = {
    name: str,
    fields: [FieldInfo],
}
```

### Implementation

- [ ] **Spec**: TypeInfo structure
  - [ ] All fields defined
  - [ ] TypeKind variants
  - [ ] FieldInfo, VariantInfo

- [ ] **Stdlib**: TypeInfo types
  - [ ] In std.reflect
  - [ ] Read-only accessors

- [ ] **Codegen**: Generate type metadata
  - [ ] Emit TypeInfo at compile time
  - [ ] Optimize for space

- [ ] **Test**: `tests/spec/reflect/type_info.ori`
  - [ ] Struct TypeInfo
  - [ ] Enum TypeInfo
  - [ ] Nested types

---

## 20.3 Any Type

**Spec section**: `spec/27-reflection.md § Any Type`

### Syntax

```ori
// Any can hold any value with type info
type Any = {
    value: *void,
    type_info: TypeInfo,
}

impl Any {
    @new<T: Reflect> (value: T) -> Any
    @type_name (self) -> str
    @is<T> (self) -> bool
    @downcast<T> (self) -> Option<T>
}

// Usage
let any: Any = Any.new(42)
print(any.type_name())  // "int"

if any.is<int>() then
    let value = any.downcast<int>().unwrap()
    print(`Value: {value}`)
```

### Implementation

- [ ] **Spec**: Any type semantics
  - [ ] Boxing
  - [ ] Type checking
  - [ ] Downcasting

- [ ] **Stdlib**: Any type
  - [ ] Generic new
  - [ ] Type checking
  - [ ] Safe downcast

- [ ] **Codegen**: Type ID generation
  - [ ] Unique ID per type
  - [ ] Runtime comparison

- [ ] **Test**: `tests/spec/reflect/any.ori`
  - [ ] Create Any
  - [ ] Type checking
  - [ ] Downcast success/failure

---

## 20.4 Dynamic Field Access

**Spec section**: `spec/27-reflection.md § Dynamic Access`

### Syntax

```ori
// Access field by name
@get_field (obj: dyn Reflect, name: str) -> Option<dyn Any> = run(
    let info = obj.type_info()
    for i in 0..obj.field_count() do
        if obj.field_name(index: i) == Some(name) then
            return obj.field_value(index: i)
    None
)

// Usage
let person = Person { name: "Alice", age: 30 }
let name = get_field(obj: person, name: "name")  // Some("Alice")
let missing = get_field(obj: person, name: "email")  // None
```

### Implementation

- [ ] **Spec**: Dynamic access semantics
  - [ ] By index
  - [ ] By name
  - [ ] Error handling

- [ ] **Stdlib**: Access helpers
  - [ ] get_field function
  - [ ] Field iteration

- [ ] **Test**: `tests/spec/reflect/dynamic_access.ori`
  - [ ] Get by name
  - [ ] Get by index
  - [ ] Missing field

---

## 20.5 Serialization Use Case

**Spec section**: `spec/27-reflection.md § Serialization`

### Example: JSON Serialization

```ori
use std.reflect { Reflect, TypeInfo, TypeKind }
use std.json { JsonValue }

@to_json<T: Reflect> (value: T) -> JsonValue = run(
    let info = value.type_info()

    match(info.kind,
        Primitive -> primitive_to_json(value: value),
        Struct -> run(
            let mut obj = JsonValue.object()
            for i in 0..value.field_count() do
                let name = value.field_name(index: i).unwrap()
                let field_val = value.field_value(index: i).unwrap()
                obj = obj.set(key: name, value: to_json(value: field_val))
            obj
        ),
        List -> run(
            let mut arr = JsonValue.array()
            // iterate list...
            arr
        ),
        _ -> JsonValue.null(),
    )
)

@primitive_to_json (value: dyn Any) -> JsonValue = run(
    if value.is<int>() then
        JsonValue.number(n: float(value.downcast<int>().unwrap()))
    else if value.is<str>() then
        JsonValue.string(s: value.downcast<str>().unwrap())
    else if value.is<bool>() then
        JsonValue.bool(b: value.downcast<bool>().unwrap())
    else
        JsonValue.null()
)

// Usage
#derive(Reflect)
type User = { name: str, age: int, active: bool }

let user = User { name: "Alice", age: 30, active: true }
let json = to_json(value: user)
// {"name": "Alice", "age": 30, "active": true}
```

### Implementation

- [ ] **Stdlib**: JSON serialization
  - [ ] Generic to_json
  - [ ] Generic from_json

- [ ] **Documentation**: Serialization guide
  - [ ] Custom serializers
  - [ ] Performance considerations

- [ ] **Test**: `tests/spec/reflect/serialization.ori`
  - [ ] Struct to JSON
  - [ ] JSON to struct
  - [ ] Nested types

---

## 20.6 Performance Considerations

**Spec section**: `spec/27-reflection.md § Performance`

### Opt-in Design

```ori
// Only types with #derive(Reflect) have reflection overhead
type FastType = { x: int }  // No reflection, no overhead

#derive(Reflect)
type ReflectableType = { x: int }  // Has metadata

// Type info is static, computed once
let info = value.type_info()  // Fast: returns pointer to static data
```

### Overhead

| Operation | Cost |
|-----------|------|
| type_info() | O(1) - pointer to static |
| field_count() | O(1) - stored in TypeInfo |
| field_name() | O(1) - indexed lookup |
| field_value() | O(1) - computed offset |
| downcast() | O(1) - TypeId comparison |

### Implementation

- [ ] **Codegen**: Minimize overhead
  - [ ] Static TypeInfo tables
  - [ ] No per-instance cost
  - [ ] Lazy initialization

- [ ] **Documentation**: Performance guide
  - [ ] When to use reflection
  - [ ] Alternatives for hot paths

- [ ] **Benchmarks**: Reflection performance
  - [ ] Type info access
  - [ ] Field iteration
  - [ ] Serialization

---

## 20.7 Limitations

**Spec section**: `spec/27-reflection.md § Limitations`

### Read-Only (Phase 1)

```ori
// Cannot set fields dynamically (initially)
// This would require:
// - Mutable reflection
// - Type system integration
// - Safety considerations
```

### No Dynamic Type Creation

```ori
// Cannot create types at runtime
// Ori is statically typed
```

### No Method Reflection

```ori
// Initially, no method invocation via reflection
// Focus on data (fields) first
```

### Implementation

- [ ] **Spec**: Document limitations
  - [ ] What's not supported
  - [ ] Rationale
  - [ ] Future possibilities

- [ ] **Diagnostics**: Clear errors
  - [ ] When attempting unsupported operations

---

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/27-reflection.md` complete
- [ ] CLAUDE.md updated with reflection syntax
- [ ] Reflect trait works
- [ ] TypeInfo accessible
- [ ] Any type works
- [ ] Dynamic field access works
- [ ] JSON serialization example works
- [ ] All tests pass: `cargo test && ori test tests/spec/reflect/`

**Exit Criteria**: Can implement generic JSON serialization/deserialization

---

## Example: Debug Printer

```ori
use std.reflect { Reflect, TypeKind }

// Generic debug printer using reflection
@debug<T: Reflect> (value: T) -> str = run(
    let info = value.type_info()

    match(info.kind,
        Primitive -> value.to_str(),

        Struct -> run(
            let mut result = `{info.name} \{`
            for i in 0..value.field_count() do run(
                if i > 0 then result = result + ", "
                let name = value.field_name(index: i).unwrap()
                let field_val = value.field_value(index: i).unwrap()
                result = result + `{name}: {debug(value: field_val)}`
            )
            result + "}"
        ),

        Enum -> run(
            // Handle enum variants
            `{info.name}::{value.variant_name()}`
        ),

        List -> run(
            let mut result = "["
            // iterate and debug each element
            result + "]"
        ),

        _ -> `<{info.name}>`,
    )
)

// Usage
#derive(Reflect)
type Point = { x: int, y: int }

#derive(Reflect)
type Line = { start: Point, end: Point }

let line = Line {
    start: Point { x: 0, y: 0 },
    end: Point { x: 10, y: 20 },
}

print(debug(value: line))
// Line {start: Point {x: 0, y: 0}, end: Point {x: 10, y: 20}}
```
