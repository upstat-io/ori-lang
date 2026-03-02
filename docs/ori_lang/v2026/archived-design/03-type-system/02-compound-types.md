# Compound Types

This document covers Ori's built-in compound types: List, Map, Set, Tuple, Range, Option, Result, and Ordering.

---

## `[T]` — List

Ordered collection of elements of type T.

### Literals

```ori
let numbers = [1, 2, 3, 4, 5]
let names = ["Alice", "Bob", "Carol"]
let empty: [int] = []
let nested = [[1, 2], [3, 4], [5, 6]]
```

### Operations

```ori
// Indexing
let first = numbers[0]
let last = numbers[# - 1]
let middle = numbers[# / 2]

// Length
// 5
len(.collection: numbers)

// Concatenation
// [1, 2, 3, 4]
let combined = [1, 2] + [3, 4]

// Methods
// true
numbers.contains(3)
// false
numbers.is_empty()
// Option<int> -> Some(1)
numbers.first()
// Option<int> -> Some(5)
numbers.last()
// [5, 4, 3, 2, 1]
numbers.reverse()
// sorted copy
numbers.sort()
```

### With Patterns

```ori
@sum (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: 0,
    .operation: +,
)

@doubled (numbers: [int]) -> [int] = map(
    .over: numbers,
    .transform: number -> number * 2,
)

@evens (numbers: [int]) -> [int] = filter(
    .over: numbers,
    .predicate: number -> number % 2 == 0,
)
```

### Immutability

Lists are immutable. Operations return new lists:

```ori
let original = [1, 2, 3]
// original unchanged
let updated = original + [4]
// original = [1, 2, 3]
// updated = [1, 2, 3, 4]
```

---

## `{K: V}` — Map

Key-value collection.

### Literals

```ori
let ages = {"Alice": 30, "Bob": 25, "Carol": 35}
let empty: {str: int} = {}
```

### Operations

```ori
// Access — returns Option<V>
// Option<int> -> Some(30)
ages["Alice"]
// Option<int> -> None
ages["Missing"]

// Access with default
// int -> 30
ages["Alice"] ?? 0
// int -> 0
ages["Missing"] ?? 0

// Check existence
// true
ages.has("Alice")
// false
ages.has("Dave")

// Keys and values
// [str]
ages.keys()
// [int]
ages.values()
// [(str, int)]
ages.entries()

// Size
// 3
len(.collection: ages)
// false
ages.is_empty()
```

**Note:** Map indexing always returns `Option<V>` — no hidden panics on missing keys. Use `??` to provide a default value.

### Key Requirements

Keys must implement `Eq` and `Hashable`:

```ori
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

```ori
let numbers = Set<int>.new()
// {1, 2, 3}
let from_list = Set<int>.from([1, 2, 3, 2, 1])
```

### Operations

```ori
// Adding elements
let s = Set<int>.new()
s.insert(1)
s.insert(2)
// no effect, already present
s.insert(1)

// Checking membership
// true
s.contains(1)
// false
s.contains(5)

// Size
// 2
s.len()
// false
s.is_empty()

// Removal
// removes element
s.remove(1)
// removes all elements
s.clear()
```

### Set Operations

```ori
let a = Set<int>.from([1, 2, 3])
let b = Set<int>.from([2, 3, 4])

// {1, 2, 3, 4}
a.union(b)
// {2, 3}
a.intersection(b)
// {1}
a.difference(b)
// {1, 4}
a.symmetric_diff(b)

// false
a.is_subset(b)
// false
a.is_superset(b)
// false (they share elements)
a.is_disjoint(b)
```

### Element Requirements

Elements must implement `Eq` and `Hashable`:

```ori
// OK: primitive types
Set<int>
Set<str>

// OK: custom types with derive
#[derive(Eq, Hashable)]
type UserId = str
Set<UserId>

// ERROR: unhashable type
// lists are not hashable
Set<[int]>
```

### Iteration

```ori
let ids = Set<int>.from([1, 2, 3])

// Iterate over elements (order not guaranteed)
for id in ids do print(.message: str(id))

// Convert to list
// [int], order not guaranteed
let list = ids.to_list()
```

### Common Uses

```ori
// Deduplication
@unique<T: Eq + Hashable> (elements: [T]) -> [T] =
    Set<T>.from(elements).to_list()

