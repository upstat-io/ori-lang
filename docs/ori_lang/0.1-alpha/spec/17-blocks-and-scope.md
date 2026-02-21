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
| `{ }` block | All bindings within the block |
| `for` loop | Loop variable and body |
| `match` arm | Pattern bindings and arm expression |
| `if` branches | Each branch expression |
| `loop { }` | Body expression |
| `with...in` | Capability binding and `in` expression |
| Lambda | Parameters and body expression |

## Lexical Scoping

Ori uses _lexical scoping_. A name refers to the binding in the innermost enclosing scope that declares that name.

```ori
{
    let x = 10;
    {
        let y = x + 5;   // x visible from outer scope

        y
    }
    // y not visible here
    x
}
```

Names are resolved at the point of use by searching outward through enclosing scopes. If no binding is found, the compiler reports an error.

## Visibility

A binding is visible from its declaration to the end of its enclosing scope.

```ori
{
    // x not yet visible
    let x = 10;
    let y = x + 5;   // x visible

    y
}
// x, y not visible
```

Bindings in a block are visible to all subsequent expressions in the block:

```ori
{
    let a = 1;
    let b = a + 1;   // a visible
    let c = b + 1;   // a and b visible

    c
}
```

## No Hoisting

Bindings are not hoisted. A name cannot be used before its declaration:

```ori
{
    let y = x + 1;   // error: x not declared
    let x = 10;

    y
}
```

## Shadowing

A binding may _shadow_ an earlier binding with the same name. The new binding hides the previous one within its scope.

```ori
{
    let x = 10;
    let x = x + 5;  // shadows, x is now 15

    x
}
```

Shadowing applies to all bindings, including function parameters:

```ori
@increment (x: int) -> int = {
    let x = x + 1;  // shadows parameter

    x
}
```

The shadowed binding becomes inaccessible; there is no way to refer to it.

## Lambda Capture

Lambdas capture variables from enclosing scopes by value. A _captured variable_ is a _free variable_ (referenced but not defined within the lambda) that exists in an enclosing scope.

```ori
{
    let base = 10;
    let add_base = (x) -> x + base;  // captures base = 10

    add_base(5)  // returns 15
}
```

### What Gets Captured

A lambda captures all free variables referenced in its body:

```ori
{
    let a = 1;
    let b = 2;
    let c = 3;
    let f = () -> a + b;  // captures a and b, not c

    f()
}
```

Variables not referenced are not captured.

### Capture Timing

Capture occurs at the moment of lambda creation, not at invocation:

```ori
{
    let closures = [];
    for i in 0..3 do
        closures = closures + [() -> i];  // each captures current i

    closures[0]();  // 0
    closures[1]();  // 1
    closures[2]()   // 2
}
```

### Capture Semantics

Capture is a snapshot at lambda creation time. Reassigning the outer binding does not affect the captured value:

```ori
{
    let x = 10;
    let f = () -> x * 2;  // captures x = 10
    x = 20;               // reassigns x in outer scope

    f()                   // returns 20, not 40
}
```

### Immutability of Captured Bindings

Lambdas cannot mutate captured bindings:

```ori
{
    let x = 0;
    let inc = () -> x = x + 1;  // error: cannot mutate captured binding

    inc()
}
```

This restriction prevents side effects through closures and ensures ARC safety.

A lambda may shadow a captured binding with a local one:

```ori
{
    let x = 10;
    let f = () -> {
        let x = 20;  // shadows captured x

        x
    };
    f()   // returns 20
}
```

### Escaping Closures

An _escaping closure_ outlives the scope in which it was created:

```ori
@make_adder (n: int) -> (int) -> int =
    x -> x + n;  // escapes: returned from function
```

Because closures capture by value, escaping is always safe. The closure owns its captured data; no dangling references are possible.

### Task Boundary Restrictions

Closures passed to task-spawning patterns (`parallel`, `spawn`, `nursery`) must capture only `Sendable` values. Captured values are moved into the task, making the original binding inaccessible. See [Concurrency Model § Capture and Ownership](23-concurrency-model.md#capture-and-ownership).

## Nested Scopes

Scopes may be nested to arbitrary depth. Inner scopes can access bindings from all enclosing scopes:

```ori
{
    let a = 1;
    {
        let b = 2;
        {
            let c = a + b;  // both visible

            c
        }
    }
}
```

Each scope is independent; bindings in one branch do not affect another:

```ori
if condition then
    { let x = 1; x }
else
    { let x = 2; x }  // different x, no conflict
```
