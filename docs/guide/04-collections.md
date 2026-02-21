---
title: "Collections"
description: "Lists, maps, sets, tuples, and functional operations."
order: 4
part: "Data"
---

# Collections

Programs manipulate data. This guide teaches you how to work with Ori's collection types: lists, maps, sets, and tuples, plus the functional operations that make data transformation elegant.

## Lists

Lists are ordered collections of values of the same type.

### Creating Lists

```ori
let numbers = [1, 2, 3, 4, 5];
let names = ["Alice", "Bob", "Charlie"];
let mixed = [1, 2, 3];              // Type inferred as [int]
let empty: [int] = [];              // Empty list needs type annotation
```

All elements must have the same type:

```ori
let valid = [1, 2, 3];              // OK: all int
let also_valid = ["a", "b", "c"];   // OK: all str
let invalid = [1, "two", 3];        // ERROR: mixed types
```

### Accessing Elements

Use bracket notation with zero-based indices:

```ori
let fruits = ["apple", "banana", "cherry"];
fruits[0];    // "apple"
fruits[1];    // "banana"
fruits[2];    // "cherry"
```

**Special `#` symbol:** Inside brackets, `#` represents the list's length:

```ori
fruits[# - 1];    // "cherry" (last element)
fruits[# - 2];    // "banana" (second to last)
fruits[# / 2];    // "banana" (middle element)
```

**Out-of-bounds access panics:**

```ori
fruits[10];       // PANIC: index out of bounds
```

When you're not sure if an index is valid, use safe access methods:

```ori
let maybe = fruits.get(index: 10);    // None (safe)
let value = fruits.get(index: 0);     // Some("apple")
```

### Common List Methods

**`len()`** — get the length:

```ori
len(collection: fruits);    // 3
fruits.len();               // 3 (method form)
```

**`is_empty()`** — check if empty:

```ori
is_empty(collection: []);       // true
is_empty(collection: fruits);   // false
```

**`contains()`** — check membership:

```ori
fruits.contains(item: "apple");     // true
fruits.contains(item: "mango");     // false
```

**`first()` and `last()`** — safe access to ends:

```ori
fruits.first();    // Some("apple")
fruits.last();     // Some("cherry")
[].first();        // None
```

**`push()` and `pop()`** — add and remove:

```ori
let items = [1, 2, 3];
items.push(item: 4);       // [1, 2, 3, 4]
items.pop();               // (Some(4), [1, 2, 3])
```

Note: `pop()` returns a tuple of the removed value and the new list.

### Transforming Lists

The three most important operations are **map**, **filter**, and **fold**.

**`map`** — transform every element:

```ori
let numbers = [1, 2, 3, 4, 5];

// Double each number
let doubled = numbers.map(x -> x * 2);
// [2, 4, 6, 8, 10]

// Convert to strings
let strings = numbers.map(x -> `number: {x}`);
// ["number: 1", "number: 2", ...]

// Extract a field
let users = [User { name: "Alice" }, User { name: "Bob" }];
let names = users.map(u -> u.name);
// ["Alice", "Bob"]
```

**`filter`** — keep elements that match a condition:

```ori
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// Keep even numbers
let evens = numbers.filter(x -> x % 2 == 0);
// [2, 4, 6, 8, 10]

// Keep numbers greater than 5
let big = numbers.filter(x -> x > 5);
// [6, 7, 8, 9, 10]

// Keep non-empty strings
let words = ["hello", "", "world", ""];
let non_empty = words.filter(w -> !is_empty(collection: w));
// ["hello", "world"]
```

**`fold`** — reduce to a single value:

```ori
let numbers = [1, 2, 3, 4, 5];

// Sum all numbers
let sum = numbers.fold(
    initial: 0,
    op: (acc, x) -> acc + x,
);
// 15

// Find maximum
let max = numbers.fold(
    initial: numbers[0],
    op: (acc, x) -> if x > acc then x else acc,
);
// 5

// Build a string
let csv = numbers.fold(
    initial: "",
    op: (acc, x) -> if acc == "" then `{x}` else `{acc},{x}`,
);
// "1,2,3,4,5"
```

