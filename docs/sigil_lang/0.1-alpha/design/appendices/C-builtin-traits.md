# Appendix C: Built-in Traits

This appendix documents all traits provided by Sigil's standard library.

---

## Core Traits

### Eq — Equality

```sigil
trait Eq {
    @equals (self, other: Self) -> bool
}
```

**Purpose:** Value equality comparison.

**Operator:** `==` and `!=`

**Derivable:** Yes

```sigil
#[derive(Eq)]
type Point = { x: int, y: int }

p1 == p2  // Calls equals()
```

**Implementation Notes:**
- Must be reflexive: `a == a` is true
- Must be symmetric: `a == b` implies `b == a`
- Must be transitive: `a == b` and `b == c` implies `a == c`

---

### Comparable — Ordering

```sigil
trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}

type Ordering = Less | Equal | Greater
```

**Purpose:** Total ordering for sorting and comparison.

**Operators:** `<`, `>`, `<=`, `>=`

**Derivable:** Yes (lexicographic by field order)

```sigil
#[derive(Comparable)]
type Version = { major: int, minor: int, patch: int }

v1 < v2  // Compares major, then minor, then patch
```

**Implementation Notes:**
- Must be consistent with `Eq`: `compare(a, b) == Equal` iff `a == b`
- Must be total: every pair has a defined ordering

---

### Hashable — Hashing

```sigil
trait Hashable: Eq {
    @hash (self) -> int
}
```

**Purpose:** Hash values for use in hash maps and sets.

**Derivable:** Yes

```sigil
#[derive(Eq, Hashable)]
type UserId = { value: str }

// Can now be used as map key
users: {UserId: User} = {}
```

**Implementation Notes:**
- If `a == b`, then `hash(a) == hash(b)` (required)
- If `hash(a) != hash(b)`, then `a != b` (contrapositive)

---

### Clone — Copying

```sigil
trait Clone {
    @clone (self) -> Self
}
```

**Purpose:** Create independent copy of a value.

**Derivable:** Yes

```sigil
#[derive(Clone)]
type Config = { timeout: int, retries: int }

config2 = config1.clone()
```

**Implementation Notes:**
- Clone creates a deep copy
- Primitives (`int`, `bool`, `str`) are implicitly cloneable

---

### Drop — Destructor

```sigil
trait Drop {
    @drop (self) -> void
}
```

**Purpose:** Run cleanup code when a value's reference count reaches zero.

**Derivable:** No (must be implemented manually)

```sigil
type FileHandle = { fd: int }

impl Drop for FileHandle {
    @drop (self) -> void = close_fd(self.fd)
}
```

**Implementation Notes:**
- Called automatically when refcount reaches zero
- Used for resource cleanup (files, sockets, locks)
- Runs in reverse creation order when multiple values are dropped
- Should not panic

---

### Default — Default Values

```sigil
trait Default {
    @default () -> Self
}
```

**Purpose:** Provide a sensible default value for a type.

**Derivable:** Yes (if all fields implement `Default`)

```sigil
#[derive(Default)]
type Config = {
    timeout: int,    // defaults to 0
    debug: bool      // defaults to false
}

config = Config.default()
```

**Built-in Defaults:**
- `int`: `0`
- `float`: `0.0`
- `bool`: `false`
- `str`: `""`
- `[T]`: `[]`
- `{K: V}`: `{}`
- `Option<T>`: `None`

---

### Printable — String Conversion

```sigil
trait Printable {
    @to_string (self) -> str
}
```

**Purpose:** Convert value to human-readable string.

**Derivable:** Yes (debug-style output)

```sigil
#[derive(Printable)]
type Point = { x: int, y: int }

Point { x: 10, y: 20 }.to_string()  // "Point { x: 10, y: 20 }"
```

**Built-in Implementations:**
- `int`: `"42"`
- `float`: `"3.14"`
- `bool`: `"true"` or `"false"`
- `str`: itself
- `[T]`: `"[1, 2, 3]"`
- `{K: V}`: `"{a: 1, b: 2}"`

---

## Conversion Traits

### From — Type Conversion

```sigil
trait From<T> {
    @from (value: T) -> Self
}
```

