# Proposal: Debug Trait

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-27

---

## Summary

Add a `Debug` trait separate from `Printable` for developer-facing structural representation of values. `Debug` is automatically derivable and shows the complete internal structure, while `Printable` remains for intentional user-facing output.

```ori
trait Debug {
    @debug (self) -> str
}

#[derive(Debug)]
type Point = { x: int, y: int }

let p = Point { x: 1, y: 2 }
p.debug()  // "Point { x: 1, y: 2 }"
```

---

## Motivation

### The Problem

Ori currently has `Printable` for converting values to strings:

```ori
trait Printable {
    @to_str (self) -> str
}
```

But `Printable` serves two conflicting purposes:

1. **User-facing display** — What end users should see
2. **Developer debugging** — What developers need for troubleshooting

These are often different:

```ori
type User = { id: int, name: str, password_hash: str, email: str }

// For users: just the name
impl Printable for User {
    @to_str (self) -> str = self.name
}

// For debugging: need to see everything
// But there's no way to get the full structure!
```

### The Distinction

| Trait | Purpose | Example Output | Audience |
|-------|---------|----------------|----------|
| `Printable` | Display | `"Alice"` | End users |
| `Debug` | Inspect | `User { id: 42, name: "Alice", ... }` | Developers |

Rust makes this distinction with `Display` vs `Debug`. Ori should too.

---

## Design

### Trait Definition

```ori
trait Debug {
    @debug (self) -> str
}
```

Single method that returns a developer-readable string representation.

### Derivable

`Debug` can be derived for any type whose fields all implement `Debug`:

```ori
#[derive(Debug)]
type Point = { x: int, y: int }

#[derive(Debug)]
type Color = Red | Green | Blue

#[derive(Debug)]
type Tree<T: Debug> = Leaf(value: T) | Branch(left: Tree<T>, right: Tree<T>)
```

### Derived Output Format

The derived implementation produces consistent, readable output:

```ori
// Struct types
Point { x: 1, y: 2 }.debug()
// "Point { x: 1, y: 2 }"

// Sum types (unit variants)
Color.Red.debug()
// "Red"

// Sum types (with fields)
Tree.Leaf(value: 42).debug()
// "Leaf(value: 42)"

Tree.Branch(left: Leaf(value: 1), right: Leaf(value: 2)).debug()
// "Branch(left: Leaf(value: 1), right: Leaf(value: 2))"
```

### Standard Implementations

All primitive and built-in types implement `Debug`:

```ori
impl Debug for int   { @debug (self) -> str = self as str }
impl Debug for float { @debug (self) -> str = self as str }
impl Debug for bool  { @debug (self) -> str = if self then "true" else "false" }
impl Debug for str   { @debug (self) -> str = "\"" + self.escape() + "\"" }
impl Debug for char  { @debug (self) -> str = "'" + self.escape() + "'" }
impl Debug for byte  { @debug (self) -> str = (self as int) as str }
impl Debug for void  { @debug (self) -> str = "()" }

impl<T: Debug> Debug for [T] {
    @debug (self) -> str = "[" + self.iter()
        .map(transform: x -> x.debug())
        .join(sep: ", ") + "]"
}

impl<K: Debug, V: Debug> Debug for {K: V} {
    @debug (self) -> str = "{" + self.iter()
        .map(transform: (k, v) -> k.debug() + ": " + v.debug())
        .join(sep: ", ") + "}"
}

impl<T: Debug> Debug for Set<T> {
    @debug (self) -> str = "Set {" + self.iter()
        .map(transform: x -> x.debug())
        .join(sep: ", ") + "}"
}

impl<T: Debug> Debug for Option<T> {
    @debug (self) -> str = match(
        self,
        Some(v) -> "Some(" + v.debug() + ")",
        None -> "None",
    )
}

impl<T: Debug, E: Debug> Debug for Result<T, E> {
    @debug (self) -> str = match(
        self,
        Ok(v) -> "Ok(" + v.debug() + ")",
        Err(e) -> "Err(" + e.debug() + ")",
    )
}

impl<A: Debug, B: Debug> Debug for (A, B) {
    @debug (self) -> str = "(" + self.0.debug() + ", " + self.1.debug() + ")"
}
// ... extends to all tuple arities
```

### String Escaping

`Debug` for `str` and `char` shows escaped representations:

```ori
"hello".debug()      // "\"hello\""
"line\nbreak".debug() // "\"line\\nbreak\""
'\n'.debug()         // "'\\n'"
'\t'.debug()         // "'\\t'"
```

