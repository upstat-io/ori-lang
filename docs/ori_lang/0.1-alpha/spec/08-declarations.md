---
title: "Declarations"
description: "Ori Language Specification — Declarations"
order: 8
section: "Declarations"
---

# Declarations

Functions, types, traits, and implementations.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS

## Functions

```ori
@add (a: int, b: int) -> int = a + b

pub @identity<T> (x: T) -> T = x

@sort<T: Comparable> (items: [T]) -> [T] = ...

@fetch (url: str) -> Result<str, Error> uses Http = Http.get(url)
```

- `@` prefix required
- Return type required (`void` for no value)
- Parameters are immutable
- Private by default; `pub` exports
- `uses` declares capability dependencies

### Multiple Clauses

A function may have multiple definitions (clauses) with patterns in parameter position:

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

@fib (0: int) -> int = 0
@fib (1) -> int = 1
@fib (n) -> int = fib(n - 1) + fib(n - 2)
```

Clauses are matched top-to-bottom. All clauses must have:
- Same name
- Same number of parameters
- Same return type
- Same capabilities

The first clause establishes the function signature:
- **Visibility**: `pub` only on first clause
- **Generics**: Type parameters declared on first clause; in scope for all clauses
- **Type annotations**: Required on first clause parameters; optional on subsequent clauses

```ori
pub @len<T> ([]: [T]) -> int = 0
@len ([_, ..tail]) -> int = 1 + len(tail)
```

Guards use `if` before `=`:

```ori
@abs (n: int) -> int if n < 0 = -n
@abs (n) -> int = n
```

All clauses together must be exhaustive. The compiler warns about unreachable clauses.

### Default Parameter Values

Parameters may specify default values:

```ori
@greet (name: str = "World") -> str = `Hello, {name}!`

@connect (host: str, port: int = 8080, timeout: Duration = 30s) -> Connection
```

- Callers may omit parameters with defaults
- Named arguments allow any defaulted parameter to be omitted, not just trailing ones
- Default expressions are evaluated at call time, not definition time
- Default expressions must not reference other parameters

```ori
greet()                        // "Hello, World!"
greet(name: "Alice")           // "Hello, Alice!"
connect(host: "localhost")     // uses default port and timeout
connect(host: "localhost", timeout: 60s)  // override timeout only
```

See [Expressions § Function Call](09-expressions.md#function-call) for call semantics.

### Variadic Parameters

A _variadic parameter_ accepts zero or more arguments of the same type:

```ori
@sum (numbers: ...int) -> int =
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)

sum(1, 2, 3)     // 6
sum()            // 0 (empty variadic)
```

Inside the function, the variadic parameter is received as a list.

**Constraints:**

| Rule | Description |
|------|-------------|
| One per function | At most one variadic parameter allowed |
| Must be last | Variadic parameter must appear after all required parameters |
| No default | Variadic parameters cannot have default values (default is empty list) |
| Positional at call | Variadic arguments are always positional; parameter name cannot be used |

```ori
@log (level: str, messages: ...str) -> void
log(level: "INFO", "Request", "User: 123")  // Named + variadic

@max (first: int, rest: ...int) -> int  // Requires at least one argument
```

**Allowed variadic types:**

| Type | Example | Behavior |
|------|---------|----------|
| Concrete | `...int` | All arguments must be `int` |
| Generic | `...T` | Type inferred from arguments |
| Trait object | `...Printable` | Arguments boxed as trait objects |

```ori
@print_all<T: Printable> (items: ...T) -> void
print_all(1, 2, 3)        // T = int
print_all(1, "a")         // ERROR: cannot unify int and str

@print_any (items: ...Printable) -> void
print_any(1, "hello", true)  // OK: all implement Printable
```

**Spread into variadic:**

The spread operator `...` expands a list into variadic arguments:

```ori
let nums = [1, 2, 3]
sum(...nums)           // 6
sum(0, ...nums, 10)    // 14
```

Spread in non-variadic function calls remains an error.

**Type inference:**

When a generic type parameter is only constrained by a variadic parameter, calls with zero arguments cannot infer the type:

```ori
@collect<T> (items: ...T) -> [T] = items

