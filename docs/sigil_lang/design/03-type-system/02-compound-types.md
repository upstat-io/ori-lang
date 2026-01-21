# Compound Types

This document covers Sigil's built-in compound types: List, Map, Tuple, Option, and Result.

---

## `[T]` — List

Ordered collection of elements of type T.

### Literals

```sigil
numbers = [1, 2, 3, 4, 5]
names = ["Alice", "Bob", "Carol"]
empty: [int] = []
nested = [[1, 2], [3, 4], [5, 6]]
```

### Operations

```sigil
// Indexing
first = numbers[0]
last = numbers[# - 1]
middle = numbers[# / 2]

// Length
len(numbers)  // 5

// Concatenation
combined = [1, 2] + [3, 4]  // [1, 2, 3, 4]

// Methods
numbers.contains(3)   // true
numbers.is_empty()    // false
numbers.first()       // Option<int> -> Some(1)
numbers.last()        // Option<int> -> Some(5)
numbers.reverse()     // [5, 4, 3, 2, 1]
numbers.sort()        // sorted copy
```

### With Patterns

```sigil
@sum (arr: [int]) -> int = fold(arr, 0, +)
@doubled (arr: [int]) -> [int] = map(arr, x -> x * 2)
@evens (arr: [int]) -> [int] = filter(arr, x -> x % 2 == 0)
```

### Immutability

Lists are immutable. Operations return new lists:

```sigil
original = [1, 2, 3]
updated = original + [4]  // original unchanged
// original = [1, 2, 3]
// updated = [1, 2, 3, 4]
```

---

## `{K: V}` — Map

Key-value collection.

### Literals

```sigil
ages = {"Alice": 30, "Bob": 25, "Carol": 35}
empty: {str: int} = {}
```

### Operations

```sigil
// Access — returns Option<V>
ages["Alice"]    // Option<int> -> Some(30)
ages["Missing"]  // Option<int> -> None

// Access with default
ages["Alice"] ?? 0    // int -> 30
ages["Missing"] ?? 0  // int -> 0

// Check existence
ages.has("Alice")  // true
ages.has("Dave")   // false

// Keys and values
ages.keys()    // [str]
ages.values()  // [int]
ages.entries() // [(str, int)]

// Size
len(ages)  // 3
ages.is_empty()  // false
```

**Note:** Map indexing always returns `Option<V>` — no hidden panics on missing keys. Use `??` to provide a default value.

### Key Requirements

Keys must implement `Eq` and `Hashable`:

```sigil
// OK: str, int are hashable
{str: int}
{int: str}

// OK: custom types with #[derive(Eq, Hashable)]
#[derive(Eq, Hashable)]
type UserId = str

{UserId: User}
```

---

## `(T, U, ...)` — Tuple

Fixed-size, heterogeneous collection.

### Literals

```sigil
pair = (1, "hello")
triple = (true, 3.14, "world")
unit = ()              // unit tuple (empty tuple)
```

### Unit Tuple `()`

The empty tuple `()` is the unit type, used when no meaningful value is needed:

```sigil
// Function that returns nothing meaningful
@log (msg: str) -> () = print(msg)

// As accumulator placeholder in fold
items.fold((), (_, x) -> process(x))
```

### Access

```sigil
// By position
first = pair.0   // 1
second = pair.1  // "hello"

// Destructuring
(x, y) = pair
(a, b, c) = triple
```

### Common Uses

```sigil
// Return multiple values
@divide_with_remainder (a: int, b: int) -> (int, int) =
    (a / b, a % b)

// Usage
(quotient, remainder) = divide_with_remainder(17, 5)
```

### Type Annotation

```sigil
point: (int, int) = (10, 20)
record: (str, int, bool) = ("Alice", 30, true)
```

---

## `Option<T>` — Optional Value

Represents a value that may or may not exist.

### Variants

```sigil
type Option<T> = Some(T) | None
```

### Creating

```sigil
present = Some(42)
absent: Option<int> = None
```

### Checking

```sigil
is_some(present)  // true
is_none(present)  // false
is_some(absent)   // false
is_none(absent)   // true
```

### Extracting Values

```sigil
// Pattern matching (preferred)
@describe (opt: Option<int>) -> str = match(opt,
    Some(n) -> "value: " + str(n),
    None -> "no value"
)

// Unwrap (panics if None)
value = present.unwrap()  // 42

// With default
value = present ?? 0  // 42
value = absent ?? 0   // 0
```

### Common Operations

```sigil
// Map over Some
opt.map(x -> x * 2)  // Some(84) or None

// Chain options
opt.and_then(x -> if x > 0 then Some(x) else None)

// Convert to Result
opt.ok_or("no value")  // Result<int, str>
```

### Built-In

`Option<T>` is built-in—no import required.

---

## `Result<T, E>` — Success or Error

Represents an operation that can succeed or fail.

### Variants

```sigil
type Result<T, E> = Ok(T) | Err(E)
```

### Creating

```sigil
success = Ok(42)
failure: Result<int, str> = Err("something went wrong")
```

### Checking

```sigil
is_ok(success)   // true
is_err(success)  // false
is_ok(failure)   // false
is_err(failure)  // true
```

### Extracting Values

```sigil
// Pattern matching (preferred)
@describe (r: Result<int, str>) -> str = match(r,
    Ok(n) -> "success: " + str(n),
    Err(e) -> "error: " + e
)

// Unwrap (panics if Err)
value = success.unwrap()  // 42

// Unwrap error (panics if Ok)
error = failure.unwrap_err()  // "something went wrong"

// With default
value = success ?? 0  // 42 (returns default on Err)
```

### Error Propagation

Use the `try` pattern:

```sigil
@process (path: str) -> Result<Data, Error> = try(
    content = read_file(path),   // propagates if Err
    parsed = parse(content),     // propagates if Err
    Ok(transform(parsed))
)
```

### Common Operations

```sigil
// Map over Ok
result.map(x -> x * 2)  // Ok(84) or original Err

// Map over Err
result.map_err(e -> "Error: " + e)

// Chain results
result.and_then(x -> divide(x, 2))

// Convert to Option
result.ok()   // Option<T>
result.err()  // Option<E>
```

### Built-In

`Result<T, E>` is built-in—no import required.

---

## Type Nesting

Compound types can be nested:

```sigil
// List of options
users: [Option<User>]

// Map with list values
groups: {str: [User]}

// Result containing option
result: Result<Option<int>, Error>

// Option of result
opt: Option<Result<int, str>>
```

---

## See Also

- [Primitive Types](01-primitive-types.md)
- [User-Defined Types](03-user-defined-types.md)
- [Error Handling](../05-error-handling/index.md)
