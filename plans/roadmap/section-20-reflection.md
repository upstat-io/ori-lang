---
section: 20
title: Runtime Reflection
status: not-started
tier: 7
goal: Enable runtime type introspection and dynamic operations
spec:
  - spec/27-reflection.md
sections:
  - id: "20.1"
    title: Reflect Trait
    status: not-started
  - id: "20.2"
    title: TypeInfo and Related Types
    status: not-started
  - id: "20.3"
    title: Unknown Type
    status: not-started
  - id: "20.4"
    title: Standard Reflect Implementations
    status: not-started
  - id: "20.5"
    title: Field Iteration Extension
    status: not-started
  - id: "20.6"
    title: Generic Serialization Use Case
    status: not-started
  - id: "20.7"
    title: Error Handling
    status: not-started
  - id: "20.8"
    title: Performance Considerations
    status: not-started
---

# Section 20: Runtime Reflection

**Goal**: Enable runtime type introspection and dynamic operations

**Criticality**: Low — Serialization, debugging, metaprogramming

**Dependencies**: Section 3 (Traits), Section 5 (Type Declarations), Section 7 (Derive Macros), Section 11 (Generics)

**Proposal**: `proposals/approved/reflection-api-proposal.md` — ✅ APPROVED 2026-01-31

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Scope | Opt-in per type via `#derive(Reflect)` | Performance, code size |
| Operations | Read-only (v1) | Safety, complexity |
| Type-erased container | `Unknown` type | Safe downcasting, type-safe API |
| Type info | Static `TypeInfo` metadata | O(1) access, no runtime construction |
| Integration | `Reflect` trait in prelude | Consistent with Ori |
| Enum support | `@current_variant` method | Essential for enum inspection |

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

**Proposal**: `proposals/approved/reflection-api-proposal.md § The Reflect Trait`

### Syntax

```ori
// Trait for reflectable types (defined in std.reflect, exported to prelude)
trait Reflect {
    @type_info (self) -> TypeInfo
    @field_count (self) -> int
    @field_by_index (self, index: int) -> Option<Unknown>
    @field_by_name (self, name: str) -> Option<Unknown>
    @current_variant (self) -> Option<VariantInfo>
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
print(msg: `Type: {info.name}`)  // "Person"

for (name, value) in person.fields() do
    print(msg: `{name}: {value.type_name()}`)
```

### Implementation