collect(1, 2, 3)   // T = int inferred
collect()          // ERROR: cannot infer T
collect<int>()     // OK: explicit type
```

**Function type representation:**

A variadic function's type is represented as accepting a list:

```ori
@sum (numbers: ...int) -> int = ...

let f: ([int]) -> int = sum  // Variadic stored as list-accepting function
f([1, 2, 3])                 // Must call with list when using function value
sum(1, 2, 3)                 // Direct call retains variadic syntax
```

### C Variadic Functions

C variadic functions use a different, untyped mechanism:

```ori
extern "c" from "libc" {
    @printf (format: CPtr, ...) -> c_int as "printf"
}
```

| Feature | Ori `...T` | C `...` |
|---------|------------|---------|
| Type safety | Homogeneous, checked | Unchecked |
| Context | Safe code | `unsafe(...)` only |
| Type annotation | Required | None |

C-style `...` (without type) is only valid in `extern "c"` declarations. Calling C variadic functions requires `unsafe`.

See [FFI](24-ffi.md) for details on C interop.

## Types

```ori
type Point = { x: int, y: int }

type Status = Pending | Running | Done | Failed(reason: str)

type UserId = int

#derive(Eq, Clone)
type User = { id: int, name: str }
```

## Traits

```ori
trait Printable {
    @to_str (self) -> str
}

trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}

trait Iterator {
    type Item
    @next (self) -> Option<Self.Item>
}
```

- `self` — instance
- `Self` — implementing type

### Default Type Parameters

Type parameters on traits may have default values:

```ori
trait Add<Rhs = Self> {
    type Output
    @add (self, rhs: Rhs) -> Self.Output
}
```

Semantics:

1. Default applies when impl omits the type argument
2. `Self` in default position refers to the implementing type at the impl site
3. Defaults are evaluated at impl site, not trait definition site
4. Parameters with defaults must appear after all parameters without defaults

```ori
impl Add for Point {
    // Rhs defaults to Self = Point
    @add (self, rhs: Point) -> Self = ...
}

impl Add<int> for Vector2 {
    // Explicit Rhs = int
    @add (self, rhs: int) -> Self = ...
}
```

Later default parameters may reference earlier ones:

```ori
trait Transform<Input = Self, Output = Input> {
    @transform (self, input: Input) -> Output
}

impl Transform for Parser { ... }           // Input = Self = Parser, Output = Parser
impl Transform<str> for Parser { ... }      // Input = str, Output = str
impl Transform<str, Ast> for Parser { ... } // Input = str, Output = Ast
```

### Default Associated Types

Associated types in traits may have default values:

```ori
trait Add<Rhs = Self> {
    type Output = Self  // Defaults to implementing type
    @add (self, rhs: Rhs) -> Self.Output
}

trait Container {
    type Item
    type Iter = [Self.Item]  // Default references another associated type
}
```

Semantics:

1. Default applies when impl omits the associated type
2. `Self` in default position refers to the implementing type at the impl site
3. Defaults may reference type parameters and other associated types
4. Defaults are evaluated at impl site, not trait definition site

```ori
impl Add for Point {
    // Output defaults to Self = Point
    @add (self, rhs: Point) -> Self = ...
}

impl Add<int> for Vector2 {
    type Output = Vector2  // Explicit override
    @add (self, rhs: int) -> Vector2 = ...
}
```

#### Bounds on Defaults

Defaults must satisfy any bounds on the associated type:

```ori
trait Process {
    type Output: Clone = Self  // Default only valid if Self: Clone
    @process (self) -> Self.Output
}

impl Process for String {  // OK: String: Clone
    @process (self) -> Self = self.clone()
}

impl Process for Connection {  // ERROR if Connection: !Clone and no override
    // Must provide explicit Output type since Self doesn't satisfy Clone
    type Output = ConnectionHandle
    @process (self) -> ConnectionHandle = ...
}
```

When an impl uses a default:

1. Substitute `Self` with the implementing type
2. Substitute any referenced associated types
3. Verify the resulting type satisfies all bounds

If the default does not satisfy bounds after substitution, it is a compile error at the impl site.

### Trait Associated Functions

Traits may define associated functions (methods without `self`) that implementors must provide:

```ori
trait Default {
    @default () -> Self
}

