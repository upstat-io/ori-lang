---
title: "Types"
description: "Ori Language Specification — Types"
order: 6
section: "Types & Values"
---

# Types

Every value has a type determined at compile time.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § TYPES

## Primitive Types

| Type | Description | Default |
|------|-------------|---------|
| `int` | 64-bit signed integer | `0` |
| `float` | 64-bit IEEE 754 | `0.0` |
| `bool` | `true` or `false` | `false` |
| `str` | UTF-8 string | `""` |
| `byte` | 8-bit unsigned | `0` |
| `char` | Unicode scalar value (U+0000–U+10FFFF, excluding surrogates) | — |
| `void` | Unit type, alias for `()` | `()` |
| `Never` | Bottom type, uninhabited | — |
| `Duration` | Time span (nanoseconds) | — |
| `Size` | Byte count | — |

`Never` is the return type for functions that never return (panic, infinite loop). Coerces to any type.

## Compound Types

### List

```
[T]
```

Ordered, homogeneous collection. Heap-allocated with dynamic size.

### Fixed-Capacity List

```
[T, max N]
```

Ordered, homogeneous collection with compile-time maximum capacity `N`. Stored inline (not heap-allocated). Length is dynamic at runtime (0 to N elements).

`N` must be a compile-time constant: a positive integer literal or a `$` constant binding.

```ori
let buffer: [int, max 10] = []      // Empty, capacity 10
let coords: [int, max 3] = [1, 2, 3] // Full, capacity 3
```

**Subtype relationship:** `[T, max N]` is a subtype of `[T]`. A fixed-capacity list can be passed where a dynamic list is expected. The capacity limit is retained even when viewed as `[T]`.

**Methods:**

| Method | Return | Description |
|--------|--------|-------------|
| `.capacity()` | `int` | Compile-time capacity N |
| `.is_full()` | `bool` | `len(self) == capacity` |
| `.remaining()` | `int` | `capacity - len(self)` |
| `.push(item: T)` | `void` | Add element; panic if full |
| `.try_push(item: T)` | `bool` | Add element; return false if full |
| `.push_or_drop(item: T)` | `void` | Drop item if full |
| `.push_or_oldest(item: T)` | `void` | Remove index 0 if full, push to end |
| `.to_dynamic()` | `[T]` | Convert to heap-allocated list |

**Conversion from dynamic list:**

```ori
let dynamic: [int] = [1, 2, 3]
let fixed: [int, max 10] = dynamic.to_fixed<10>()      // Panic if len > 10
let maybe: Option<[int, max 10]> = dynamic.try_to_fixed<10>()
```

**Trait implementations:** Fixed-capacity lists implement the same traits as regular lists (`Eq`, `Hashable`, `Comparable`, `Clone`, `Debug`, `Printable`, `Sendable`, `Iterable`, `DoubleEndedIterator`, `Collect`) with the same constraints.

### Map

```
{K: V}
```

Key-value pairs. Keys must implement `Eq` and `Hashable`.

### Set

```
Set<T>
```

Unordered unique elements. Elements must implement `Eq` and `Hashable`.

### Tuple

```
(T1, T2, ...)
()
```

Fixed-size heterogeneous collection. `()` is the unit value.

### Function

```
(T1, T2) -> R
```

### Range

```
Range<T>
```

Produced by `..` (exclusive) and `..=` (inclusive). Bounds must be `Comparable`.

```ori
0..10       // 0 to 9
0..=10      // 0 to 10
```

## Generic Types

Type parameters in angle brackets:

```ori
Option<int>
Result<User, Error>
type Pair<T> = { first: T, second: T }
```

### Const Generic Parameters

A _const generic parameter_ is a compile-time constant value (not a type) that parameterizes a type or function. Const generic parameters use the `$` sigil followed by a type annotation:

```ori
@swap_ends<T, $N: int> (items: [T, max N]) -> [T, max N] = ...

type RingBuffer<T, $N: int> = {
    data: [T, max N],
    head: int,
    tail: int
}
```

**Allowed const types:** `int`, `bool`

Const generic parameters can be used wherever a compile-time constant is expected, including:
- Fixed-capacity list capacities: `[T, max N]`
- Const expressions in type positions
- Const bounds in where clauses

```ori
// Const bound in where clause
@non_empty_array<$N: int> () -> [int, max N]
    where N > 0
= ...
```

