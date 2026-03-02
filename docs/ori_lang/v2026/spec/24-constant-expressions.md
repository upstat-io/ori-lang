---
title: "Constant Expressions"
description: "Clause 24: Ori Language Specification — Constant Expressions"
order: 24
section: "Language"
---

# 24 Constant expressions

A _constant expression_ is an expression that can be fully evaluated at compile time.

> **Grammar:** See [grammar.ebnf](grammar.md) § CONSTANT EXPRESSIONS, DECLARATIONS (const_function)

## 24.1 Constant contexts

Constant expressions are required in:

- Config variable initializers (`$name = expr`)
- Fixed-size array lengths
- Const generic parameters
- Attribute arguments

## 24.2 Allowed in constant expressions

The following are valid in constant expressions:

### 24.2.1 Literals

All literal values:

```ori
$count = 42;
$name = "config";
$enabled = true;
$rate = 3.14;
$timeout = 30s;
$buffer = 4kb;
```

### 24.2.2 Arithmetic and logic

Operators on constant operands:

```ori
$double = $count * 2;
$offset = $base + 100;
$flag = $debug && $verbose;
```

### 24.2.3 String concatenation

```ori
$prefix = "app";
$full_name = $prefix + "_config";
```

### 24.2.4 References to config variables

Config variables may reference other config variables:

```ori
$base_timeout = 30s;
$extended_timeout = $base_timeout * 2;
```

The compiler evaluates config variables in dependency order. Circular dependencies are an error.

### 24.2.5 Conditionals

```ori
$timeout = if $debug then 60s else 30s;
$level = if $verbose then "debug" else "info";
```

### 24.2.6 Const function calls

Calls to const functions with constant arguments:

```ori
$factorial_10 = $factorial(n: 10);
```

## 24.3 Const functions

A _const function_ can be evaluated at compile time. Const functions use the `$` sigil:

```ori
$square (x: int) -> int = x * x;

$factorial (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1);

$max (a: int, b: int) -> int =
    if a > b then a else b;
```

The `$` sigil indicates compile-time evaluation, consistent with config variables.

### 24.3.1 Allowed operations

Const functions may:

- Use arithmetic, comparisons, and boolean logic
- Use conditionals (`if`, `match`)
- Call other const functions
- Use recursion
- Use blocks `{ }` for sequencing
- Construct structs and collections
- Manipulate strings
- Use local mutable bindings
- Use loop expressions (`for`, `loop`)

### 24.3.2 Restrictions

Const functions shall not:

- Use capabilities (`uses` clause)
- Perform I/O or side effects
- Call non-const functions
- Access external data
- Use random values
- Access current time

### 24.3.3 Local mutable bindings

Const functions may use mutable bindings for local computation. Local mutation is deterministic — given the same inputs, the function produces the same output:

```ori
$sum_to (n: int) -> int = {
    let total = 0;
    for i in 1..=n do total = total + i;
    total
}

$sum_squares (n: int) -> int = {
    let result = 0;
    for i in 1..=n do result = result + i * i;
    result
}
```

### 24.3.4 Compile-time evaluation

When a const function is called with constant arguments, evaluation occurs at compile time:

```ori
$power (base: int, exp: int) -> int =
    if exp == 0 then 1 else base * $power(base: base, exp: exp - 1);

$kb = $power(base: 2, exp: 10);  // evaluated to 1024 at compile time
```

When called with runtime arguments, the function executes at runtime:

```ori
let user_exp = read_int();
let result = $power(base: 2, exp: user_exp);  // evaluated at runtime
```

### 24.3.5 Partial evaluation

When a const function is called with some constant and some runtime arguments, the compiler shall evaluate the constant portions at compile time where doing so produces equivalent results:

```ori
$multiply (a: int, b: int) -> int = a * b;

// Both args const → evaluated at compile time
let $twelve = $multiply(a: 3, b: 4);  // Compiles to: let $twelve = 12

// One arg runtime → evaluated at runtime with const folded
@compute (x: int) -> int = $multiply(a: x, b: 4);  // Compiles to: x * 4
```

This is required behavior.

## 24.4 Evaluation limits

The compiler enforces limits on const evaluation to ensure compilation terminates:

| Limit | Default | Description |
|-------|---------|-------------|
| Step limit | 1,000,000 | Maximum expression evaluations |
| Recursion depth | 1,000 | Maximum stack frames |
| Memory limit | 100 MB | Maximum memory allocation |
| Time limit | 10 seconds | Maximum wall-clock time |

When a limit is exceeded, compilation fails with an error indicating which limit was exceeded and which expression caused the failure.

### 24.4.1 Configurable limits

Projects may adjust limits via configuration:

```toml
[const_eval]
max_steps = 10000000
max_depth = 2000
max_memory = "500mb"
max_time = "60s"
```

Per-expression overrides use the `#const_limit` attribute:

```ori
#const_limit(steps: 5000000)
let $large_table = $generate_lookup_table();
```

## 24.5 Error handling

### 24.5.1 Panics

A panic during const evaluation is a compilation error:

```ori
$divide (a: int, b: int) -> int = a / b;

let $oops = $divide(a: 1, b: 0);  // Compilation error: division by zero
```

### 24.5.2 Integer overflow

Integer overflow during const evaluation follows runtime rules and results in a compilation error:

```ori
let $big = $multiply(a: 1000000000000, b: 1000000000000);
// Compilation error: integer overflow in const evaluation
```

### 24.5.3 Option and Result

Const functions may return `Option` or `Result`:

```ori
$safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b);

let $result = $safe_div(a: 10, b: 0);  // $result = None (at compile time)
```

## 24.6 Caching

Evaluated const expressions are cached by the compiler:

1. Cache key: hash of function body and argument values
2. Subsequent compilations reuse cached results
3. Cache invalidated when function source changes

When a library exports const values, the compiled artifact contains the evaluated values, not the computation. The serialization format for const values is implementation-defined.

## 24.7 Not allowed in constant expressions

The following are not valid in constant expressions:

- Runtime variable references
- Non-const function calls
- Capability usage
- Error propagation (`?`)
- I/O operations
- Random values
- Current time access

```ori
$invalid = read_file("config.txt");  // error: not a constant expression
$also_invalid = some_list[0];        // error: runtime value
```

## 24.8 Diagnostics

| Code | Description |
|------|-------------|
| E0500 | Step limit exceeded |
| E0501 | Recursion depth exceeded |
| E0502 | Memory limit exceeded |
| E0503 | Time limit exceeded |
| E0504 | Non-const operation in const function |
