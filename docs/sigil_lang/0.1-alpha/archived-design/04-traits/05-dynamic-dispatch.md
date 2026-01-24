# Dynamic Dispatch

This document covers `dyn Trait` for runtime polymorphism.

---

## Static vs Dynamic Dispatch

### Static Dispatch (Generics)

```sigil
@greet<T> (pet: T) -> str where T: Named =
    "Hello, " + pet.name()
```

- Compile-time resolution
- Generates specialized code for each type
- Zero runtime overhead
- Larger binary

### Dynamic Dispatch (`dyn`)

```sigil
@greet (pet: dyn Named) -> str =
    "Hello, " + pet.name()
```

- Runtime resolution via vtable
- Single function handles all types
- Runtime overhead
- Smaller binary

---

## Syntax

```sigil
dyn TraitName
```

### In Parameters

```sigil
@process (item: dyn Processable) -> void = ...
```

### In Collections

```sigil
items: [dyn Named] = [dog, cat, bird]
```

### In Return Types

```sigil
@create_pet (kind: str) -> dyn Named = ...
```

---

## When to Use `dyn`

### Heterogeneous Collections

When you need a collection of different types:

```sigil
// Without dyn: can't mix types
// only dogs
dogs: [Dog] = [dog1, dog2, dog3]

// With dyn: can mix any Named type
// mixed types
pets: [dyn Named] = [dog1, cat1, bird1]
```

### Unknown Types at Compile Time

```sigil
@load_plugin (path: str) -> dyn Plugin = ...
// The actual type depends on what's in the plugin file
```

### Reducing Binary Size

When many types implement a trait and binary size matters:

```sigil
// Static: generates process_TypeA, process_TypeB, process_TypeC, ...
@process<T> (item: T) -> void where T: Processable = ...

// Dynamic: single function, smaller binary
@process (item: dyn Processable) -> void = ...
```

---

## How It Works

### Trait Objects

A `dyn Trait` value is a "trait object" containing:
1. A pointer to the data
2. A pointer to a vtable (virtual method table)

### Vtable

The vtable contains pointers to the trait's methods for that specific type:

```
┌─────────────────┐
│   dyn Named     │
├─────────────────┤
│ data_ptr ───────┼──> actual Dog/Cat/etc data
│ vtable_ptr ─────┼──> ┌──────────────┐
└─────────────────┘    │ name() ──────┼──> Dog.name or Cat.name
                       └──────────────┘
```

---

## Conversion to `dyn`

### Implicit (In Context)

When a function expects `dyn Trait`, conversion is automatic:

```sigil
@greet (pet: dyn Named) -> str = ...

dog: Dog = ...
// Dog automatically converted to dyn Named
greet(dog)
```

### Explicit

```sigil
pet: dyn Named = dog as dyn Named
```

---

## Conversion from `dyn`

Going from `dyn` back to a concrete type requires explicit casts:

### Checked Downcast

```sigil
@process (pet: dyn Named) -> void =
    if is_type(pet, Dog) then run(
        // runtime checked, safe
        let dog = pet as Dog,
        print(dog.breed),
    )
    else void
```

### Unchecked Downcast

```sigil
// Panics if wrong type
dog = pet as! Dog
```

---

## Limitations

### Object Safety

Not all traits can be used with `dyn`. A trait is "object-safe" if:

1. No `Self` in method signatures (except as receiver)
2. No generic methods
3. All methods have `self` parameter

```sigil
// Object-safe
trait Named {
    @name (self) -> str
}

// NOT object-safe: returns Self
trait Clone {
    // can't use with dyn
    @clone (self) -> Self
}

// NOT object-safe: generic method
trait Converter {
    // can't use with dyn
    @convert<T> (self) -> T
}
```

### No Associated Types with `dyn`

```sigil
trait Iterator {
    type Item
    @next (self) -> Option<Self.Item>
}

// ERROR: can't use dyn Iterator directly
// What is Item?
items: [dyn Iterator]

// Must specify the associated type
// OK
items: [dyn Iterator<Item = int>]
```

---

## Performance Considerations

### Overhead

- Extra indirection through vtable
- Prevents inlining
- Can't optimize for specific types

### When It Matters

```sigil
// Hot loop with many iterations: prefer static
@process_millions<T> (items: [T]) -> void where T: Processable = ...

// Called infrequently: dyn is fine
@handle_event (event: dyn Event) -> void = ...
```

### Measurement

Profile before assuming `dyn` is a bottleneck. The flexibility often outweighs the cost.

---

## Combining Static and Dynamic

```sigil
// Static dispatch within a function
@process_one<T> (item: T) -> str where T: Named = item.name()

// Dynamic dispatch for collection
@process_all (items: [dyn Named]) -> [str] =
    map(.over: items, .transform: item -> item.name())
```

---

## Multiple Traits

### Multiple Bounds

```sigil
// Require multiple traits
@describe (entity: dyn Named + Speaks) -> str =
    entity.name() + " says " + entity.speak()
```

### Creating Multi-Trait Objects

```sigil
// Type that implements both
type Dog = { ... }
impl Named for Dog { ... }
impl Speaks for Dog { ... }

// Convert to multi-trait object
pet: dyn Named + Speaks = dog
```

---

## Best Practices

### Default to Static

Use generics by default. Only use `dyn` when needed:

```sigil
// Prefer this
@process<T> (item: T) -> void where T: Processable = ...

// Use dyn only when necessary
@process (item: dyn Processable) -> void = ...
```

### Document Dynamic Usage

```sigil
// #Processes heterogeneous items
// Uses dynamic dispatch for flexibility
@process_mixed (items: [dyn Processable]) -> void = ...
```

### Minimize Downcasting

If you're frequently downcasting, consider if `dyn` is the right choice.

---

## See Also

- [Trait Definitions](01-trait-definitions.md)
- [Bounds and Constraints](03-bounds-and-constraints.md)
- [Compositional Model](../03-type-system/06-compositional-model.md)
