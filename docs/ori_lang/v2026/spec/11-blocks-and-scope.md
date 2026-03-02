---
title: "Blocks and Scope"
description: "Clause 11: Ori Language Specification — Blocks and Scope"
order: 11
section: "Language"
---

# 11 Blocks and Scope

A _scope_ is a region of source code within which a name refers to a specific binding.

## 11.1 Scope Hierarchy

Ori defines the following scope levels, from outermost to innermost:

1. **Universe scope** — Contains all predeclared identifiers: primitive types (`int`, `float`, `str`, `byte`, `char`, `bool`, `void`, `Never`), compound types (`Option`, `Result`, `Error`, `Ordering`, etc.), traits, built-in functions, and constructors (`Some`, `None`, `Ok`, `Err`, etc.). The universe scope encloses all modules. See [10.0.1](10-declarations.md#1001-predeclared-identifiers) for the complete list.

2. **Module scope** — Contains all top-level declarations within a module: functions, types, traits, implementations, constants, and imported identifiers. Module-scope declarations are visible throughout the module regardless of textual order (see [10.0.4](10-declarations.md#1004-forward-references)). Imports logically reside at module scope.

3. **Function scope** — Contains function parameters. Parameters are visible throughout the function body.

4. **Block scope** — Contains bindings introduced by `let` within a `{ }` block. A binding is visible from its declaration point to the end of the enclosing block.

5. **Pattern scope** — Contains bindings introduced by pattern matching in `match` arms, `for` loops, and destructuring patterns. These bindings are visible within the arm body or loop body.

Name resolution searches from the innermost scope outward: block → function → module → imports → universe. The first matching binding is used.

NOTE  Capability bindings introduced by `with...in` are resolved at block scope level.

## 11.2 Scope-Creating Constructs

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

## 11.3 Lexical Scoping

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

## 11.4 Visibility

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

## 11.5 No Hoisting

Bindings are not hoisted. A name cannot be used before its declaration:

```ori
{
    let y = x + 1;   // error: x not declared
    let x = 10;

    y
}
```

## 11.6 Shadowing

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

## 11.7 Lambda Capture

Lambdas capture variables from enclosing scopes by value. A _captured variable_ is a _free variable_ (referenced but not defined within the lambda) that exists in an enclosing scope.

```ori
{
    let base = 10;
    let add_base = (x) -> x + base;  // captures base = 10

    add_base(5)  // returns 15
}
```

### 11.7.1 What Gets Captured

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

### 11.7.2 Capture Timing

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

### 11.7.3 Capture Semantics

Capture is a snapshot at lambda creation time. Reassigning the outer binding does not affect the captured value:

```ori
{
    let x = 10;
    let f = () -> x * 2;  // captures x = 10
    x = 20;               // reassigns x in outer scope

    f()                   // returns 20, not 40
}
```

### 11.7.4 Immutability of Captured Bindings

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

### 11.7.5 Escaping Closures

An _escaping closure_ outlives the scope in which it was created:

```ori
@make_adder (n: int) -> (int) -> int =
    x -> x + n;  // escapes: returned from function
```

Because closures capture by value, escaping is always safe. The closure owns its captured data; no dangling references are possible.

### 11.7.6 Task Boundary Restrictions

Closures passed to task-spawning patterns (`parallel`, `spawn`, `nursery`) shall capture only `Sendable` values. Captured values are moved into the task, making the original binding inaccessible. See [Concurrency Model § Capture and Ownership](22-concurrency-model.md#226-capture-and-ownership).

## 11.8 Nested Scopes

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

## 11.9 Label Scope

Labels (`loop:name`, `for:name`) occupy a separate namespace from variable bindings. A label name does not conflict with a variable of the same name.

A label is visible within the body of its labeled statement. Label names shall be unique within the enclosing function body; shadowing a label is a compile-time error.

```ori
for:outer i in 0..10 do
    for:inner j in 0..10 do
        if i * j > 50 then break:outer;
```

See [Clause 16](16-control-flow.md) for `break` and `continue` with labels.

## 11.10 Type Parameter Scope

Type parameters are visible in the following regions:

| Declaration | Type parameter visible in |
|-------------|---------------------------|
| `@f<T>(...)` | Parameter types, return type, `where` clause, body |
| `type T<A> = ...` | Field types, `where` clause |
| `trait T<A> { ... }` | Method signatures, associated types, `where` clause |
| `impl<T> ...` | Target type, trait name, method bodies, `where` clause |

Type parameter names may shadow outer type parameters or type names, but this is discouraged.

## 11.11 Self and self

The keyword `Self` (capitalized) refers to the implementing type. It is visible within:

- `impl Type { ... }` — `Self` is `Type`
- `trait Foo { ... }` — `Self` is the type that will implement the trait (abstract)

`Self` is not visible in standalone functions, module scope, or `extend` blocks.

The keyword `self` (lowercase) is a parameter name available in methods — functions declared with a `self` parameter. It is immutable (like all parameters) and has type `Self`.
