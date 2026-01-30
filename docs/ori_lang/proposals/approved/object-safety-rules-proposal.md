# Proposal: Object Safety Rules

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-30
**Affects:** Compiler, type system, trait objects

---

## Summary

This proposal formally specifies which traits can be used as trait objects, defining the object safety rules that the compiler enforces.

---

## Problem Statement

The spec mentions that "traits with methods returning `Self` may not be object-safe" but doesn't provide:

1. Complete list of object safety requirements
2. Rationale for each requirement
3. Error messages for violations
4. Workarounds for common patterns

---

## What is Object Safety?

### Trait Objects

A trait object (`Trait` as a type) allows dynamic dispatch — the concrete type is unknown at compile time:

```ori
@process (items: [Printable]) -> void =  // Printable is a trait object
    for item in items do print(msg: item.to_str())
```

### Object Safety

A trait is **object-safe** if it can be used as a trait object. Not all traits qualify — some require knowing the concrete type, which defeats the purpose of dynamic dispatch.

---

## Object Safety Rules

A trait is object-safe if ALL of the following are true:

### Rule 1: No `Self` in Return Position

Methods cannot return `Self`:

```ori
// NOT object-safe: returns Self
trait Clone {
    @clone (self) -> Self  // Self = concrete type, unknown for trait objects
}

// Object-safe: returns fixed type
trait Printable {
    @to_str (self) -> str  // str is concrete, always known
}
```

**Rationale**: With a trait object, the compiler doesn't know the concrete type to allocate for the return value.

### Rule 2: No `Self` in Parameter Position (Except Receiver)

Methods cannot take `Self` as a parameter (except for the first `self` receiver):

```ori
// NOT object-safe: Self as parameter
trait Eq {
    @equals (self, other: Self) -> bool  // What is other's type?
}

// Object-safe: takes trait object
trait Comparable {
    @compare (self, other: Comparable) -> Ordering  // Takes trait object
}
```

**Rationale**: With a trait object, we can't verify that `other` has the same concrete type as `self`.

### Rule 3: No Generic Methods

Methods cannot have type parameters:

```ori
// NOT object-safe: generic method
trait Converter {
    @convert<T> (self) -> T  // What's T at runtime?
}

// Object-safe: no generics
trait Formatter {
    @format (self, spec: FormatSpec) -> str
}
```

**Rationale**: Generic methods are monomorphized at compile time, but trait objects defer type information to runtime.

---

## Object-Safe Traits in the Standard Library

### Object-Safe

| Trait | Why Safe |
|-------|----------|
| `Printable` | Returns `str`, not `Self` |
| `Formattable` | Returns `str`, not `Self` |
| `Debug` | Returns `str`, not `Self` |
| `Hashable` | Returns `int`, no `Self` params |

### Not Object-Safe

| Trait | Why Unsafe | Workaround |
|-------|------------|------------|
| `Clone` | Returns `Self` | Use `Arc<CloneArc>` wrapper |
| `Default` | Returns `Self` | Use factory function returning `Arc<Trait>` |
| `Eq` | `Self` as parameter | Use trait object parameter type |
| `Comparable` | `Self` as parameter | Use trait object parameter type |
| `Iterator` | Returns `Self` in `next()` | Design-dependent |
| `Collect` | Returns `Self` | Use specific collection types |

---

## Making Traits Object-Safe

### Pattern: Object-Safe Wrapper

Create an object-safe version of a non-safe trait:

```ori
// Original non-object-safe trait
trait Clone {
    @clone (self) -> Self
}

// Object-safe wrapper
trait CloneArc {
    @clone_arc (self) -> Arc<CloneArc>
}

impl<T: Clone> CloneArc for T {
    @clone_arc (self) -> Arc<CloneArc> = Arc(self.clone())
}
```

### Pattern: Remove Self from Signature

```ori
// NOT object-safe
trait Mergeable {
    @merge (self, other: Self) -> Self
}

// Object-safe alternative
trait MergeableObj {
    @merge_with (self, other: MergeableObj) -> Arc<MergeableObj>
}
```

### Pattern: Use Trait Object as Parameter

```ori
// NOT object-safe
trait Eq {
    @equals (self, other: Self) -> bool
}

// Object-safe (but loses type safety)
trait EqDyn {
    @equals_any (self, other: EqDyn) -> bool
    // Implementation must handle type mismatches
}
```