**Instance methods with const generics:**

```ori
// Conversion methods on [T]
[T].to_fixed<$N: int>() -> [T, max N]
[T].try_to_fixed<$N: int>() -> Option<[T, max N]>
```

## Built-in Types

```
type Option<T> = Some(T) | None
type Result<T, E> = Ok(T) | Err(E)
type Ordering = Less | Equal | Greater
type Error = { message: str, source: Option<Error> }  // trace field internal
type TraceEntry = { function: str, file: str, line: int, column: int }
type NurseryErrorMode = CancelRemaining | CollectAll | FailFast
```

## Channel Types

Role-based channel types enforce producer/consumer separation at compile time.

```ori
type Producer<T: Sendable>           // Can only send
type Consumer<T: Sendable>           // Can only receive
type CloneableProducer<T: Sendable>  // Producer that implements Clone
type CloneableConsumer<T: Sendable>  // Consumer that implements Clone
```

### Channel Constructors

```ori
// One-to-one (exclusive, fastest)
@channel<T: Sendable> (buffer: int) -> (Producer<T>, Consumer<T>)

// Fan-in (many-to-one, producer cloneable)
@channel_in<T: Sendable> (buffer: int) -> (CloneableProducer<T>, Consumer<T>)

// Fan-out (one-to-many, consumer cloneable)
@channel_out<T: Sendable> (buffer: int) -> (Producer<T>, CloneableConsumer<T>)

// Many-to-many (both cloneable)
@channel_all<T: Sendable> (buffer: int) -> (CloneableProducer<T>, CloneableConsumer<T>)
```

### Producer Methods

```ori
impl<T: Sendable> Producer<T> {
    @send (self, value: T) -> void uses Async  // Consumes value
    @close (self) -> void
    @is_closed (self) -> bool
}
```

Sending a value transfers ownership. The value cannot be used after send.

### Consumer Methods

```ori
impl<T: Sendable> Consumer<T> {
    @receive (self) -> Option<T> uses Async
    @is_closed (self) -> bool
}

impl<T: Sendable> Iterable for Consumer<T> {
    type Item = T
}
```

`receive` returns `None` when the channel is closed and empty.

### Cloneability

`CloneableProducer` and `CloneableConsumer` implement `Clone`. Regular `Producer` and `Consumer` do not.

```ori
let (p, c) = channel<int>(buffer: 10)
// p.clone()  // error: Producer<int> does not implement Clone

let (p, c) = channel_in<int>(buffer: 10)
let p2 = p.clone()  // OK: CloneableProducer implements Clone
```

## User-Defined Types

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS (type_def)

### Struct

```ori
type Point = { x: int, y: int }
```

### Sum Type

```ori
type Status = Pending | Running | Done | Failed(reason: str)
```

### Newtype

```ori
type UserId = int
```

Creates distinct nominal type.

### Derive

```ori
#derive(Eq, Hashable, Clone)]
type Point = { x: int, y: int }
```

Derivable: `Eq`, `Hashable`, `Comparable`, `Printable`, `Debug`, `Clone`, `Default`, `Serialize`, `Deserialize`.

## Nominal Typing

User-defined types are nominally typed. Identical structure does not imply same type.

## Trait Objects

A trait name used as a type represents "any value implementing this trait":

```ori
@display (item: Printable) -> void = print(item.to_str())

let items: [Printable] = [point, user, "hello"]
```

The compiler determines the dispatch mechanism. Users specify *what* (any Printable), not *how* (vtable vs monomorphization).

### Trait Object vs Generic Bound

| Syntax | Meaning |
|--------|---------|
| `item: Printable` | Any Printable value (trait object) |
| `<T: Printable> item: T` | Generic over Printable types |

Use trait objects for heterogeneous collections. Use generics when all elements share a concrete type.

### Object Safety

A trait is _object-safe_ if it can be used as a trait object. Not all traits qualify — some require compile-time type information that is unavailable for trait objects.

A trait is object-safe if ALL of the following rules are satisfied:

**Rule 1: No `Self` in Return Position**

Methods cannot return `Self`:

```ori
// NOT object-safe: returns Self
trait Clone {
    @clone (self) -> Self
}

// Object-safe: returns fixed type
trait Printable {
    @to_str (self) -> str
}
```

