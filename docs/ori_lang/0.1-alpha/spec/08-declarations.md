---
title: "Declarations"
description: "Ori Language Specification — Declarations"
order: 8
section: "Declarations"
---

# Declarations

Functions, types, traits, and implementations.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS

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

### Method Resolution Order

When calling `value.method()`:

1. **Inherent methods** — methods in `impl Type { }` (not trait impl)
2. **Trait methods from explicit bounds** — methods from `where T: Trait`
3. **Trait methods from in-scope traits** — traits imported into current scope
4. **Extension methods** — methods added via `extend`

If multiple traits provide the same method and none are inherent, the call is ambiguous. Use fully-qualified syntax:

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

It is an error if two applicable implementations have equal specificity.

### Extension Method Conflicts

Only one extension for a given method may be in scope. Conflicts are detected based on what is in scope, including re-exports:

```ori
extension "a" { Iterator.sum }
extension "b" { Iterator.sum }  // ERROR: conflicting extension imports
```

## Tests

```ori
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
)
```

See [Testing](13-testing.md).
