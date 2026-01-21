# Variables

This section defines variable declarations, bindings, and mutability.

## Variable Bindings

A variable binding introduces a new identifier into scope and associates it with a value.

### Syntax

```
binding       = "let" [ "mut" ] identifier [ ":" type ] "=" expression .
```

### Semantics

A `let` binding:

1. Introduces a new identifier into the current scope
2. Evaluates the expression on the right-hand side
3. Associates the identifier with the resulting value
4. The binding is immutable unless declared with `mut`

```sigil
let x = 42
let name = "Alice"
let point = Point { x: 0, y: 0 }
```

### Type Annotation

An optional type annotation may follow the identifier:

```sigil
let x: int = 42
let name: str = "Alice"
let items: [int] = [1, 2, 3]
```

If the type annotation is omitted, the type is inferred from the expression.

It is an error if the annotated type does not match the inferred type of the expression.

## Mutability

### Immutable Bindings

By default, bindings are immutable. An immutable binding cannot be reassigned:

```sigil
let x = 10
x = 20  // ERROR: cannot assign to immutable binding
```

### Mutable Bindings

The `mut` modifier creates a mutable binding that can be reassigned:

```sigil
let mut counter = 0
counter = counter + 1  // OK
counter = 10           // OK
```

### Reassignment

Reassignment uses the `=` operator:

```
assignment    = identifier "=" expression .
```

It is an error to reassign:

1. An immutable binding (one declared without `mut`)
2. A function parameter
3. A config variable

```sigil
@example (x: int) -> int = run(
    x = 10,  // ERROR: cannot assign to parameter
    x,
)
```

## Scope

### Block Scope

Bindings are scoped to the block in which they are declared. A block is delimited by the `run` or `try` pattern, or by match arms.

```sigil
@example () -> int = run(
    let x = 10,     // x is in scope here
    let y = 20,     // y is in scope here
    x + y,          // both x and y are in scope
)
// x and y are not in scope here
```

### Shadowing

A binding in an inner scope may shadow a binding with the same name in an outer scope:

```sigil
@example () -> int = run(
    let x = 10,
    let result = run(
        let x = 20,  // shadows outer x
        x,           // refers to inner x (20)
    ),
    x + result,      // x refers to outer x (10)
)
// result: 30
```

Shadowing is permitted within the same scope:

```sigil
@example () -> int = run(
    let x = 10,
    let x = x + 5,   // shadows previous x
    x,               // 15
)
```

### Visibility

Bindings are visible from the point of declaration to the end of the enclosing scope. A binding is not visible before its declaration:

```sigil
@example () -> int = run(
    let y = x,  // ERROR: x not yet in scope
    let x = 10,
    y,
)
```

## Destructuring

Bindings may use patterns to destructure composite values.

### Struct Destructuring

```sigil
let { x, y } = point
let { name, age } = user
let { x: px, y: py } = point  // rename bindings
```

### Tuple Destructuring

```sigil
let (a, b) = pair
let (first, second, third) = triple
```

### List Destructuring

```sigil
let [head, ..tail] = list
let [a, b, ..rest] = items
```

### Nested Destructuring

```sigil
let { position: { x, y }, velocity } = entity
let (Point { x, y }, z) = point_3d
```

It is an error if the pattern does not match the structure of the value.

## Pattern Bindings

Variables may be bound within patterns in `match` expressions:

```sigil
match(value,
    Some(x) -> x,         // x is bound to the inner value
    None -> default,
)
```

Variables bound in patterns are immutable and scoped to the match arm.

## Function Parameters

Function parameters are implicitly immutable bindings:

```sigil
@add (a: int, b: int) -> int = a + b
```

Parameters `a` and `b` are bound to the argument values and cannot be reassigned within the function body.