// Fast membership testing
@has_permission (user: User, required: Set<Permission>) -> bool =
    required.is_subset(user.permissions)

// Tracking seen items
@process_unique (items: [Item]) -> [Result] = run(
    let seen = Set<ItemId>.new(),
    let unique_items = filter(
        .over: items,
        .predicate: item ->
            if seen.contains(item.id) then false
            else run(seen.insert(item.id), true),
    ),
    map(
        .over: unique_items,
        .transform: process,
    ),
)
```

---

## `(T, U, ...)` — Tuple

Fixed-size, heterogeneous collection.

### Literals

```ori
let pair = (1, "hello")
let triple = (true, 3.14, "world")
// unit tuple (empty tuple)
let unit = ()
```

### Unit Tuple `()`

The empty tuple `()` is the unit type, equivalent to `void`. Use `()` when you need to pass or store unit as a value:

```ori
// As accumulator placeholder in fold
items.fold((), (_, item) -> process(item))

// As a type parameter
type Callback = () -> ()

// In data structures
type Event = { timestamp: int, data: Option<()> }
```

**Note:** For function return types, prefer `void` for clarity. See [void](01-primitive-types.md#void--no-meaningful-value).

### Access

```ori
// By position
// 1
let first = pair.0
// "hello"
let second = pair.1

// Destructuring
let (x, y) = pair
let (a, b, c) = triple
```

### Common Uses

```ori
// Return multiple values
@divide_with_remainder (a: int, b: int) -> (int, int) =
    (a / b, a % b)

// Usage
let (quotient, remainder) = divide_with_remainder(17, 5)
```

### Type Annotation

```ori
let point: (int, int) = (10, 20)
let record: (str, int, bool) = ("Alice", 30, true)
```

---

## `Range<T>` — Range

A range of values from a start to an end bound.

### Creating Ranges

Ranges are created using the range operators:

```ori
// Exclusive end (..)
// 0, 1, 2, ..., 9
0..10
// 'a', 'b', ..., 'y'
'a'..'z'

// Inclusive end (..=)
// 0, 1, 2, ..., 10
0..=10
// 'a', 'b', ..., 'z'
'a'..='z'
```

### Type

The type of a range is `Range<T>` where `T` is the type of the bounds:

```ori
let r: Range<int> = 0..10
let c: Range<char> = 'a'..'z'
```

### Requirements

Range bounds must implement the `Comparable` trait:

```ori
// OK: int and char are comparable
0..10
'a'..'z'

// ERROR: str is not comparable in this way
"a".."z"
```

### Iteration

Ranges are iterable:

```ori
// For loop
for i in 0..10 do print(.message: str(i))

// With collect pattern
let squares = collect(
    .range: 1..=10,
    .transform: number -> number * number,
)
// [1, 4, 9, 16, 25, 36, 49, 64, 81, 100]

// Converting to list
// [0, 1, 2, 3, 4]
let nums = (0..5).to_list()
```

### Range Operations

```ori
let r = 0..10

// true
r.contains(5)
// false (exclusive end)
r.contains(10)
// false
r.contains(-1)

// false
r.is_empty()
// true (empty range)
(5..5).is_empty()
```

### Common Uses

```ori
// Indexing with range (slicing)
let items = [1, 2, 3, 4, 5]
// [2, 3, 4]
items[1..4]
// [1, 2, 3] (from start)
items[..3]
// [3, 4, 5] (to end)
items[2..]

// Generating sequences
let indices = collect(
    .range: 0..len(.collection: items),
    .transform: index -> index,
)

// Bounded iteration
for i in 0..$max_retries do attempt(i)
```

### Step Ranges

For ranges with a step other than 1, use `step_by`:

```ori
// 0, 2, 4, 6, 8
(0..10).step_by(2)

