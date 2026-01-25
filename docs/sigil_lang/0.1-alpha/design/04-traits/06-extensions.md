# Trait Extensions

This document covers trait extensions - a way to add methods to existing traits without modifying them.

---

## Overview

Trait extensions let you add methods to all implementors of a trait, without modifying the original trait definition. Extensions must be explicitly imported to use.

```sigil
// Define an extension
extend Iterator {
    @count (self) -> int = run(
        let n = 0,
        while self.next().is_some() do n = n + 1,
        n,
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

Sigil's extensions are **explicit**:

```sigil
// Must explicitly import to use
extension std.iter.extensions { Iterator.count }
```

No surprises. No hidden behavior. You ask for what you want.

---

## Defining Extensions

### Basic Extension

Use `extend` to add methods to a trait:

```sigil
extend Iterator {
    @count (self) -> int = run(
        let n = 0,
        while self.next().is_some() do n = n + 1,
        n,
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

```sigil
extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item =
        fold(self, Self.Item.default(), (acc, x) -> acc + x)
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

```sigil
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

```sigil
// Import from local file
extension './my_extensions' { Iterator.count, Iterator.sum }

// Import from standard library
extension std.iter.extensions { Iterator.take, Iterator.skip }

// Import from external package
extension some_package.extensions { Display.print }
```

### Method-Level Granularity

You must specify exactly which methods you want:

```sigil
// Import specific methods
extension './extensions' { Iterator.count, Iterator.last }

// Only .count() and .last() are available
range(1, 10).count()   // Works
range(1, 10).last()    // Works
range(1, 10).sum()     // ERROR - not imported
```

### Why Not Import All?

Sigil intentionally does not support wildcard extension imports:

```sigil
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

```sigil
// Defined inside the trait
trait Eq {
    @equals (self, other: Self) -> bool
    @not_equals (self, other: Self) -> bool = !self.equals(other)  // Default
}

// Always available to implementors
impl Eq for Point { ... }
point1.not_equals(point2)  // Works without any import
```

### Extensions

```sigil
// Defined outside the trait
extend Eq {
    @is_same (self, other: Self) -> bool = self.equals(other)
}

// Only available when imported
extension './eq_extensions' { Eq.is_same }
point1.is_same(point2)  // Works only after import
```

---

## Organizing Extensions

### Extension Modules

Group related extensions in dedicated modules:

```sigil
// std/iter/extensions.si

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

```sigil
// accounting/money_extensions.si

type Money = { amount: int, currency: str }

extend Iterator where Self.Item = Money {
    @total (self) -> Money =
        fold(self, Money { amount: 0, currency: "USD" },
             (acc, m) -> Money { amount: acc.amount + m.amount, currency: acc.currency })

    @in_currency (self, target: str) -> [Money] uses Exchange =
        map(self, m -> Exchange.convert(m, target))
}
```

Usage:
```sigil
use './types' { Money }
extension './accounting/money_extensions' { Iterator.total, Iterator.in_currency }

@summarize (transactions: [Money]) -> Money uses Exchange =
    transactions.in_currency("USD").total()
```

---

## Resolving Conflicts

If two extensions define the same method, only import one:

```sigil
// lib_a/extensions.si
extend Iterator {
    @count (self) -> int = ...  // Implementation A
}

// lib_b/extensions.si
extend Iterator {
    @count (self) -> int = ...  // Implementation B (different!)
}

// User code - pick one
extension './lib_a/extensions' { Iterator.count }  // Use A's version
// NOT: extension './lib_b/extensions' { Iterator.count }  // Would conflict
```

If you need both, use different names via aliases (if supported) or wrap in your own extension.

---

## Standard Library Extensions

The standard library provides common extensions:

```sigil
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
    Iterator.sum,      // requires Item: Add
    Iterator.min,      // requires Item: Comparable
    Iterator.max,      // requires Item: Comparable
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

```sigil
// Good: related methods together
extend Iterator {
    @count (self) -> int = ...
    @is_empty (self) -> bool = ...
}

// Avoid: unrelated methods
extend Iterator {
    @count (self) -> int = ...
    @to_json (self) -> str = ...  // Should be in different extension
}
```

### Document Extensions

```sigil
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

```sigil
// Good: clear what it does
extend Iterator {
    @take_while (self, pred: Self.Item -> bool) -> TakeWhileIter<Self> = ...
}

// Avoid: cryptic names
extend Iterator {
    @tw (self, p: Self.Item -> bool) -> ... = ...
}
```

---

## Error Messages

### Extension Not Imported

```
error: no method `count` found for type `Range`
  --> src/main.si:5:10
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
  --> src/main.si:2:30
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
