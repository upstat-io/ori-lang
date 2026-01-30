---
title: "Constant Expressions"
description: "Ori Language Specification — Constant Expressions"
order: 21
section: "Expressions"
---

# Constant Expressions

A _constant expression_ is an expression that can be fully evaluated at compile time.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § CONSTANT EXPRESSIONS, DECLARATIONS (const_function)

## Constant Contexts

Constant expressions are required in:

- Config variable initializers (`$name = expr`)
- Fixed-size array lengths
- Const generic parameters

## Allowed in Constant Expressions

The following are valid in constant expressions:

### Literals

All literal values:

```ori
$count = 42
$name = "config"
$enabled = true
$rate = 3.14
$timeout = 30s
$buffer = 4kb
```

### Arithmetic and Logic

Operators on constant operands:

```ori
$double = $count * 2
$offset = $base + 100
$flag = $debug && $verbose
```

### String Concatenation

```ori
$prefix = "app"
$full_name = $prefix + "_config"
```

### References to Config Variables

Config variables may reference other config variables:

```ori
$base_timeout = 30s
$extended_timeout = $base_timeout * 2
```

The compiler evaluates config variables in dependency order. Circular dependencies are an error.

### Conditionals

```ori
$timeout = if $debug then 60s else 30s
$level = if $verbose then "debug" else "info"
```

### Const Function Calls

Calls to const functions with constant arguments:

```ori
$factorial_10 = $factorial(n: 10)
```

## Const Functions

A _const function_ can be evaluated at compile time. Const functions use the `$` sigil:

```ori
$square (x: int) -> int = x * x

$factorial (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1)

$max (a: int, b: int) -> int =
    if a > b then a else b
```

The `$` sigil indicates compile-time evaluation, consistent with config variables.

### Restrictions

Const functions must not:

- Use capabilities (`uses` clause)
- Perform I/O or side effects
- Call non-const functions
- Use mutable bindings
- Access runtime state

Const functions may:

- Use conditionals (`if`, `match`)
- Call other const functions
- Use recursion
- Use `run(...)` for sequencing

### Compile-Time Evaluation

When a const function is called with constant arguments, evaluation occurs at compile time:

```ori
$power (base: int, exp: int) -> int =
    if exp == 0 then 1 else base * $power(base: base, exp: exp - 1)

$kb = $power(base: 2, exp: 10)  // evaluated to 1024 at compile time
```

When called with runtime arguments, the function executes at runtime:

```ori
let user_exp = read_int()
let result = $power(base: 2, exp: user_exp)  // evaluated at runtime
```

## Not Allowed in Constant Expressions

The following are not valid in constant expressions:

- Runtime variable references
- Non-const function calls
- Capability usage
- Mutable bindings
- Loop expressions (`for`, `loop`)
- Error propagation (`?`)

```ori
$invalid = read_file("config.txt")  // error: not a constant expression
$also_invalid = some_list[0]        // error: runtime value
```
