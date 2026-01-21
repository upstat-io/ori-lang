# Derive

This document covers auto-deriving trait implementations.

---

## What Is Derive?

`#[derive(...)]` automatically generates trait implementations based on a type's structure:

```sigil
#[derive(Eq, Hashable, Printable, Clone)]
type User = {
    id: int,
    name: str,
    email: str
}
```

This generates implementations for `Eq`, `Hashable`, `Printable`, and `Clone` automatically.

---

## Syntax

```sigil
#[derive(Trait1, Trait2, ...)]
type TypeName = { ... }
```

The `#[derive(...)]` attribute goes immediately before the type definition.

---

## Derivable Traits

### Eq — Equality

```sigil
#[derive(Eq)]
type Point = { x: int, y: int }
```

**Generated behavior:** Field-by-field equality comparison.

```sigil
// Generated implementation
impl Eq for Point {
    @equals (self, other: Self) -> bool =
        self.x == other.x && self.y == other.y
}
```

**Requirement:** All fields must implement `Eq`.

### Hashable — Hashing

```sigil
#[derive(Hashable)]
type Point = { x: int, y: int }
```

**Generated behavior:** Combines hashes of all fields.

**Requirement:** All fields must implement `Hashable`.

### Comparable — Ordering

```sigil
#[derive(Comparable)]
type Version = { major: int, minor: int, patch: int }
```

**Generated behavior:** Lexicographic ordering by field order.

```sigil
// Compares major first, then minor, then patch
v1 = Version { major: 1, minor: 2, patch: 0 }
v2 = Version { major: 1, minor: 3, patch: 0 }
// v1 < v2 (because 2 < 3 in minor)
```

**Requirement:** All fields must implement `Comparable`.

### Printable — String Conversion

```sigil
#[derive(Printable)]
type Point = { x: int, y: int }
```

**Generated behavior:** Debug-style output.

```sigil
point = Point { x: 10, y: 20 }
point.to_string()  // "Point { x: 10, y: 20 }"
```

**Requirement:** All fields must implement `Printable`.

### Clone — Copying

```sigil
#[derive(Clone)]
type Point = { x: int, y: int }
```

**Generated behavior:** Field-by-field cloning.

**Requirement:** All fields must implement `Clone`.

### Default — Default Values

```sigil
#[derive(Default)]
type Config = {
    timeout: int,
    retries: int,
    debug: bool
}
```

**Generated behavior:** Default values for all fields.

```sigil
config = Config.default()
// Config { timeout: 0, retries: 0, debug: false }
```

**Requirement:** All fields must implement `Default`.

### Serialize — JSON Serialization

```sigil
#[derive(Serialize)]
type User = { name: str, age: int }
```

**Generated behavior:** Converts to JSON.

```sigil
user = User { name: "Alice", age: 30 }
user.to_json()  // {"name":"Alice","age":30}
```

### Deserialize — JSON Deserialization

```sigil
#[derive(Deserialize)]
type User = { name: str, age: int }
```

**Generated behavior:** Parses from JSON.

```sigil
User.from_json("{\"name\":\"Alice\",\"age\":30}")
// Ok(User { name: "Alice", age: 30 })
```

---

## Deriving for Sum Types

```sigil
#[derive(Eq, Printable)]
type Status = Pending | Running(progress: float) | Done | Failed(error: str)
```

**Generated Eq:** Variants must match, and any data must be equal.

**Generated Printable:**
```sigil
Running(progress: 0.5).to_string()  // "Running(progress: 0.5)"
Failed(error: "timeout").to_string()  // "Failed(error: \"timeout\")"
```

---

## Combining Derives

Derive multiple traits at once:

```sigil
#[derive(Eq, Hashable, Printable, Clone)]
type Point = { x: int, y: int }
```

### Common Combinations

```sigil
// For map keys
#[derive(Eq, Hashable)]
type Id = { value: str }

// For debugging
#[derive(Printable, Clone)]
type State = { ... }

// For data transfer
#[derive(Serialize, Deserialize, Eq)]
type Message = { ... }

// Full featured
#[derive(Eq, Hashable, Comparable, Printable, Clone, Default)]
type Record = { ... }
```

---

## When Derive Fails

### Missing Field Implementation

```sigil
type NonHashable = { data: SomeType }  // SomeType doesn't impl Hashable

#[derive(Hashable)]
type Container = { item: NonHashable }
// ERROR: cannot derive Hashable, field 'item' doesn't implement Hashable
```

### Private Fields

All fields must be accessible for derive to work.

---

## Manual Override

You can implement some traits manually and derive others:

```sigil
#[derive(Hashable, Clone)]
type User = { id: int, name: str }

// Manual Eq implementation (different from field-by-field)
impl Eq for User {
    @equals (self, other: Self) -> bool = self.id == other.id
}
```

---

## Derive vs Manual Implementation

### Use Derive When

- Standard field-by-field behavior is correct
- All fields have the required implementations
- You want less boilerplate

### Implement Manually When

- You need custom behavior (e.g., Eq by ID only)
- Some fields should be ignored
- You need specialized logic

```sigil
type User = { id: int, name: str, password_hash: str }

// Manual: don't include password_hash in equality
impl Eq for User {
    @equals (self, other: Self) -> bool = self.id == other.id
}

// Manual: don't include password_hash in string output
impl Printable for User {
    @to_string (self) -> str = "User { id: " + str(self.id) + ", name: " + self.name + " }"
}
```

---

## Best Practices

### Derive First, Customize Later

Start with derive. Only write manual implementations when needed.

### Keep Consistent

If you derive `Eq`, consider deriving `Hashable` too (they should be consistent).

### Document Non-Derived Behavior

```sigil
// #User equality is based on ID only, not all fields
impl Eq for User {
    @equals (self, other: Self) -> bool = self.id == other.id
}
```

---

## See Also

- [Trait Definitions](01-trait-definitions.md)
- [Implementations](02-implementations.md)
- [User-Defined Types](../03-type-system/03-user-defined-types.md)
