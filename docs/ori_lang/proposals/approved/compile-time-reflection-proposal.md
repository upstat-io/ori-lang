# Proposal: Compile-Time Reflection

**Status:** Approved
**Approved:** 2026-03-26
**Author:** Eric (with AI assistance)
**Created:** 2026-03-26
**Affects:** Compiler (parser, type system, monomorphization, evaluator, LLVM codegen), stdlib, spec
**Supersedes:** `approved/reflection-api-proposal.md` (approved 2026-01-31, zero implementation done)
**Prerequisites:** `approved/const-evaluation-termination-proposal.md` (const eval bridge)

---

## Part I: Vision & Motivation

### 1.1 Summary

This proposal introduces **compile-time structural reflection** for Ori — a set of language primitives that enable inspecting type structure at compile time and generating per-field/per-variant code with zero runtime overhead. The system consists of:

1. **Type introspection intrinsics**: `fields_of(T)`, `variants_of(T)`, `name_of(T)`
2. **Compile-time expansion**: `$for` — a general-purpose statement that unrolls at compile time
3. **Compile-time branching**: `$if` — type-checked only on the taken branch
4. **Splice access**: `value.[field]` — access a struct field by compile-time field reference

Together, these enable writing generic, type-safe, zero-cost code that operates over the structure of any type — without attributes, without macros, without runtime overhead.

The flagship target: a **pure Ori JSON parser and serializer** — no C FFI, no external libraries — that matches the performance of C++26 reflection-based serializers like simdjson. This proves Ori can compete at the lowest level while remaining safe and ergonomic.

### 1.2 Problem Statement

Ori currently lacks the ability to write code that generically operates over a type's fields. This prevents:

1. **Generic serialization** — cannot write a single `to_json()` that works for any struct
2. **Generic deserialization** — cannot populate a struct from parsed data without per-type boilerplate
3. **Schema generation** — cannot derive JSON Schema, OpenAPI, or GraphQL schemas from types
4. **Structural diff/patch** — cannot compare two values field-by-field generically
5. **ORM/database mapping** — cannot map structs to/from database rows
6. **Configuration loading** — cannot populate structs from config files generically
7. **Validation frameworks** — cannot apply per-field validation rules
8. **Builder patterns** — cannot generate builders from struct definitions
9. **Property-based testing** — cannot generate arbitrary instances of a type
10. **User-defined derives** — cannot express `#derive(Eq)` logic in user code

The existing approved reflection proposal (`reflection-api-proposal.md`) addresses these via **runtime** reflection — opt-in `#derive(Reflect)`, type-erased `Unknown`, runtime string matching for field names. This approach has fundamental limitations:

- **Runtime cost**: type erasure, dynamic dispatch, string matching at every field access
- **Opt-in friction**: every type must explicitly derive `Reflect`
- **Type safety gap**: `Unknown` requires runtime downcasts that can fail
- **Attribute-based**: `#derive(Reflect)` is decorative — the compiler already knows the structure

This proposal supersedes runtime reflection with a **compile-time** alternative that is faster (zero overhead), safer (statically typed throughout), and more ergonomic (works on any type, no opt-in).

### 1.3 Design Philosophy

This proposal follows Ori's core principles:

- **Expression-based**: `fields_of(T)` returns a value; `$for` is an expression with a result
- **Explicit over implicit**: user writes the per-field logic; the compiler provides the data
- **`$` for compile-time**: `$for` and `$if` follow the existing `$name`/`$function` convention
- **Zero-cost**: compiled output is identical to hand-written code
- **No magic**: what you see is what runs (after expansion)
- **Composable**: `fields_of` + `$for` + splice = build anything

### 1.4 Prior Art

#### C++26 Static Reflection (P2996)

C++26 introduces `^^T` (reflect operator), `template for` (expansion statement), and `[:expr:]` (splice operator). Combined with `std::meta::` functions, this enables zero-cost reflection:

```cpp
template for (constexpr auto mem : std::define_static_array(
    std::meta::nonstatic_data_members_of(^^T, ...))) {
  obj[std::meta::identifier_of(mem)].get(out.[:mem:]);
}
```

**Strengths**: Zero overhead, fully typed, compile-time expansion.
**Weaknesses**: Complex API surface (`std::meta::` namespace), operator-heavy syntax (`^^`, `[: :]`), requires C++ template machinery.

#### Zig Comptime

Zig uses `@typeInfo(T)` returning a `Type` union, `@field(obj, name)` for splice access, and `inline for` for compile-time expansion:

```zig
inline for (structInfo.fields) |Field| {
    try self.write(@field(v, Field.name));
}
```

**Strengths**: Simple mental model, types are first-class comptime values, integrated with language.
**Weaknesses**: No type system for constraining comptime operations (duck typing), error messages can be cryptic, no trait-based dispatch.

#### Rust (proc macros + serde)

Rust uses proc macros (`#[derive(Serialize)]`) that generate code at the token level. The `serde` crate is the standard serialization framework:

```rust
#[derive(Serialize, Deserialize)]
struct User { name: String, age: u32 }
```

**Strengths**: Mature ecosystem, works today.
**Weaknesses**: Proc macros are opaque (not type-checked, not IDE-friendly), attribute-heavy, separate build step, error messages from macro expansion are confusing.

#### What Makes Ori's Approach Unique

1. **No attributes**: Works on any struct — no `#derive`, no annotations, no opt-in
2. **Expression-based**: `$for` is an expression with a result type, composable with `yield`, `break`, etc.
3. **Trait integration**: compile-time expansion produces code that works with Ori's trait dispatch — no separate dispatch mechanism needed
4. **`$` convention**: compile-time constructs are visually distinct (`$for`, `$if`) and follow existing patterns
5. **Minimal surface area**: three intrinsics + two control-flow constructs + one access syntax = complete reflection

---

## Part II: Core Design

### 2.1 Type Introspection Intrinsics

Three compile-time intrinsics provide type metadata. They are built-in functions evaluated at compile time — not user-definable, not overridable.

#### `fields_of(T) -> [$FieldMeta]`

Returns a compile-time list of field metadata for a struct type.

```ori
type User = { name: str, age: int, email: str }

// At compile time:
// fields_of(User) == [
//   $FieldMeta { name: "name",  index: 0 },
//   $FieldMeta { name: "age",   index: 1 },
//   $FieldMeta { name: "email", index: 2 },
// ]
```

