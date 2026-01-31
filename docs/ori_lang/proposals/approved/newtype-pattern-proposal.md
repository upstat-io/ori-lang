# Proposal: Newtype Pattern

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-31
**Affects:** Compiler, type system

---

## Summary

This proposal formalizes newtype semantics, including type distinctness, conversions, trait inheritance, and use cases.

---

## Problem Statement

The spec shows `type UserId = int` syntax but leaves unclear:

1. **Distinctness**: How distinct is a newtype from its underlying type?
2. **Conversions**: How do you convert between newtype and underlying type?
3. **Trait inheritance**: Does the newtype inherit traits from the underlying type?
4. **Method access**: Can you call methods of the underlying type?
5. **Performance**: Is there runtime overhead?

---

## Syntax

```ori
type NewType = ExistingType
```

Creates a new nominal type wrapping the existing type.

```ori
type UserId = int
type Email = str
type Meters = float
type UserList = [User]
```

---

## Type Distinctness

### Nominal Typing

Newtypes are nominally distinct from their underlying type:

```ori
type UserId = int
type PostId = int

let user_id: UserId = UserId(42)
let post_id: PostId = PostId(42)

user_id == post_id  // ERROR: cannot compare UserId and PostId
```

### No Implicit Conversion

Newtypes do not implicitly convert:

```ori
type Meters = float
type Feet = float

@distance (m: Meters) -> str = `{m} meters`

let feet: Feet = Feet(10.0)
distance(m: feet)  // ERROR: expected Meters, found Feet
```

---

## Construction

### Constructor Syntax

Newtypes use their type name as a constructor:

```ori
type UserId = int

let id = UserId(42)  // Construct from underlying value
```

### From Literal

Literals cannot directly become newtypes:

```ori
type UserId = int

let id: UserId = 42  // ERROR: expected UserId, found int
let id: UserId = UserId(42)  // OK
```

---

## Conversions

### To Underlying Type

Access the underlying value via `.inner`:

```ori
type UserId = int

let id = UserId(42)
let raw: int = id.inner  // Access underlying value
```

The `.inner` accessor is always public, regardless of the newtype's visibility. The type-safety boundary is at construction, not access.

### From Underlying Type

Use the constructor:

```ori
type UserId = int

let raw = 42
let id = UserId(raw)
```

### Between Newtypes

No direct conversion between newtypes of the same underlying type:

```ori
type UserId = int
type PostId = int

let user_id = UserId(42)
let post_id = PostId(user_id)  // ERROR: expected int, found UserId
let post_id = PostId(user_id.inner)  // OK: via underlying value
```

---

## Trait Behavior

### No Automatic Inheritance

Newtypes do NOT automatically inherit traits:

```ori
type UserId = int

let a = UserId(1)
let b = UserId(2)
a == b  // ERROR: UserId does not implement Eq
a + b   // ERROR: UserId does not implement Add
```

### Explicit Derivation

Derive traits explicitly:

```ori
#derive(Eq, Hashable, Comparable, Clone, Debug)
type UserId = int

let a = UserId(1)
let b = UserId(2)
a == b  // OK: false
```

### Custom Implementation

Implement traits with custom behavior:

```ori
type Email = str

impl Printable for Email {
    @to_str (self) -> str = `<{self.inner}>`  // Custom format
}
```

---

## Method Access

### No Automatic Method Access

Newtype does not expose underlying type's methods:

```ori
type Email = str

let email = Email("user@example.com")
email.len()  // ERROR: Email has no method len
email.inner.len()  // OK: access underlying str's len
```

### Define Own Methods

Add methods via `impl`:

```ori
type Email = str

impl Email {
    @domain (self) -> str = run(
        let parts = self.inner.split(sep: "@"),
        parts[1],
    )

    @local_part (self) -> str = run(
        let parts = self.inner.split(sep: "@"),
        parts[0],
    )
}

let email = Email("user@example.com")
email.domain()  // "example.com"
```

### Delegation Pattern

Explicitly delegate methods:

```ori
type SafeString = str

impl SafeString {
    @len (self) -> int = self.inner.len()
    @is_empty (self) -> bool = self.inner.is_empty()
    // Only expose safe operations
}
```

---

## Performance

