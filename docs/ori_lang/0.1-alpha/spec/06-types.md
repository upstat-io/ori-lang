---
title: "Types"
description: "Ori Language Specification — Types"
order: 6
section: "Types & Values"
---

# Types

Every value has a type determined at compile time.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § TYPES

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
| `Duration` | Time span (nanoseconds) | `0ns` |
| `Size` | Byte count | `0b` |

`Never` is the _bottom type_ — a type with no values. It represents computations that never complete normally.

### Never Semantics

**Uninhabited:** No value has type `Never`. This makes it useful for:
- Functions that never return
- Match arms that never execute
- Unreachable code paths

**Coercion:** `Never` coerces to any type `T`. Since `Never` has no values, the coercion never actually executes — the expression diverges before producing a value.

```ori
let x: int = panic(msg: "unreachable")  // Never coerces to int
let y: str = unreachable()              // Never coerces to str
```

**Expressions producing Never:**

| Expression | Description |
|------------|-------------|
| `panic(msg:)` | Terminates program |
| `todo()`, `todo(reason:)` | Placeholder, terminates |
| `unreachable()`, `unreachable(reason:)` | Assertion, terminates |
| `break`, `continue` | Loop control (inside loops) |
| `expr?` on `Err`/`None` | Early return path |
| `loop(...)` with no `break` | Infinite loop |

**Type inference:** In conditionals, `Never` does not constrain the result type:

```ori
let x = if condition then 42 else panic(msg: "fail")
// Type: int (Never coerces to int)
```

If all paths return `Never`, the expression has type `Never`:

```ori
let x = if condition then panic(msg: "a") else panic(msg: "b")
// x: Never
```

**Generic contexts:** `Never` can be a type argument:

```ori
Result<Never, E>  // Can only be Err
Result<T, Never>  // Can only be Ok
Option<Never>     // Can only be None
```

**Restrictions:**

`Never` cannot appear as a struct field type:

```ori
type Bad = { value: Never }  // error E0920: uninhabited struct field
```

`Never` may appear in sum type variant payloads. Such variants are unconstructable:

```ori
type MaybeNever = Value(int) | Impossible(Never)
// Only Value(int) values can exist
```

### Duration

`Duration` represents a span of time with nanosecond precision. Internally stored as a 64-bit signed integer counting nanoseconds (range: approximately ±292 years).

**Literal syntax:**

| Suffix | Unit | Nanoseconds |
|--------|------|-------------|
| `ns` | nanoseconds | 1 |
| `us` | microseconds | 1,000 |
| `ms` | milliseconds | 1,000,000 |
| `s` | seconds | 1,000,000,000 |
| `m` | minutes | 60,000,000,000 |
| `h` | hours | 3,600,000,000,000 |

```ori
let timeout = 30s
let delay = 100ms
let precise = 500us
```

Decimal notation is supported as compile-time sugar:

```ori
let half_second = 0.5s      // 500,000,000 nanoseconds
let precise = 1.56s         // 1,560,000,000 nanoseconds
let quarter_hour = 0.25h    // 15 minutes
```

The decimal portion is converted to an exact integer value using integer arithmetic—no floating-point operations are involved. The result must be a whole number of nanoseconds; otherwise, it is an error:

```ori
// Valid - results in whole nanoseconds
1.5s           // 1,500,000,000 ns
0.001s         // 1,000,000 ns (1ms)
1.123456789s   // 1,123,456,789 ns

// Invalid - sub-nanosecond precision
1.5ns          // Error: 1.5 nanoseconds is not a whole number
1.0000000001s  // Error: result has sub-nanosecond precision
```

**Arithmetic:**

| Operation | Types | Result |
|-----------|-------|--------|
| `d1 + d2` | Duration + Duration | Duration |
| `d1 - d2` | Duration - Duration | Duration |
| `d * n` | Duration * int | Duration |
| `n * d` | int * Duration | Duration |
| `d / n` | Duration / int | Duration |
| `d1 / d2` | Duration / Duration | int (ratio) |
| `d1 % d2` | Duration % Duration | Duration (remainder) |
| `-d` | -Duration | Duration |

Arithmetic panics on overflow.

**Conversion methods:**