---

## Compiler Error Messages

### Self in Return Position

```
error[E0800]: trait `Clone` cannot be made into an object
  --> src/main.ori:5:1
   |
5  | @process (items: [Clone]) -> void
   |                   ^^^^^ the trait `Clone` is not object-safe
   |
   = note: method `clone` returns `Self` which has unknown size
   = help: consider using a wrapper type that returns an Arc-wrapped trait object
```

### Self as Parameter

```
error[E0801]: trait `Eq` cannot be made into an object
  --> src/main.ori:10:1
   |
10 | @compare_all (items: [Eq]) -> bool
   |                       ^^ the trait `Eq` is not object-safe
   |
   = note: method `equals` takes `Self` as a parameter
   = help: consider using a trait object parameter type instead of `Self`
```

### Generic Method

```
error[E0802]: trait `Converter` cannot be made into an object
  --> src/main.ori:15:1
   |
15 | let converters: [Converter] = ...
   |                  ^^^^^^^^^ the trait `Converter` is not object-safe
   |
   = note: method `convert` has generic type parameters
   = help: consider removing the generic parameter and using a specific type
```

---

## Object Safety and Trait Bounds

### Bounded Trait Objects

Trait objects can have additional bounds:

```ori
// Trait object that is both Printable and Hashable
@store (item: Printable + Hashable) -> void
```

All component traits must be object-safe.

### Where Clauses with Trait Objects

```ori
@process<T> (item: T) where T: Printable + Hashable = ...
// T is generic, not trait object — different rules apply

@process_dyn (item: Printable + Hashable) = ...
// Trait object — object safety required
```

---

## Trait Objects vs Generics

| Aspect | Trait Object | Generic |
|--------|--------------|---------|
| Type known at | Runtime | Compile time |
| Code size | One implementation | Per-type instantiation |
| Performance | Virtual dispatch | Direct calls (inlinable) |
| Object safety | Required | Not required |
| Use case | Heterogeneous collections | Homogeneous, performance-critical |

### Choosing Between Them

```ori
// Trait object: heterogeneous collection
@draw_all (shapes: [Drawable]) -> void =
    for shape in shapes do shape.draw()
// shapes can contain Circle, Rectangle, Triangle mixed

// Generic: homogeneous, fast
@draw_all<T: Drawable> (shapes: [T]) -> void =
    for shape in shapes do shape.draw()
// All shapes must be same concrete type
```

---

## Examples

### Object-Safe Trait Design

```ori
// Object-safe logging trait
trait Logger {
    @log (self, level: Level, message: str) -> void
    @is_enabled (self, level: Level) -> bool
}

// Can use as trait object
@with_logger (logger: Logger, action: () -> void) -> void = run(
    logger.log(level: Level.Info, message: "Starting"),
    action(),
    logger.log(level: Level.Info, message: "Complete"),
)
```

### Non-Object-Safe with Workaround

```ori
// Non-object-safe: returns Self
trait Builder {
    @with_option (self, opt: Option) -> Self
    @build (self) -> Product
}

// Object-safe wrapper
trait BuilderObj {
    @with_option_dyn (self, opt: Option) -> Arc<BuilderObj>
    @build (self) -> Product
}

impl<B: Builder> BuilderObj for B {
    @with_option_dyn (self, opt: Option) -> Arc<BuilderObj> =
        Arc(self.with_option(opt: opt))
    @build (self) -> Product = self.build()
}
```

---

## Spec Changes Required

### Update `06-types.md`

Add comprehensive object safety section:
1. Definition of trait objects
2. All three object safety rules
3. Examples of safe and unsafe traits

### Update `08-declarations.md`

Add guidance on:
1. Designing object-safe traits
2. Wrapper patterns for unsafe traits
3. When to use trait objects vs generics

### Add Diagnostics

Define error codes:
- `E0800`: Self in return position
- `E0801`: Self as non-receiver parameter
- `E0802`: Generic method in trait

---

## Summary

| Rule | Violation | Reason |
|------|-----------|--------|
| No `Self` return | `@clone (self) -> Self` | Unknown size at runtime |
| No `Self` param | `@eq (self, other: Self)` | Can't verify type match |
| No generics | `@convert<T> (self) -> T` | Requires monomorphization |