**Purpose:** Convert from another type.

```sigil
impl From<int> for str {
    @from (value: int) -> str = int_to_string(value)
}

s = str.from(42)  // "42"
```

**Note:** `From<T>` for `Self` is automatically implemented (identity conversion).

---

### Into — Type Conversion (Reverse)

```sigil
trait Into<T> {
    @into (self) -> T
}
```

**Purpose:** Convert self into another type.

**Automatic:** If `From<A>` is implemented for `B`, then `Into<B>` is automatic for `A`.

```sigil
// Given: impl From<int> for str
n = 42
s = n.into()  // Inferred: into str
```

---

### TryFrom — Fallible Conversion

```sigil
trait TryFrom<T> {
    type Error
    @try_from (value: T) -> Result<Self, Self.Error>
}
```

**Purpose:** Conversion that might fail.

```sigil
impl TryFrom<str> for int {
    type Error = ParseError
    @try_from (s: str) -> Result<int, ParseError> = parse_int(s)
}

int.try_from("42")   // Ok(42)
int.try_from("abc")  // Err(ParseError)
```

---

### TryInto — Fallible Conversion (Reverse)

```sigil
trait TryInto<T> {
    type Error
    @try_into (self) -> Result<T, Self.Error>
}
```

**Automatic:** If `TryFrom<A>` is implemented for `B`, then `TryInto<B>` is automatic for `A`.

---

## Collection Traits

### Iterable — Iteration

```sigil
trait Iterable {
    type Item
    @iter (self) -> Iterator<Self.Item>
}
```

**Purpose:** Enable iteration over elements.

```sigil
impl Iterable for [T] {
    type Item = T
    @iter (self) -> Iterator<T> = ...
}

// Enables use with map, filter, fold
map(list, x -> x * 2)
```

---

### Iterator — Iterator Protocol

```sigil
trait Iterator {
    type Item
    @next (self) -> Option<Self.Item>
}
```

**Purpose:** Lazy iteration protocol.

```sigil
@sum<I> (iter: I) -> int where I: Iterator<Item = int> =
    fold(iter, 0, (acc, x) -> acc + x)
```

---

### Indexable — Index Access

```sigil
trait Indexable<I> {
    type Output
    @get (self, index: I) -> Option<Self.Output>
}
```

**Purpose:** Enable `[]` subscript syntax.

**Operator:** `x[i]`

```sigil
impl Indexable<int> for [T] {
    type Output = T
    @get (self, index: int) -> Option<T> = ...
}

list[0]  // Calls get(0)
```

---

### Sized — Known Size

```sigil
trait Sized {
    @size (self) -> int
}
```

**Purpose:** Get the number of elements.

```sigil
impl Sized for [T] {
    @size (self) -> int = list_length(self)
}

list.size()  // Returns element count
```

---

### Empty — Check If Empty

```sigil
trait Empty {
    @is_empty (self) -> bool
}
```

**Purpose:** Check if collection has no elements.

```sigil
impl Empty for [T] {
    @is_empty (self) -> bool = self.size() == 0
}
```

---

## Serialization Traits

### Serialize — To JSON

```sigil
trait Serialize {
    @to_json (self) -> str
}
```

**Purpose:** Convert to JSON string.

**Derivable:** Yes

```sigil
#[derive(Serialize)]
type User = { name: str, age: int }

user.to_json()  // {"name":"Alice","age":30}
```

---

### Deserialize — From JSON

```sigil
trait Deserialize {
    @from_json (json: str) -> Result<Self, JsonError>
}
```

**Purpose:** Parse from JSON string.

**Derivable:** Yes

```sigil
#[derive(Deserialize)]
type User = { name: str, age: int }

User.from_json("{\"name\":\"Alice\",\"age\":30}")
// Ok(User { name: "Alice", age: 30 })
```

---

## Async Traits

> **Note:** Sigil uses capability-based async via `uses Async` instead of `async` type modifiers. See [Capabilities](../14-capabilities/index.md).

### AsyncIterable — Async Iteration

```sigil
trait AsyncIterable {
    type Item
    @async_iter (self) -> AsyncIterator<Self.Item>
}
```