```ori
impl Duration {
    @nanoseconds (self) -> int
    @microseconds (self) -> int
    @milliseconds (self) -> int
    @seconds (self) -> int
    @minutes (self) -> int
    @hours (self) -> int

    @from_nanoseconds (ns: int) -> Duration
    @from_microseconds (us: int) -> Duration
    @from_milliseconds (ms: int) -> Duration
    @from_seconds (s: int) -> Duration
    @from_minutes (m: int) -> Duration
    @from_hours (h: int) -> Duration
}
```

Extraction methods truncate toward zero: `90s.minutes()` returns `1`.

**Traits:** `Eq`, `Comparable`, `Hashable`, `Clone`, `Debug`, `Printable`, `Default`, `Sendable`

### Size

`Size` represents a byte count. Internally stored as a 64-bit signed integer (non-negative, range: 0 to ~8 exabytes).

**Literal syntax:**

| Suffix | Unit | Bytes |
|--------|------|-------|
| `b` | bytes | 1 |
| `kb` | kilobytes | 1,000 |
| `mb` | megabytes | 1,000,000 |
| `gb` | gigabytes | 1,000,000,000 |
| `tb` | terabytes | 1,000,000,000,000 |

Size uses SI/decimal units (powers of 1000). Programs requiring exact powers of 1024 should use explicit byte counts: `1024b`, `1048576b`.

```ori
let buffer = 64kb
let limit = 10mb
let heap = 2gb
```

Decimal notation is supported as compile-time sugar:

```ori
let half_kb = 0.5kb         // 500 bytes
let one_and_half_mb = 1.5mb // 1,500,000 bytes
let quarter_gb = 0.25gb     // 250,000,000 bytes
```

The decimal portion is converted to an exact integer value using integer arithmetic. The result must be a whole number of bytes; otherwise, it is an error:

```ori
// Valid - results in whole bytes
1.5kb          // 1,500 bytes
0.001mb        // 1,000 bytes (1kb)

// Invalid - sub-byte precision
0.5b           // Error: 0.5 bytes is not a whole number
```

**Arithmetic:**

| Operation | Types | Result |
|-----------|-------|--------|
| `s1 + s2` | Size + Size | Size |
| `s1 - s2` | Size - Size | Size (panics if negative) |
| `s * n` | Size * int | Size |
| `n * s` | int * Size | Size |
| `s / n` | Size / int | Size |
| `s1 / s2` | Size / Size | int (ratio) |
| `s1 % s2` | Size % Size | Size (remainder) |

Unary negation (`-`) is not permitted on Size. It is a compile-time error.

**Conversion methods:**

```ori
impl Size {
    @bytes (self) -> int
    @kilobytes (self) -> int
    @megabytes (self) -> int
    @gigabytes (self) -> int
    @terabytes (self) -> int

    @from_bytes (b: int) -> Size
    @from_kilobytes (kb: int) -> Size
    @from_megabytes (mb: int) -> Size
    @from_gigabytes (gb: int) -> Size
    @from_terabytes (tb: int) -> Size
}
```

Extraction methods truncate toward zero: `1536kb.megabytes()` returns `1`.

**Traits:** `Eq`, `Comparable`, `Hashable`, `Clone`, `Debug`, `Printable`, `Default`, `Sendable`

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

#### Default Values

Const generics can have default values:

```ori
@buffer<$N: int = 64> () -> [byte, max N] = ...

buffer()          // Uses N = 64
buffer<128>()     // Overrides to N = 128

type Vector<T, $N: int = 3> = [T, max N]

Vector<float>         // 3D vector
Vector<float, 4>      // 4D vector
```

Default const values must be:
1. Compile-time constant expressions
2. Valid for any bounds on the parameter

```ori
@sized<$N: int = 10> ()
    where N > 0       // OK: 10 > 0
= ...

@bad<$N: int = 0> ()
    where N > 0       // ERROR: default value 0 violates bound
= ...
```

#### Parameter Ordering

When mixing type and const generic parameters, either ordering is allowed:

```ori
@example<T, $N: int> (...)      // Type first (preferred)
@example<$N: int, T> (...)      // Const first (allowed)
@example<T, $N: int, U> (...)   // Interleaved (allowed)
```

**Style recommendation**: Place type parameters before const generics for consistency.

#### Instantiation

**Explicit instantiation:**

```ori
zeros<10>()                      // [int, max 10]
replicate<str, 5>(value: "hi")   // [str, max 5]
```

**Inferred instantiation:**

When possible, const generics are inferred from context:

```ori
let buffer: [int, max 100] = zeros()  // N = 100 inferred
```

**Partial inference:**

