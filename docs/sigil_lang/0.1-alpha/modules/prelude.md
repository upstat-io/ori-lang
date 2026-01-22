# Prelude

The prelude is the set of items automatically imported into every Sigil program. No `use` statement is required.

---

## Types

### Option

```sigil
type Option<T> = Some(T) | None
```

Represents an optional value.

```sigil
let found: Option<User> = find_user(id)

match(found,
    Some(user) -> greet(user),
    None -> print("not found"),
)

// With default
let name = found.map(u -> u.name) ?? "anonymous"
```

**Methods:**
- `is_some() -> bool` — Returns true if Some
- `is_none() -> bool` — Returns true if None
- `unwrap() -> T` — Returns value or panics
- `unwrap_or(default: T) -> T` — Returns value or default
- `map<U>(f: T -> U) -> Option<U>` — Transforms inner value
- `and_then<U>(f: T -> Option<U>) -> Option<U>` — Chains options
- `ok_or<E>(err: E) -> Result<T, E>` — Converts to Result

---

### Result

```sigil
type Result<T, E> = Ok(T) | Err(E)
```

Represents success or failure.

```sigil
let result: Result<Config, Error> = load_config(path)

match(result,
    Ok(config) -> start(config),
    Err(e) -> log_error(e),
)

// With ? operator (in try context)
let config = load_config(path)?
```

**Methods:**
- `is_ok() -> bool` — Returns true if Ok
- `is_err() -> bool` — Returns true if Err
- `unwrap() -> T` — Returns value or panics
- `unwrap_err() -> E` — Returns error or panics
- `map<U>(f: T -> U) -> Result<U, E>` — Transforms success value
- `map_err<F>(f: E -> F) -> Result<T, F>` — Transforms error
- `and_then<U>(f: T -> Result<U, E>) -> Result<U, E>` — Chains results
- `ok() -> Option<T>` — Converts to Option (discards error)
- `err() -> Option<E>` — Gets error as Option

---

### Ordering

```sigil
type Ordering = Less | Equal | Greater
```

Result of comparing two values.

```sigil
let cmp = compare(a, b)

match(cmp,
    Less -> print("a < b"),
    Equal -> print("a == b"),
    Greater -> print("a > b"),
)
```

**Methods:**
- `is_less() -> bool`
- `is_equal() -> bool`
- `is_greater() -> bool`
- `reverse() -> Ordering` — Flips Less/Greater
- `then(other: Ordering) -> Ordering` — Chains comparisons

---

### Error

```sigil
type Error = {
    message: str,
    source: Option<Error>,
}
```

Standard error type for general error handling.

```sigil
let err = Error { message: "connection failed", source: None }

// With error chaining
let wrapped = Error {
    message: "failed to fetch user",
    source: Some(original_error),
}
```

---

## Collection Types

### List `[T]`

Ordered, homogeneous collection.

```sigil
let nums = [1, 2, 3, 4, 5]
let first = nums[0]
let length = len(nums)
```

### Map `{K: V}`

Key-value collection.

```sigil
let ages = {"alice": 30, "bob": 25}
let age = ages["alice"] ?? 0
```

### Set `Set<T>`

Unordered collection of unique elements.

```sigil
let ids = Set<int>.from([1, 2, 3])
ids.contains(2)  // true
```

### Range `Range<T>`

Range of values.

```sigil
for i in 0..10 do print(str(i))
for c in 'a'..='z' do print(str(c))
```

---

## Primitive Types

All primitive types are in the prelude:

| Type | Description |
|------|-------------|
| `int` | 64-bit signed integer |
| `float` | 64-bit floating point |
| `bool` | Boolean (`true`/`false`) |
| `str` | UTF-8 string |
| `char` | Unicode scalar value |
| `byte` | 8-bit unsigned integer |
| `void` | Unit type (no value) |
| `Never` | Bottom type (never returns) |
| `Duration` | Time span |
| `Size` | Byte size |

---

## Core Functions

### print

```sigil
@print (value: Printable) -> void
```

Prints a value to stdout with newline.

```sigil
print("Hello, world!")
print(42)
print(user)  // if User implements Printable
```

---

### len

```sigil
@len<T: Collection> (c: T) -> int
```

Returns the number of elements in a collection.

```sigil
len([1, 2, 3])      // 3
len("hello")        // 5
len({"a": 1})       // 1
```

---

### str

```sigil
@str<T: Printable> (value: T) -> str
```

Converts a value to its string representation.

```sigil
str(42)        // "42"
str(3.14)      // "3.14"
str(true)      // "true"
```

---

### compare

```sigil
@compare<T: Comparable> (a: T, b: T) -> Ordering
```

Compares two values.

```sigil
compare(1, 2)   // Less
compare(2, 2)   // Equal
compare(3, 2)   // Greater
```

---

### panic

```sigil
@panic (message: str) -> Never
```

Terminates the program with an error message.

```sigil
if critical_failure then panic("unrecoverable error")
```

---

## Core Traits

These traits are in the prelude:

| Trait | Description |
|-------|-------------|
| `Eq` | Equality comparison (`==`, `!=`) |
| `Comparable` | Ordering comparison (`<`, `>`, `<=`, `>=`) |
| `Hashable` | Can be used as map/set key |
| `Printable` | Can be converted to string |
| `Clone` | Can be explicitly copied |
| `Default` | Has a default value |

---

## Marker Capabilities

### Async

```sigil
trait Async {}
```

A marker capability that indicates a function may suspend execution. Unlike resource capabilities (like `Http` or `FileSystem`), `Async` has no methods — it's a compile-time signal that affects how the runtime handles the function.

```sigil
// With Async: non-blocking, may suspend
@fetch_user (id: str) -> Result<User, Error> uses Http, Async =
    Http.get("/users/" + id)?.parse()

// Without Async: blocking, runs to completion
@fetch_user_sync (id: str) -> Result<User, Error> uses Http =
    Http.get("/users/" + id)?.parse()
```

**Key properties:**
- Empty trait — no methods to implement
- Declares that a function may suspend (yield control to runtime)
- Propagates through call chains (if `f` calls `g` which `uses Async`, then `f` must also declare `uses Async` or provide the capability)
- Enables sync mocks in tests — mock implementations that don't declare `Async` run synchronously

**Sync vs Async:**

| With `uses Async` | Without `uses Async` |
|-------------------|----------------------|
| Non-blocking | Blocking |
| May suspend | Runs to completion |
| Requires async runtime | Runs synchronously |

See [Async via Capabilities](../design/10-async/01-async-await.md) for detailed explanation.

---

## See Also

- [Types Specification](../spec/06-types.md)
- [Built-in Functions](../spec/11-built-in-functions.md)
- [Traits](../design/04-traits/)