### Zero-Cost Abstraction

Newtypes have no runtime overhead:
- Same memory layout as underlying type
- No indirection
- Compiler erases newtype wrapper

```ori
type UserId = int
// UserId has same size and alignment as int
```

### Optimization

The compiler can optimize through newtype boundaries:

```ori
type Index = int

@sum_indices (indices: [Index]) -> Index =
    indices.fold(initial: Index(0), combine: (a, b) -> Index(a.inner + b.inner))

// Compiles to same code as summing [int]
```

---

## Generic Newtypes

### With Type Parameters

```ori
type NonEmpty<T> = [T]  // Semantically non-empty list

impl<T> NonEmpty<T> {
    @first (self) -> T = self.inner[0]  // Safe: guaranteed non-empty
}
```

### Constraints on Construction

```ori
type NonEmpty<T> = [T]

@non_empty<T> (items: [T]) -> Option<NonEmpty<T>> =
    if is_empty(collection: items) then None
    else Some(NonEmpty(items))
```

---

## Common Patterns

### ID Types

```ori
#derive(Eq, Hashable, Clone, Debug)
type UserId = int

#derive(Eq, Hashable, Clone, Debug)
type PostId = int

#derive(Eq, Hashable, Clone, Debug)
type CommentId = int

// Cannot accidentally pass wrong ID type
@get_user (id: UserId) -> User = ...
@get_post (id: PostId) -> Post = ...
```

### Units of Measure

```ori
#derive(Eq, Comparable, Clone, Debug)
type Meters = float

#derive(Eq, Comparable, Clone, Debug)
type Feet = float

@meters_to_feet (m: Meters) -> Feet = Feet(m.inner * 3.28084)

impl Meters {
    @add (self, other: Meters) -> Meters = Meters(self.inner + other.inner)
}
```

### Validated Types

```ori
type Email = str

@parse_email (s: str) -> Result<Email, str> =
    if s.contains(substr: "@") then Ok(Email(s))
    else Err("invalid email")

// Email can only be constructed via parse_email
// Guarantees all Email values are valid
```

### Semantic Wrappers

```ori
type HtmlSafe = str  // Escaped HTML content
type RawHtml = str   // Unescaped HTML content

@escape (raw: RawHtml) -> HtmlSafe = HtmlSafe(html_escape(raw.inner))

@render (safe: HtmlSafe) -> void = ...  // Only accepts escaped content
```

---

## Error Messages

### Type Mismatch

```
error[E0900]: mismatched types
  --> src/main.ori:10:15
   |
10 |     get_user(id: post_id)
   |                  ^^^^^^^ expected `UserId`, found `PostId`
   |
   = note: `UserId` and `PostId` are distinct types
   = help: convert explicitly: `UserId(post_id.inner)`
```

### Missing Trait

```
error[E0901]: `UserId` does not implement `Eq`
  --> src/main.ori:5:1
   |
 5 | user_a == user_b
   | ^^^^^^^^^^^^^^^^ no implementation of `Eq` for `UserId`
   |
   = note: newtypes do not inherit traits from underlying type
   = help: add `#derive(Eq)` to `UserId` definition
```

### Method Not Found

```
error[E0902]: method `len` not found on `Email`
  --> src/main.ori:5:7
   |
 5 | email.len()
   |       ^^^ method not found
   |
   = note: `Email` is a newtype over `str`
   = help: access underlying value: `email.inner.len()`
   = help: or define the method: `impl Email { @len (self) -> int = self.inner.len() }`
```

---

## Spec Changes Required

### Update `06-types.md`

Expand Newtype section with:
1. Construction syntax
2. Conversion rules (`.inner` accessor)
3. Trait non-inheritance
4. Method access rules
5. Performance guarantees

### Fix Spec Inconsistency

Update `07-properties-of-types.md` Into trait example to use `.inner` instead of `.0`.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Syntax | `type NewType = ExistingType` |
| Distinctness | Nominally distinct |
| Construction | `NewType(value)` |
| Access underlying | `.inner` field (always public) |
| Trait inheritance | None (must derive explicitly) |
| Method inheritance | None (must delegate explicitly) |
| Runtime overhead | Zero |
| Generic support | Yes: `type Wrapper<T> = [T]` |
