---
title: "Variables"
description: "Ori Language Specification — Variables"
order: 5
---

# Variables

Variables are storage locations identified by name.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § EXPRESSIONS (let_expr, assignment, binding_pattern)

## Bindings

A `let` binding introduces an identifier into the current scope. Bindings are mutable by default.

```ori
let x = 42                  // mutable
let name: str = "Alice"     // mutable, with type annotation
let $timeout = 30s          // immutable ($ prefix)
```

Type annotations are optional; types are inferred when omitted. Annotated type must match inferred type.

## Mutability

Bindings without `$` prefix are mutable:

```ori
let x = 0
x = x + 1       // OK: mutable binding

let $y = 10
$y = 20         // error: cannot assign to immutable binding '$y'
```

The `$` prefix marks a binding as immutable. See [Constants](04-constants.md) for details.

### Cannot Reassign

- Immutable bindings (`$`-prefixed)
- Function parameters
- Loop variables

```ori
@add (a: int, b: int) -> int = run(
    a = 10,  // error: cannot assign to parameter
    a + b,
)

for item in items do
    item = other  // error: cannot assign to loop variable
```

## Scope

Bindings are visible from declaration to end of enclosing block.

```ori
run(
    let x = 10,
    let y = x + 5,  // x visible
    y,
)
// x, y not visible
```

### Shadowing

Bindings may shadow earlier bindings with the same name. Shadowing can change mutability:

```ori
run(
    let x = 10,           // mutable
    let $x = x + 5,       // immutable, shadows outer x
    $x,                   // 15
)

run(
    let $x = 10,          // immutable
    run(
        let x = $x * 2,   // mutable, shadows outer $x
        x = x + 1,        // OK: inner x is mutable
        x,
    ),
)
```

The `$` prefix must match between definition and usage within the same binding scope.

## Destructuring

Patterns destructure composite values. The `$` prefix applies to individual bindings:

```ori
let { x, y } = point                  // both mutable
let { $x, y } = point                 // x immutable, y mutable
let { x: px, y: py } = point          // rename, both mutable
let (a, b) = pair                     // both mutable
let ($a, $b) = pair                   // both immutable
let [head, ..tail] = list             // head mutable, tail mutable
let [$head, ..tail] = list            // head immutable, tail mutable
let { position: { x, y } } = entity   // nested destructure
```

Pattern must match value structure.

## Function Parameters

Parameters are immutable bindings scoped to the function body:

```ori
@add (a: int, b: int) -> int = a + b
```

Parameters cannot be reassigned regardless of `$` prefix.