Type parameters can be inferred while const generics are explicit:

```ori
let items = replicate<_, 5>(value: "hello")  // T = str inferred, N = 5 explicit
```

#### Monomorphization

Each unique combination of const generic values produces a distinct monomorphized function or type:

```ori
zeros<5>()   // Generates zeros_5
zeros<10>()  // Generates zeros_10

// These are distinct types:
let a: [int, max 5] = ...
let b: [int, max 10] = a  // ERROR: type mismatch
```

The compiler may warn for excessive instantiations that impact compile time or binary size.

#### Const Expressions in Types

Const generics enable arithmetic in type positions:

```ori
@double_capacity<T, $N: int> (items: [T, max N]) -> [T, max N * 2] = ...

@halve<T, $N: int> (items: [T, max N]) -> [T, max N / 2]
    where N > 0
    where N % 2 == 0
= ...
```

Allowed arithmetic operations in type positions:
- Addition: `N + M`, `N + 1`
- Subtraction: `N - M`, `N - 1`
- Multiplication: `N * M`, `N * 2`
- Division: `N / M`, `N / 2` (truncating)
- Modulo: `N % M`
- Bitwise: `N & M`, `N | M`, `N ^ M`, `N << M`, `N >> M`

### Const Bounds

A _const bound_ constrains the values a const generic parameter may take. Const bounds appear in `where` clauses and are checked at compile time.

**Allowed operators in const bounds:**

| Category | Operators |
|----------|-----------|
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Logical | `&&`, `\|\|`, `!` |
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Bitwise | `&`, `\|`, `^`, `<<`, `>>` |

```ori
where N > 0                      // Simple bound
where N >= 1 && N <= 100         // Compound bound
where N % 2 == 0                 // Divisibility
where N & (N - 1) == 0           // Power of two (bitwise)
where A || B                     // Bool parameters
```

Multiple `where` clauses are implicitly combined with `&&`:

```ori
where R > 0
where C > 0
// equivalent to: where R > 0 && C > 0
```

**Evaluation timing:**

- When concrete values are known at the call site, bounds are checked immediately
- When values depend on outer const parameters, checking is deferred to monomorphization

**Constraint propagation:**

When calling a function with const bounds, the caller's bounds must _imply_ the callee's bounds. The compiler performs linear arithmetic implication checking:

```ori
@inner<$N: int> () -> [int, max N]
    where N >= 10
= ...

@outer<$M: int> () -> [int, max M]
    where M >= 20  // M >= 20 implies M >= 10
= inner<M>()       // OK
```

**Overflow handling:**

Arithmetic overflow during const bound evaluation is a compile-time error (E1033). Const bound arithmetic uses 64-bit signed integers.

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

### Ordering

The `Ordering` type represents the result of comparing two values.

| Variant | Meaning |
|---------|---------|
| `Less` | Left operand is less than right |
| `Equal` | Left operand equals right |
| `Greater` | Left operand is greater than right |

#### Ordering Methods

```ori
impl Ordering {
    @is_less (self) -> bool
    @is_equal (self) -> bool
    @is_greater (self) -> bool
    @is_less_or_equal (self) -> bool
    @is_greater_or_equal (self) -> bool
    @reverse (self) -> Ordering
    @then (self, other: Ordering) -> Ordering
    @then_with (self, f: () -> Ordering) -> Ordering
}
```

The `then` method chains comparisons for lexicographic ordering. It returns `self` unless `self` is `Equal`, in which case it returns `other`.

The `then_with` method is a lazy variant that only evaluates its argument when `self` is `Equal`.

```ori
// Lexicographic comparison of (a1, a2) with (b1, b2)
compare(left: a1, right: b1).then(other: compare(left: a2, right: b2))

// Lazy version — second comparison only evaluated if first is Equal
compare(left: a1, right: b1).then_with(f: () -> compare(left: a2, right: b2))
```

#### Ordering Traits

`Ordering` implements: `Eq`, `Comparable`, `Clone`, `Debug`, `Printable`, `Hashable`, `Default`.

The `Default` value is `Equal`.

The `Comparable` ordering is: `Less < Equal < Greater`.

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

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (type_def)

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

A _newtype_ creates a distinct nominal type that wraps an existing type.

**Construction:**

Newtypes use their type name as a constructor:

```ori
type UserId = int
let id = UserId(42)
```

Literals cannot directly become newtypes:

```ori
let id: UserId = 42  // error: expected UserId, found int
```

