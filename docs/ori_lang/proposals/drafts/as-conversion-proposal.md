# Proposal: `as` Conversion Syntax

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-27

---

## Summary

Replace the special-cased `int()`, `float()`, `str()`, `byte()` type conversion functions with a unified `as` keyword syntax, backed by traits for extensibility.

```ori
// Infallible conversions
42 as float           // 42.0
42 as str             // "42"
'A' as byte           // 65

// Fallible conversions
"42" as? int          // Some(42)
"hello" as? int       // None
```

This removes the only exception to Ori's named-argument rule and provides cleaner, more readable conversion syntax.

---

## Motivation

### The Problem

Ori currently has four special-cased type conversion functions:

```ori
int(x)
float(x)
str(x)
byte(x)
```

These are problematic because:

1. **They violate Ori's named argument rule** — Every other function call requires named arguments, but these use positional. The spec literally says "positional allowed for type conversions" as a special exception.

2. **They look like constructors** — `int(x)` could be constructing an int or converting to one. The intent is ambiguous.

3. **They don't compose well** — `int(input.trim())` reads inside-out. Chain-friendly syntax reads left-to-right.

4. **They're not extensible** — User-defined types can't participate in this conversion pattern.

### The Ori Way

Ori values:
- **Consistency** — No special cases
- **Readability** — Code reads like intent
- **Explicit effects** — You know what can fail

The `as` keyword addresses all three:

```ori
// Reads naturally: "input trimmed as an integer"
input.trim() as? int

// Clear that this can fail (as? returns Option)
"42" as? int

// Obvious conversion intent
user_id as str
```

---

## Design

### Syntax

Two forms of conversion:

```ori
expression as Type      // Infallible conversion
expression as? Type     // Fallible conversion, returns Option<Type>
```

### Backing Traits

Conversions are backed by two traits in the prelude:

```ori
trait As<T> {
    @as (self) -> T
}

trait TryAs<T> {
    @try_as (self) -> Option<T>
}
```

- `x as T` desugars to `As<T>.as(self: x)`
- `x as? T` desugars to `TryAs<T>.try_as(self: x)`

### Infallible vs Fallible: Compile-Time Enforcement

The compiler enforces that `as` is only used for conversions that cannot fail:

```ori
// These compile — infallible conversions
42 as float         // int -> float always succeeds
42 as str           // int -> str always succeeds
true as str         // bool -> str always succeeds
'A' as byte         // char -> byte always succeeds (ASCII)
'A' as int          // char -> int always succeeds (codepoint)

// These are compile errors — must use as?
"42" as int         // ERROR: str -> int can fail, use `as?`
3.14 as int         // ERROR: float -> int is lossy, use explicit method
256 as byte         // ERROR: int -> byte can overflow, use `as?`
```

```ori
// Correct fallible conversions
"42" as? int        // Some(42)
"hello" as? int     // None
256 as? byte        // None (overflow)
```

### Lossy Conversions Require Explicit Methods

For conversions that lose information (like float to int), `as` and `as?` are both inappropriate. Use explicit methods that communicate intent:

```ori
// float -> int: multiple valid interpretations
3.99.truncate()     // 3 (toward zero)
3.99.round()        // 4 (nearest)
3.99.floor()        // 3 (toward negative infinity)
3.99.ceil()         // 4 (toward positive infinity)

// These are compile errors
3.99 as int         // ERROR: lossy conversion, use truncate/round/floor/ceil
3.99 as? int        // ERROR: not about failure, it's about intent
```

### Standard Library Implementations

#### Infallible Conversions (As trait)

```ori
// Widening numeric conversions
impl As<float> for int   { @as (self) -> float = /* intrinsic */ }
impl As<int> for byte    { @as (self) -> int = /* intrinsic */ }

// To string (always succeeds)
impl As<str> for int     { @as (self) -> str = /* intrinsic */ }
impl As<str> for float   { @as (self) -> str = /* intrinsic */ }
impl As<str> for bool    { @as (self) -> str = /* intrinsic */ }
impl As<str> for char    { @as (self) -> str = /* intrinsic */ }
impl As<str> for byte    { @as (self) -> str = /* intrinsic */ }

// Char conversions
impl As<int> for char    { @as (self) -> int = /* codepoint */ }
impl As<byte> for char   { @as (self) -> byte = /* ASCII, panics if > 127 */ }
```

#### Fallible Conversions (TryAs trait)

```ori
// Parsing
impl TryAs<int> for str   { @try_as (self) -> Option<int> = /* parse */ }
impl TryAs<float> for str { @try_as (self) -> Option<float> = /* parse */ }
impl TryAs<bool> for str  { @try_as (self) -> Option<bool> = /* "true"/"false" */ }

// Narrowing numeric conversions (can overflow)
impl TryAs<byte> for int  { @try_as (self) -> Option<byte> = /* range check */ }
impl TryAs<char> for int  { @try_as (self) -> Option<char> = /* valid codepoint? */ }
```

### User-Defined Conversions

Types can implement `As` and `TryAs` for custom conversions:

```ori
type UserId = { value: int }
type Username = { value: str }

// Infallible: UserId always converts to string
impl As<str> for UserId {
    @as (self) -> str = "user_" + (self.value as str)
}

// Fallible: String might not be valid username
impl TryAs<Username> for str {
    @try_as (self) -> Option<Username> = run(
        if self.is_empty() || self.len() > 32 then
            None
        else
            Some(Username { value: self }),
    )
}
```

Usage:

```ori
let id = UserId { value: 42 }
let display = id as str              // "user_42"

let name = "alice" as? Username      // Some(Username { value: "alice" })
let invalid = "" as? Username        // None
```

### Precedence

`as` and `as?` have the same precedence as other postfix operators (`.`, `[]`, `()`):

```ori
// These are equivalent
input.trim() as? int
(input.trim()) as? int

// as binds tighter than binary operators
a + b as str        // a + (b as str)
```

---

## Migration

### Deprecation Path

1. **Phase 1**: Add `as`/`as?` syntax, keep `int()` etc. as deprecated aliases
2. **Phase 2**: Emit warnings for old syntax
3. **Phase 3**: Remove old syntax

### Automated Migration

The `ori fmt` tool can automatically migrate:

```ori
// Before
let x = int(input)
let y = float(value)
let s = str(count)

// After (auto-migrated)
let x = input as? int    // or input as int if infallible
let y = value as float
let s = count as str
```

---

## Examples

### Parsing User Input

```ori
@parse_port (input: str) -> Result<int, str> = run(
    let port = input.trim() as? int,
    match(
        port,
        Some(p) -> if p > 0 && p <= 65535
            then Ok(p)
            else Err("port out of range"),
        None -> Err("invalid port number"),
    ),
)
```

### Display Formatting

```ori
@format_user (id: UserId, name: str, score: int) -> str =
    "User #" + (id.value as str) + " (" + name + "): " + (score as str) + " points"
```

### Chained Conversions

```ori
// Read config, parse as int, convert to Duration
let timeout = config.get(key: "timeout")
    .unwrap_or(default: "30")
    as? int
    .map(transform: seconds -> seconds as Duration)
    .unwrap_or(default: 30s)
```

### Generic Conversion Function

```ori
@convert_all<T, U> (items: [T]) -> [Option<U>]
    where T: TryAs<U>
= items.map(transform: item -> item as? U)
```

---

## Design Rationale

### Why `as` Instead of Methods?

| Approach | Example | Trade-off |
|----------|---------|-----------|
| Functions | `int(x)` | Violates named-arg rule |
| Methods | `x.to_int()` | Verbose, many method names |
| **Keyword** | `x as int` | Clean, universal, reads naturally |

### Why Separate `as` and `as?`?

Making fallibility explicit at the syntax level:
- `as` — "this conversion always works"
- `as?` — "this conversion might fail"

The compiler enforces correctness. You can't accidentally use `as` for a fallible conversion.

### Why Not `as` for Lossy Conversions?

Lossy conversions (like `float -> int`) aren't about success/failure — they're about *which* conversion you want. Multiple valid answers exist:

```ori
3.7 as int   // Is this 3? 4? It's ambiguous.
```

Explicit methods remove ambiguity:

```ori
3.7.truncate()   // Clearly 3
3.7.round()      // Clearly 4
```

### Why Traits?

Traits enable:
1. User-defined types to participate in `as` syntax
2. Generic code over convertible types
3. Clear documentation of what conversions exist

---

## Spec Changes Required

### `03-lexical-elements.md`

Add `as` to reserved keywords (if not already present).

### `05-expressions.md`

Add conversion expression:

```markdown
### Conversion Expressions

```ebnf
conversion_expr = expression "as" type
                | expression "as?" type
```

The `as` operator converts a value to another type using the `As<T>` trait.
The `as?` operator attempts conversion using the `TryAs<T>` trait, returning `Option<T>`.

```ori
42 as float       // 42.0
"42" as? int      // Some(42)
```
```

### `06-types.md`

Add `As` and `TryAs` traits:

```markdown
### Conversion Traits

```ori
trait As<T> {
    @as (self) -> T
}

trait TryAs<T> {
    @try_as (self) -> Option<T>
}
```

`As<T>` defines infallible conversion. `TryAs<T>` defines fallible conversion returning `Option<T>`.
```

### `12-modules.md`

Update prelude to include `As` and `TryAs` traits.

Remove `int()`, `float()`, `str()`, `byte()` from built-in functions (or mark deprecated).

### `/CLAUDE.md`

Update Quick Reference:
- Remove `int(x)`, `float(x)`, `str(x)`, `byte(x)` from function_val
- Add conversion syntax to Expressions section
- Add `As`, `TryAs` to prelude traits

---

## Summary

| Aspect | Decision |
|--------|----------|
| Syntax | `x as T` (infallible), `x as? T` (fallible) |
| Backing traits | `As<T>`, `TryAs<T>` in prelude |
| Compile-time safety | `as` only allowed for infallible conversions |
| Lossy conversions | Explicit methods (`truncate`, `round`, etc.) |
| Extensibility | User types implement traits |
| Migration | Deprecate then remove `int()` etc. |

This proposal removes a language inconsistency while adding a cleaner, more powerful conversion system that fits Ori's philosophy of explicit, safe, readable code.
