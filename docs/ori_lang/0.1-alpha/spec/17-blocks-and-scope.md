---
title: "Blocks and Scope"
description: "Ori Language Specification — Blocks and Scope"
order: 17
section: "Declarations"
---

# Blocks and Scope

A _scope_ is a region of source code within which a name refers to a specific binding.

## Scope-Creating Constructs

The following constructs create scopes:

| Construct | Scope contains |
|-----------|----------------|
| Function body | Parameters and body expression |
| `run(...)` | All bindings within the sequence |
| `for` loop | Loop variable and body |
| `match` arm | Pattern bindings and arm expression |
| `if` branches | Each branch expression |
| `loop(...)` | Body expression |
| `with...in` | Capability binding and `in` expression |
| Lambda | Parameters and body expression |

## Lexical Scoping

Ori uses _lexical scoping_. A name refers to the binding in the innermost enclosing scope that declares that name.

```ori
run(
    let x = 10,
    run(
        let y = x + 5,  // x visible from outer scope
        y,
    ),
    // y not visible here
    x,
)
```

Names are resolved at the point of use by searching outward through enclosing scopes. If no binding is found, the compiler reports an error.

## Visibility

A binding is visible from its declaration to the end of its enclosing scope.

```ori
run(
    // x not yet visible
    let x = 10,
    let y = x + 5,  // x visible
    y,
)
// x, y not visible
```

Bindings in `run(...)` are visible to all subsequent expressions in the sequence:

```ori
run(
    let a = 1,
    let b = a + 1,  // a visible
    let c = b + 1,  // a and b visible
    c,
)
```

## No Hoisting

Bindings are not hoisted. A name cannot be used before its declaration:

```ori
run(
    let y = x + 1,  // error: x not declared
    let x = 10,
    y,
)
```

## Shadowing

A binding may _shadow_ an earlier binding with the same name. The new binding hides the previous one within its scope.

```ori
run(
    let x = 10,
    let x = x + 5,  // shadows, x is now 15
    x,
)
```

Shadowing applies to all bindings, including function parameters:

```ori
@increment (x: int) -> int = run(
    let x = x + 1,  // shadows parameter
    x,
)
```

The shadowed binding becomes inaccessible; there is no way to refer to it.

## Lambda Capture

Lambdas capture variables from enclosing scopes by value.

```ori
run(
    let base = 10,
    let add_base = (x) -> x + base,  // captures base = 10
    add_base(5),  // returns 15
)
```

### Capture Semantics

Capture is a snapshot at lambda creation time:

```ori
run(
    let mut x = 10,
    let f = () -> x * 2,  // captures x = 10
    x = 20,
    f(),  // returns 20, not 40
)
```

Lambdas cannot mutate captured bindings:

```ori
run(
    let mut x = 0,
    let inc = () -> x = x + 1,  // error: cannot mutate outer scope
    inc(),
)
```

This restriction prevents side effects through closures.

## Nested Scopes

Scopes may be nested to arbitrary depth. Inner scopes can access bindings from all enclosing scopes:

```ori
run(
    let a = 1,
    run(
        let b = 2,
        run(
            let c = a + b,  // both visible
            c,
        ),
    ),
)
```

Each scope is independent; bindings in one branch do not affect another:

```ori
if condition then
    run(let x = 1, x)
else
    run(let x = 2, x)  // different x, no conflict
```
