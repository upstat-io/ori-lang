---
title: "Constants"
description: "Ori Language Specification — Constants"
order: 4
section: "Types & Values"
---

# Constants

Constants are immutable bindings declared with the `$` prefix.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (constant_decl), CONSTANT EXPRESSIONS

## Immutable Bindings

A binding prefixed with `$` is immutable — it cannot be reassigned after initialization.

```ori
let $timeout = 30s;
let $api_base = "https://api.example.com";
let $max_retries = 3;
pub let $default_limit = 100;
```

The `$` prefix appears at definition, import, and usage sites:

```ori
// Definition
let $timeout = 30s;

// Usage
retry(op: fetch(url), attempts: $max_retries, timeout: $timeout);
```

## Module-Level Constants

All module-level bindings must be immutable. Mutable state is not permitted at module scope.

```ori
let $timeout = 30s;      // OK: immutable
pub let $api_base = "https://...";  // OK: public, immutable

let counter = 0;         // error: module-level bindings must be immutable
```

Module-level constants may be initialized with any expression:

```ori
let $a = 5;                    // literal
let $b = $a * 2;               // constant expression
let $c = $square(x: 10);       // const function call
```

The compiler evaluates constant expressions at compile time when possible. Expressions that cannot be evaluated at compile time produce runtime immutable bindings.

## Local Immutable Bindings

The `$` prefix may be used in local scope to create immutable local bindings:

```ori
@process (input: int) -> int = {
    let $base = expensive_calculation(input);
    // ... $base cannot be reassigned ...
    $base * 2
}
```

## Identifier Rules

The `$` prefix is a modifier on the identifier, not part of the name. A binding for `$x` and a binding for `x` refer to the same name — they cannot coexist in the same scope.

```ori
let x = 5;
let $x = 10;  // error: 'x' is already defined in this scope
```

The `$` must match between definition and usage:

```ori
let $timeout = 30s;
$timeout       // OK
timeout        // error: undefined variable 'timeout'
```

## Const Functions

A const function is a pure function bound to an immutable name. Const functions may be evaluated at compile time when all arguments are constant.

```ori
let $square = (x: int) -> int = x * x;
let $factorial = (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1);

// Evaluated at compile time
let $fact_10 = $factorial(n: 10);  // 3628800
```

Const functions must be pure:
- No capabilities (`uses` clause)
- No side effects
- No mutable state access

If called with non-constant arguments, the call is evaluated at runtime.

## Constant Expressions

Literals are constant. Arithmetic, comparison, logical, and string concatenation operations are constant if all operands are constant.

```ori
42                          // constant
1 + 2 * 3                   // constant
"hello" + " world"          // constant
true && false               // constant
```

Non-constant expressions include:
- Non-pure function calls
- Mutable variable references
- Expressions using capabilities

## Imports

When importing immutable bindings, the `$` must be included:

```ori
// config.ori
pub let $timeout = 30s;

// client.ori
use "./config" { $timeout };  // OK
use "./config" { timeout };   // error: 'timeout' not found
```

## Constraints

- Module-level bindings must use `$` prefix (immutable required)
- `$`-prefixed bindings cannot be reassigned
- `$` and non-`$` bindings with the same name cannot coexist in the same scope
