---
title: "Variables"
description: "Ori Language Specification — Variables"
order: 5
---

# Variables

Variables are storage locations identified by name.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § EXPRESSIONS (let_expr, assignment, binding_pattern)

## Bindings

A `let` binding introduces an identifier into the current scope. Bindings are immutable by default.

```ori
let x = 42
let name: str = "Alice"
let mut counter = 0
```

Type annotations are optional; types are inferred when omitted. Annotated type must match inferred type.

## Mutability

Mutable bindings use `mut` modifier:

```ori
let mut x = 0
x = x + 1       // OK

let y = 10
y = 20          // error: immutable binding
```

Cannot reassign:
- Immutable bindings
- Function parameters
- Config variables

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

Bindings may shadow earlier bindings with the same name:

```ori
run(
    let x = 10,
    let x = x + 5,  // shadows, now 15
    x,
)
```

## Destructuring

Patterns destructure composite values:

```ori
let { x, y } = point
let { x: px, y: py } = point          // rename
let (a, b) = pair
let [head, ..tail] = list
let { position: { x, y } } = entity   // nested
```

Pattern must match value structure.

## Function Parameters

Parameters are immutable bindings scoped to the function body:

```ori
@add (a: int, b: int) -> int = a + b
```