**Underlying Value Access:**

The underlying value is accessed via `.inner`:

```ori
let id = UserId(42)
let raw: int = id.inner
```

The `.inner` accessor is always public, regardless of the newtype's visibility. The type-safety boundary is at construction, not access.

**No Trait Inheritance:**

Newtypes do not automatically inherit traits from their underlying type:

```ori
type UserId = int
let a = UserId(1)
let b = UserId(2)
a == b  // error: UserId does not implement Eq
```

Derive traits explicitly:

```ori
#derive(Eq, Hashable, Clone, Debug)
type UserId = int
```

**No Method Inheritance:**

Newtypes do not expose the underlying type's methods:

```ori
type Email = str
let email = Email("user@example.com")
email.len()        // error: Email has no method len
email.inner.len()  // OK
```

**Generic Newtypes:**

```ori
type NonEmpty<T> = [T]

impl<T> NonEmpty<T> {
    @first (self) -> T = self.inner[0]
}
```

**Performance:**

Newtypes have zero runtime overhead. They share the same memory layout as their underlying type; the compiler erases the wrapper.

### Derive

```ori
#derive(Eq, Hashable, Clone)
type Point = { x: int, y: int }
```

The `#derive` attribute generates trait implementations automatically for user-defined types.

**Derivable Traits:**

| Trait | Struct | Sum Type | Newtype | Requirement |
|-------|--------|----------|---------|-------------|
| `Eq` | Yes | Yes | Yes | All fields/underlying implement `Eq` |
| `Hashable` | Yes | Yes | Yes | All fields/underlying implement `Hashable` |
| `Comparable` | Yes | Yes | Yes | All fields/underlying implement `Comparable` |
| `Clone` | Yes | Yes | Yes | All fields/underlying implement `Clone` |
| `Default` | Yes | No | Yes | All fields/underlying implement `Default` |
| `Debug` | Yes | Yes | Yes | All fields/underlying implement `Debug` |
| `Printable` | Yes | Yes | Yes | All fields/underlying implement `Printable` |

**Derivation Rules:**

- `Eq`: Field-wise equality comparison; newtypes delegate to underlying type
- `Hashable`: Combined field hashes using `hash_combine`; warning if derived without `Eq`; newtypes delegate to underlying type
- `Comparable`: Lexicographic comparison by field declaration order; sum type variants compare by declaration order; newtypes delegate to underlying type
- `Clone`: Field-wise cloning via `.clone()` method; newtypes delegate to underlying type
- `Default`: Field-wise default construction; cannot be derived for sum types (ambiguous variant); newtypes delegate to underlying type
- `Debug`: Structural representation: `TypeName { field1: value1, field2: value2 }`; newtypes show `TypeName(value)`
- `Printable`: Human-readable format: `TypeName(value1, value2)`; newtypes show `TypeName(value)`

**Generic Types:**

Generic types derive traits conditionally based on type parameter constraints:

```ori
#derive(Eq, Clone)
type Pair<T> = { first: T, second: T }

// Generated:
impl<T: Eq> Eq for Pair<T> { ... }
impl<T: Clone> Clone for Pair<T> { ... }
```

**Recursive Types:**

Recursive types can derive traits; generated implementations handle recursion correctly.

**Non-Derivable Traits:**

| Trait | Reason |
|-------|--------|
| `Iterator` | Requires custom `next` logic |
| `Iterable` | Requires custom `iter` logic |
| `Into` | Requires custom conversion logic |
| `Drop` | Requires custom cleanup logic |
| `Sendable` | Automatically derived by compiler |

**Notes:**