This distinguishes debug output from the raw value and makes whitespace visible.

### Manual Implementation

Types can implement `Debug` manually for custom formatting:

```ori
type SecretKey = { value: [byte] }

impl Debug for SecretKey {
    @debug (self) -> str = "SecretKey { value: [REDACTED] }"
}

type LargeBuffer = { data: [byte] }

impl Debug for LargeBuffer {
    @debug (self) -> str =
        "LargeBuffer { len: " + (self.data.len() as str) + " }"
}
```

### Relationship to Printable

The two traits are independent:

```ori
type User = { id: int, name: str, email: str }

#[derive(Debug)]  // Auto-generate debug representation
impl Printable for User {
    @to_str (self) -> str = self.name  // Custom user-facing output
}

let user = User { id: 1, name: "Alice", email: "alice@example.com" }
user.to_str()  // "Alice"
user.debug()   // "User { id: 1, name: \"Alice\", email: \"alice@example.com\" }"
```

A type may implement:
- Both `Debug` and `Printable` (common)
- Only `Debug` (internal types not shown to users)
- Only `Printable` (rare, but allowed)

### Default Printable from Debug

Types that derive `Debug` but don't implement `Printable` could optionally get a default:

```ori
// If no Printable impl exists, could fall back to Debug
// This is a design choice - may want to keep them strictly separate
```

**Recommendation:** Keep them strictly separate. If you want a type to be printable, be intentional about it.

---

## Examples

### Debugging Complex Structures

```ori
#[derive(Debug)]
type Config = {
    host: str,
    port: int,
    options: {str: str},
}

let config = Config {
    host: "localhost",
    port: 8080,
    options: { "timeout": "30", "retry": "3" },
}

config.debug()
// Config { host: "localhost", port: 8080, options: {"timeout": "30", "retry": "3"} }
```

### Debug in Error Messages

```ori
@process<T: Debug> (value: T) -> Result<Output, str> = run(
    if !is_valid(value: value) then
        Err("invalid value: " + value.debug())
    else
        Ok(compute(value: value)),
)
```

### Debug Constraints in Generics

```ori
@assert_eq<T: Eq + Debug> (actual: T, expected: T) -> void =
    if actual != expected then
        panic(msg: "assertion failed: " + actual.debug() + " != " + expected.debug())
    else
        ()
```

---

## Integration with `dbg` Function

The `Debug` trait enables the `dbg` function (separate proposal):

```ori
@dbg<T: Debug> (value: T) -> T = run(
    print(msg: "[" + location() + "] " + value.debug()),
    value,
)
```

Without `Debug`, `dbg` couldn't show the value's structure.

---

## Design Rationale

### Why Separate from Printable?

1. **Different audiences** — Users vs developers
2. **Different content** — Curated display vs complete structure
3. **Security** — Debug might show sensitive data, Printable shouldn't
4. **Automatic derivation** — Debug can always be derived; Printable requires intent

### Why Not a Format Parameter?

Alternative: single trait with format mode

```ori
trait Printable {
    @to_str (self, mode: Format) -> str
}
```

Problems:
- Every implementation must handle multiple modes
- Can't derive one and manually implement the other
- More complex trait definition

Separate traits are simpler and more flexible.

### Why `debug()` Method Name?

Alternatives considered:
- `repr()` — Python style, less clear
- `inspect()` — Ruby style, could work
- `debug_str()` — verbose
- `debug()` — clear, matches trait name

---

## Spec Changes Required

### `06-types.md`

Add Debug trait:

```markdown
### Debug Trait

```ori
trait Debug {
    @debug (self) -> str
}
```

Returns a developer-facing string representation of a value. Unlike `Printable`, which is for user-facing display, `Debug` shows the complete internal structure and is always derivable.

```ori
#[derive(Debug)]
type Point = { x: int, y: int }

Point { x: 1, y: 2 }.debug()  // "Point { x: 1, y: 2 }"
```
```

### `08-declarations.md`

Add `Debug` to derivable traits list.

### `12-modules.md`

Add `Debug` to prelude traits.

### `/CLAUDE.md`

Add `Debug` to prelude traits list.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Trait | `trait Debug { @debug (self) -> str }` |
| Purpose | Developer-facing structural representation |
| Derivable | Yes, if all fields implement Debug |
| Relationship to Printable | Independent, serves different purpose |
| String escaping | Debug shows escaped strings (`"\"hello\""`) |
| Standard implementations | All primitives, collections, Option, Result |

This proposal adds a fundamental trait for debugging and development, separate from user-facing display, enabling tools like `dbg` and better error messages.