- [ ] **Spec**: Add `spec/27-reflection.md`
  - [ ] Reflect trait definition
  - [ ] TypeInfo, FieldInfo, VariantInfo types
  - [ ] Unknown type
  - [ ] **LLVM Support**: LLVM codegen for Reflect trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Stdlib**: Reflect trait in std.reflect
  - [ ] Trait definition with all 5 methods
  - [ ] Export to prelude
  - [ ] **LLVM Support**: LLVM codegen for std.reflect
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Derive**: `#derive(Reflect)` macro
  - [ ] Generate static TypeInfo constant
  - [ ] Generate field accessors (match-based)
  - [ ] Generate `current_variant` for enums (None for structs)
  - [ ] Conditional derivation for generics (`T: Reflect`)
  - [ ] **LLVM Support**: LLVM codegen for derive macro
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/basic.ori`
  - [ ] Derive Reflect for struct
  - [ ] Derive Reflect for enum
  - [ ] Access type info
  - [ ] Iterate fields
  - [ ] Current variant for enums
  - [ ] **LLVM Support**: LLVM codegen for basic tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.2 TypeInfo and Related Types

**Proposal**: `proposals/approved/reflection-api-proposal.md § The TypeInfo Structure`

### Types

```ori
type TypeInfo = {
    name: str,           // "Person"
    module: str,         // "myapp.models"
    kind: TypeKind,      // Struct, Enum, Primitive, etc.
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

### Implementation

- [ ] **Spec**: TypeInfo structure
  - [ ] All fields defined
  - [ ] TypeKind variants
  - [ ] FieldInfo, VariantInfo
  - [ ] **LLVM Support**: LLVM codegen for TypeInfo structure
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Stdlib**: TypeInfo types in std.reflect
  - [ ] All types defined
  - [ ] Export to prelude
  - [ ] **LLVM Support**: LLVM codegen for TypeInfo types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Codegen**: Generate static type metadata
  - [ ] Emit TypeInfo at compile time
  - [ ] One instance per type (interned)
  - [ ] O(1) access from type_info() method
  - [ ] **LLVM Support**: LLVM codegen for static metadata
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/type_info.ori`
  - [ ] Struct TypeInfo
  - [ ] Enum TypeInfo with variants
  - [ ] Nested types
  - [ ] Generic types
  - [ ] **LLVM Support**: LLVM codegen for TypeInfo tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.3 Unknown Type

**Proposal**: `proposals/approved/reflection-api-proposal.md § The Unknown Type`

### Syntax

```ori
// Type-erased container with safe downcasting
type Unknown = {
    ::value: ErasedValue,    // Internal: opaque compiler type
    ::type_info: TypeInfo,
}

impl Unknown {
    @new<T: Reflect> (value: T) -> Unknown
    @type_name (self) -> str
    @type_info (self) -> TypeInfo
    @is<T: Reflect> (self) -> bool
    @downcast<T: Reflect> (self) -> Option<T>
    @unwrap<T: Reflect> (self) -> T
    @unwrap_or<T: Reflect> (self, default: T) -> T
}

// Usage
let value: Unknown = Unknown.new(value: 42)
print(msg: value.type_name())  // "int"

match value.downcast<int>() {
    Some(n) -> print(msg: `Value: {n}`),
    None -> print(msg: "Not an int"),
}
```

### Implementation

- [ ] **Spec**: Unknown type semantics
  - [ ] Type erasure mechanism
  - [ ] Safe downcasting
  - [ ] Reference counting (ARC-compatible)
  - [ ] **LLVM Support**: LLVM codegen for Unknown type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Stdlib**: Unknown type in std.reflect
  - [ ] All methods implemented
  - [ ] Export to prelude
  - [ ] **LLVM Support**: LLVM codegen for Unknown stdlib
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Codegen**: Type identity
  - [ ] Unique type ID per type (name + module hash)
  - [ ] Runtime type comparison
  - [ ] ErasedValue internal representation
  - [ ] **LLVM Support**: LLVM codegen for type identity
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/unknown.ori`
  - [ ] Create Unknown from primitives
  - [ ] Create Unknown from structs
  - [ ] Type checking with is<T>()
  - [ ] Downcast success/failure
  - [ ] unwrap and unwrap_or
  - [ ] **LLVM Support**: LLVM codegen for Unknown tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.4 Standard Reflect Implementations

**Proposal**: `proposals/approved/reflection-api-proposal.md § Standard Library Reflect Implementations`

### Types with Built-in Reflect

```ori
// Primitives (all implement Reflect)
int, float, str, bool, char, byte, void

// Collections (conditional on element types)
[T] where T: Reflect
{K: V} where K: Reflect, V: Reflect
Set<T> where T: Reflect

// Option and Result (conditional)
Option<T> where T: Reflect
Result<T, E> where T: Reflect, E: Reflect

// Tuples (conditional on all elements)
(), (T), (T, U), etc.

// Special types
Duration, Size
```

### Implementation

- [ ] **Stdlib**: Primitive Reflect implementations
  - [ ] int, float, str, bool, char, byte, void
  - [ ] TypeKind::Primitive for all
  - [ ] **LLVM Support**: LLVM codegen for primitive Reflect
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Stdlib**: Collection Reflect implementations
  - [ ] Lists, maps, sets with conditional bounds
  - [ ] Tuples up to reasonable arity
  - [ ] **LLVM Support**: LLVM codegen for collection Reflect
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Stdlib**: Option/Result Reflect implementations
  - [ ] TypeKind::Enum with variants
  - [ ] **LLVM Support**: LLVM codegen for Option/Result Reflect
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/stdlib.ori`
  - [ ] Primitive type info
  - [ ] Collection type info
  - [ ] Option/Result type info
  - [ ] **LLVM Support**: LLVM codegen for stdlib Reflect tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.5 Field Iteration Extension

**Proposal**: `proposals/approved/reflection-api-proposal.md § Iteration Over Fields`

### Syntax

```ori
// Extension for field iteration
extend<T: Reflect> T {
    @fields (self) -> impl Iterator where Item == (str, Unknown)
}

// Usage
for (name, value) in person.fields() do
    print(msg: `{name}: {value.type_name()}`)
```

### Implementation

- [ ] **Stdlib**: fields() extension method
  - [ ] In std.reflect
  - [ ] Returns iterator of (name, Unknown) tuples
  - [ ] **LLVM Support**: LLVM codegen for fields() extension
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/field_iteration.ori`
  - [ ] Iterate struct fields
  - [ ] Iterate enum variant fields
  - [ ] Empty iteration for unit types
  - [ ] **LLVM Support**: LLVM codegen for iteration tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.6 Generic Serialization Use Case

**Proposal**: `proposals/approved/reflection-api-proposal.md § Examples`

### Example: JSON Serialization

```ori
use std.json { JsonValue }
use std.reflect { Reflect, TypeKind }

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
        yield (name, to_json_unknown(value: field_value)),
    JsonValue.Object(pairs.collect()),
)

@to_json_unknown (value: Unknown) -> JsonValue = match value.type_info().kind {
    Primitive -> match value.type_name() {
        "int" -> JsonValue.Number(value.unwrap<int>() as float),
        "float" -> JsonValue.Number(value.unwrap<float>()),
        "str" -> JsonValue.String(value.unwrap<str>()),
        "bool" -> JsonValue.Bool(value.unwrap<bool>()),
        _ -> JsonValue.Null,
    },
    _ -> JsonValue.Null,
}
```

### Implementation

- [ ] **Stdlib**: Generic JSON serialization in std.json
  - [ ] to_json for Reflect types
  - [ ] from_json for Reflect + Default types
  - [ ] **LLVM Support**: LLVM codegen for JSON serialization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Documentation**: Serialization guide
  - [ ] Custom serializers
  - [ ] Performance considerations
  - [ ] **LLVM Support**: LLVM codegen for guide examples
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/serialization.ori`
  - [ ] Struct to JSON
  - [ ] Nested structs to JSON
  - [ ] Enum to JSON
  - [ ] **LLVM Support**: LLVM codegen for serialization tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.7 Error Handling

**Proposal**: `proposals/approved/reflection-api-proposal.md § Error Handling`

### Error Codes

| Code | Error | Context |
|------|-------|---------|
| E0450 | Cannot derive Reflect | Field type doesn't implement Reflect |

### Panic Messages

```ori
// Downcast failure
let value: Unknown = Unknown.new(value: "hello")
let n = value.unwrap<int>()
// panic: type mismatch: expected `int`, found `str`
```

### Implementation

- [ ] **Diagnostics**: E0450 for derive failures
  - [ ] Point to non-Reflect field
  - [ ] Suggest derive or different type
  - [ ] **LLVM Support**: LLVM codegen for diagnostics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Test**: `tests/spec/reflect/errors.ori`
  - [ ] Compile fail for non-Reflect field
  - [ ] Runtime panic message for unwrap failure
  - [ ] **LLVM Support**: LLVM codegen for error tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## 20.8 Performance Considerations

**Proposal**: `proposals/approved/reflection-api-proposal.md § Performance Considerations`

### Overhead

| Operation | Cost |
|-----------|------|
| type_info() | O(1) — pointer to static |
| field_count() | O(1) — stored in TypeInfo |
| field_by_index() | O(1) — match dispatch |
| field_by_name() | O(1) — static hash map |
| Unknown.new() | O(1) — one allocation + TypeInfo pointer |
| downcast() | O(1) — type ID comparison |

### Opt-Out

```ori
// No #derive(Reflect) = zero reflection overhead
type HotPath = {
    data: [float],
    count: int,
}
```

### Implementation

- [ ] **Codegen**: Minimize overhead
  - [ ] Static TypeInfo tables (read-only memory)
  - [ ] No per-instance cost
  - [ ] Compile-time hash map for field_by_name
  - [ ] **LLVM Support**: LLVM codegen for optimized overhead
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

- [ ] **Benchmarks**: Reflection performance
  - [ ] Type info access
  - [ ] Field iteration
  - [ ] Serialization throughput
  - [ ] **LLVM Support**: LLVM codegen for benchmarks
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/reflection_tests.rs`

---

## Deferred to Future Proposals

Per `proposals/approved/reflection-api-proposal.md § Deferred Decisions`:

1. **Trait Object Reflection** — Whether trait objects store TypeInfo in vtable
2. **Function Reflection** — TypeInfo for function parameter/return types
3. **Const Generics Reflection** — How `[T, max N]` reflects capacity
4. **Derive Attributes** — `#reflect(skip)`, `#reflect(rename: ...)`
5. **Method Reflection** — Dynamic method invocation
6. **Mutable Reflection** — Modifying values through reflection

---

## Section Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/27-reflection.md` complete
- [ ] CLAUDE.md updated with Reflect, Unknown, TypeInfo
- [ ] Reflect trait works with derive
- [ ] TypeInfo accessible at runtime
- [ ] Unknown type works with safe downcasting
- [ ] current_variant works for enums
- [ ] fields() extension works
- [ ] JSON serialization example works
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Can implement generic JSON serialization/deserialization using reflection

---

## Example: Debug Printer

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
            let variant = value.current_variant().unwrap().name,
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

// Usage
#derive(Reflect)
type Point = { x: int, y: int }

#derive(Reflect)
type Line = { start: Point, end: Point }

let line = Line {
    start: Point { x: 0, y: 0 },
    end: Point { x: 10, y: 20 },
}

print(msg: debug_print(value: line))
// Line {
//   start: Point {
//     x: 0,
//     y: 0
//   },
//   end: Point {
//     x: 10,
//     y: 20
//   }
// }
```