### Chaining Operations

Chain transformations together for powerful data pipelines:

```ori
let result = numbers
    .filter(x -> x > 2)           // [3, 4, 5]
    .map(x -> x * 10)             // [30, 40, 50]
    .fold(                         // 120
        initial: 0,
        op: (acc, x) -> acc + x,
    );
```

Real-world example:

```ori
type Order = { customer: str, total: float, paid: bool }

let orders = [
    Order { customer: "Alice", total: 100.0, paid: true },
    Order { customer: "Bob", total: 50.0, paid: false },
    Order { customer: "Charlie", total: 200.0, paid: true },
];

// Total revenue from paid orders
let revenue = orders
    .filter(o -> o.paid)
    .map(o -> o.total)
    .fold(initial: 0.0, op: (acc, x) -> acc + x);
// 300.0

// Names of customers with unpaid orders
let unpaid_customers = orders
    .filter(o -> !o.paid)
    .map(o -> o.customer);
// ["Bob"]
```

### For Loops with Lists

**Imperative form** — `for...do`:

```ori
for item in items do
    print(msg: item);

// Multiple statements in a block
for item in items do {
    let processed = transform(data: item);
    save(data: processed);
};
```

**Collection form** — `for...yield`:

```ori
// Equivalent to .map()
let doubled = for x in numbers yield x * 2;

// With filter (equivalent to .filter().map())
let big_doubled = for x in numbers if x > 5 yield x * 2;
```

**When to use each:**

- Use `.map()`, `.filter()`, `.fold()` for single transformations
- Use `for...yield` when combining filter and transform
- Use `for...do` for side effects (printing, saving)

### Combining Lists

**Spread operator** — merge lists:

```ori
let a = [1, 2, 3];
let b = [4, 5, 6];
let combined = [...a, ...b];    // [1, 2, 3, 4, 5, 6]

// Insert in the middle
let with_middle = [...a, 100, ...b];    // [1, 2, 3, 100, 4, 5, 6]
```

**`concat()`** — method form:

```ori
let combined = a.concat(other: b);    // [1, 2, 3, 4, 5, 6]
```

**`flatten()`** — merge nested lists:

```ori
let nested = [[1, 2], [3, 4], [5]];
let flat = nested.flatten();    // [1, 2, 3, 4, 5]
```

## Maps

Maps store key-value pairs with fast lookup by key.

### Creating Maps

```ori
let ages = {"Alice": 30, "Bob": 25, "Charlie": 35};
let config = {"timeout": 30, "retries": 3};
let empty: {str: int} = {};    // Empty map needs type annotation
```

Keys can be any hashable type (strings, numbers, etc.):

```ori
let by_id: {int: str} = {1: "Alice", 2: "Bob"};
```

### Accessing Values

Map access returns `Option<V>` since the key might not exist:

```ori
let ages = {"Alice": 30, "Bob": 25};

ages["Alice"];      // Some(30)
ages["Unknown"];    // None
```

**Coalesce with `??`** for default values:

```ori
let age = ages["Charlie"] ?? 0;    // 0 (key doesn't exist)
let age = ages["Alice"] ?? 0;      // 30 (key exists)
```

**`unwrap_or()`** — same idea, method form:

```ori
let age = ages["Charlie"].unwrap_or(default: 0);
```

### Modifying Maps

Maps are immutable by default. Methods return new maps:

```ori
let ages = {"Alice": 30};

// Add or update a key
let updated = ages.insert(key: "Bob", value: 25);
// {"Alice": 30, "Bob": 25}

// Remove a key
let without = ages.remove(key: "Alice");
// {}
```

The original map is unchanged:

```ori
ages;    // Still {"Alice": 30}
```

### Iterating Maps

**Keys:**

```ori
for key in ages.keys() do
    print(msg: key);
```

**Values:**

```ori
for value in ages.values() do
    print(msg: `{value}`);
```

