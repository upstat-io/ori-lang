# Proposal: Trait Resolution and Conflict Handling

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-30
**Affects:** Compiler, type system, trait system

---

## Summary

This proposal specifies rules for resolving trait implementation conflicts, including the diamond problem in trait inheritance, conflicting default implementations, coherence rules (orphan rules), and extension method conflicts.

---

## Problem Statement

The spec defines traits and implementations but doesn't address:

1. **Diamond problem**: What happens when a type inherits the same trait through multiple paths?
2. **Conflicting defaults**: When multiple trait bounds provide different default implementations
3. **Coherence rules**: Who can implement which traits for which types?
4. **Extension conflicts**: When extension methods conflict with inherent methods
5. **Super trait calls**: Can implementations call parent trait methods?

---

## Trait Inheritance and Diamond Problem

### Diamond Scenario

```ori
trait A { @method (self) -> int }
trait B: A { }
trait C: A { }
trait D: B + C { }  // D inherits A through both B and C
```

### Resolution Rule

**Single Implementation**: A type implementing `D` provides ONE implementation of `A.method`. There is no duplication or conflict because traits define interface, not implementation.

```ori
impl D for MyType {
    @method (self) -> int = 42  // Single implementation satisfies A via B and C
}
```

### Conflicting Default Implementations

If both `B` and `C` override `A`'s default:

```ori
trait A { @method (self) -> int = 0 }
trait B: A { @method (self) -> int = 1 }
trait C: A { @method (self) -> int = 2 }
trait D: B + C { }

impl D for MyType { }  // ERROR: ambiguous default for @method
```

**Resolution**: The implementing type MUST provide an explicit implementation when defaults conflict.

```ori
impl D for MyType {
    @method (self) -> int = 3  // Explicit implementation resolves ambiguity
}
```

---

## Coherence Rules (Orphan Rules)

### Definition

**Coherence** ensures that for any type `T` and trait `Trait`, there is at most one implementation of `Trait for T` visible in any compilation unit.

### Orphan Rules

An implementation `impl Trait for Type` is allowed only if **at least one** of these is true:

1. `Trait` is defined in the current module
2. `Type` is defined in the current module
3. `Type` is a generic parameter constrained in the current module

```ori
// In module my_app

// OK: Type is local
type MyType = { ... }
impl ExternalTrait for MyType { }

// OK: Trait is local
trait MyTrait { ... }
impl MyTrait for ExternalType { }

// ERROR: Both trait and type are external (orphan)
impl std.Display for std.Vec { }  // Error: orphan implementation
```

### Blanket Implementations

Blanket implementations (`impl<T> Trait for T where ...`) follow the same rules:

```ori
// OK in std library: both Printable and From<T> are std traits
impl<T: Printable> From<T> for str { }

// ERROR in user code: cannot add blanket impl for external trait
impl<T> ExternalTrait for T { }  // Error: orphan blanket
```

### Rationale

Orphan rules prevent:
- Conflicting implementations from different libraries
- "Spooky action at a distance" where importing a module changes behavior
- Unpredictable trait resolution based on import order

---

## Trait Method Resolution Order

### Method Lookup Priority

When calling `value.method()`:

1. **Inherent methods** — methods in `impl Type { }` (not trait impl)
2. **Trait methods from explicit bounds** — methods from `where T: Trait`
3. **Trait methods from in-scope traits** — traits imported into current scope
4. **Extension methods** — methods added via `extend`

```ori
type Foo = { }

impl Foo {
    @method (self) -> int = 1  // Priority 1: inherent
}

trait Bar { @method (self) -> int }
impl Bar for Foo {
    @method (self) -> int = 2  // Priority 3: trait (if Bar in scope)
}

extend Baz {
    @method (self) -> int = 3  // Priority 4: extension
}

let x = Foo { }
x.method()  // Returns 1 (inherent wins)
```

### Ambiguity Resolution

If multiple traits provide the same method and none are inherent:

```ori
trait A { @method (self) -> int }
trait B { @method (self) -> int }

impl A for Foo { @method (self) -> int = 1 }
impl B for Foo { @method (self) -> int = 2 }

let x: Foo = ...
x.method()  // ERROR: ambiguous method call
```

**Resolution**: Use fully-qualified syntax:

```ori
A.method(x)  // Calls A's implementation
B.method(x)  // Calls B's implementation
```

---

## Super Trait Method Calls

### Calling Parent Default

An implementation can call the parent trait's default implementation using `Trait.method(self)`:

```ori
trait Parent {
    @method (self) -> int = 10
}

trait Child: Parent {
    @method (self) -> int = Parent.method(self) + 1
}
```

### Calling in Impl Override

The same syntax works when overriding in `impl`:

```ori
impl Parent for MyType {
    @method (self) -> int = Parent.method(self) * 2
}
```

`Parent.method(self)` always calls the trait's default implementation, regardless of whether it's used in a trait definition or an impl block.

---

## Extension Method Conflicts

### Extension vs Inherent