**Purpose:** Enable async iteration over a source that may suspend.

```sigil
@process (stream: dyn AsyncIterable<Item = Message>) -> void uses Async =
    for msg in stream do handle(msg)
```

---

### AsyncIterator — Async Iterator Protocol

```sigil
trait AsyncIterator {
    type Item
    @next (self) -> Option<Self.Item> uses Async
}
```

**Purpose:** Lazy async iteration protocol. The `next()` method may suspend (requires `Async` capability).

```sigil
@collect_all<I> (iter: I) -> [I.Item] uses Async where I: AsyncIterator = run(
    let mut results = [],
    loop(
        match(iter.next(),
            Some(item) -> results = results + [item],
            None -> break,
        ),
    ),
    results,
)
```

**Note:** The key difference from `Iterator` is that `next()` uses the `Async` capability, allowing the iterator to suspend between items.

---

## Operator Traits

### Add — Addition

```sigil
trait Add<Rhs = Self> {
    type Output
    @add (self, rhs: Rhs) -> Self.Output
}
```

**Operator:** `+`

```sigil
impl Add for Point {
    type Output = Point
    @add (self, rhs: Point) -> Point =
        Point { x: self.x + rhs.x, y: self.y + rhs.y }
}

p1 + p2  // Calls add()
```

---

### Sub — Subtraction

```sigil
trait Sub<Rhs = Self> {
    type Output
    @sub (self, rhs: Rhs) -> Self.Output
}
```

**Operator:** `-`

---

### Mul — Multiplication

```sigil
trait Mul<Rhs = Self> {
    type Output
    @mul (self, rhs: Rhs) -> Self.Output
}
```

**Operator:** `*`

---

### Div — Division

```sigil
trait Div<Rhs = Self> {
    type Output
    @div (self, rhs: Rhs) -> Self.Output
}
```

**Operator:** `/`

---

### Neg — Negation

```sigil
trait Neg {
    type Output
    @neg (self) -> Self.Output
}
```

**Operator:** `-x` (unary minus)

---

### Not — Logical Not

```sigil
trait Not {
    type Output
    @not (self) -> Self.Output
}
```

**Operator:** `!x`

---

## Trait Hierarchy

```
Eq
 └── Comparable
 └── Hashable

Clone

Default

Printable

From<T>
 └── Into<T> (blanket impl)

TryFrom<T>
 └── TryInto<T> (blanket impl)

Iterable
 └── Iterator

Indexable<I>

Sized
 └── Empty

Serialize

Deserialize

Future

Add, Sub, Mul, Div, Neg, Not
```

---

## Derivability Summary

| Trait | Derivable | Requirement |
|-------|-----------|-------------|
| Eq | Yes | All fields implement Eq |
| Comparable | Yes | All fields implement Comparable |
| Hashable | Yes | All fields implement Hashable |
| Clone | Yes | All fields implement Clone |
| Default | Yes | All fields implement Default |
| Printable | Yes | All fields implement Printable |
| Serialize | Yes | All fields implement Serialize |
| Deserialize | Yes | All fields implement Deserialize |

---

## Object Safety

Traits that can be used with `dyn`:

| Trait | Object-Safe | Reason |
|-------|-------------|--------|
| Eq | No | Takes `Self` as parameter |
| Comparable | No | Takes `Self` as parameter |
| Hashable | No | Requires `Eq` |
| Clone | No | Returns `Self` |
| Default | No | No `self` parameter |
| Printable | Yes | Only takes `self` |
| Iterator | Yes* | With associated type specified |
| Serialize | Yes | Only takes `self` |

*Use `dyn Iterator<Item = T>` with associated type specified.

---

## Prelude Traits

These traits are automatically imported:

- `Eq`
- `Comparable`
- `Clone`
- `Default`
- `Printable`
- `From`, `Into`
- `Iterable`, `Iterator`

---

## See Also

- [Trait Definitions](../04-traits/01-trait-definitions.md)
- [Derive](../04-traits/04-derive.md)
- [Dynamic Dispatch](../04-traits/05-dynamic-dispatch.md)