- Types implementing `Printable` automatically implement `Formattable` via blanket implementation
- Multiple `#derive` attributes are equivalent to a single attribute with combined traits
- Derive order does not affect behavior

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
#derive(Clone)
type Point = { x: int, y: int }
```

Derived implementation clones each field.

### Non-Cloneable Types

Some types do not implement `Clone`:
- Unique resources (file handles, network connections)
- Types with identity where duplicates would be semantically wrong

## Drop Trait

The `Drop` trait enables custom cleanup when a value's reference count reaches zero:

```ori
trait Drop {
    @drop (self) -> void
}
```

### Execution Timing

Drop is called when the ARC reference count reaches zero:

```ori
run(
    let resource = acquire_resource(),  // refcount: 1
    use_resource(resource),             // refcount may increase
)                                       // refcount: 0, drop called
```

For values not shared, drop occurs at scope exit. Drop is also called on early exit (via `?`, `break`, or panic).

### Drop Order

Values are dropped in reverse declaration order (LIFO):

```ori
run(
    let a = Resource { name: "a" },
    let b = Resource { name: "b" },
    let c = Resource { name: "c" },
)
// Drop order: c, b, a
```

**Struct fields:** Dropped in reverse declaration order, then the struct.

**Collections:** Elements dropped in reverse order (back-to-front).

### Constraints

**No async operations:** Drop cannot perform suspending operations. Drop runs synchronously during stack unwinding. Async operations could deadlock.

```ori
impl Drop for Connection {
    @drop (self) -> void uses Suspend = ...  // error E0980: Drop cannot be async
}
```

**Must return void:** Drop must return `void`.

**Panic during unwind:** If drop panics while another panic is unwinding, the program aborts immediately.

### Not Derivable

`Drop` cannot be derived. Drop behavior is specific to each type; automatic derivation would be either no-op or incorrect.

### Explicit Drop

The `drop_early` function forces drop before scope exit:

```ori
@drop_early<T> (value: T) -> void

run(
    let file = open_file(path),
    let content = read_all(file),
    drop_early(value: file),  // Close immediately
    // ... continue processing content
)
```

### Standard Implementations

Most types do not need `Drop`:
- Primitives: `int`, `float`, `bool`, `str`, `char`, `byte`
- Collections: `[T]`, `{K: V}`, `Set<T>` (elements dropped automatically)
- Options and Results: `Option<T>`, `Result<T, E>` (values dropped automatically)

Types wrapping external resources typically implement Drop:

```ori
impl Drop for FileHandle {
    @drop (self) -> void = close_file_descriptor(self.fd)
}

impl Drop for Lock {
    @drop (self) -> void = release_lock(self.handle)
}
```

### Error Handling in Drop

Drop should handle its own errors. Drop cannot return errors; propagating would require panic, which is dangerous during unwinding:

```ori
impl Drop for Connection {
    @drop (self) -> void = match(self.close(),
        Ok(_) -> (),
        Err(e) -> log(msg: `close failed: {e}`),  // Log, don't propagate
    )
}
```

## Conversion Traits

Three traits formalize type conversions:

### Into Trait

The `Into` trait represents semantic, lossless type conversion:

```ori
trait Into<T> {
    @into (self) -> T
}
```

**Usage:**

```ori
let error: Error = "something went wrong".into()
let f: float = 42.into()
```

Conversion requires explicit `.into()` method call. Ori does NOT perform implicit conversion.

**Standard Implementations:**

| From | To | Notes |
|------|-----|-------|
| `str` | `Error` | Creates Error with message |
| `int` | `float` | Lossless widening |
| `Set<T>` | `[T]` | Requires `T: Eq + Hashable` |

**No Blanket Identity:**

There is no blanket `impl<T> Into<T> for T`. Each conversion must be explicitly implemented.

**No Automatic Chaining:**

`Into` does not chain automatically. Given `A: Into<B>` and `B: Into<C>`, converting `A` to `C` requires `a.into().into()`.

### As Trait

The `As` trait defines infallible type conversion:

```ori
trait As<T> {
    @as (self) -> T
}
```

The `as` operator desugars to this trait:

```ori
42 as float
// Desugars to:
As<float>.as(self: 42)
```

**Standard Implementations:**

| From | To | Notes |
|------|-----|-------|
| `int` | `float` | Widening |
| `byte` | `int` | Widening |
| `int` | `str` | Formatting |
| `float` | `str` | Formatting |
| `bool` | `str` | "true" or "false" |
| `char` | `str` | Single character string |
| `byte` | `str` | Formatting |
| `char` | `int` | Codepoint value |

### TryAs Trait

The `TryAs` trait defines fallible type conversion:

```ori
trait TryAs<T> {
    @try_as (self) -> Option<T>
}
```

The `as?` operator desugars to this trait:

```ori
"42" as? int
// Desugars to:
TryAs<int>.try_as(self: "42")
```

**Standard Implementations:**

| From | To | Notes |
|------|-----|-------|
| `str` | `int` | Parsing |
| `str` | `float` | Parsing |
| `str` | `bool` | "true"/"false" only |
| `int` | `byte` | Range check (0-255) |
| `char` | `byte` | ASCII check (0-127) |
| `int` | `char` | Valid codepoint check |

### Comparison

| Mechanism | Fallible | Extensible | Use Case |
|-----------|----------|------------|----------|
| `as` | No | Yes | Infallible representation changes |
| `as?` | Yes | Yes | Parsing, checked conversions |
| `Into` | No | Yes | Semantic type conversions |

### Lossy Conversions

Lossy conversions (like `float -> int`) are not supported by `as` or `as?`. Use explicit methods that communicate intent:

```ori
3.7.truncate()   // 3 (toward zero)
3.7.round()      // 4 (nearest)
3.7.floor()      // 3 (toward negative infinity)
3.7.ceil()       // 4 (toward positive infinity)
```

It is a compile-time error to use `as` for a lossy conversion.

### User-Defined Conversions

Types can implement conversion traits:

```ori
type UserId = int

