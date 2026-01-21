# Compositional Type Model

This document describes Sigil's type model: no subtyping, no inheritance, traits for behavior, explicit polymorphism.

---

## Core Principle

**Types are exact.** A value has one type, and that type is known precisely. There are no subtype relationships between user-defined types.

---

## No Subtyping

### Types Match or They Don't

```sigil
type Dog = { name: str, breed: str }
type Cat = { name: str, color: str }

// Dog is NOT a subtype of anything
// Cat is NOT a subtype of anything
// They are simply different types
```

### No Structural Subtyping

Having the same fields doesn't make types compatible:

```sigil
type Point2D = { x: int, y: int }
type Vector2D = { x: int, y: int }

// Point2D != Vector2D, even with identical structure
@move_point (p: Point2D) -> Point2D = ...
@scale_vector (v: Vector2D) -> Vector2D = ...

point = Point2D { x: 1, y: 2 }
vector = Vector2D { x: 1, y: 2 }

move_point(vector)  // ERROR: Vector2D != Point2D
```

### No Inheritance

```sigil
// NOT supported
type Animal = { name: str }
type Dog = Animal + { breed: str }  // ERROR

// Instead, use composition
type Dog = {
    name: str,
    breed: str
}
```

---

## Why No Subtyping?

### Simplicity

Traditional subtyping questions:
- "Is `[Dog]` a subtype of `[Animal]`?" (covariance)
- "Is `(Animal) -> void` a subtype of `(Dog) -> void`?" (contravariance)
- "What about mutable references?" (invariance)

Sigil's answer: **Types don't have subtype relationships.**

### AI-Friendliness

Subtyping rules are complex and error-prone:
- Even experienced programmers make mistakes
- AI would generate subtyping errors constantly
- Debugging requires understanding variance rules

Sigil's simple rule: "Types match or use `dyn`."

---

## Behavior via Traits

Shared behavior comes from traits, not type relationships:

```sigil
trait Named {
    @name (self) -> str
}

trait Speaks {
    @speak (self) -> str
}

type Dog = { name: str, breed: str }
type Cat = { name: str, color: str }

impl Named for Dog {
    @name (self) -> str = self.name
}

impl Named for Cat {
    @name (self) -> str = self.name
}

impl Speaks for Dog {
    @speak (self) -> str = "woof"
}

impl Speaks for Cat {
    @speak (self) -> str = "meow"
}
```

Types don't inherit—they implement traits independently.

---

## Explicit Polymorphism

Sigil has two forms of polymorphism, both explicit.

### Static Polymorphism (Generics)

```sigil
@greet<T> (pet: T) -> str where T: Named =
    "Hello, " + pet.name()

// Compiler generates specialized versions:
greet(dog)  // generates greet_Dog
greet(cat)  // generates greet_Cat
```

### Dynamic Polymorphism (Trait Objects)

```sigil
@greet_any (pet: dyn Named) -> str =
    "Hello, " + pet.name()

// Single function, runtime dispatch via vtable:
greet_any(dog)  // boxes Dog, calls via vtable
greet_any(cat)  // boxes Cat, calls via vtable
```

### Choosing Between Them

| Static (`<T>`) | Dynamic (`dyn`) |
|----------------|-----------------|
| Zero runtime cost | Runtime dispatch cost |
| Larger binary | Smaller binary |
| Homogeneous collections | Heterogeneous collections |
| Compile-time resolution | Runtime resolution |

---

## Heterogeneous Collections

### Homogeneous (Default)

```sigil
// Same type, no overhead
dogs: [Dog] = [dog1, dog2, dog3]
```

### Heterogeneous (Explicit `dyn`)

```sigil
// Different types, requires dyn
pets: [dyn Named] = [dog1, cat1, dog2]
```

### Cannot Mix Without `dyn`

```sigil
// ERROR: what's the element type?
mixed = [dog1, cat1]

// OK: explicit dyn
mixed: [dyn Named] = [dog1, cat1]
```

---

## The Never Type

`Never` is the one exception—the bottom type:

```sigil
// Never is the type of expressions that don't return
@panic (msg: str) -> Never = ...
@infinite_loop () -> Never = ...

// Never safely coerces to any type
@get_or_panic (opt: Option<int>) -> int = match(opt,
    Some(n) -> n,
    None -> panic("required")  // Never → int
)
```

Why this exception:
- `panic()` needs a type
- It should work in any context (since it never returns)
- Doesn't compromise the model (Never never exists at runtime)

---

## No Implicit Conversions

All type conversions are explicit:

```sigil
// No implicit numeric conversion
x: int = 5
y: float = x        // ERROR

y: float = float(x)  // OK: explicit

// No implicit trait object conversion
dog: Dog = ...
pet: dyn Named = dog  // ERROR (without context)

pet: dyn Named = dog as dyn Named  // OK: explicit
```

When `dyn` is expected, conversion is implicit:

```sigil
@greet (p: dyn Named) -> str = ...
greet(dog)  // OK: dyn Named expected in signature
```

---

## No Variance

Variance doesn't apply because types are exact:

```sigil
// Traditional question: variance of List<T>?
// - Covariant: [Dog] <: [Animal]?
// - Contravariant: [Animal] <: [Dog]?
// - Invariant: neither?

// Sigil: not applicable
// [Dog] is [Dog]. [Cat] is [Cat]. They're unrelated.
```

What this eliminates:
- Covariance rules
- Contravariance rules
- Invariance rules
- Variance annotations (`in`, `out`, `inout`)
- Complex generic subtyping

---

## Type Assertions

When you need to recover a concrete type from `dyn`:

### Checked Downcast

```sigil
@process (n: dyn Named) -> void = run(
    if is_type(n, Dog) then
        dog = n as Dog  // runtime checked
        print(dog.breed)
)
```

### Unchecked Downcast

```sigil
// Panics if wrong type
dog = some_dyn as! Dog
```

---

## Compositional Patterns

Build complex behavior by composing traits:

```sigil
trait Named { @name (self) -> str }
trait Speaks { @speak (self) -> str }
trait Flies { @fly (self) -> void }
trait Swims { @swim (self) -> void }

type Dog = { name: str, breed: str }
type Parrot = { name: str, color: str }
type Duck = { name: str }

// Dog: Named + Speaks + Swims
impl Named for Dog { ... }
impl Speaks for Dog { ... }
impl Swims for Dog { ... }

// Parrot: Named + Speaks + Flies
impl Named for Parrot { ... }
impl Speaks for Parrot { ... }
impl Flies for Parrot { ... }

// Duck: Named + Speaks + Flies + Swims
impl Named for Duck { ... }
impl Speaks for Duck { ... }
impl Flies for Duck { ... }
impl Swims for Duck { ... }

// Function requiring multiple capabilities
@describe<T> (x: T) -> str where T: Named + Speaks =
    x.name() + " says " + x.speak()
```

Benefits:
- No diamond problem
- Clear what capabilities each type has
- Add capabilities without changing type definitions

---

## Mental Model

**For AI and humans:**

> Types are exact. For shared behavior, use traits. For polymorphism, use generics (static) or `dyn` (dynamic). That's it.

---

## See Also

- [Traits](../04-traits/index.md)
- [Generics](04-generics.md)
- [Type Inference](05-type-inference.md)