impl Default for Point {
    @default () -> Self = Point { x: 0, y: 0 }
}
```

Associated functions returning `Self` prevent the trait from being used as a trait object. See [Object Safety](#object-safety) in Types.

## Implementations

```ori
impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }
}

impl Printable for Point {
    @to_str (self) -> str = "(" + str(self.x) + ", " + str(self.y) + ")"
}

impl<T: Printable> Printable for [T] {
    @to_str (self) -> str = ...
}
```

### Associated Functions

An _associated function_ is a method defined in an `impl` block without a `self` parameter. Associated functions are called on the type itself, not on an instance.

```ori
impl Point {
    // Associated function (no self)
    @origin () -> Point = Point { x: 0, y: 0 }
    @new (x: int, y: int) -> Self = Point { x, y }

    // Instance method (has self)
    @distance (self, other: Point) -> float = ...
}
```

Associated functions are called using `Type.method(args)`:

```ori
let p = Point.origin()
let q = Point.new(x: 10, y: 20)
```

`Self` may be used as a return type in associated functions, referring to the implementing type.

For generic types, full type arguments are required:

```ori
let x: Option<int> = Option<int>.some(value: 42)
```

Extensions cannot define associated functions. Use inherent `impl` blocks for associated functions.

## Default Implementations

A _default implementation_ provides the standard behavior for a trait:

```ori
pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    @post (url: str, body: str) -> Result<Response, Error> = ...
}
```

When a module exports both a trait and its `def impl`, importing the trait automatically binds the default implementation.

Default implementation methods do not have a `self` parameter — they are stateless. For configuration, use module-level bindings:

```ori
let $timeout = 30s

pub def impl Http {
    @get (url: str) -> Result<Response, Error> =
        __http_get(url: url, timeout: $timeout)
}
```

Constraints:

- One `def impl` per trait per module
- Must implement all trait methods
- Method signatures must match the trait
- No `self` parameter

### Import Conflicts

A scope can have at most one `def impl` for each trait. Importing the same trait with defaults from two modules is a compile error:

```ori
use "module_a" { Logger }   // Brings def impl
use "module_b" { Logger }   // Error: conflicting default for Logger
```

To import a trait without its default:

```ori
use "module_a" { Logger without def }  // Import trait, skip def impl
```

### Resolution Order

When resolving a capability name:

1. Innermost `with...in` binding
2. Imported `def impl`
3. Module-local `def impl`

Imported `def impl` takes precedence over module-local `def impl`.

See [Capabilities](14-capabilities.md) for usage with capability traits.

## Trait Resolution

### Trait Inheritance (Diamond Problem)

When a type inherits a trait through multiple paths, a single implementation satisfies all paths:

```ori
trait A { @method (self) -> int }
trait B: A { }
trait C: A { }
trait D: B + C { }  // D inherits A through both B and C

impl D for MyType {
    @method (self) -> int = 42  // Single implementation satisfies A via B and C
}
```

### Conflicting Default Implementations

When multiple supertraits provide different default implementations for the same method, the implementing type must provide an explicit implementation:

```ori
trait A { @method (self) -> int = 0 }
trait B: A { @method (self) -> int = 1 }
trait C: A { @method (self) -> int = 2 }
trait D: B + C { }

impl D for MyType { }  // ERROR: ambiguous default for @method

impl D for MyType {
    @method (self) -> int = 3  // Explicit implementation resolves ambiguity
}
```

### Coherence Rules

_Coherence_ ensures that for any type `T` and trait `Trait`, there is at most one implementation of `Trait for T` visible in any compilation unit.

An implementation `impl Trait for Type` is allowed only if at least one of these is true:

1. `Trait` is defined in the current module
2. `Type` is defined in the current module
3. `Type` is a generic parameter constrained in the current module

```ori
// OK: Type is local
type MyType = { ... }
impl ExternalTrait for MyType { }

// OK: Trait is local
trait MyTrait { ... }
impl MyTrait for ExternalType { }

