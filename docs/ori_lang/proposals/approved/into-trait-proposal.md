# Proposal: Into Trait

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, type system, conversions

---

## Summary

This proposal formalizes the `Into` trait for type conversions, including its relationship to explicit conversion, standard implementations, and use patterns.

---

## Problem Statement

The spec lists `Into` in the prelude but leaves unclear:

1. **Definition**: What is the exact trait signature?
2. **Usage**: When is `.into()` called vs explicit conversion?
3. **Standard implementations**: Which types implement Into?
4. **Custom implementations**: How to implement for user types?
5. **Relationship to `as`**: How does Into differ from `as` conversion?

---

## Definition

```ori
trait Into<T> {
    @into (self) -> T
}
```

`Into<T>` represents a conversion from `Self` to `T`.

---

## Usage

### Explicit Method Call

```ori
let error: Error = "something went wrong".into()
```

### In Function Calls

When a function accepts `impl Into<T>`, the caller must explicitly call `.into()`:

```ori
@fail (err: impl Into<Error>) -> Never = panic(msg: err.into().message)

fail(err: "simple message".into())  // Explicit .into() required
fail(err: Error { message: "detailed" })  // No conversion needed, already Error
```

**Note**: Unlike some languages, Ori does NOT perform implicit conversion at call sites. The `impl Into<T>` bound means the function accepts any type implementing `Into<T>`, but the caller is responsible for the explicit `.into()` call. This maintains Ori's "no implicit conversions" philosophy.

---

## Standard Implementations

### str to Error

```ori
impl Into<Error> for str {
    @into (self) -> Error = Error { message: self, source: None }
}
```

```ori
let e: Error = "failed to connect".into()
```

### Numeric Widening

```ori
impl Into<float> for int {
    @into (self) -> float = self as float
}
```

```ori
let f: float = 42.into()  // 42.0
```

**Note**: `Into` is for lossless conversions only. Lossy conversions (like `float` to `int` truncation) require explicit `as` syntax to acknowledge potential data loss.

### Collection Conversions

```ori
impl<T: Eq + Hashable> Into<[T]> for Set<T> {
    @into (self) -> [T] = self.iter().collect()
}
```

---

## Relationship to `as`

### `as` — Explicit Type Conversion

`as` is for conversions that:
- May change representation
- Are infallible
- Require explicit acknowledgment

```ori
42 as float     // Infallible conversion
3.14 as int     // Truncation
```

### `as?` — Fallible Conversion

`as?` returns `Option` for conversions that may fail:

```ori
"42" as? int    // Some(42)
"abc" as? int   // None
```

### `Into` — Trait-Based Conversion

`Into` is for:
- Semantic conversions (not just representation)
- Lossless conversions
- Extensible by users

```ori
"error message".into()  // -> Error
```

### Comparison

| Mechanism | Fallible | Implicit | Extensible | Use Case |
|-----------|----------|----------|------------|----------|
| `as` | No | No | No | Primitive representation changes |
| `as?` | Yes | No | No | Parsing, checked conversions |
| `Into` | No | No | Yes | Semantic type conversions |

---

## impl Into<T> Parameters

Functions can accept `impl Into<T>` for flexible APIs:

```ori
@set_name (name: impl Into<str>) -> void = ...

set_name(name: "literal")           // str, no conversion needed
set_name(name: char_buffer.into())  // Assuming CharBuffer: Into<str>
```

### Explicit Conversion Required

The caller must call `.into()` explicitly when the argument type doesn't match:

```ori
@process (value: impl Into<float>) -> float = value.into() * 2.0

process(value: 10.0)         // float, no conversion needed
process(value: 10.into())    // int, explicit .into() required
```

---

## Custom Implementations

### For User Types

```ori
type UserId = int

impl Into<str> for UserId {
    @into (self) -> str = `user-{self.inner}`
}

let id = UserId(42)
let s: str = id.into()  // "user-42"
```

### Bidirectional Conversions

```ori
type Celsius = float
type Fahrenheit = float

impl Into<Fahrenheit> for Celsius {
    @into (self) -> Fahrenheit = Fahrenheit(self.inner * 9.0 / 5.0 + 32.0)
}

impl Into<Celsius> for Fahrenheit {
    @into (self) -> Celsius = Celsius((self.inner - 32.0) * 5.0 / 9.0)
}
```

---

## No Blanket Identity

There is NO blanket `impl<T> Into<T> for T`. Each type must implement conversions explicitly.

### Rationale

Blanket identity would make `impl Into<T>` equivalent to `T`, defeating the purpose. By requiring explicit implementations, `Into` documents meaningful conversions.

---

## Conversion Chains

Into does NOT chain automatically:

```ori
// Given:
impl Into<B> for A { ... }
impl Into<C> for B { ... }

let a: A = ...
let c: C = a.into()  // ERROR: A does not implement Into<C>
let c: C = a.into().into()  // OK: A -> B -> C
```

### Rationale

Automatic chaining could create surprising implicit conversions.

---

## Orphan Rules

Into implementations follow standard orphan rules:
- Implement in the module defining the source type, OR
- Implement in the module defining the target type

```ori
// In my_module
type MyType = { ... }

// OK: implementing Into for our type (defined in this module)
impl Into<str> for MyType { ... }

// OK: implementing Into our type (target type in this module)
impl Into<MyType> for str { ... }

// ERROR: cannot implement foreign Into for foreign types
impl Into<str> for int { ... }
```

---

## Error Messages

### No Into Implementation

```
error[E0960]: `MyType` does not implement `Into<str>`
  --> src/main.ori:5:20
   |
 5 | let s: str = value.into()
   |                    ^^^^ trait not implemented
   |
   = note: `MyType` cannot be converted to `str`
   = help: implement `Into<str>` for `MyType`
```

### Ambiguous Into

```
error[E0961]: multiple `Into` implementations apply
  --> src/main.ori:5:20
   |
 5 | let x = value.into()
   |               ^^^^ ambiguous conversion
   |
   = note: `MyType` implements both `Into<A>` and `Into<B>`
   = help: specify target type: `let x: A = value.into()`
```

---

## Spec Changes Required

### Update `07-properties-of-types.md`

Add Into trait section with:
1. Trait definition
2. Standard implementations
3. Relationship to `as`/`as?`
4. Custom implementation guidelines

---

## Summary

| Aspect | Details |
|--------|---------|
| Trait | `trait Into<T> { @into (self) -> T }` |
| Purpose | Semantic, lossless type conversion |
| Usage | Explicit `.into()` method call |
| vs `as` | `as` is built-in, `Into` is extensible trait |
| Standard | str→Error, int→float, Set\<T\>→[T] |
| Chaining | Not automatic |
| Identity | Not blanket-implemented |
| Implicit | Never — caller must call `.into()` explicitly |
