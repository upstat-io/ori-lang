# Proposal: Extension Methods

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, type system, module system

---

## Summary

This proposal formalizes extension method semantics, including definition syntax, import mechanics, conflict resolution, and scoping rules.

---

## Problem Statement

The spec documents extension syntax but leaves unclear:

1. **Scoping**: When are extension methods visible?
2. **Conflict resolution**: What happens when multiple extensions define the same method?
3. **Orphan rules**: Can extensions be defined for foreign types?
4. **Method resolution order**: How do extensions interact with inherent methods and traits?

---

## Extension Definition

### Syntax

Extensions add methods to existing types without modifying their definition:

```ori
extend Iterator {
    @count (self) -> int = run(
        let count = 0,
        for _ in self do count = count + 1,
        count,
    )
}
```

### Constrained Extensions

Extensions can have type constraints:

```ori
extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item = self.fold(
        initial: Self.Item.default(),
        combine: (acc, x) -> acc + x,
    )
}
```

### What Can Be Extended

Extensions may be defined for:
- Concrete types: `extend Point { ... }`
- Generic types: `extend [T] { ... }`
- Trait implementors: `extend Iterator { ... }`
- Constrained generics: `extend [T] where T: Printable { ... }`

Extensions cannot:
- Add fields to types
- Implement traits (use `impl Trait for Type` instead)
- Override existing methods

---

## Extension Import

### Explicit Import Required

Extension methods are NOT automatically available. They must be explicitly imported:

```ori
extension std.iter.extensions { Iterator.count, Iterator.sum }
```

### Import Syntax

```ori
extension "module_path" { Type.method, Type.other_method }
extension "./local_ext" { Point.distance }
```

Method-level granularity is required. Wildcard imports are not supported:

```ori
// ERROR: wildcards not allowed
extension std.iter.extensions { Iterator.* }

// OK: explicit methods
extension std.iter.extensions { Iterator.count, Iterator.sum, Iterator.last }
```

### Visibility

Extensions follow normal visibility rules:

```ori
// In extensions.ori
pub extend Iterator {
    @count (self) -> int = ...  // Publicly importable
}

extend Iterator {
    @internal_helper (self) -> int = ...  // Module-private
}
```

---

## Method Resolution

### Resolution Order

When calling `value.method()`:

1. **Inherent methods** — methods defined in the type's `impl` block
2. **Trait methods** — methods from implemented traits
3. **Extension methods** — methods from imported extensions

Earlier entries shadow later ones.

### Conflict Resolution

If multiple imported extensions define the same method for a type:

```ori
extension "./ext_a" { Point.distance }
extension "./ext_b" { Point.distance }

let p = Point { x: 1, y: 2 }
p.distance()  // ERROR: ambiguous extension method
```

Resolve ambiguity with qualified syntax:

```ori
use "./ext_a" as ext_a
ext_a.Point.distance(p)
```

### No Implicit Override

Extensions cannot override inherent or trait methods:

```ori
type Point = { x: int, y: int }

impl Point {
    @distance (self) -> float = ...
}

extend Point {
    @distance (self) -> float = ...  // ERROR: cannot override inherent method
}
```

---

## Orphan Rules

### Same-Package Rule

An extension must be defined in the same package as either:
- The type being extended, OR
- At least one trait bound in a constrained extension

```ori
// In my_package

// OK: extending local type
type MyType = { ... }
extend MyType { @helper (self) -> int = ... }

// OK: extending foreign type with local trait bound
extend [T] where T: MyLocalTrait { @special (self) -> int = ... }

// ERROR: extending foreign type without local trait
extend std.collections.HashMap { @my_method (self) -> int = ... }
```

### Rationale

The orphan rule prevents:
- Global method pollution
- Conflicting extensions in different packages
- Surprising behavior from transitive dependencies

---

## Scoping

### File-Level Scope

Extension imports are scoped to the file where they appear:

```ori
// file_a.ori
extension std.iter.extensions { Iterator.count }
items.iter().count()  // OK

// file_b.ori
items.iter().count()  // ERROR: count not imported
```

### No Transitive Export

Extension imports do not propagate through re-exports:

```ori
// lib.ori
extension std.iter.extensions { Iterator.count }
pub use "./helper" { process }  // count not re-exported

// main.ori
use "my_lib" { process }
items.iter().count()  // ERROR: count not available
```

To make an extension available, re-export the extension:

```ori
// lib.ori
pub extension std.iter.extensions { Iterator.count }
```

---

## Generic Extensions

### Type Parameter Extensions

```ori
extend<T: Clone> [T] {
    @duplicate_all (self) -> [T] = self.map(transform: x -> x.clone())
}
```

### Associated Type Constraints

```ori
extend Iterator where Self.Item: Printable {
    @print_all (self) -> void = for item in self do print(msg: item.to_str())
}
```

---

## Examples

### Standard Library Pattern

```ori
// std/iter/extensions.ori
pub extend Iterator {
    @count (self) -> int = ...
    @last (self) -> Option<Self.Item> = ...
    @nth (self, n: int) -> Option<Self.Item> = ...
}

pub extend DoubleEndedIterator {
    @rfind (self, predicate: (Self.Item) -> bool) -> Option<Self.Item> = ...
}
```

### User Extension

```ori
// my_extensions.ori
pub extend str {
    @word_count (self) -> int = self.split(sep: " ").count()
}

// main.ori
extension "./my_extensions" { str.word_count }

let text = "hello world example"
text.word_count()  // 3
```

---

## Error Messages

### Ambiguous Extension

```
error[E0850]: ambiguous extension method
  --> src/main.ori:10:5
   |
10 |     point.distance()
   |     ^^^^^^^^^^^^^^^^ `distance` is defined in multiple extensions
   |
   = note: candidates from:
           - extension "./ext_a" { Point.distance }
           - extension "./ext_b" { Point.distance }
   = help: use qualified syntax: ext_a.Point.distance(point)
```

### Extension Not Imported

```
error[E0851]: method `count` not found on `Iterator`
  --> src/main.ori:5:10
   |
 5 |     items.iter().count()
   |                  ^^^^^ method not found
   |
   = note: `count` is an extension method in `std.iter.extensions`
   = help: add `extension std.iter.extensions { Iterator.count }`
```

### Orphan Violation

```
error[E0852]: cannot define extension for foreign type
  --> src/ext.ori:1:1
   |
 1 | extend HashMap { ... }
   | ^^^^^^^^^^^^^^ `HashMap` is defined in `std.collections`
   |
   = note: extensions must be in the same package as the type
   = help: define a newtype wrapper or use a local trait bound
```

---

## Spec Changes Required

### Update `12-modules.md`

Expand Extensions section with:
1. Complete resolution order
2. Conflict resolution rules
3. Orphan rules
4. Scoping rules

### Update `08-declarations.md`

Add extension definition section with:
1. Syntax and semantics
2. Constraint syntax
3. Visibility rules

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Import style | Explicit, method-level granularity |
| Resolution order | Inherent > Trait > Extension |
| Conflicts | Compile error, use qualified syntax |
| Orphan rule | Same package as type or trait bound |
| Scope | File-level, no transitive propagation |
| Override | Not allowed |