// ERROR: Both trait and type are external (orphan)
impl std.Display for std.Vec { }  // Error: orphan implementation
```

Blanket implementations (`impl<T> Trait for T where ...`) follow the same rules.

A duplicate implementation — where the same `Trait` and `Type` combination is implemented twice — is an error (E2010). When two blanket implementations with equal specificity could both apply, it is an error (E2021).

### Method Resolution Order

When calling `value.method()`:

1. **Inherent methods** — methods in `impl Type { }` (not trait impl)
2. **Trait methods from explicit bounds** — methods from `where T: Trait`
3. **Trait methods from in-scope traits** — traits imported into current scope
4. **Extension methods** — methods added via `extend`

If multiple traits provide the same method and none are inherent, the call is ambiguous (E2023). Use fully-qualified syntax to disambiguate:

```ori
A.method(x)  // Calls A's implementation
B.method(x)  // Calls B's implementation
```

### Super Trait Method Calls

An implementation can call the parent trait's default implementation using `Trait.method(self)`:

```ori
trait Parent {
    @method (self) -> int = 10
}

trait Child: Parent {
    @method (self) -> int = Parent.method(self) + 1
}

impl Parent for MyType {
    @method (self) -> int = Parent.method(self) * 2
}
```

### Associated Type Disambiguation

When a type implements multiple traits with same-named associated types, use qualified paths:

```ori
trait A { type Item }
trait B { type Item }

// Qualified path syntax: Type::Trait::AssocType
@f<C: A + B> (c: C) where C::A::Item: Clone = ...

// To require both Items to be the same type:
@g<C: A + B> (c: C) where C::A::Item == C::B::Item = ...
```

### Implementation Specificity

When multiple implementations could apply, the most specific wins:

1. **Concrete** — `impl Trait for MyType` (most specific)
2. **Constrained blanket** — `impl<T: Clone> Trait for T`
3. **Generic blanket** — `impl<T> Trait for T` (least specific)

It is an error if two applicable implementations have equal specificity (E2021).

### Extension Method Conflicts

Only one extension for a given method may be in scope. Conflicts are detected based on what is in scope, including re-exports:

```ori
extension "a" { Iterator.sum }
extension "b" { Iterator.sum }  // ERROR: conflicting extension imports
```

## Attributes

Attributes modify declarations with metadata or directives.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § ATTRIBUTES

### Syntax

```ori
#attribute_name
#attribute_name(args)
```

Attributes precede the declaration they modify. Multiple attributes can be applied:

```ori
#derive(Eq, Clone)
#repr("c")
type Point = { x: int, y: int }
```

### Standard Attributes

#### #derive

Generates trait implementations automatically:

```ori
#derive(Eq, Hashable, Clone, Debug)
type Point = { x: int, y: int }
```

See [Types § Derive](06-types.md#derive) for derivable traits and semantics.

#### #repr

Controls memory representation:

| Syntax | Effect |
|--------|--------|
| `#repr("c")` | C-compatible struct layout |

```ori
#repr("c")
type CTimeSpec = {
    tv_sec: int,
    tv_nsec: int
}
```

Required for structs passed to C via FFI. See [FFI § C Structs](24-ffi.md#c-structs).

#### #target

Conditional compilation based on platform:

```ori
#target(os: "linux")
@linux_only () -> void = ...

#target(arch: "x86_64", os: "linux")
@linux_x64 () -> void = ...
```

See [Conditional Compilation](25-conditional-compilation.md) for full syntax.

#### #cfg

Conditional compilation based on build configuration:

```ori
#cfg(debug)
@debug_log (msg: str) -> void = print(msg: `[DEBUG] {msg}`)

#cfg(feature: "ssl")
@secure_connect () -> void = ...
```

See [Conditional Compilation](25-conditional-compilation.md) for full syntax.

### Test Attributes

#### #skip

Skips a test with an optional reason:

```ori
#skip("pending implementation")
@test_feature tests @feature () -> void = ...
```

#### #compile_fail

Asserts that code fails to compile with the expected error:

```ori
#compile_fail("E0100")
@test_type_error tests @f () -> void =
    let x: int = "string"  // Expected type error
```

#### #fail

Asserts that a test panics with the expected message:

```ori
#fail("index out of bounds")
@test_panic tests @f () -> void =
    let list: [int] = []
    list[0]  // Expected panic
```

See [Testing](13-testing.md) for test semantics.

### File-Level Attributes

The `#!` prefix applies an attribute to the entire file:

```ori
#!target(os: "linux")

// Entire file is Linux-only
```

File-level attributes must appear before any declarations.

## Tests

```ori
@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5)
}
```

See [Testing](13-testing.md).