impl As<str> for UserId {
    @as (self) -> str = "user_" + (self.inner as str)
}

impl TryAs<Username> for str {
    @try_as (self) -> Option<Username> =
        if self.is_empty() || self.len() > 32 then
            None
        else
            Some(Username { value: self })
}
```

## Debug Trait

The `Debug` trait provides developer-facing string representation:

```ori
trait Debug {
    @debug (self) -> str
}
```

Unlike `Printable`, which is for user-facing display, `Debug` shows the complete internal structure and is always derivable.

```ori
#derive(Debug)
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
#derive(Debug)
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
    @from_iter<I: Iterator> (iter: I) -> Self where I.Item == T
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

## Existential Types

An _existential type_ written `impl Trait` represents an opaque type that satisfies the specified trait bounds. The concrete type is known to the compiler internally but hidden from callers.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § TYPES (impl_trait_type)

### Syntax

```ori
@make_iterator (items: [int]) -> impl Iterator where Item == int =
    items.iter()
```

The grammar for existential types:

```ebnf
impl_trait_type   = "impl" trait_bounds [ impl_where_clause ] .
trait_bounds      = type_path { "+" type_path } .
impl_where_clause = "where" assoc_constraint { "," assoc_constraint } .
assoc_constraint  = identifier "==" type .
```

Multiple trait bounds use `+`:

```ori
@clonable_iter () -> impl Iterator + Clone where Item == int = ...
```

### Semantics

**Opaqueness:** Callers see only the trait interface. The concrete type's fields and methods beyond the trait bounds are inaccessible.

```ori
let iter = make_iterator(items: [1, 2, 3])
iter.next()   // OK: Iterator method
iter.list     // error: cannot access concrete type's fields
```

**Single Concrete Type:** All return paths must yield the same concrete type:

```ori
// OK: all paths return ListIterator<int>
@numbers (flag: bool) -> impl Iterator where Item == int =
    if flag then [1, 2, 3].iter() else [4, 5, 6].iter()

// error: different concrete types
@bad (flag: bool) -> impl Iterator where Item == int =
    if flag then [1, 2, 3].iter()     // ListIterator<int>
    else (1..10).iter()               // RangeIterator<int>
```

**Static Dispatch:** The compiler monomorphizes each call site. No vtable overhead.

### Valid Positions

`impl Trait` is allowed only in return position:

| Position | Allowed |
|----------|---------|
| Function return | Yes |
| Method return | Yes |
| Trait method return | Yes |
| Argument position | No — use generics |
| Struct fields | No — use generics |

```ori
// Argument position: use generic parameter
@process<I: Iterator> (iter: I) -> int where I.Item == int = ...

// Struct field: use generic parameter
type Container<I: Iterator> = { iter: I } where I.Item == int
```

### Comparison with Trait Objects

| Aspect | `impl Trait` | Trait Object (`Trait`) |
|--------|--------------|------------------------|
| Dispatch | Static (monomorphized) | Dynamic (vtable) |
| Size | Concrete type size | Pointer size |
| Performance | Better (inlinable) | Vtable overhead |
| Flexibility | Single concrete type | Any type at runtime |
| Object safety | All traits | Object-safe traits only |

Use `impl Trait` when a single concrete type is returned and performance matters. Use trait objects when multiple types may be returned at runtime.

### Error Codes

- `E0810`: `impl Trait` only allowed in return position
- `E0811`: All return paths must have the same concrete type
- `E0812`: Concrete type does not implement required trait bounds

## Type Inference

Types inferred where possible. Required annotations:
- Function parameters
- Function return types
- Type definitions
