---
title: "Extensions"
description: "Adding methods to existing traits without modification."
order: 18
part: "Abstraction"
---

# Extensions

Extensions let you add methods to existing traits without modifying them. This enables extending library code and creating domain-specific utilities.

## What Are Extensions?

Extensions add new methods to traits after they're defined:

```ori
// Original trait in std library
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)
}

// Your extension adds new methods
extend Iterator {
    @sum (self) -> int where Self.Item == int =
        self.fold(initial: 0, op: (acc, x) -> acc + x)
}
```

Now any `Iterator` of `int` has a `.sum()` method.

## Defining Extensions

### Basic Extension

```ori
extend Iterator {
    @count_where (self, predicate: (Self.Item) -> bool) -> int =
        self.filter(predicate: predicate).count()
}
```

### Extension with Constraints

Use `where` to limit which types can use the method:

```ori
extend Iterator {
    @sum (self) -> int where Self.Item == int =
        self.fold(initial: 0, op: (acc, x) -> acc + x)

    @product (self) -> int where Self.Item == int =
        self.fold(initial: 1, op: (acc, x) -> acc * x)

    @join_strings (self, sep: str) -> str where Self.Item == str =
        self.fold(initial: "", op: (acc, s) -> if acc == "" then s else `{acc}{sep}{s}`)
}
```

### Extension with Generic Constraints

```ori
extend Iterator {
    @max_by<K: Comparable> (self, key: (Self.Item) -> K) -> Option<Self.Item> = run(
        let result: Option<(Self.Item, K)> = None,

        for item in self do run(
            let k = key(item),
            result = match(
                result,
                None -> Some((item, k)),
                Some((_, max_k)) -> if k > max_k then Some((item, k)) else result,
            ),
        ),

        result.map(transform: (item, _) -> item),
    )
}
```

## Importing Extensions

Extensions must be explicitly imported to use:

```ori
// Import specific extension methods
extension std.iter.extensions { Iterator.sum, Iterator.product }

// Import from local file
extension "./my_extensions" { Iterator.count_where }

// Now you can use them
let numbers = [1, 2, 3, 4, 5]
let total = numbers.iter().sum()  // 15
```

### Import Syntax

```ori
extension "path" { Trait.method, Trait.other_method }
extension module.path { Trait.method }
```

## Using Extensions

Once imported, use them like regular methods:

```ori
extension std.iter.extensions { Iterator.sum, Iterator.product }

let numbers = [1, 2, 3, 4, 5]
let total = numbers.iter().sum()      // 15
let product = numbers.iter().product() // 120
```

## Extension Patterns

### Domain-Specific Utilities

```ori
// extensions/money.ori
type Money = { amount: int, currency: str }

extend Iterator {
    @total_amount (self) -> int where Self.Item == Money =
        self.map(transform: m -> m.amount).fold(initial: 0, op: (a, b) -> a + b)

    @by_currency (self) -> {str: [Money]} where Self.Item == Money = run(
        let groups: {str: [Money]} = {},
        for money in self do run(
            let current = groups[money.currency] ?? [],
            groups = { ...groups, money.currency: [...current, money] },
        ),
        groups,
    )
}
```

```ori
// Usage
extension "./extensions/money" { Iterator.total_amount, Iterator.by_currency }

let transactions = [
    Money { amount: 100, currency: "USD" },
    Money { amount: 50, currency: "EUR" },
    Money { amount: 75, currency: "USD" },
]

let usd_total = transactions.iter()
    .filter(predicate: m -> m.currency == "USD")
    .total_amount()  // 175
```

### Statistics Extensions

```ori
// extensions/stats.ori
extend Iterator {
    @mean (self) -> float where Self.Item == float = run(
        let sum = 0.0,
        let count = 0,
        for x in self do run(
            sum = sum + x,
            count = count + 1,
        ),
        if count == 0 then 0.0 else sum / count as float,
    )

    @variance (self) -> float where Self.Item == float = run(
        let values = self.collect(),
        let m = values.iter().mean(),
        let squared_diffs = values.iter()
            .map(transform: x -> (x - m) * (x - m)),
        squared_diffs.mean(),
    )

    @std_dev (self) -> float where Self.Item == float =
        self.variance().sqrt()
}
```

### String Extensions

```ori
// extensions/strings.ori
extend Iterator {
    @join (self, sep: str) -> str where Self.Item == str = run(
        let result = "",
        let first = true,
        for s in self do run(
            result = if first then s else `{result}{sep}{s}`,
            first = false,
        ),
        result,
    )

    @lines (self) -> [str] where Self.Item == char = run(
        let lines: [str] = [],
        let current = "",
        for c in self do
            if c == '\n' then run(
                lines = [...lines, current],
                current = "",
            ) else
                current = `{current}{c}`,
        if current != "" then
            lines = [...lines, current],
        lines,
    )
}
```

### Numeric Extensions

```ori
// extensions/numeric.ori
extend Iterator {
    @running_sum (self) -> impl Iterator where Self.Item == int = run(
        let total = 0,
        self.map(transform: x -> run(
            total = total + x,
            total,
        )),
    )

    @differences (self) -> impl Iterator where Self.Item == int = run(
        let prev: Option<int> = None,
        self.filter_map(transform: x -> run(
            let result = prev.map(transform: p -> x - p),
            prev = Some(x),
            result,
        )),
    )
}
```

## Extension vs Default Implementation