// Counting down
// 10, 9, 8, ..., 0
(10..=0).step_by(-1)
```

---

## `Option<T>` — Optional Value

Represents a value that may or may not exist.

### Variants

```ori
type Option<T> = Some(T) | None
```

### Creating

```ori
let present = Some(42)
let absent: Option<int> = None
```

### Checking

```ori
// true
is_some(.opt: present)
// false
is_none(.opt: present)
// false
is_some(.opt: absent)
// true
is_none(.opt: absent)
```

### Extracting Values

```ori
// Pattern matching (preferred)
@describe (opt: Option<int>) -> str = match(opt,
    Some(value) -> "value: " + str(value),
    None -> "no value"
)

// Unwrap (panics if None)
// 42
let value = present.unwrap()

// With default
// 42
let value = present ?? 0
// 0
let value = absent ?? 0
```

### Common Operations

```ori
// Map over Some
// Some(84) or None
opt.map(value -> value * 2)

// Chain options
opt.and_then(value -> if value > 0 then Some(value) else None)

// Convert to Result
// Result<int, str>
opt.ok_or("no value")
```

### Built-In

`Option<T>` is built-in—no import required.

---

## `Result<T, E>` — Success or Error

Represents an operation that can succeed or fail.

### Variants

```ori
type Result<T, E> = Ok(T) | Err(E)
```

### Creating

```ori
let success = Ok(42)
let failure: Result<int, str> = Err("something went wrong")
```

### Checking

```ori
// true
is_ok(.result: success)
// false
is_err(.result: success)
// false
is_ok(.result: failure)
// true
is_err(.result: failure)
```

### Extracting Values

```ori
// Pattern matching (preferred)
@describe (result: Result<int, str>) -> str = match(result,
    Ok(value) -> "success: " + str(value),
    Err(error) -> "error: " + error
)

// Unwrap (panics if Err)
// 42
let value = success.unwrap()

// Unwrap error (panics if Ok)
// "something went wrong"
let error = failure.unwrap_err()

// With default
// 42 (returns default on Err)
let value = success ?? 0
```

### Error Propagation

Use the `try` pattern:

```ori
@process (path: str) -> Result<Data, Error> = try(
    // propagates if Err
    let content = read_file(path)?,
    // propagates if Err
    let parsed = parse(content)?,
    Ok(transform(parsed)),
)
```

### Common Operations

```ori
// Map over Ok
// Ok(84) or original Err
result.map(value -> value * 2)

// Map over Err
result.map_err(error -> "Error: " + error)

// Chain results
result.and_then(value -> divide(value, 2))

// Convert to Option
// Option<T>
result.ok()
// Option<E>
result.err()
```

### Built-In

`Result<T, E>` is built-in—no import required.

---

## `Ordering` — Comparison Result

Represents the result of comparing two values.

### Definition

```ori
type Ordering = Less | Equal | Greater
```

### Creating

`Ordering` values are typically returned by comparison functions:

```ori
// Less
compare(1, 2)
// Equal
compare(2, 2)
// Greater
compare(3, 2)
```

### Usage

**In the `Comparable` trait:**

```ori
trait Comparable {
    @compare (self, other: Self) -> Ordering
}
```

**In sorting:**

```ori
// Custom sort order
@sort_by_length (strings: [str]) -> [str] =
    strings.sort_by((left, right) -> compare(len(left), len(right)))
```

**Pattern matching:**

```ori
@describe_comparison (left: int, right: int) -> str = match(compare(left, right),
    Less -> str(left) + " is less than " + str(right),
    Equal -> str(left) + " equals " + str(right),
    Greater -> str(left) + " is greater than " + str(right),
)
```

### Methods

```ori
let ord = compare(left, right)

// true if Less
ord.is_less()
// true if Equal
ord.is_equal()
// true if Greater
ord.is_greater()

// Less -> Greater, Greater -> Less, Equal -> Equal
ord.reverse()
```

### Chaining Comparisons

For multi-field comparisons, chain `Ordering` values:

```ori
@compare_person (left: Person, right: Person) -> Ordering =
    compare(left.last_name, right.last_name)
        .then(compare(left.first_name, right.first_name))
        .then(compare(left.age, right.age))
```

The `then` method returns the first non-Equal ordering:

```ori
// Less
Less.then(Greater)
// Greater
Equal.then(Greater)
// Greater
Greater.then(Less)
```

### Built-In

`Ordering` is built-in—no import required.

---

## Type Nesting

Compound types can be nested:

```ori
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