**Key-value pairs:**

```ori
for (key, value) in ages.entries() do
    print(msg: `{key} is {value} years old`);
```

### Spread with Maps

Merge maps with spread:

```ori
let defaults = {"timeout": 30, "retries": 3, "debug": false};
let custom = {"timeout": 60, "debug": true};

let config = {...defaults, ...custom};
// {"timeout": 60, "retries": 3, "debug": true}
// Later values win on conflict
```

## Sets

Sets store unique values with fast membership testing.

### Creating Sets

```ori
use std.collections { Set };

let numbers = Set.from(items: [1, 2, 3, 2, 1]);    // {1, 2, 3}
let names = Set.from(items: ["Alice", "Bob"]);
let empty = Set<int>.new();
```

### Set Operations

```ori
let a = Set.from(items: [1, 2, 3, 4]);
let b = Set.from(items: [3, 4, 5, 6]);

a.contains(item: 3);           // true
a.union(other: b);             // {1, 2, 3, 4, 5, 6}
a.intersection(other: b);      // {3, 4}
a.difference(other: b);        // {1, 2}
```

### Common Set Methods

```ori
set.insert(item: value);    // Add an element
set.remove(item: value);    // Remove an element
set.len();                  // Number of elements
set.is_empty();             // Check if empty
```

## Tuples

Tuples group a fixed number of values that can have different types.

### Creating Tuples

```ori
let pair = (1, "hello");
let triple = (true, 42, "world");
let nested = ((1, 2), (3, 4));
```

### Accessing Tuple Elements

Use `.0`, `.1`, `.2`, etc.:

```ori
let pair = (10, "hello");
pair.0;    // 10
pair.1;    // "hello"
```

### Destructuring Tuples

```ori
let (x, y) = (10, 20);
// x = 10, y = 20

let (first, second, third) = ("a", "b", "c");
// first = "a", second = "b", third = "c"
```

Ignore values with `_`:

```ori
let (x, _) = (10, 20);    // Only care about first value
```

### Common Tuple Patterns

**Returning multiple values:**

```ori
@divide_with_remainder (a: int, b: int) -> (int, int) = (a / b, a % b);

let (quotient, remainder) = divide_with_remainder(a: 17, b: 5);
// quotient = 3, remainder = 2
```

**Swapping values:**

```ori
let (a, b) = (b, a);    // Swap a and b
```

**The unit tuple:**

```ori
let unit = ();    // Type is void, value is ()
```

## Ranges

Ranges create sequences of numbers.

### Range Types

```ori
0..5;        // Exclusive: 0, 1, 2, 3, 4
0..=5;       // Inclusive: 0, 1, 2, 3, 4, 5
0..10 by 2;  // Stepped: 0, 2, 4, 6, 8
10..0 by -1; // Descending: 10, 9, 8, 7, 6, 5, 4, 3, 2, 1
```

### Range Rules

- Step must be non-zero (panics at runtime)
- Mismatched direction produces empty range (no panic)
- Float ranges are not iterable (precision issues)

### Using Ranges

**In loops:**

```ori
for i in 0..10 do
    print(msg: `{i}`);

for i in 10..0 by -1 do
    print(msg: `Countdown: {i}`);
```

**Collecting to list:**

```ori
let nums = (0..5).collect();    // [0, 1, 2, 3, 4]
```

## Functional Operations Deep Dive

### Method Chaining

Ori's collections support fluent method chaining:

```ori
let result = data
    .filter(x -> x.active)
    .map(x -> x.value)
    .filter(v -> v > 0)
    .take(count: 10)
    .collect();
```

Each method returns a new collection (or iterator), enabling the chain.

### Common Patterns

**Sum and product:**

```ori
let sum = numbers.fold(initial: 0, op: (a, b) -> a + b);
let product = numbers.fold(initial: 1, op: (a, b) -> a * b);
```

**Find first match:**

```ori
let first_even = numbers.find(x -> x % 2 == 0);  // Option<int>
```

**Check conditions:**