Inherent methods always win over extensions:

```ori
impl str {
    @trim (self) -> str = ...  // Inherent
}

extend str {
    @trim (self) -> str = ...  // Extension - never called
}

"hello".trim()  // Calls inherent
```

### Extension vs Extension

When multiple extensions provide the same method:

```ori
// In module a
extend Iterator {
    @sum (self) -> int = ...
}

// In module b
extend Iterator {
    @sum (self) -> int = ...
}

// In module c
extension "a" { Iterator.sum }
extension "b" { Iterator.sum }  // ERROR: conflicting extension imports
```

Conflicts are detected based on what is *in scope*, not just explicit `extension` statements. Re-exported extensions also conflict:

```ori
// In module c
use "./utils" { sum_extension }    // re-exports a.Iterator.sum
extension "b" { Iterator.sum }      // ERROR: Iterator.sum already in scope
```

**Resolution**: Only one extension for a given method may be in scope.

---

## Associated Type Constraints

### Constraint Syntax

```ori
trait Container {
    type Item
}

@process<C: Container> (c: C) -> C.Item where C.Item: Clone = ...
```

### Associated Type Disambiguation

When a type implements multiple traits with same-named associated types, use qualified paths to disambiguate:

```ori
trait A { type Item }
trait B { type Item }

// Qualified path syntax: Type::Trait::AssocType
@f<C: A + B> (c: C) where C::A::Item: Clone = ...

// To require both Items to be the same type:
@g<C: A + B> (c: C) where C::A::Item == C::B::Item, C::A::Item: Clone = ...
```

If a type implements both `A` and `B`, it has two distinct associated types:
- `C::A::Item` — the Item from trait A
- `C::B::Item` — the Item from trait B

Without qualification, `C.Item` is ambiguous when multiple traits define `Item`.

---

## Implementation Priority

### Specificity Rules

When multiple impls could apply, more specific wins:

```ori
impl<T> Trait for T { }          // Generic blanket
impl<T: Clone> Trait for T { }   // Constrained blanket (more specific)
impl Trait for MyType { }         // Concrete (most specific)
```

For `MyType`:
- If `MyType: Clone`, concrete impl wins
- Concrete always beats blanket

### No Overlap Guarantee

The compiler ensures no two applicable impls have equal specificity:

```ori
impl<T: A> Trait for T { }
impl<T: B> Trait for T { }

type Foo = { }
impl A for Foo { }
impl B for Foo { }

// ERROR at impl sites: overlapping implementations
// If Foo: A + B, which impl of Trait applies?
```

---

## Error Messages

### Conflicting Implementations

```
error[E0600]: conflicting implementations of trait `Display`
  --> src/main.ori:10:1
   |
5  | impl Display for MyType { ... }
   | ------------------------------ first implementation here
...
10 | impl Display for MyType { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ conflicting implementation
```

### Orphan Implementation

```
error[E0601]: orphan implementation
  --> src/main.ori:5:1
   |
5  | impl std.Display for std.Vec { }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: implement a local trait for external type, or a trait for local type
   = note: this restriction prevents conflicting implementations across crates
```

### Ambiguous Method

```
error[E0602]: ambiguous method call
  --> src/main.ori:15:5
   |
15 |     x.method()
   |       ^^^^^^ method found in multiple traits
   |
   = note: candidate #1: `A.method` from trait `A`
   = note: candidate #2: `B.method` from trait `B`
   = help: use fully-qualified syntax: `A.method(x)` or `B.method(x)`
```

### Conflicting Extensions

```
error[E0603]: conflicting extension methods
  --> src/main.ori:8:1
   |
5  | extension "a" { Iterator.sum }
   | ------------------------------ Iterator.sum first imported here
...
8  | extension "b" { Iterator.sum }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ conflicting extension import
   |
   = help: only one extension for a given method may be in scope
```

---

## Spec Changes Required

### Update `08-declarations.md`

Add sections for:

1. **Coherence rules**
   - Orphan rules
   - Blanket implementation restrictions
   - Overlap detection

2. **Trait inheritance resolution**
   - Diamond problem handling
   - Conflicting default implementations

3. **Method resolution order**
   - Inherent > Trait > Extension priority
   - Ambiguity resolution with fully-qualified syntax

4. **Super trait method calls**
   - `Trait.method(self)` syntax

5. **Associated type disambiguation**
   - Qualified path syntax `Type::Trait::AssocType`

---

## Summary

| Aspect | Rule |
|--------|------|
| Diamond problem | Single implementation satisfies all paths |
| Conflicting defaults | Explicit impl required |
| Orphan rule | Trait or type must be local |
| Resolution order | Inherent > Trait > Extension |
| Ambiguous methods | Fully-qualified syntax required |
| Super calls | `Trait.method(self)` |
| Associated types | `Type::Trait::AssocType` for disambiguation |
| Extension conflicts | Only one per method in scope (includes re-exports) |
| Overlapping impls | Compile error |
| Specificity | Concrete > Constrained > Generic |