**Rules:**
- Only valid when `T` is a concrete type (resolved by monomorphization)
- Returns only **public** fields (private fields with `::` prefix are excluded)
- Field order matches declaration order (stable, deterministic)
- Returns empty list `[]` for non-struct types (primitives, sum types, collections, tuples)
- **Newtypes**: For newtype declarations (`type UserId = int`), returns `[$FieldMeta { name: "inner", index: 0 }]` — newtypes are structural wrappers with a single public field
- **Tuples**: Returns `[]` for tuple types — tuple elements are accessed via `.0`, `.1` positional syntax, not via named fields
- Compile-time error if `T` is an unresolved type variable
- **Warning W0461**: When called on a known concrete non-struct type (e.g., `fields_of(int)`), the compiler emits a warning suggesting `is_struct(T)` guard. This is NOT an error — the call is valid and returns `[]`

#### `variants_of(T) -> [$VariantMeta]`

Returns a compile-time list of variant metadata for a sum type.

```ori
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Point

// At compile time:
// variants_of(Shape) == [
//   $VariantMeta { name: "Circle",    index: 0, fields: [$FieldMeta { name: "radius", index: 0 }] },
//   $VariantMeta { name: "Rectangle", index: 1, fields: [$FieldMeta { name: "width", index: 0 }, $FieldMeta { name: "height", index: 1 }] },
//   $VariantMeta { name: "Point",     index: 2, fields: [] },
// ]
```

**Rules:**
- Only valid when `T` is a concrete sum type
- Returns only public variants
- Variant order matches declaration order
- Returns empty list `[]` for non-sum types
- Each variant's `fields` follows the same rules as `fields_of`

#### `name_of(T) -> str`

Returns the name of a type as a compile-time string.

```ori
name_of(User)          // "User"
name_of(int)           // "int"
name_of(Option<str>)   // "Option"
name_of([int])         // "List"
```

**Rules:**
- Returns the unqualified type name (no module path)
- For generic types, returns the base name without type arguments
- For primitives, returns the keyword name

### 2.2 Compile-Time Metadata Types

Two types represent compile-time field and variant information. These are **compiler-internal types** that exist only at compile time — they have no runtime representation and cannot be stored, returned from non-const functions, or passed to runtime code.

```ori
// Compiler-provided, not user-definable
type $FieldMeta = {
    name: str,       // Field name as string literal
    index: int,      // 0-based position in struct
}

type $VariantMeta = {
    name: str,           // Variant name
    index: int,          // 0-based position in sum type
    fields: [$FieldMeta], // Payload fields (empty for unit variants)
}
```

The `$` prefix signals compile-time-only types, consistent with `$name` (const binding) and `$function` (const function).

**Key design choice**: `$FieldMeta` does **not** carry a `type` field. Types are not first-class values in Ori. Instead, the type of `value.[field]` is resolved by the compiler at expansion time — the same way `value.name` has a known type. See §2.5 for details.

### 2.3 Compile-Time Expansion: `$for`

`$for` is a **compile-time expansion statement** that unrolls a loop body once per element of a compile-time iterable. It is syntactically identical to `for...yield` / `for...do` but prefixed with `$`.

#### Syntax

```
$for <pattern> in <const-iterable> yield <body>
$for <pattern> in <const-iterable> do <body>
$for <pattern> in <const-iterable> if <const-guard> yield <body>
```

#### Homogeneous Expansion (const lists)

When iterating over a compile-time list of uniform type, `$for` unrolls to equivalent code:

```ori
// Source
let $items = [1, 2, 3]
$for x in items yield x * 2

// Expands to (conceptually)
[1 * 2, 2 * 2, 3 * 2]
```

This is straightforward — each iteration has the same type.

#### Heterogeneous Expansion (fields_of / variants_of)

When iterating over `fields_of(T)` or `variants_of(T)`, each iteration may produce a **differently typed** expression. This is the power of compile-time expansion:

```ori
type User = { name: str, age: int }

@show_fields<T>(value: T) -> [str] = {
    $for field in fields_of(T) yield {
        let val = value.[field]    // Iteration 0: str, Iteration 1: int
        `{field.name}: {val}`      // Both produce str (via interpolation)
    }
}
```

**Expansion of `show_fields<User>(user)`:**

```ori
// The compiler generates (conceptually):
@show_fields_User(value: User) -> [str] = {
    [
        { let val = value.name; `name: {val}` },
        { let val = value.age;  `age: {val}` },
    ]
}
```

Each expanded body is **independently type-checked**. This is what enables zero-cost heterogeneous code generation.

#### Rules

- The iterable must be a compile-time constant (const binding, literal, `fields_of`/`variants_of` result)
- `$for...yield` produces a list; `$for...do` produces `void`
- Guards (`if`) must also be compile-time evaluable
- `break` and `continue` are not valid in `$for` (it is not a loop — it is expansion)
- Expansion happens during monomorphization, when all type parameters are concrete
- Nested `$for` is allowed: `$for v in variants_of(T) do { $for f in v.fields yield ... }`
- Empty iteration produces `[]` for yield, `void` for do

### 2.4 Compile-Time Branching: `$if`

`$if` is a **compile-time conditional** where only the taken branch is type-checked. The dead branch is syntactically parsed but not type-checked or compiled. This is Ori's expression-based equivalent of C++'s `if constexpr`.

#### Syntax

```
$if <const-condition> then <expr> else <expr>
$if <const-condition> then <expr>
```

#### Use Cases

```ori
@serialize<T>(value: T) -> str = {
    $if fields_of(T).is_empty() then {
        // T is not a struct (primitive, etc.) — use Printable
        value.to_str()
    } else {
        // T is a struct — iterate fields
        let parts = $for field in fields_of(T) yield {
            `"{field.name}": {serialize(value.[field])}`
        }
        `{ {parts.join(sep: ", ")} }`
    }
}
```

**Why `$if` is needed**: Regular `if` requires both branches to type-check. But `value.to_str()` might not work for all `T`, and `fields_of(T)` iteration might not work for primitives. `$if` ensures only the relevant branch is checked.

#### Rules