```ori
let any_negative = numbers.any(predicate: x -> x < 0);
let all_positive = numbers.all(predicate: x -> x > 0);
```

**Count matches:**

```ori
let even_count = numbers.filter(x -> x % 2 == 0).count();
```

**Take and skip:**

```ori
let first_three = items.take(count: 3);
let rest = items.skip(count: 3);
```

### Combining Collections

**Zip two lists:**

```ori
let names = ["Alice", "Bob", "Charlie"];
let ages = [30, 25, 35];

let people = names.iter()
    .zip(other: ages.iter())
    .map(transform: (name, age) -> `{name}: {age}`)
    .collect();
// ["Alice: 30", "Bob: 25", "Charlie: 35"]
```

**Enumerate with indices:**

```ori
for (index, item) in items.iter().enumerate() do
    print(msg: `{index}: {item}`);
```

## Complete Example

```ori
type Product = { name: str, price: float, category: str, in_stock: bool }

let products = [
    Product { name: "Laptop", price: 999.99, category: "Electronics", in_stock: true },
    Product { name: "Phone", price: 699.99, category: "Electronics", in_stock: true },
    Product { name: "Desk", price: 299.99, category: "Furniture", in_stock: false },
    Product { name: "Chair", price: 199.99, category: "Furniture", in_stock: true },
    Product { name: "Monitor", price: 399.99, category: "Electronics", in_stock: true },
];

// Get available electronics, sorted by price
@available_electronics (products: [Product]) -> [Product] =
    products
        .filter(p -> p.category == "Electronics" && p.in_stock)
        .collect()
        .sort_by(key: p -> p.price);

@test_available tests @available_electronics () -> void = {
    let result = available_electronics(products: products);
    assert_eq(actual: len(collection: result), expected: 3);
    assert_eq(actual: result[0].name, expected: "Monitor");
}

// Calculate total value of inventory
@inventory_value (products: [Product]) -> float =
    products
        .filter(p -> p.in_stock)
        .map(p -> p.price)
        .fold(initial: 0.0, op: (sum, price) -> sum + price);

@test_inventory tests @inventory_value () -> void = {
    let value = inventory_value(products: products);
    assert_eq(actual: value, expected: 2299.96);
}

// Group by category
@by_category (products: [Product]) -> {str: [Product]} =
    products.fold(
        initial: {},
        op: (groups, product) -> {
            let category = product.category;
            let existing = groups[category] ?? [];
            groups.insert(key: category, value: [...existing, product])
        },
    );

@test_by_category tests @by_category () -> void = {
    let grouped = by_category(products: products);
    assert_eq(actual: len(collection: grouped["Electronics"] ?? []), expected: 3);
    assert_eq(actual: len(collection: grouped["Furniture"] ?? []), expected: 2);
}
```

## Quick Reference

### Lists

```ori
[1, 2, 3]
list[0], list[# - 1]
list.len(), list.is_empty()
list.first(), list.last()
list.get(index: i)
list.contains(item: x)
list.map(x -> ...), list.filter(x -> ...)
list.fold(initial: val, op: (acc, x) -> ...)
[...a, ...b]
```

### Maps

```ori
{"key": value}
map["key"]          // Returns Option
map["key"] ?? default
map.insert(key: k, value: v)
map.remove(key: k)
map.keys(), map.values(), map.entries()
{...defaults, ...overrides}
```

### Sets

```ori
Set.from(items: [...])
Set<T>.new()
set.contains(item: x)
set.insert(item: x)
set.remove(item: x)
set.union(other: b), set.intersection(other: b)
```

### Tuples

```ori
(a, b, c)
tuple.0, tuple.1
let (x, y) = tuple;
```

### Ranges

```ori
0..10           // Exclusive
0..=10          // Inclusive
0..10 by 2      // Stepped
10..0 by -1     // Descending
```

## What's Next

Now that you can work with collections:

- **[Custom Types](/guide/05-custom-types)** — Structs and sum types
- **[Pattern Matching](/guide/06-pattern-matching)** — Destructuring and matching
