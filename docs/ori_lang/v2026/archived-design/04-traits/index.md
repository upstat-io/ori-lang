# Traits

This section covers Ori's trait system for behavior abstraction.

---

## Documents

| Document | Description |
|----------|-------------|
| [Trait Definitions](01-trait-definitions.md) | Defining traits with methods |
| [Implementations](02-implementations.md) | impl blocks, external impls, orphan rule |
| [Bounds and Constraints](03-bounds-and-constraints.md) | where clauses, multiple bounds |
| [Derive](04-derive.md) | Auto-deriving trait implementations |
| [Dynamic Dispatch](05-dynamic-dispatch.md) | dyn Trait for runtime polymorphism |
| [Trait Extensions](06-extensions.md) | Extend traits with additional methods |

---

## Overview

Traits define shared behavior without inheritance:

```ori
trait Printable {
    @to_string (self) -> str
}

impl Printable for User {
    @to_string (self) -> str = self.name + " <" + self.email + ">"
}
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| `trait` | Defines a set of method signatures |
| `impl` | Provides trait methods for a type |
| `where` | Constrains generic types to traits |
| `#[derive]` | Auto-generates trait implementations |
| `dyn Trait` | Runtime polymorphism (trait objects) |

### No Inheritance

Ori uses traits instead of class inheritance:

```ori
// NOT supported
type Admin extends User { ... }

// Use composition
type Admin = {
    user: User,
    permissions: [Permission]
}

// Share behavior via traits
impl Identifiable for User { ... }
impl Identifiable for Admin { ... }
```

### Traits for Extensibility

Use traits when library users need to add their own types.

**Sum types are closed** — all variants fixed at definition:
```ori
// Users CANNOT add variants
type Status = Pending | Running | Done
```

**Traits are open** — anyone can implement:
```ori
// Library defines trait
trait Widget {
    @render (self, ctx: Context) -> void
}

// Library provides implementations
impl Widget for Button { ... }
impl Widget for Slider { ... }

// Users add their own
impl Widget for MyCustomChart { ... }
```

**Guideline:** If you're writing a library and want users to extend your abstraction, use a trait. Use sum types only for truly closed sets.

See [Sum Types vs Traits](../03-type-system/03-user-defined-types.md#sum-types-vs-traits-the-expression-problem) for detailed guidance.

---

## See Also

- [Main Index](../00-index.md)
- [Type System](../03-type-system/index.md)
- [Compositional Model](../03-type-system/06-compositional-model.md)
