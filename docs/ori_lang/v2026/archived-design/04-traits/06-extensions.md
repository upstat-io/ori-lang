# Trait Extensions

This document covers trait extensions - a way to add methods to existing traits without modifying them.

---

## Overview

Trait extensions let you add methods to all implementors of a trait, without modifying the original trait definition. Extensions must be explicitly imported to use.

```ori
// Define an extension
extend Iterator {
    @count (self) -> int = run(
        let count = 0,
        while self.next().is_some() do count = count + 1,
        count,
    )
}

// Import specific extension methods
extension './my_extensions' { Iterator.count }

// Use the extension method
let total = range(1, 100).count()
```

---

## Why Extensions Instead of Blanket Impls?

Many languages use "blanket implementations" to add methods to categories of types:

```rust
// Rust blanket impl - implicit, always active
impl<T: Iterator> IteratorExt for T {
    fn count(self) -> usize { ... }
}
```

This has problems:
- **Implicit** - methods appear without you asking
- **Hidden side effects** - your type's interface changes due to distant code
- **Conflicts** - two libraries can conflict
- **Hard to trace** - "where did this method come from?"

Ori's extensions are **explicit**:

```ori
// Must explicitly import to use
extension std.iter.extensions { Iterator.count }
```

No surprises. No hidden behavior. You ask for what you want.

---

## Defining Extensions

### Basic Extension

Use `extend` to add methods to a trait:

```ori
extend Iterator {
    @count (self) -> int = run(
        let count = 0,
        while self.next().is_some() do count = count + 1,
        count,
    )

    @last (self) -> Option<Self.Item> = run(
        let result = None,
        for item in self do result = Some(item),
        result,
    )
}
```

### Extension with Constraints

Add constraints with `where`:

```ori
extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item =
        fold(self, Self.Item.default(), (accumulator, item) -> accumulator + item)
}

extend Iterator where Self.Item = int {
    @average (self) -> float = run(
        let sum = 0,
        let count = 0,
        for item in self do run(
            sum = sum + item,
            count = count + 1,
        ),
        float(sum) / float(count),
    )
}
```

### Extension Using Capabilities

Extensions can use capabilities:

```ori
extend Display {
    @print (self) -> void uses Console =
        Console.write(self.display())

    @println (self) -> void uses Console =
        Console.writeln(self.display())
}
```

---

## Importing Extensions

### The `extension` Keyword

Extensions are imported with the `extension` keyword (not `use`):

```ori
// Import from local file
extension './my_extensions' { Iterator.count, Iterator.sum }

// Import from standard library
extension std.iter.extensions { Iterator.take, Iterator.skip }

// Import from external package
extension some_package.extensions { Display.print }
```

### Method-Level Granularity

You must specify exactly which methods you want:

```ori
// Import specific methods
extension './extensions' { Iterator.count, Iterator.last }

// Only .count() and .last() are available
// Works
range(1, 10).count()
// Works
range(1, 10).last()
// ERROR - not imported
range(1, 10).sum()
```

### Why Not Import All?

Ori intentionally does not support wildcard extension imports:

```ori
// NOT supported
extension './extensions' { Iterator.* }
```

Reasons:
1. **Explicit is better** - you see exactly what methods are added
2. **No surprises** - no methods appearing unexpectedly
3. **Self-documenting** - import statement shows what's used
4. **No conflicts** - can't accidentally import conflicting methods

---

## Extension vs Default Methods

Both add methods to traits. The difference:

| Aspect | Default Methods | Extensions |
|--------|-----------------|------------|
| Where defined | Inside trait | Outside trait |
| When available | Always | Only when imported |
| Who can add | Trait author | Anyone |
| Can override | Yes | No |

### Default Methods

```ori
// Defined inside the trait
trait Eq {
    @equals (self, other: Self) -> bool
    // Default
    @not_equals (self, other: Self) -> bool = !self.equals(other)
}

// Always available to implementors
impl Eq for Point { ... }
// Works without any import
point1.not_equals(point2)
```

### Extensions

```ori
// Defined outside the trait
extend Eq {
    @is_same (self, other: Self) -> bool = self.equals(other)
}

// Only available when imported
extension './eq_extensions' { Eq.is_same }
// Works only after import
point1.is_same(point2)
```

---

## Organizing Extensions

### Extension Modules

