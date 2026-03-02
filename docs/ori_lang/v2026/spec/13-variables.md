---
title: "Variables"
description: "Clause 13: Ori Language Specification — Variables"
order: 13
section: "Language"
---

# 13 Variables

A _variable_ is a storage location identified by a name. Every variable has a type and holds a value of that type.

> **Grammar:** See [grammar.ebnf](grammar.md) § EXPRESSIONS (let_expr, assignment, binding_pattern)

## 13.1 Bindings

A `let` binding introduces an identifier into the current scope. Every `let` binding shall have an initializer expression. Uninitialized variables are not permitted.

```ori
let x = 42;                  // mutable
let name: str = "Alice";     // mutable, with type annotation
let $timeout = 30s;          // immutable ($ prefix)

let y: int;                  // error: binding requires initializer
```

Bindings are mutable by default. The `$` prefix marks a binding as immutable (see [Clause 12](12-constants.md)).

Type annotations are optional; types are inferred when omitted. When a type annotation is present, the annotated type shall match the inferred type.

## 13.2 Mutability

Bindings without `$` prefix are mutable:

```ori
let x = 0;
x = x + 1;       // OK: mutable binding

let $y = 10;
$y = 20;         // error: cannot assign to immutable binding '$y'
```

The `$` prefix marks a binding as immutable. See [Constants](12-constants.md) for details.

### 13.2.1 Cannot Reassign

- Immutable bindings (`$`-prefixed)
- Function parameters
- Loop variables

```ori
@add (a: int, b: int) -> int = {
    a = 10;  // error: cannot assign to parameter
    a + b
}

for item in items do
    item = other  // error: cannot assign to loop variable
```

## 13.3 Scope

Bindings are visible from declaration to end of enclosing block.

```ori
{
    let x = 10;
    let y = x + 5;  // x visible
    y
}
// x, y not visible
```

### 13.3.1 Shadowing

Bindings may shadow earlier bindings with the same name. Shadowing can change mutability:

```ori
{
    let x = 10;           // mutable
    let $x = x + 5;       // immutable, shadows outer x
    $x                    // 15
}

{
    let $x = 10;          // immutable
    {
        let x = $x * 2;   // mutable, shadows outer $x
        x = x + 1;        // OK: inner x is mutable
        x
    }
}
```

The `$` prefix shall match between definition and usage within the same binding scope.

## 13.4 Destructuring

Patterns destructure composite values. The `$` prefix applies to individual bindings:

```ori
let { x, y } = point;                  // both mutable
let { $x, y } = point;                 // x immutable, y mutable
let { x: px, y: py } = point;          // rename, both mutable
let (a, b) = pair;                     // both mutable
let ($a, $b) = pair;                   // both immutable
let [head, ..tail] = list;             // head mutable, tail mutable
let [$head, ..tail] = list;            // head immutable, tail mutable
let { position: { x, y } } = entity;   // nested destructure
```

Pattern shall match value structure.

## 13.5 Function Parameters

Parameters are immutable bindings scoped to the function body:

```ori
@add (a: int, b: int) -> int = a + b;
```

Parameters cannot be reassigned regardless of `$` prefix.

## 13.6 Assignment Semantics

Assignment is _value copy_. Ori uses value semantics: after `x = y`, `x` and `y` hold independent values. No aliasing exists between them.

```ori
let a = [1, 2, 3];
let b = a;         // b is an independent copy
b[0] = 99;         // does not affect a
```

NOTE  The compiler may use copy-on-write (COW) or reference counting (ARC) internally to defer physical copying until mutation occurs. This is an optimization that does not affect observable semantics.

Assignment to an immutable binding (`$`-prefixed) is a compile-time error. Assignment to a function parameter or loop variable is a compile-time error. See 13.2.1.

### 13.6.1 Field assignment

Field assignment `x.field = v` desugars to `x = { ...x, field: v }`. The binding `x` shall be mutable.

### 13.6.2 Index assignment

Index assignment `x[i] = v` desugars to `x = x.updated(key: i, value: v)`. The binding `x` shall be mutable.

### 13.6.3 Compound assignment

Compound assignment `x op= y` desugars to `x = x op y`. See [7.4.3](07-lexical-elements.md#743-compound-assignment-operators).

## 13.7 Drop Ordering

A value is _dropped_ (its `Drop` implementation is called, if any) when its binding goes out of scope. Within a block, values are dropped in reverse declaration order:

```ori
{
    let a = acquire_a();  // dropped third
    let b = acquire_b();  // dropped second
    let c = acquire_c();  // dropped first
    result
}
```

Drop order for function parameters after the body returns is implementation-defined.

The built-in function `drop_early(value:)` explicitly drops a value before the end of its scope. After `drop_early(value: x)`, the binding `x` is inaccessible; any subsequent use is a compile-time error. `drop_early` works on both mutable and immutable bindings — it concerns ownership, not mutability.

See [Clause 21](21-memory-model.md) for details on the ARC memory model.

## 13.8 Blank Identifier

The underscore `_` used alone in a pattern is the _blank identifier_. It matches any value and does not create a binding.

```ori
let _ = compute();       // evaluates compute(), discards result
match x { _ -> 0 }       // matches anything
let (_, b) = pair;       // discards first element
```

In `let _ = expr`, the expression is fully evaluated (including side effects and any `Drop` implementation of the result), but no binding is created.

Multiple `_` patterns may appear in the same pattern, unlike named bindings which shall be unique.

See [Clause 15](15-patterns.md) for pattern matching rules.
