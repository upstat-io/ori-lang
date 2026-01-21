# Declarations

This section defines functions, types, traits, and implementation blocks.

## Function Declarations

### Syntax

```
function      = [ "pub" ] "@" identifier [ generics ] params "->" type [ uses_clause ] "=" expression .
params        = "(" [ param { "," param } ] ")" .
param         = identifier ":" type .
generics      = "<" generic_param { "," generic_param } ">" .
generic_param = identifier [ ":" bounds ] .
bounds        = type_path { "+" type_path } .
uses_clause   = "uses" identifier { "," identifier } .
```

### Semantics

A function declaration introduces a named function into the module scope.

```sigil
@add (a: int, b: int) -> int = a + b
```

Components:

1. `@` — function sigil (required)
2. `identifier` — function name
3. `params` — parameter list with types
4. `type` — return type
5. `expression` — function body

### Function Name

The function name must be a valid identifier. By convention, function names use `snake_case`.

### Parameters

Each parameter has a name and type. Parameters are immutable bindings within the function body.

```sigil
@greet (name: str, count: int) -> str = ...
```

A function may have zero parameters:

```sigil
@get_pi () -> float = 3.14159
```

### Return Type

The return type must be specified. For functions that return no meaningful value, use `void`:

```sigil
@log (message: str) -> void = print(message)
```

### Function Body

The body is a single expression. Complex logic uses pattern expressions:

```sigil
@process (x: int) -> int = run(
    let doubled = x * 2,
    let result = doubled + 1,
    result,
)
```

### Visibility

Functions are private by default. The `pub` modifier exports the function:

```sigil
pub @add (a: int, b: int) -> int = a + b
@helper (x: int) -> int = x * 2  // private
```

### Generic Functions

Type parameters declare generic functions:

```sigil
@identity<T> (x: T) -> T = x
@pair<T, U> (a: T, b: U) -> (T, U) = (a, b)
```

### Constrained Generics

Type parameters may have trait bounds:

```sigil
@sort<T: Comparable> (items: [T]) -> [T] = ...
@max<T> (a: T, b: T) -> T where T: Comparable = ...
```

### Capability Dependencies

The `uses` clause declares required capabilities:

```sigil
@fetch (url: str) -> Result<str, Error> uses Http = Http.get(url)
```

See [Capabilities](14-capabilities.md) for details.

## Type Declarations

### Syntax

```
type_def      = [ "pub" ] [ derive ] "type" identifier [ generics ] "=" type_body .
derive        = "#[derive(" identifier { "," identifier } ")]" .
type_body     = struct_body | sum_body | type .
struct_body   = "{" [ field { "," field } [ "," ] ] "}" .
field         = identifier ":" type .
sum_body      = variant { "|" variant } .
variant       = identifier [ "(" [ field { "," field } ] ")" ] .
```

### Struct Type

```sigil
type Point = { x: int, y: int }

type User = {
    id: int,
    name: str,
    email: str,
}
```

### Sum Type

```sigil
type Status = Pending | Running | Done | Failed

type Option<T> = Some(T) | None

type Result<T, E> = Ok(T) | Err(E)
```

### Newtype

```sigil
type UserId = str
type Email = str
```

### Derive Attribute

The derive attribute auto-implements traits:

```sigil
#[derive(Eq, Clone)]
type Point = { x: int, y: int }
```

## Trait Declarations

### Syntax

```
trait_def     = [ "pub" ] "trait" identifier [ generics ] [ ":" bounds ] "{" { trait_item } "}" .
trait_item    = method_sig | default_method | assoc_type .
method_sig    = "@" identifier params "->" type .
default_method = "@" identifier params "->" type "=" expression .
assoc_type    = "type" identifier .
```

### Trait Definition

```sigil
trait Printable {
    @to_string (self) -> str
}

trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}
```

### Method Signatures

Trait methods declare the required interface:

```sigil
trait Container<T> {
    @get (self, index: int) -> Option<T>
    @len (self) -> int
}
```

### Default Methods

Traits may provide default implementations:

```sigil
trait Eq {
    @equals (self, other: Self) -> bool
    @not_equals (self, other: Self) -> bool = !self.equals(other)
}
```

### Associated Types

Traits may declare associated types:

```sigil
trait Iterator {
    type Item
    @next (self) -> Option<Self.Item>
}
```

### Self and self

- `self` — the instance of the implementing type
- `Self` — the implementing type itself

```sigil
trait Clone {
    @clone (self) -> Self
}
```

## Implementation Blocks

### Syntax

```
impl_block    = inherent_impl | trait_impl .
inherent_impl = "impl" [ generics ] type_path [ where_clause ] "{" { method } "}" .
trait_impl    = "impl" [ generics ] type_path "for" type [ where_clause ] "{" { method } "}" .
method        = "@" identifier params "->" type "=" expression .
where_clause  = "where" constraint { "," constraint } .
constraint    = identifier ":" bounds .
```

### Inherent Implementation

Methods directly on a type:

```sigil
impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }
    @distance (self, other: Point) -> float = ...
}
```

### Trait Implementation

Implementing a trait for a type:

```sigil
impl Printable for Point {
    @to_string (self) -> str = "(" + str(self.x) + ", " + str(self.y) + ")"
}
```

### Generic Implementations

```sigil
impl<T: Printable> Printable for [T] {
    @to_string (self) -> str = ...
}
```

## Test Declarations

### Syntax

```
test          = "@" identifier "tests" "@" identifier { "tests" "@" identifier } params "->" "void" "=" expression .
```

### Semantics

A test declaration associates a test function with one or more target functions:

```sigil
@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5),
)
```

Tests may target multiple functions:

```sigil
@test_roundtrip tests @parse tests @format () -> void = ...
```

See [Testing](13-testing.md) for complete specification.