| Feature | Default in Trait | Extension |
|---------|-----------------|-----------|
| Defined | Inside trait definition | Outside trait |
| Visibility | Always available | Requires import |
| Override | Implementers can override | Cannot be overridden |
| Use case | Core functionality | Optional utilities |

### When to Use Extensions

- Adding convenience methods without bloating the trait
- Domain-specific utilities
- Experimental features
- Methods that only make sense for certain type combinations

### When to Use Default Implementations

- Methods that all implementers should have
- Core functionality of the trait
- Methods that implementers might want to optimize

## Testing Extensions

```ori
// extensions/iter_extensions.ori
extend Iterator {
    @sum (self) -> int where Self.Item == int =
        self.fold(initial: 0, op: (acc, x) -> acc + x)
}

// Test file
extension "./iter_extensions" { Iterator.sum }

@test_sum tests _ () -> void = run(
    let numbers = [1, 2, 3, 4, 5],
    assert_eq(actual: numbers.iter().sum(), expected: 15),

    let empty: [int] = [],
    assert_eq(actual: empty.iter().sum(), expected: 0),
)
```

## Complete Example

```ori
// extensions/collections.ori

// Grouping extension
extend Iterator {
    @group_by<K: Eq + Hashable> (
        self,
        key: (Self.Item) -> K,
    ) -> {K: [Self.Item]} = run(
        let groups: {K: [Self.Item]} = {},
        for item in self do run(
            let k = key(item),
            let current = groups[k] ?? [],
            groups = { ...groups, k: [...current, item] },
        ),
        groups,
    )
}

// Partitioning extension
extend Iterator {
    @partition (
        self,
        predicate: (Self.Item) -> bool,
    ) -> ([Self.Item], [Self.Item]) = run(
        let matching: [Self.Item] = [],
        let not_matching: [Self.Item] = [],
        for item in self do
            if predicate(item) then
                matching = [...matching, item]
            else
                not_matching = [...not_matching, item],
        (matching, not_matching),
    )
}

// Chunking extension
extend Iterator {
    @chunks (self, size: int) -> [[Self.Item]] = run(
        let result: [[Self.Item]] = [],
        let current: [Self.Item] = [],
        for item in self do run(
            current = [...current, item],
            if len(collection: current) == size then run(
                result = [...result, current],
                current = [],
            ),
        ),
        if !is_empty(collection: current) then
            result = [...result, current],
        result,
    )
}

// Intersperse extension
extend Iterator {
    @intersperse (self, sep: Self.Item) -> impl Iterator = run(
        let first = true,
        self.flat_map(transform: item -> run(
            if first then run(
                first = false,
                [item].iter(),
            ) else
                [sep, item].iter(),
        )),
    )
}
```

```ori
// Usage
extension "./extensions/collections" {
    Iterator.group_by,
    Iterator.partition,
    Iterator.chunks,
    Iterator.intersperse,
}

type Person = { name: str, age: int, city: str }

@example () -> void = run(
    let people = [
        Person { name: "Alice", age: 30, city: "NYC" },
        Person { name: "Bob", age: 25, city: "LA" },
        Person { name: "Charlie", age: 35, city: "NYC" },
        Person { name: "Diana", age: 28, city: "LA" },
    ],

    // Group by city
    let by_city = people.iter().group_by(key: p -> p.city),
    // { "NYC": [Alice, Charlie], "LA": [Bob, Diana] }

    // Partition by age
    let (over_30, under_30) = people.iter().partition(predicate: p -> p.age >= 30),
    // ([Alice, Charlie], [Bob, Diana])

    // Chunk into pairs
    let pairs = people.iter().chunks(size: 2),
    // [[Alice, Bob], [Charlie, Diana]]

    // Join names with separator
    let names = people.iter()
        .map(transform: p -> p.name)
        .intersperse(sep: " and ")
        .collect()
        .join(sep: ""),
    // "Alice and Bob and Charlie and Diana"
)

@test_group_by tests _ () -> void = run(
    extension "./extensions/collections" { Iterator.group_by }

    let numbers = [1, 2, 3, 4, 5, 6],
    let grouped = numbers.iter().group_by(key: n -> n % 2),
    assert_eq(actual: len(collection: grouped[0] ?? []), expected: 3),  // 2, 4, 6
    assert_eq(actual: len(collection: grouped[1] ?? []), expected: 3),  // 1, 3, 5
)

@test_partition tests _ () -> void = run(
    extension "./extensions/collections" { Iterator.partition }

    let numbers = [1, 2, 3, 4, 5, 6],
    let (evens, odds) = numbers.iter().partition(predicate: n -> n % 2 == 0),
    assert_eq(actual: evens, expected: [2, 4, 6]),
    assert_eq(actual: odds, expected: [1, 3, 5]),
)

@test_chunks tests _ () -> void = run(
    extension "./extensions/collections" { Iterator.chunks }

    let numbers = [1, 2, 3, 4, 5],
    let chunked = numbers.iter().chunks(size: 2),
    assert_eq(actual: chunked, expected: [[1, 2], [3, 4], [5]]),
)
```

## Quick Reference

### Define Extension

```ori
extend Trait {
    @method (self) -> Type = ...
}

extend Trait {
    @method (self) -> Type where Self.Item == int = ...
}
```

### Import Extension

```ori
extension "path" { Trait.method }
extension module.path { Trait.method }
```

### Use Extension

```ori
value.method()  // After importing
```

## What's Next

Now that you understand extensions:

- **[Compiler Patterns](/guide/19-compiler-patterns)** — Advanced pattern usage
- **[Memory Model](/guide/20-memory-model)** — Understanding ARC