Group related extensions in dedicated modules:

```ori
// std/iter/extensions.ori

extend Iterator {
    @count (self) -> int = ...
    @last (self) -> Option<Self.Item> = ...
    @nth (self, n: int) -> Option<Self.Item> = ...
}

extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item = ...
}

extend Iterator where Self.Item: Comparable {
    @min (self) -> Option<Self.Item> = ...
    @max (self) -> Option<Self.Item> = ...
    @sorted (self) -> [Self.Item] = ...
}
```

### Domain-Specific Extensions

Create extensions for your domain:

```ori
// accounting/money_extensions.ori

type Money = { amount: int, currency: str }

extend Iterator where Self.Item = Money {
    @total (self) -> Money =
        fold(self, Money { amount: 0, currency: "USD" },
             (accumulator, item) -> Money { amount: accumulator.amount + item.amount, currency: accumulator.currency })

    @in_currency (self, target: str) -> [Money] uses Exchange =
        map(self, money -> Exchange.convert(money, target))
}
```

Usage:
```ori
use './types' { Money }
extension './accounting/money_extensions' { Iterator.total, Iterator.in_currency }

@summarize (transactions: [Money]) -> Money uses Exchange =
    transactions.in_currency("USD").total()
```

---

## Resolving Conflicts

If two extensions define the same method, only import one:

```ori
// lib_a/extensions.ori
extend Iterator {
    // Implementation A
    @count (self) -> int = ...
}

// lib_b/extensions.ori
extend Iterator {
    // Implementation B (different!)
    @count (self) -> int = ...
}

// User code - pick one
// Use A's version
extension './lib_a/extensions' { Iterator.count }
// NOT: extension './lib_b/extensions' { Iterator.count }
// Would conflict
```

If you need both, use different names via aliases (if supported) or wrap in your own extension.

---

## Standard Library Extensions

The standard library provides common extensions:

```ori
// std.iter.extensions
extension std.iter.extensions {
    Iterator.count,
    Iterator.last,
    Iterator.nth,
    Iterator.take,
    Iterator.skip,
    Iterator.filter,
    Iterator.map,
    Iterator.fold,
    Iterator.collect,
    // requires Item: Add
    Iterator.sum,
    // requires Item: Comparable
    Iterator.min,
    // requires Item: Comparable
    Iterator.max,
}

// std.fmt.extensions
extension std.fmt.extensions {
    Display.print,
    Display.println,
    Display.to_string,
}
```

---

## Best Practices

### Keep Extensions Focused

```ori
// Good: related methods together
extend Iterator {
    @count (self) -> int = ...
    @is_empty (self) -> bool = ...
}

// Avoid: unrelated methods
extend Iterator {
    @count (self) -> int = ...
    // Should be in different extension
    @to_json (self) -> str = ...
}
```

### Document Extensions

```ori
// #Extensions for working with numeric iterators
extend Iterator where Self.Item = int {
    // #Returns the sum of all elements
    // >range(1, 4).sum() -> 6
    @sum (self) -> int = fold(self, 0, +)

    // #Returns the average of all elements
    // >range(1, 5).average() -> 2.5
    @average (self) -> float = ...
}
```

### Use Meaningful Names

```ori
// Good: clear what it does
extend Iterator {
    @take_while (self, predicate: Self.Item -> bool) -> TakeWhileIter<Self> = ...
}

// Avoid: cryptic names
extend Iterator {
    @tw (self, predicate: Self.Item -> bool) -> ... = ...
}
```

---

## Error Messages

### Extension Not Imported

```
error: no method `count` found for type `Range`
  --> src/main.ori:5:10
   |
 5 |     range(1, 10).count()
   |                  ^^^^^ method not found
   |
   = help: `count` is an extension method on `Iterator`
   = help: add: extension std.iter.extensions { Iterator.count }
```

### Unknown Extension Method

```
error: `Iterator.foo` not found in extension module
  --> src/main.ori:2:30
   |
 2 | extension std.iter.extensions { Iterator.foo }
   |                                          ^^^ not found
   |
   = help: available methods: count, last, take, skip, ...
```

---

## See Also

- [Trait Definitions](01-trait-definitions.md) - Defining traits
- [Implementations](02-implementations.md) - Implementing traits
- [Modules](../09-modules/index.md) - Module and import system