The compiler cannot determine the concrete return type size at runtime.

**Rule 2: No `Self` in Parameter Position (Except Receiver)**

Methods cannot take `Self` as a parameter (except for the first `self` receiver):

```ori
// NOT object-safe: Self as parameter
trait Eq {
    @equals (self, other: Self) -> bool
}

// Object-safe: takes trait object
trait EqDyn {
    @equals_any (self, other: EqDyn) -> bool
}
```

The compiler cannot verify that `other` has the same concrete type as `self`.

**Rule 3: No Generic Methods**

Methods cannot have type parameters:

```ori
// NOT object-safe: generic method
trait Converter {
    @convert<T> (self) -> T
}

// Object-safe: no generics
trait Formatter {
    @format (self, spec: FormatSpec) -> str
}
```

Generic methods require monomorphization at compile time, but trait objects defer type information to runtime.

**Bounded Trait Objects**

Trait objects can have additional bounds. All component traits must be object-safe:

```ori
@store (item: Printable + Hashable) -> void
```

**Error Codes**

- `E0800`: Self in return position
- `E0801`: Self as non-receiver parameter
- `E0802`: Generic method in trait

## Clone Trait

The `Clone` trait enables explicit value duplication:

```ori
trait Clone {
    @clone (self) -> Self
}
```

`Clone` creates an independent copy of a value. The clone operation:
- For value types: returns a copy of the value
- For reference types: allocates new memory with refcount 1
- Element-wise recursive: cloning a container clones each element via `.clone()`

After cloning, original and clone have independent reference counts. Modifying the clone does not affect the original.

### Standard Implementations

All primitive types implement `Clone`:

| Type | Implementation |
|------|----------------|
| `int`, `float`, `bool`, `str`, `char`, `byte` | Returns copy of self |
| `Duration`, `Size` | Returns copy of self |

Collections implement `Clone` when their element types implement `Clone`:

| Type | Constraint |
|------|------------|
| `[T]` | `T: Clone` |
| `{K: V}` | `K: Clone, V: Clone` |
| `Set<T>` | `T: Clone` |
| `Option<T>` | `T: Clone` |
| `Result<T, E>` | `T: Clone, E: Clone` |
| `(A, B, ...)` | All element types: Clone |

### Derivable

`Clone` is derivable for user-defined types when all fields implement `Clone`:

```ori
#derive(Clone)]
type Point = { x: int, y: int }
```

Derived implementation clones each field.

### Non-Cloneable Types

Some types do not implement `Clone`:
- Unique resources (file handles, network connections)
- Types with identity where duplicates would be semantically wrong

## Debug Trait

The `Debug` trait provides developer-facing string representation:

```ori
trait Debug {
    @debug (self) -> str
}
```

Unlike `Printable`, which is for user-facing display, `Debug` shows the complete internal structure and is always derivable.

```ori
#derive(Debug)]
type Point = { x: int, y: int }

Point { x: 1, y: 2 }.debug()  // "Point { x: 1, y: 2 }"
```

### Standard Implementations

All primitive types implement `Debug`:

| Type | Output Format |
|------|---------------|
| `int`, `float`, `byte` | Numeric string |
| `bool` | `"true"` or `"false"` |
| `str` | Quoted with escapes: `"\"hello\""` |
| `char` | Quoted with escapes: `"'\\n'"` |
| `void` | `"()"` |
| `Duration`, `Size` | Human-readable format |

Collections implement `Debug` when their element types implement `Debug`:

| Type | Output Format |
|------|---------------|
| `[T]` | `"[1, 2, 3]"` |
| `{K: V}` | `"{\"a\": 1, \"b\": 2}"` |
| `Set<T>` | `"Set {1, 2, 3}"` |
| `Option<T>` | `"Some(42)"` or `"None"` |
| `Result<T, E>` | `"Ok(42)"` or `"Err(\"message\")"` |
| `(A, B, ...)` | `"(1, \"hello\")"` |

### Derivable

`Debug` is derivable for user-defined types when all fields implement `Debug`:

```ori
#derive(Debug)]
type Config = { host: str, port: int }

Config { host: "localhost", port: 8080 }.debug()
// "Config { host: \"localhost\", port: 8080 }"
```

### Manual Implementation

Types may implement `Debug` manually for custom formatting:

```ori
type SecretKey = { value: [byte] }

impl Debug for SecretKey {
    @debug (self) -> str = "SecretKey { value: [REDACTED] }"
}
```

## Iterator Traits

Four traits formalize iteration:

```ori
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)
}

trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)
}

trait Iterable {
    type Item
    @iter (self) -> impl Iterator where Item == Self.Item
}

trait Collect<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Self
}
```

`Iterator.next()` returns a tuple of the optional value and the updated iterator. This functional approach fits Ori's immutable parameter semantics.

**Fused Guarantee:** Once `next()` returns `(None, iter)`, all subsequent calls must return `(None, _)`.

`Range<float>` does not implement `Iterable` due to floating-point precision ambiguity.

### Standard Implementations

| Type | Implements |
|------|------------|
| `[T]` | `Iterable`, `DoubleEndedIterator`, `Collect` |
| `{K: V}` | `Iterable` (not double-ended) |
| `Set<T>` | `Iterable`, `Collect` (not double-ended) |
| `str` | `Iterable`, `DoubleEndedIterator` |
| `Range<int>` | `Iterable`, `DoubleEndedIterator` |
| `Option<T>` | `Iterable` |

## Sendable Trait

The `Sendable` trait marks types that can safely cross task boundaries.

```ori
trait Sendable {}
```

`Sendable` is a marker trait with no methods. It is automatically implemented when:

1. All fields are `Sendable`
2. No interior mutability
3. No non-Sendable captured state (for closures)

### Interior Mutability

Interior mutability does not exist in user-defined Ori types. Ori's memory model prohibits shared mutable references, making interior mutability impossible by design.

The only types with interior mutability are runtime-provided resources. These wrap OS or runtime state that changes independently of Ori's ownership rules:

- File descriptors (kernel-managed state)
- Network connections (internal buffers)
- Database connections (session state)

### Manual Implementation

`Sendable` cannot be implemented manually. It is automatically derived by the compiler when all conditions are met. This ensures thread safety cannot be circumvented.

```ori
impl Sendable for MyType { }  // error: cannot implement Sendable manually
```

### Standard Implementations

| Type | Sendable |
|------|----------|
| `int`, `float`, `bool`, `str`, `char`, `byte` | Yes |
| `Duration`, `Size` | Yes |
| `void`, `Never` | Yes |
| `[T]` where `T: Sendable` | Yes |
| `{K: V}` where `K: Sendable, V: Sendable` | Yes |
| `Set<T>` where `T: Sendable` | Yes |
| `Option<T>` where `T: Sendable` | Yes |
| `Result<T, E>` where `T: Sendable, E: Sendable` | Yes |
| `(T1, T2, ...)` where all `Ti: Sendable` | Yes |
| `(T) -> R` where captures are `Sendable` | Yes |
| `Producer<T>` where `T: Sendable` | Yes |
| `Consumer<T>` where `T: Sendable` | Yes |
| `CloneableProducer<T>` where `T: Sendable` | Yes |
| `CloneableConsumer<T>` where `T: Sendable` | Yes |

### Non-Sendable Types

| Type | Reason |
|------|--------|
| `FileHandle` | OS resource with thread affinity |
| `Socket` | OS resource, not safely movable |
| `DatabaseConnection` | Session state, not safely movable |
| `Nursery` | Scoped to specific execution context |

User-defined types are not `Sendable` when they contain non-Sendable fields.

### Closure Sendability

The compiler analyzes closure captures to determine Sendability:

```ori
let x: int = 10              // int: Sendable
let handle: FileHandle = ... // FileHandle: NOT Sendable

let f = () -> x + 1          // f is Sendable
let g = () -> handle.read()  // g is NOT Sendable
```

When closures cross task boundaries, the compiler verifies all captures are Sendable:

```ori
parallel(
    tasks: [
        () -> process(x),      // OK: x is Sendable
        () -> read(handle),    // error: handle is not Sendable
    ],
)
```

### Channel Constraint

Channel types require `T: Sendable`:

```ori
let (p, c) = channel<int>(buffer: 10)  // OK: int is Sendable

type Handle = { file: FileHandle }
let (p, c) = channel<Handle>(buffer: 10)  // error: Handle is not Sendable
```

## Type Inference

Types inferred where possible. Required annotations:
- Function parameters
- Function return types
- Type definitions