- Condition must be a compile-time evaluable boolean expression
- Dead branch is parsed but not type-checked (syntax errors still reported)
- Both branches must be syntactically valid Ori expressions
- `$if` is an expression — it returns a value (the taken branch's result)
- Nested `$if` is allowed
- When the condition cannot be determined at compile time, it is a compile error

### 2.5 Splice Access: `value.[field]`

The **splice operator** `.[field]` accesses a struct field using a compile-time `$FieldMeta` reference. It resolves to a direct field access at compile time.

#### Syntax

```
<expr>.[<field-meta-expr>]
```

#### Semantics

```ori
type User = { name: str, age: int }

@example (user: User) -> void = {
    $for field in fields_of(User) do {
        let val = user.[field]
        // Iteration 0: val has type str (user.name)
        // Iteration 1: val has type int (user.age)
        print(msg: `{field.name} = {val}`)
    }
}
```

**Expansion:**

```ori
// Iteration 0: field = $FieldMeta { name: "name", index: 0 }
let val = user.name     // type: str

// Iteration 1: field = $FieldMeta { name: "age", index: 1 }
let val = user.age      // type: int
```

#### Rules

- `field` must be a compile-time `$FieldMeta` value (typically from `$for` over `fields_of`)
- The expression left of `.[field]` must be a struct type that contains the referenced field
- The result type is the type of the accessed field (resolved at compile time)
- Compile error if `field` does not belong to the struct type
- `.[field]` on the left side of assignment: `value.[field] = x` is valid (follows existing index/field assignment rules)
- Nested splice: `value.[outer_field].[inner_field]` works if the outer field is itself a struct

#### Why `.[]` Syntax

- Consistent with existing index syntax: `list[i]`, `map[key]`
- The `.` prefix distinguishes field splice from index access (fields are structural, not keyed)
- Reads naturally: "value dot (this field)"
- No new operator needed — extends existing dot-access with computed field

### 2.6 Type Classification Intrinsics

Compile-time predicates for type classification, usable in `$if` conditions:

```ori
is_struct(T) -> bool       // true for struct types
is_enum(T) -> bool         // true for sum types
is_primitive(T) -> bool    // true for int, float, bool, str, char, byte, void
is_collection(T) -> bool   // true for [T], {K: V}, Set<T>
is_option(T) -> bool       // true for Option<T>
is_result(T) -> bool       // true for Result<T, E>
is_tuple(T) -> bool        // true for (T, U, ...)
```

These are compile-time intrinsics — they accept either a **type parameter** or an **expression**:

- **Type parameter form**: `is_struct(T)` — checks the type `T` directly
- **Expression form**: `is_option(value.[field])` — inspects the **static type** of the expression at compile time. The expression is not evaluated — only its compile-time type is inspected.

The expression form is essential inside `$for` over `fields_of`, where each iteration's splice has a different type:

```ori
$for field in fields_of(T) do {
    $if is_option(value.[field]) then {
        // This field's type is Option<_> — handle nullable
    } else {
        // Non-optional field
    }
}
```

They cannot be called with values whose type is not statically known at compile time.

```ori
@format_value<T>(value: T) -> str = {
    $if is_struct(T) then {
        let parts = $for field in fields_of(T) yield {
            `{field.name}: {format_value(value.[field])}`
        }
        `{name_of(T)} { {parts.join(sep: ", ")} }`
    } else $if is_enum(T) then {
        // Enum handling with variants_of
        ...
    } else {
        value.to_str()
    }
}
```

### 2.7 Variant Matching with `$for`

For sum types, `variants_of(T)` provides variant metadata. Matching against variants requires combining `$for` with `match`:

```ori
@enum_to_json<T>(value: T) -> JsonValue = {
    match value {
        $for variant in variants_of(T) {
            // Each iteration generates a match arm
            $if variant.fields.is_empty() then {
                variant.pattern -> JsonValue.String(variant.name)
            } else {
                variant.pattern(captured) -> {
                    let fields = $for field in variant.fields yield {
                        (field.name, captured.[field].to_json())
                    }
                    JsonValue.Object(
                        {str: JsonValue}.from_iter(iter: [
                            ("type", JsonValue.String(variant.name)),
                            ...fields,
                        ].iter())
                    )
                }
            }
        }
    }
}
```

**Open question**: The syntax for generating match arms from `$for` needs careful design. `variant.pattern` is a placeholder for "the pattern that matches this variant." See Part VII, Open Questions.

### 2.8 Access Control

- `fields_of(T)` returns only **public fields** by default
- Fields prefixed with `::` (Ori's private visibility marker) are excluded
- This matches the principle that private fields are implementation details
- **No `uses Unsafe` escape hatch for private fields** — if you need to reflect on private fields, expose them through a public API

Rationale: serialization, validation, ORM — all operate on the public API surface. Private fields are private for a reason. If a type wants custom serialization that includes private data, it implements the trait manually.

### 2.9 Field Annotations (Future Extension Point)

This proposal does **not** include field-level annotations (like Rust's `#[serde(rename = "user_name")]`). However, the design is forward-compatible with a future annotation system:

```ori
// Hypothetical future syntax (NOT part of this proposal)
type User = {
    #json(rename: "user_name")
    name: str,

    #json(skip)
    internal_id: int,
}
```

If annotations are added later, `$FieldMeta` would be extended with an `annotations` field, and `$for` iteration would naturally expose them.

---

## Part III: Semantics

### 3.1 Expansion Timing

`$for` and `$if` expansion occurs **during monomorphization** — after type inference, when all type parameters have been resolved to concrete types.

**Pipeline position:**

```
Parse → Type Check → Monomorphize → ARC Lower → LLVM Codegen
                          ↑
                    $for expansion happens here
                    fields_of(T) evaluated here
                    $if conditions resolved here
```

This timing is critical because:
- `T` must be concrete for `fields_of(T)` to produce field metadata
- Each expanded iteration must be independently type-checked with concrete types
- Monomorphization already produces per-type copies — expansion is a natural extension

**Why not during type checking?** In a generic function like `@f<T>(...)`, `T` is abstract. `fields_of(T)` cannot produce concrete results. Only when `f` is instantiated as `f<User>` does `T = User` become known.

**Why not during codegen?** By monomorphization time, type checking has completed. But expanded code needs type checking (each iteration may call different generic functions). Expansion must happen early enough for the type checker to verify each expanded copy.

**Resolution:** Expansion is a **sub-phase of monomorphization**. When the monomorphizer encounters `$for` or `$if` in a generic function being instantiated:

1. Evaluate the const iterable/condition (with concrete type substitutions)
2. Expand the loop body N times (or select the taken branch)
3. Type-check each expanded copy (with concrete types)
4. Continue monomorphization on the expanded code

### 3.2 Heterogeneous Type Checking

Each iteration of `$for` over `fields_of(T)` produces a differently-typed expression. The type checker handles this by treating each expanded copy as a separate expression:

```ori
$for field in fields_of(User) yield serialize(value.[field])
```

Expands to (conceptually):
```ori
[
    serialize(value.name),   // type-check: serialize<str>(str) -> str ✓
    serialize(value.age),    // type-check: serialize<int>(int) -> str ✓
]
```

The result type of `$for...yield` is `[T]` where `T` is the **unified type** of all expanded bodies. If all bodies produce `str`, the result is `[str]`. If bodies produce incompatible types, it is a compile error.

**Special case**: if the body type varies per iteration but all types implement a common trait, the user must ensure the yield expression has a uniform type (e.g., by converting to `str` via interpolation or `.to_str()`).

### 3.3 Interaction with Generics

`$for` and `fields_of` are designed to work inside generic functions:

```ori
@struct_to_map<T>(value: T) -> {str: str} = {
    let entries = $for field in fields_of(T) yield {
        (field.name, value.[field].to_str())
    }
    {str: str}.from_iter(iter: entries.iter())
}
```

When `struct_to_map<User>(user)` is called:
1. Monomorphizer creates `struct_to_map<User>`
2. `fields_of(User)` evaluates to User's field list
3. `$for` expands to concrete field access
4. Each `.to_str()` call resolves to the concrete type's `Printable` impl
5. Result is a fully monomorphized function with no generics

**Recursive reflection**: Generic functions using `$for` can call themselves recursively for nested structs:

```ori
@deep_debug<T>(value: T) -> str = {
    $if is_struct(T) then {
        let parts = $for field in fields_of(T) yield {
            `{field.name}: {deep_debug(value.[field])}`
        }
        `{name_of(T)} { {parts.join(sep: ", ")} }`
    } else {
        value.to_str()
    }
}
```

Each recursive call is independently monomorphized: `deep_debug<User>` calls `deep_debug<str>` and `deep_debug<int>`, each of which hit the `$if` else branch.

### 3.4 Interaction with Traits

Compile-time reflection composes naturally with Ori's trait system:

#### Default Impls with Reflection

```ori
trait ToJson {
    @to_json (self) -> JsonValue
}

// Default implementation using compile-time reflection
pub def impl ToJson {
    @to_json (self) -> JsonValue = {
        $if is_struct(Self) then {
            let entries = $for field in fields_of(Self) yield {
                (field.name, self.[field].to_json())
            }
            JsonValue.Object({str: JsonValue}.from_iter(iter: entries.iter()))
        } else $if is_enum(Self) then {
            // ... variant handling ...
            todo()
        } else {
            // Primitives must provide their own impl
            compile_error("ToJson requires either a struct type or an explicit impl")
        }
    }
}
```

When a user type doesn't have an explicit `ToJson` impl, the default impl is used. `Self` resolves to the concrete type, enabling `fields_of(Self)` to work.

#### Trait Bounds on Reflected Fields

If a generic function using `$for` calls a trait method on `value.[field]`, the compiler verifies that each concrete field type satisfies the trait:

```ori
@serialize_all<T>(value: T) -> [str] = {
    $for field in fields_of(T) yield value.[field].to_str()
}

// serialize_all<User>(user) expands to:
// [user.name.to_str(), user.age.to_str()]
// Both str and int implement Printable, so this type-checks ✓

type BadType = { callback: (int) -> int }
// serialize_all<BadType>(bad) would fail:
// error: (int) -> int does not implement Printable
```

Errors are reported at the **expansion site** with clear context: which field, which type, which trait is missing.

### 3.5 Interaction with Const Evaluation

`$for` and `$if` build on the const evaluation infrastructure defined in the Const Evaluation Termination proposal:

- `fields_of(T)` and `variants_of(T)` are const intrinsics (evaluated at compile time)
- `field.name` and `field.index` are const values
- `$if` conditions are const-evaluated
- `$for` iterables must be const

However, `$for` is **not** regular const evaluation — it is **expansion**. The body of `$for` is not const-evaluated; it is replicated and compiled as normal code. Only the iteration structure is compile-time.

```ori
// The iterable is const, the body is runtime
$for field in fields_of(T) do {
    print(msg: value.[field].to_str())   // runtime: print has side effects
}
```

### 3.6 Zero-Cost Guarantee

The compiled output of code using `$for` + `fields_of` shall be **identical** to hand-written code that directly accesses each field.

Given:
```ori
@sum_fields (user: User) -> str = {
    $for field in fields_of(User) yield {
        `{field.name}: {user.[field]}`
    }
}
```

The LLVM IR shall be identical to:
```ori
@sum_fields_handwritten (user: User) -> str = {
    [`name: {user.name}`, `age: {user.age}`, `email: {user.email}`]
}
```

Specifically:
- No runtime metadata structures are allocated
- No string comparisons for field lookup
- No type erasure or dynamic dispatch from reflection
- Field access compiles to direct `getelementptr` + `load` (same as `user.name`)
- Field names are string literal constants in the binary (same as `"name"`)
- `$for` produces no loop — only unrolled direct code

---

## Part IV: Use Cases

### 4.1 Flagship: Pure Ori JSON Parser

The primary proving ground for compile-time reflection is a **pure Ori JSON library** — no C FFI, no yyjson, no simdjson — that achieves competitive performance through:

1. **Compile-time reflection** for zero-cost struct mapping (`$for` + `fields_of`)
2. **SIMD intrinsics** (`uses Intrinsics`) for vectorized structural scanning
3. **Fixed-capacity lists** (`[byte, max 64]`) for SIMD chunk types
4. **Buffer/Writer** for allocation-efficient output

This supersedes the previous `stdlib-json-api-ffi-revision.md` proposal (which proposed yyjson via C FFI). The goal is to prove Ori can compete at the lowest level without escape hatches.

#### Architecture

```
                    Pure Ori — no C, no FFI
                    ┌─────────────────────────────────┐
JSON text ──parse──→│  SIMD scanner (uses Intrinsics)  │
                    │  Structural character finder      │
                    │  Branch-free number parsing        │
                    └────────────┬────────────────────┘
                                 │
                    ┌────────────▼────────────────────┐
                    │  On-demand parser                 │
                    │  Lazy field access, no DOM build   │
                    │  Compile-time key matching         │
                    └────────────┬────────────────────┘
                                 │
                    ┌────────────▼────────────────────┐
                    │  $for + fields_of(T)             │
                    │  Zero-cost struct construction    │
                    │  Monomorphized per target type    │
                    └─────────────────────────────────┘

User { name: "Alice", age: 30 }   ← direct, no intermediate JsonValue
```

#### Serialization (Typed → JSON text)

```ori
use std.json { JsonValue, JsonWriter }

trait ToJson {
    @to_json (self) -> JsonValue
    @write_json (self, writer: JsonWriter) -> void   // fast path: direct buffer writes
}

// Primitive implementations
impl int: ToJson {
    @to_json (self) -> JsonValue = JsonValue.Number(value: self as float)
    @write_json (self, writer: JsonWriter) -> void = writer.write_int(value: self)
}

impl str: ToJson {
    @to_json (self) -> JsonValue = JsonValue.String(self)
    @write_json (self, writer: JsonWriter) -> void = {
        writer.write_byte(value: b'"')
        writer.write_escaped(value: self)
        writer.write_byte(value: b'"')
    }
}

impl bool: ToJson {
    @to_json (self) -> JsonValue = JsonValue.Bool(self)
    @write_json (self, writer: JsonWriter) -> void = {
        writer.write_str(value: if self then "true" else "false")
    }
}

impl<T: ToJson> [T]: ToJson {
    @to_json (self) -> JsonValue = {
        JsonValue.Array(self.map(transform: t -> t.to_json()))
    }
    @write_json (self, writer: JsonWriter) -> void = {
        writer.write_byte(value: b'[')
        for item in self.enumerate() do {
            if item.0 > 0 then writer.write_byte(value: b',')
            item.1.write_json(writer: writer)
        }
        writer.write_byte(value: b']')
    }
}

impl<T: ToJson> Option<T>: ToJson {
    @to_json (self) -> JsonValue = match self {
        Some(v) -> v.to_json(),
        None -> JsonValue.Null,
    }
    @write_json (self, writer: JsonWriter) -> void = match self {
        Some(v) -> v.write_json(writer: writer),
        None -> writer.write_str(value: "null"),
    }
}

// Default impl: any struct whose fields implement ToJson gets it for free
pub def impl ToJson {
    @to_json (self) -> JsonValue = {
        let entries = $for field in fields_of(Self) yield {
            (field.name, self.[field].to_json())
        }
        JsonValue.Object({str: JsonValue}.from_iter(iter: entries.iter()))
    }

    // Fast path: write directly to buffer, no intermediate JsonValue
    @write_json (self, writer: JsonWriter) -> void = {
        writer.write_byte(value: b'{')
        $for field in fields_of(Self) do {
            $if field.index > 0 then writer.write_byte(value: b',')
            writer.write_byte(value: b'"')
            writer.write_str(value: field.name)    // compile-time string literal
            writer.write_str(value: "\":")
            self.[field].write_json(writer: writer) // monomorphized per field type
        }
        writer.write_byte(value: b'}')
    }
}
```

The `write_json` fast path writes directly to a buffer — no intermediate `JsonValue` allocation. Each field name is a compile-time string literal. Each `write_json` call is monomorphized per field type. The `$for` is fully unrolled. The generated code is identical to hand-written serialization.

#### Deserialization (JSON text → Typed)

```ori
trait FromJson {
    @from_json (json: JsonValue) -> Result<Self, JsonError>
}

// Primitive implementations
impl int: FromJson {
    @from_json (json: JsonValue) -> Result<int, JsonError> = match json {
        JsonValue.Number(n) -> Ok(n as int),
        _ -> Err(JsonError { kind: TypeMismatch, path: "", message: "expected number" }),
    }
}

// ... str, float, bool, etc.

// Default impl for structs
pub def impl FromJson {
    @from_json (json: JsonValue) -> Result<Self, JsonError> = match json {
        JsonValue.Object(obj) -> {
            $for field in fields_of(Self) do {
                let field_json = obj[field.name] ?? JsonValue.Null
                let parsed = FromJson.from_json(json: field_json)?
                // Construct field... (see Open Questions §7.1 for struct construction)
            }
            // ... build Self from parsed fields
            todo()
        },
        _ -> Err(JsonError { kind: TypeMismatch, path: "", message: `expected object for {name_of(Self)}` }),
    }
}
```

**Note**: Deserialization requires constructing a struct field-by-field, which is an open design question (§7.1).

#### On-Demand Parsing (Future — Peak Performance)

The ultimate performance target: skip `JsonValue` entirely and parse JSON text directly into typed structs. With compile-time reflection, the parser knows the field names at compile time and can generate a per-type parser:

```ori
// Hypothetical on-demand API — separate proposal, shown here for vision
@parse_into<T>(input: [byte]) -> Result<T, JsonError> uses Intrinsics = {
    let scanner = SimdScanner.new(input: input)
    scanner.expect_byte(value: b'{')
    $for field in fields_of(T) do {
        // Compile-time known field name → can generate perfect hash or trie
        let key = scanner.read_key()
        // ... match key against field.name, parse value into field type
    }
    // ... construct T from parsed fields
    todo()
}
```

This is the simdjson "on-demand" model — pure Ori, SIMD-accelerated, zero intermediate allocation. It depends on:
- Compile-time reflection (this proposal)
- SIMD intrinsics (`uses Intrinsics` — §06, spec'd)
- Fixed-capacity lists (`[byte, max 64]` — §18.2)
- Const generics for SIMD lane width (`$N: int` — §18.1)
- Struct construction from `$for` (Open Question §7.1)

A separate **std.json native parser proposal** will define the full design.

#### Usage — Zero Boilerplate

```ori
type User = { name: str, age: int, email: Option<str> }
type Team = { name: str, members: [User] }

let team = Team {
    name: "Engineering",
    members: [
        User { name: "Alice", age: 30, email: Some("alice@example.com") },
        User { name: "Bob", age: 25, email: None },
    ],
}

// Serialization: struct → JSON text (fast path, direct buffer writes)
let json_str = team.to_json_str()
// {"name":"Engineering","members":[{"name":"Alice","age":30,"email":"alice@example.com"},{"name":"Bob","age":25,"email":null}]}

// Deserialization: JSON text → struct
let parsed = Team.from_json_str(json: json_str)?
assert_eq(actual: parsed, expected: team)
```

No `#derive`. No attributes. No FFI. No C. Just Ori.

### 4.3 Structural Equality (User-Defined Derive)

What `#derive(Eq)` does internally can be expressed in user code:

```ori
@structural_eq<T>(a: T, b: T) -> bool = {
    $if is_struct(T) then {
        $for field in fields_of(T) yield {
            a.[field] == b.[field]
        }.all(predicate: x -> x)
    } else {
        a == b
    }
}
```

### 4.4 Debug Formatting

```ori
@structural_debug<T>(value: T) -> str = {
    $if is_struct(T) then {
        let parts = $for field in fields_of(T) yield {
            `{field.name}: {structural_debug(value.[field])}`
        }
        `{name_of(T)} { {parts.join(sep: ", ")} }`
    } else $if is_enum(T) then {
        // ... variant handling
        todo()
    } else {
        value.debug()
    }
}
```

### 4.5 Schema Generation

```ori
type JsonSchema = {
    type_name: str,
    properties: {str: JsonSchema},
    required: [str],
}

@generate_schema<T>(phantom: T) -> JsonSchema = {
    $if is_struct(T) then {
        let props = $for field in fields_of(T) yield {
            (field.name, generate_schema(phantom: phantom.[field]))
        }
        let required = $for field in fields_of(T) if !is_option(phantom.[field]) yield {
            field.name
        }
        JsonSchema {
            type_name: "object",
            properties: {str: JsonSchema}.from_iter(iter: props.iter()),
            required: required,
        }
    } else $if is_primitive(T) then {
        JsonSchema { type_name: name_of(T), properties: {}, required: [] }
    } else {
        JsonSchema { type_name: "unknown", properties: {}, required: [] }
    }
}
```

### 4.6 Configuration Loading

```ori
@load_from_env<T>() -> Result<T, Error> uses Env = {
    // Build a struct from environment variables using field names as keys
    // (See Open Questions §7.2 for struct construction from $for)
    todo()
}
```

### 4.7 Validation Framework

```ori
@validate<T>(value: T, rules: {str: (str) -> Result<void, str>}) -> [str] = {
    $for field in fields_of(T) yield {
        match rules[field.name] {
            Some(rule) -> match rule(value.[field].to_str()) {
                Ok(_) -> [],
                Err(msg) -> [`{field.name}: {msg}`],
            },
            None -> [],
        }
    }.flatten()
}
```

### 4.8 Structural Diff

```ori
type FieldDiff = { field_name: str, old_value: str, new_value: str }

@diff<T>(old: T, new: T) -> [FieldDiff] = {
    $for field in fields_of(T) yield {
        if old.[field] != new.[field] then {
            [FieldDiff {
                field_name: field.name,
                old_value: old.[field].to_str(),
                new_value: new.[field].to_str(),
            }]
        } else {
            []
        }
    }.flatten()
}
```

---

## Part V: Implementation Strategy

### Phase 1: Const Evaluation Bridge (Prerequisite)

**Status**: Already proposed in `const-evaluation-termination-proposal.md`. Implementation needed.

**Scope**: Enable the type checker to invoke the evaluator for compile-time expressions.

**Key deliverables:**
- Type checker → evaluator bridge (ori_types can invoke ori_eval in ConstEval mode)
- Const function call evaluation when all arguments are const
- Result caching/memoization
- Budget/limit enforcement
- `compile_error()` and `embed()` as first consumers

**Compiler changes:**
- `ori_types`: Add evaluator invocation path in const expression contexts
- `ori_eval`: Ensure `ConstEval` mode is production-ready (currently exists but unused)
- Caching layer between type checker and evaluator

### Phase 2: Type Introspection Intrinsics

**Scope**: Implement `fields_of(T)`, `variants_of(T)`, `name_of(T)`.

**Key deliverables:**
- `$FieldMeta` and `$VariantMeta` as compiler-internal types
- Intrinsic evaluation in const context
- Integration with monomorphization (resolve `T` to concrete type before evaluation)

**Compiler changes:**
- `ori_ir`: Add `$FieldMeta`, `$VariantMeta` type representations
- `ori_types`: Register intrinsics, type-check calls, evaluate during monomorphization
- `ori_eval`: Add intrinsic handlers for interpreter mode
- Tests: unit tests for each intrinsic with struct, sum, primitive, generic types

### Phase 3: `$for` Expansion

**Scope**: Implement `$for` expansion for both homogeneous and heterogeneous iteration.

**Key deliverables:**
- Parser: `$for` syntax (reuse `for` grammar with `$` prefix detection)
- Type checker: validate const iterable, expand during monomorphization
- Heterogeneous type checking per expanded copy
- Integration with `yield` and `do`

**Compiler changes:**
- `ori_parse`: Add `$for` parsing (minimal — reuse `for` production with `is_comptime` flag)
- `ori_ir`: Add `is_comptime: bool` to `For` AST node
- `ori_types`: Expansion logic in monomorphization sub-phase
- `ori_eval`: Expansion in ConstEval mode (for interpreter)
- `ori_llvm`: No changes needed (expansion happens before codegen)
- Tests: homogeneous expansion, heterogeneous expansion, nested expansion, empty iteration

### Phase 4: `$if` and Splice Access

**Scope**: Implement `$if` branching and `value.[field]` splice.

**Key deliverables:**
- Parser: `$if` syntax, `.[expr]` splice syntax
- Type checker: dead branch elimination for `$if`, field resolution for splice
- Codegen: splice lowers to direct field access

**Compiler changes:**
- `ori_parse`: Add `$if` parsing, `.[expr]` splice parsing
- `ori_ir`: Add `is_comptime: bool` to `If` AST node; add `Splice` expression variant
- `ori_types`: Conditional expansion for `$if`; type resolution for splice
- `ori_eval`: Splice evaluation (resolve field at const-eval time)
- `ori_llvm`: Splice lowers to `extract_value` / `getelementptr` (same as named field access)
- Tests: `$if` with dead branch, splice in `$for`, nested splice, assignment through splice

### Phase 5: Type Classification Intrinsics

**Scope**: Implement `is_struct(T)`, `is_enum(T)`, etc.

**Key deliverables:**
- Const intrinsics for type classification
- Integration with `$if` for type-driven code generation

**Compiler changes:**
- `ori_types`: Type classification predicates (trivial — check type pool tags)
- `ori_eval`: Intrinsic handlers
- Tests: all type classifications with various types

### Phase 6: Integration & Polish

**Scope**: End-to-end validation, error messages, documentation.

**Key deliverables:**
- Comprehensive error messages for misuse (non-const iterable, splice on non-struct, etc.)
- Spec clause update (supersede §27)
- std.json revision
- Example implementations of ToJson, FromJson, Debug, Eq using compile-time reflection

---

## Part VI: Roadmap Impact

### Supersedes

| Item | Status | Action |
|------|--------|--------|
| `reflection-api-proposal.md` | Approved, zero impl | Superseded. Move to `superseded/` |
| `stdlib-json-api-proposal.md` | Approved, zero impl | Superseded. New native JSON parser proposal needed |
| `stdlib-json-api-ffi-revision.md` | Approved, zero impl | Superseded. Pure Ori, no FFI |
| Spec Clause 27 (Reflection) | Written, no impl | Rewrite for compile-time model. Runtime `Reflect` trait, `TypeInfo`, `Unknown`, `FieldInfo`, `VariantInfo` types are **removed from the specification**. |
| Roadmap §20 (Runtime Reflection) | Not started | Replace with §20 (Compile-Time Reflection) |

The JSON proposals are superseded because they were designed around (a) a runtime `Json` trait, and (b) C FFI to yyjson. The new direction is a pure Ori JSON library using compile-time reflection + SIMD intrinsics. A separate **std.json native parser proposal** will define the full library design.

### Updates Required

| Item | Change |
|------|--------|
| Spec Clause 24 (Const Expressions) | Add `$for`, `$if`, splice to allowed const operations |
| Spec `grammar.ebnf` | Add `$for`, `$if`, `.[expr]` productions |
| Roadmap §00 (Parser) | Add `$for`, `$if`, splice syntax |
| Roadmap §18 (Const Generics) | Note shared dependency on const eval bridge |
| Roadmap §07D (Stdlib Modules) | std.json entry updated to reference native parser proposal |

### Dependencies

**For compile-time reflection itself:**

| This proposal needs | Status |
|---------------------|--------|
| Monomorphization (type generics) | Complete |
| Const function parsing & type checking | Complete |
| Const evaluation bridge (§18.0) | Not started — shared prerequisite |
| `compile_error()` (§07A) | Not started — validates bridge |

**For the flagship pure Ori JSON parser (separate proposal, shown for context):**

| Feature | Status | Why needed |
|---------|--------|------------|
| Compile-time reflection | This proposal | Zero-cost struct mapping |
| Const generics basics (§18.1) | In progress | SIMD lane width `$N: int` |
| Fixed-capacity lists (§18.2) | Not started | SIMD chunk type `[byte, max 64]` |
| SIMD intrinsics impl (§06) | Spec'd, not impl'd | Vectorized structural scanning |
| Writer/Buffer type (§07D) | Not designed | Allocation-efficient output |
| Struct construction from `$for` | Open Question §7.1 | On-demand deserialization |
| Deep safety (propagation) | Draft proposals | Capability model for `uses Intrinsics` |

The reflection proposal and the JSON dependencies are **independent tracks** that can proceed in parallel. Reflection does not require SIMD or const generics. The JSON parser requires both.

### Does NOT Depend On

| Feature | Why not |
|---------|---------|
| FFI (§11) | Pure Ori — no C libraries |
| Capability unification Phase 1 | Nice-to-have (`: Trait` syntax) but not blocking |
| Runtime reflection (§20 old) | Superseded, not prerequisite |
| Existential types (§19) | Independent feature |
| Async/concurrency (§16/17) | Independent feature |

---

## Part VII: Open Questions

### Q1: Struct Construction from `$for`

**Problem**: Deserialization requires building a struct field-by-field. `$for` iterates over fields, but how do you construct a `User` from individually parsed fields?

**Current thinking**: A `$construct<T>(fields: [$FieldValue])` intrinsic that builds a struct from a compile-time-known list of field name/value pairs. Or: `$for` with a special accumulator pattern.

**Alternatives:**
- Require all struct types to implement `Default`, then set fields via `.[field] = value`
- A `$build<T>` expression that takes a block of field assignments
- Named tuple construction: `T.from_fields(...)` auto-generated

**Decision**: ~~Deferred to a separate proposal.~~ **Resolved** by `approved/compile-time-construction-proposal.md` (approved 2026-03-26). The `$construct<T>` intrinsic builds a struct from `($FieldMeta, value)` pairs produced by `$for...yield`, with zero-cost expansion to a direct struct literal. `$construct_partial<T: Default>` handles partial construction with defaults for missing fields.

### Q2: Variant Pattern Generation

**Problem**: §2.7 shows `variant.pattern` as a placeholder for generating match arms from variant metadata. What is the actual syntax?

**Current thinking**: `$match` — a compile-time match that generates arms from `variants_of(T)`:

```ori
$match value {
    $for variant in variants_of(T) -> {
        // variant.bind gives a pattern that binds payload fields
        variant.bind -> handle(variant)
    }
}
```

**Decision**: Deferred to detailed design. Enum reflection is Phase 2+ work.

### Q3: Types as Compile-Time Values

**Problem**: Should Ori support types as first-class compile-time values (like Zig's `type` type)?

**Current thinking**: No — for v1. Ori's trait system handles polymorphic dispatch. `$for` expansion resolves types implicitly. Adding types-as-values would require significant type system changes (approaching dependent types).

**Future possibility**: If demand materializes, a `$Type` compile-time type could be added without breaking the v1 design. `fields_of` would gain a `type` field on `$FieldMeta`.

### Q4: Type Construction from Metadata

**Problem**: Should Ori support Zig's `@Type(info)` — constructing types from metadata at compile time?

**Current thinking**: No — for v1. Type construction enables powerful metaprogramming but adds significant complexity. The primary use cases (serialization, validation, debugging) do not require it.

**Future possibility**: A `$Type(fields: [$FieldDef])` intrinsic for advanced metaprogramming.

### Q5: Method Reflection

**Problem**: Should compile-time reflection expose method information?

**Current thinking**: No — for v1. Field reflection covers the primary use cases. Method reflection would require reflecting on function signatures, trait impls, and dispatch tables — significantly more complex.

### Q6: Private Field Access

**Problem**: Should there be an escape hatch for reflecting on private fields?

**Current thinking**: No. If a type needs custom serialization that includes private data, it implements the trait manually. This preserves encapsulation.

### Q7: Compile-Time String Operations in `$for`

**Problem**: Inside `$for`, can you do compile-time string operations on `field.name`? E.g., `field.name.to_uppercase()` for generating enum-style names.

**Current thinking**: Yes — `field.name` is a compile-time string, and const functions on strings should work. This depends on the const eval bridge (Phase 1) supporting string operations.

### Q8: Interaction with `#derive` Syntax Transition

**Problem**: The capability unification proposal changes `#derive(Trait)` to `type T: Trait = { ... }`. How does this interact with compile-time reflection?

**Current thinking**: Orthogonal. `#derive`/`: Trait` is about auto-generating trait impls. Compile-time reflection is about user-written generic code that inspects type structure. They serve different needs:
- `: Eq` → compiler generates equality impl (simple, predictable)
- `$for field in fields_of(T)` → user writes custom logic (flexible, powerful)

Both can coexist. Over time, users might prefer writing traits generically using `$for` rather than relying on derive.

---

## Part VIII: Non-Goals

1. **Runtime reflection** — this proposal is compile-time only. The runtime `Reflect` trait, `Unknown` type, `TypeInfo`, and runtime downcasting from the approved `reflection-api-proposal.md` are **superseded and removed** from the language specification. Runtime type introspection may be reintroduced in a future proposal under a different scope (plugin systems, scripting, hot-reload) but is not part of the v2026 specification. Compile-time reflection provides strictly more capability with zero runtime cost.

2. **Macro system** — `$for` is expansion, not macros. It does not operate on tokens or syntax trees. It operates on typed values (field metadata) and produces typed expressions.

3. **Aspect-oriented programming** — no method interception, no proxy generation, no advice.

4. **Mutable reflection** — reflection is read-only. You can read field values through splice, but structural modification (adding/removing fields) is not supported.

5. **Dynamic type creation** — types are defined at compile time. Reflection inspects existing types; it does not create new ones at runtime.

6. **Code generation beyond expansion** — `$for` does not generate new functions, types, or modules. It generates expressions within an existing function body.

---

## Part IX: Error Messages

### E0460: `$for` over non-const iterable

```
error[E0460]: $for requires a compile-time iterable
  --> src/main.ori:10:17
   |
10 |     $for x in items yield x * 2
   |               ^^^^^ `items` is not a compile-time constant
   |
   = help: $for iterates at compile time; use `for` for runtime iteration
   = help: if `items` should be const, declare with `let $items = ...`
```

### W0461: `fields_of` on known non-struct type

```
warning[W0461]: fields_of called on non-struct type
  --> src/main.ori:5:22
   |
 5 |     $for f in fields_of(int) yield ...
   |                         ^^^ `int` is a primitive type — fields_of returns []
   |
   = help: use is_struct(T) with $if to handle non-struct types
```

Note: This is a **warning**, not an error. `fields_of(int)` is valid and returns `[]`. The warning only fires when called on a known concrete non-struct type — not on generic `T`.

### E0462: Splice on non-struct value

```
error[E0462]: splice access requires a struct value
  --> src/main.ori:8:20
   |
 8 |     let val = items.[field]
   |               ^^^^^^^^^^^^  `items` has type [int], not a struct
   |
   = help: .[field] accesses a struct field by compile-time reference
```

### E0463: `$if` with non-const condition

```
error[E0463]: $if requires a compile-time condition
  --> src/main.ori:3:9
   |
 3 |     $if x > 0 then "positive" else "negative"
   |         ^^^^^ `x` is not known at compile time
   |
   = help: $if branches at compile time; use `if` for runtime conditions
```

### E0464: Heterogeneous yield type mismatch

```
error[E0464]: $for yield produces incompatible types across iterations
  --> src/main.ori:5:5
   |
 5 |     $for field in fields_of(T) yield value.[field]
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: iteration 0 (field `name`) produces type `str`
   = note: iteration 1 (field `age`) produces type `int`
   = help: convert to a common type, e.g.: value.[field].to_str()
```

---

## Part X: Summary

| Component | Syntax | Purpose |
|-----------|--------|---------|
| `fields_of(T)` | Intrinsic | Get struct field metadata at compile time |
| `variants_of(T)` | Intrinsic | Get sum type variant metadata at compile time |
| `name_of(T)` | Intrinsic | Get type name at compile time |
| `is_struct(T)` etc. | Intrinsic | Type classification at compile time |
| `$FieldMeta` | Type | Compile-time field metadata (name, index) |
| `$VariantMeta` | Type | Compile-time variant metadata (name, index, fields) |
| `$for` | Expansion | Compile-time loop unrolling with heterogeneous types |
| `$if` | Branching | Compile-time conditional with dead-branch elimination |
| `value.[field]` | Splice | Access struct field by compile-time field reference |

**Key properties:**
- Zero runtime overhead (expansion produces code identical to hand-written)
- No opt-in required (works on any type)
- No attributes or macros (compiler provides the data, user writes the logic)
- Expression-based (everything returns a value)
- Composes with generics, traits, and const evaluation
- `$` prefix convention (visually distinct, consistent with `$name` and `$function`)

**Supersedes:** `reflection-api-proposal.md` (runtime reflection, `#derive(Reflect)`, `Unknown` type)

**Repositions:** Runtime reflection as optional future work for dynamic dispatch use cases (plugin systems, scripting, hot-reload)

---

## Related Proposals

- **Supersedes**: `approved/reflection-api-proposal.md` (2026-01-31) — runtime reflection, `#derive(Reflect)`
- **Supersedes**: `approved/stdlib-json-api-proposal.md` (2026-01-30) — runtime `Json` trait design
- **Supersedes**: `approved/stdlib-json-api-ffi-revision.md` (2026-01-30) — yyjson FFI approach
- **Depends on**: `approved/const-evaluation-termination-proposal.md` (2026-01-30)
- **Interacts with**: `approved/capability-unification-generics-proposal.md` (2026-02-20)
- **Enables**: std.json native parser proposal (to be drafted — pure Ori, SIMD, no FFI)
- **Independent of**: `approved/const-generics-proposal.md` (2026-01-31)
