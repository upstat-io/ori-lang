# Compound Types

This document covers Sigil's built-in compound types: List, Map, Set, Tuple, Range, Option, Result, and Ordering.

---

## `[T]` — List

Ordered collection of elements of type T.

### Literals

```sigil
let numbers = [1, 2, 3, 4, 5]
let names = ["Alice", "Bob", "Carol"]
let empty: [int] = []
let nested = [[1, 2], [3, 4], [5, 6]]
```

### Operations

```sigil
// Indexing
let first = numbers[0]
let last = numbers[# - 1]
let middle = numbers[# / 2]

// Length
len(numbers)  // 5

// Concatenation
let combined = [1, 2] + [3, 4]  // [1, 2, 3, 4]

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
let original = [1, 2, 3]
let updated = original + [4]  // original unchanged
// original = [1, 2, 3]
// updated = [1, 2, 3, 4]
```

---

## `{K: V}` — Map

Key-value collection.

### Literals

```sigil
let ages = {"Alice": 30, "Bob": 25, "Carol": 35}
let empty: {str: int} = {}
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

## `Set<T>` — Set

Unordered collection of unique elements.

### Creating

```sigil
let numbers = Set<int>.new()
let from_list = Set<int>.from([1, 2, 3, 2, 1])  // {1, 2, 3}
```

### Operations

```sigil
// Adding elements
let s = Set<int>.new()
s.insert(1)
s.insert(2)
s.insert(1)    // no effect, already present

// Checking membership
s.contains(1)  // true
s.contains(5)  // false

// Size
s.len()        // 2
s.is_empty()   // false

// Removal
s.remove(1)    // removes element
s.clear()      // removes all elements
```

### Set Operations

```sigil
let a = Set<int>.from([1, 2, 3])
let b = Set<int>.from([2, 3, 4])

a.union(b)        // {1, 2, 3, 4}
a.intersection(b) // {2, 3}
a.difference(b)   // {1}
a.symmetric_diff(b) // {1, 4}

a.is_subset(b)    // false
a.is_superset(b)  // false
a.is_disjoint(b)  // false (they share elements)
```

### Element Requirements

Elements must implement `Eq` and `Hashable`:

```sigil
// OK: primitive types
Set<int>
Set<str>

// OK: custom types with derive
#[derive(Eq, Hashable)]
type UserId = str
Set<UserId>

// ERROR: unhashable type
Set<[int]>  // lists are not hashable
```

### Iteration

```sigil
let ids = Set<int>.from([1, 2, 3])

// Iterate over elements (order not guaranteed)
for id in ids do print(str(id))

// Convert to list
let list = ids.to_list()  // [int], order not guaranteed
```

### Common Uses

```sigil
// Deduplication
@unique<T: Eq + Hashable> (items: [T]) -> [T] =
    Set<T>.from(items).to_list()

// Fast membership testing
@has_permission (user: User, required: Set<Permission>) -> bool =
    required.is_subset(user.permissions)

// Tracking seen items
@process_unique (items: [Item]) -> [Result] = run(
    let seen = Set<ItemId>.new(),
    filter(items, item ->
        if seen.contains(item.id) then false
        else run(seen.insert(item.id), true)
    ) | map(_, process)
)
```

---

## `(T, U, ...)` — Tuple

Fixed-size, heterogeneous collection.

### Literals

```sigil
let pair = (1, "hello")
let triple = (true, 3.14, "world")
let unit = ()              // unit tuple (empty tuple)
```

### Unit Tuple `()`

The empty tuple `()` is the unit type, equivalent to `void`. Use `()` when you need to pass or store unit as a value:

```sigil
// As accumulator placeholder in fold
items.fold((), (_, x) -> process(x))

// As a type parameter
type Callback = () -> ()

// In data structures
type Event = { timestamp: int, data: Option<()> }
```

**Note:** For function return types, prefer `void` for clarity. See [void](01-primitive-types.md#void--no-meaningful-value).

### Access

```sigil
// By position
let first = pair.0   // 1
let second = pair.1  // "hello"

// Destructuring
let (x, y) = pair
let (a, b, c) = triple
```

### Common Uses

```sigil
// Return multiple values
@divide_with_remainder (a: int, b: int) -> (int, int) =
    (a / b, a % b)

// Usage
let (quotient, remainder) = divide_with_remainder(17, 5)
```

### Type Annotation

```sigil
let point: (int, int) = (10, 20)
let record: (str, int, bool) = ("Alice", 30, true)
```

---

## `Range<T>` — Range

A range of values from a start to an end bound.

### Creating Ranges

Ranges are created using the range operators:

```sigil
// Exclusive end (..)
0..10       // 0, 1, 2, ..., 9
'a'..'z'    // 'a', 'b', ..., 'y'

// Inclusive end (..=)
0..=10      // 0, 1, 2, ..., 10
'a'..='z'   // 'a', 'b', ..., 'z'
```

### Type

The type of a range is `Range<T>` where `T` is the type of the bounds:

```sigil
let r: Range<int> = 0..10
let c: Range<char> = 'a'..'z'
```

### Requirements

Range bounds must implement the `Comparable` trait:

```sigil
// OK: int and char are comparable
0..10
'a'..'z'

// ERROR: str is not comparable in this way
"a".."z"
```

### Iteration

Ranges are iterable:

```sigil
// For loop
for i in 0..10 do print(str(i))

// With collect pattern
let squares = collect(.range: 1..=10, .transform: x -> x * x)
// [1, 4, 9, 16, 25, 36, 49, 64, 81, 100]

// Converting to list
let nums = (0..5).to_list()  // [0, 1, 2, 3, 4]
```

### Range Operations

```sigil
let r = 0..10

r.contains(5)   // true
r.contains(10)  // false (exclusive end)
r.contains(-1)  // false

r.is_empty()    // false
(5..5).is_empty() // true (empty range)
```

### Common Uses

```sigil
// Indexing with range (slicing)
let items = [1, 2, 3, 4, 5]
items[1..4]     // [2, 3, 4]
items[..3]      // [1, 2, 3] (from start)
items[2..]      // [3, 4, 5] (to end)

// Generating sequences
let indices = collect(.range: 0..len(items), .transform: i -> i)

// Bounded iteration
for i in 0..$max_retries do attempt(i)
```

### Step Ranges

For ranges with a step other than 1, use `step_by`:

```sigil
(0..10).step_by(2)  // 0, 2, 4, 6, 8

// Counting down
(10..=0).step_by(-1)  // 10, 9, 8, ..., 0
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
let present = Some(42)
let absent: Option<int> = None
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
let value = present.unwrap()  // 42

// With default
let value = present ?? 0  // 42
let value = absent ?? 0   // 0
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
let success = Ok(42)
let failure: Result<int, str> = Err("something went wrong")
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
let value = success.unwrap()  // 42

// Unwrap error (panics if Ok)
let error = failure.unwrap_err()  // "something went wrong"

// With default
let value = success ?? 0  // 42 (returns default on Err)
```

### Error Propagation

Use the `try` pattern:

```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(path)?,   // propagates if Err
    let parsed = parse(content)?,     // propagates if Err
    Ok(transform(parsed)),
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

## `Ordering` — Comparison Result

Represents the result of comparing two values.

### Definition

```sigil
type Ordering = Less | Equal | Greater
```

### Creating

`Ordering` values are typically returned by comparison functions:

```sigil
compare(1, 2)   // Less
compare(2, 2)   // Equal
compare(3, 2)   // Greater
```

### Usage

**In the `Comparable` trait:**

```sigil
trait Comparable {
    @compare (self, other: Self) -> Ordering
}
```

**In sorting:**

```sigil
// Custom sort order
@sort_by_length (strings: [str]) -> [str] =
    strings.sort_by(a, b -> compare(len(a), len(b)))
```

**Pattern matching:**

```sigil
@describe_comparison (a: int, b: int) -> str = match(compare(a, b),
    Less -> str(a) + " is less than " + str(b),
    Equal -> str(a) + " equals " + str(b),
    Greater -> str(a) + " is greater than " + str(b),
)
```

### Methods

```sigil
let ord = compare(a, b)

ord.is_less()    // true if Less
ord.is_equal()   // true if Equal
ord.is_greater() // true if Greater

ord.reverse()    // Less -> Greater, Greater -> Less, Equal -> Equal
```

### Chaining Comparisons

For multi-field comparisons, chain `Ordering` values:

```sigil
@compare_person (a: Person, b: Person) -> Ordering =
    compare(a.last_name, b.last_name)
        .then(compare(a.first_name, b.first_name))
        .then(compare(a.age, b.age))
```

The `then` method returns the first non-Equal ordering:

```sigil
Less.then(Greater)   // Less
Equal.then(Greater)  // Greater
Greater.then(Less)   // Greater
```

### Built-In

`Ordering` is built-in—no import required.

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
